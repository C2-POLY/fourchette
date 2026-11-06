//! The worktree cockpit (#26): every worktree of the repository with its state (uncommitted
//! changes, ahead and behind its upstream, last commit), the pull request of its branch, and
//! which ones Tidy up can remove because their work is merged. Reading runs in a background
//! thread; `tidy` removes the worktrees and their local branches, never a remote branch, and
//! hands back what Undo needs to put them back.
use crate::git::{run_in, Repo};
use crate::github::{find_gh, gh};
use crate::pulls::{parse_pull, Pull, Pulls, State as PrState, FIELDS};
use crate::worktree::{Removed, Worktree};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Branches asked for in one GraphQL query.
const PER_QUERY: usize = 40;

/// The state of one worktree's folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// Files with uncommitted changes, new files included.
    pub changes: usize,
    /// (ahead, behind) its upstream; None without one, or when it is gone.
    pub track: Option<(usize, usize)>,
    /// Unix seconds of its last commit.
    pub last_commit: Option<i64>,
}

/// What the background read gives.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// The branch work is merged into (`ai::base_branch`).
    pub base: Option<String>,
    pub facts: HashMap<PathBuf, Facts>,
    /// Local branches whose tip is in `base`.
    pub merged: HashSet<String>,
    /// Among `merged`, the branches nothing was ever committed on: created and left as they were.
    pub fresh: HashSet<String>,
}

/// Reads a worktree folder's state.
pub fn facts(path: &Path) -> Facts {
    let changes = run_in(path, &["status", "--porcelain", "--untracked-files=all"]).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(0);
    let track = run_in(path, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]).ok().and_then(|s| {
        let mut p = s.split_whitespace();
        Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?))
    });
    let last_commit = run_in(path, &["log", "-1", "--format=%ct"]).ok().and_then(|s| s.trim().parse().ok());
    Facts { changes, track, last_commit }
}

/// Reads the state of every worktree whose folder exists, and which branches are merged.
pub fn read(repo: &Repo, worktrees: &[Worktree]) -> Report {
    let base = crate::ai::base_branch(repo);
    let facts = worktrees.iter().filter(|w| !w.missing).map(|w| (w.path.clone(), facts(&w.path))).collect();
    let mut merged = HashSet::new();
    let mut fresh = HashSet::new();
    if let Some(b) = &base {
        merged = repo.git(&["branch", "--format=%(refname:short)", "--merged", b]).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        for w in worktrees {
            let Some(name) = w.branch.as_deref().filter(|n| merged.contains(*n)) else { continue };
            // One entry in its reflog ("branch: Created from …") and none since: never worked on.
            // Without a reflog (expired), it counts as merged.
            let entries = repo.git(&["log", "-g", "--format=%H", &format!("refs/heads/{name}"), "--"]).map(|s| s.lines().count()).unwrap_or(0);
            if entries == 1 {
                fresh.insert(name.to_string());
            }
        }
    }
    Report { base, facts, merged, fresh }
}

/// The GraphQL query for the pull requests of these branches: one alias per branch.
pub fn query(branches: &[String]) -> String {
    let mut q = String::from("query($owner:String!,$name:String!){repository(owner:$owner,name:$name){");
    for (i, b) in branches.iter().enumerate() {
        // A JSON string is a valid GraphQL string, quotes and backslashes escaped.
        let name = serde_json::to_string(b).unwrap_or_default();
        q.push_str(&format!("b{i}:pullRequests(headRefName:{name},first:10,orderBy:{{field:UPDATED_AT,direction:DESC}}){{nodes{{...F}}}} "));
    }
    q.push_str("}} ");
    q.push_str(FIELDS);
    q
}

/// The pull requests in an answer of `query`.
pub fn parse(v: &Value, owner: &str) -> Result<Vec<Pull>, String> {
    if let Some(m) = v["errors"][0]["message"].as_str() {
        return Err(format!("GitHub: {m}"));
    }
    let repo = v["data"]["repository"].as_object().ok_or("GitHub doesn't know this repository.")?;
    Ok(repo.values().flat_map(|a| a["nodes"].as_array().cloned().unwrap_or_default()).filter_map(|n| parse_pull(&n, owner)).collect())
}

/// The pull requests of these branches of "owner/name", every state, in the background: one
/// call to GitHub per `PER_QUERY` branches. Each branch then shows its open one, else its latest.
pub fn load_pulls(slug: &str, remote: &str, branches: &[String]) -> Result<Pulls, String> {
    let exe = find_gh().ok_or("The GitHub command line (gh) isn't installed.")?;
    let (owner, name) = slug.split_once('/').ok_or("Not a GitHub repository")?;
    let mut list = Vec::new();
    for chunk in branches.chunks(PER_QUERY) {
        let q = format!("query={}", query(chunk));
        let s = gh(&exe, &["api", "graphql", "-f", &q, "-F", &format!("owner={owner}"), "-F", &format!("name={name}")])?;
        let v: Value = serde_json::from_str(&s).map_err(|_| "GitHub's answer wasn't in the expected form.".to_string())?;
        list.extend(parse(&v, owner)?);
    }
    Ok(Pulls::new(list, remote))
}

/// Whether Tidy up removes a worktree, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Go(String),
    Keep(String),
}

impl Verdict {
    pub fn goes(&self) -> bool {
        matches!(self, Verdict::Go(_))
    }

    pub fn reason(&self) -> &str {
        match self {
            Verdict::Go(r) | Verdict::Keep(r) => r,
        }
    }
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// Whether Tidy up removes `wt` (its folder and its local branch): only when its work is merged,
/// by its pull request (a squash counts) or because its branch is in the base branch, and nothing
/// would be lost. `here`: it is the folder open in this window. `pull`: its branch's pull request.
pub fn verdict(wt: &Worktree, here: bool, facts: Option<&Facts>, pull: Option<&Pull>, report: &Report) -> Verdict {
    let keep = |s: &str| Verdict::Keep(s.to_string());
    if wt.is_main {
        return keep("The repository's main folder");
    }
    if here {
        return keep("Open in this window");
    }
    if wt.missing {
        return keep("Its folder is gone: Clean up missing removes it");
    }
    if wt.locked {
        return keep("Locked");
    }
    let Some(branch) = wt.branch.as_deref() else { return keep("No branch (detached HEAD)") };
    let base = report.base.as_deref().unwrap_or("main");
    if Some(branch) == report.base.as_deref() {
        return keep("The base branch");
    }
    let Some(f) = facts else { return keep("Still reading its state") };
    if f.changes > 0 {
        return Verdict::Keep(format!("{} not committed", plural(f.changes, "change")));
    }
    let in_base = report.merged.contains(branch);
    if !in_base {
        if let Some((ahead, _)) = f.track.filter(|t| t.0 > 0) {
            return Verdict::Keep(format!("{} not pushed", plural(ahead, "commit")));
        }
    }
    match pull.map(|p| (p.number, p.state)) {
        Some((n, s)) if s.is_open() => Verdict::Keep(format!("Pull request #{n} is still open")),
        Some((n, PrState::Merged)) => Verdict::Go(format!("Pull request #{n} merged")),
        _ if in_base && report.fresh.contains(branch) => keep("Nothing committed on it yet"),
        _ if in_base => Verdict::Go(format!("Merged into {base}")),
        Some((n, _)) => Verdict::Keep(format!("Pull request #{n} closed without merging")),
        None => Verdict::Keep(format!("Not merged into {base}, no pull request")),
    }
}

/// What Tidy up did: what Undo needs, and what it couldn't do.
#[derive(Debug, Default)]
pub struct Tidied {
    pub removed: Vec<Removed>,
    pub branches: Vec<String>,
    pub errors: Vec<String>,
}

/// Removes these worktrees (by folder) and their local branches. Each is read again first: one
/// with uncommitted work, locked, or the repository's main folder is left alone, whatever was
/// shown. Remote branches are never touched.
pub fn tidy(repo: &Repo, paths: &[PathBuf]) -> Tidied {
    let mut done = Tidied::default();
    let all = repo.worktrees();
    for p in paths {
        let Some(wt) = all.iter().find(|w| &w.path == p) else { continue };
        let name = wt.name();
        if wt.is_main || wt.locked || wt.missing {
            done.errors.push(format!("{name} was left: it can't be tidied"));
            continue;
        }
        if crate::worktree::is_dirty(&wt.path) {
            done.errors.push(format!("{name} was left: it has uncommitted changes now"));
            continue;
        }
        match crate::worktree::remove_for_undo(repo, wt) {
            Ok(r) => done.removed.push(r),
            Err(e) => {
                done.errors.push(format!("Couldn't remove {name}: {e}"));
                continue;
            }
        }
        if let Some(b) = &wt.branch {
            // -D: a squashed pull request leaves the branch out of the base branch.
            match repo.git(&["branch", "-D", b]) {
                Ok(_) => done.branches.push(b.clone()),
                Err(e) => done.errors.push(format!("Couldn't delete {b}: {e}")),
            }
        }
    }
    done
}

/// The cockpit of the sample project, for screenshots: one worktree of each kind.
pub fn sample() -> (Vec<Worktree>, Report, Pulls) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let root = PathBuf::from("/Users/me/sample-project");
    let wt = |folder: &str, branch: Option<&str>, missing: bool| Worktree {
        path: if folder.is_empty() { root.clone() } else { root.join(".claude/worktrees").join(folder) },
        branch: branch.map(String::from),
        head: String::new(),
        is_main: folder.is_empty(),
        missing,
        locked: false,
    };
    let list = vec![
        wt("", Some("develop"), false),
        wt("46-empty-stash", Some("fix/46-empty-stash"), false),
        wt("48-curved-lanes", Some("feat/48-curved-lanes"), false),
        wt("42-lane-colours", Some("feat/42-lane-colours"), false),
        wt("40-avatar-colours", Some("fix/40-avatar-colours"), false),
        wt("search", Some("feature/search"), false),
        wt("new-graph", Some("try/new-graph"), false),
        wt("old-try", None, true),
    ];
    let f = |changes: usize, track: Option<(usize, usize)>, hours: i64| Facts { changes, track, last_commit: Some(now - hours * 3600) };
    let facts = [
        f(2, Some((3, 0)), 1),
        f(3, Some((2, 0)), 3),
        f(0, Some((0, 0)), 20),
        f(0, None, 50),
        f(0, None, 26 * 24),
        f(0, Some((0, 1)), 5 * 24),
        f(0, Some((0, 0)), 12 * 24),
    ];
    let facts = list.iter().zip(facts).map(|(w, f)| (w.path.clone(), f)).collect();
    let report = Report {
        base: Some("develop".into()),
        facts,
        merged: ["develop", "fix/40-avatar-colours"].into_iter().map(String::from).collect(),
        fresh: HashSet::new(),
    };
    let p = |number: u64, title: &str, branch: &str, state: PrState, ci: Option<crate::pulls::Ci>| Pull {
        number,
        title: title.into(),
        url: format!("https://github.com/example/sample-project/pull/{number}"),
        branch: branch.into(),
        state,
        ci,
        updated: now,
    };
    use crate::pulls::Ci;
    let pulls = Pulls::new(
        vec![
            p(61, "Draw curved lines between lanes", "feat/48-curved-lanes", PrState::Approved, Some(Ci::Success)),
            p(58, "Give every branch line its own colour", "feat/42-lane-colours", PrState::Merged, Some(Ci::Success)),
            p(55, "Search commits by author and hash", "feature/search", PrState::Draft, Some(Ci::Failure)),
            p(50, "Try a new graph layout", "try/new-graph", PrState::Closed, None),
        ],
        "origin",
    );
    (list, report, pulls)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wt(branch: &str) -> Worktree {
        Worktree { path: PathBuf::from(format!("/r/.wt/{branch}")), branch: Some(branch.into()), head: String::new(), is_main: false, missing: false, locked: false }
    }

    fn pull(n: u64, branch: &str, state: PrState) -> Pull {
        Pull { number: n, title: String::new(), url: String::new(), branch: branch.into(), state, ci: None, updated: 0 }
    }

    fn report(merged: &[&str], fresh: &[&str]) -> Report {
        Report { base: Some("main".into()), facts: HashMap::new(), merged: merged.iter().map(|s| s.to_string()).collect(), fresh: fresh.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn only_merged_and_clean_worktrees_go() {
        let clean = Facts { changes: 0, track: Some((0, 0)), last_commit: None };
        let r = report(&["done", "fresh", "merged-ahead"], &["fresh"]);
        let v = |w: &Worktree, f: Option<&Facts>, p: Option<&Pull>| verdict(w, false, f, p, &r);

        // A merged pull request, squashed or not: it goes, even out of `main`.
        assert_eq!(v(&wt("squashed"), Some(&clean), Some(&pull(5, "squashed", PrState::Merged))), Verdict::Go("Pull request #5 merged".into()));
        // In the base branch without a pull request.
        assert_eq!(v(&wt("done"), Some(&clean), None), Verdict::Go("Merged into main".into()));
        // In the base branch with commits not pushed: they are in main, so it goes.
        assert!(v(&wt("merged-ahead"), Some(&Facts { track: Some((2, 0)), ..clean.clone() }), None).goes());

        // Uncommitted work always stays, even merged.
        let dirty = Facts { changes: 3, ..clean.clone() };
        assert_eq!(v(&wt("done"), Some(&dirty), Some(&pull(5, "done", PrState::Merged))), Verdict::Keep("3 changes not committed".into()));
        // Commits not pushed and not in main stay.
        let ahead = Facts { track: Some((1, 0)), ..clean.clone() };
        assert_eq!(v(&wt("x"), Some(&ahead), Some(&pull(5, "x", PrState::Merged))), Verdict::Keep("1 commit not pushed".into()));
        // An open, draft or approved pull request, a closed one, none at all.
        assert_eq!(v(&wt("x"), Some(&clean), Some(&pull(6, "x", PrState::Draft))).reason(), "Pull request #6 is still open");
        assert_eq!(v(&wt("x"), Some(&clean), Some(&pull(7, "x", PrState::Approved))).reason(), "Pull request #7 is still open");
        assert_eq!(v(&wt("x"), Some(&clean), Some(&pull(8, "x", PrState::Closed))).reason(), "Pull request #8 closed without merging");
        assert_eq!(v(&wt("x"), Some(&clean), None).reason(), "Not merged into main, no pull request");
        // A branch just created from main is "merged" for Git, but nothing was done on it.
        assert_eq!(v(&wt("fresh"), Some(&clean), None).reason(), "Nothing committed on it yet");
        // Not read yet.
        assert!(!v(&wt("done"), None, None).goes());
    }

    #[test]
    fn some_worktrees_never_go() {
        let clean = Facts::default();
        let r = report(&["main", "done"], &[]);
        let merged = Some(pull(1, "done", PrState::Merged));
        let main = Worktree { is_main: true, ..wt("main") };
        assert_eq!(verdict(&main, false, Some(&clean), None, &r).reason(), "The repository's main folder");
        assert_eq!(verdict(&wt("done"), true, Some(&clean), merged.as_ref(), &r).reason(), "Open in this window");
        assert!(!verdict(&Worktree { missing: true, ..wt("done") }, false, None, merged.as_ref(), &r).goes());
        assert!(!verdict(&Worktree { locked: true, ..wt("done") }, false, Some(&clean), merged.as_ref(), &r).goes());
        assert!(!verdict(&Worktree { branch: None, ..wt("done") }, false, Some(&clean), None, &r).goes());
        assert_eq!(verdict(&wt("main"), false, Some(&clean), None, &r).reason(), "The base branch");
    }

    #[test]
    fn the_query_asks_for_each_branch() {
        let q = query(&["feat/26-cockpit".into(), "we\"ird".into()]);
        assert!(q.contains(r#"b0:pullRequests(headRefName:"feat/26-cockpit","#));
        assert!(q.contains(r#"b1:pullRequests(headRefName:"we\"ird","#));
        assert!(q.ends_with(FIELDS));
        let node = |n: u64, branch: &str, state: &str| {
            json!({"number": n, "title": "t", "url": "u", "state": state, "isDraft": false, "reviewDecision": null, "updatedAt": "2026-11-05T10:00:00Z",
                   "headRefName": branch, "isCrossRepository": false, "headRepositoryOwner": {"login": "o"}, "commits": {"nodes": []}})
        };
        let v = json!({"data": {"repository": {"b0": {"nodes": [node(3, "a", "MERGED"), node(4, "a", "CLOSED")]}, "b1": {"nodes": []}}}});
        let pulls = parse(&v, "o").unwrap();
        assert_eq!(pulls.iter().map(|p| p.number).collect::<Vec<_>>(), vec![3, 4]);
        assert!(parse(&json!({"errors": [{"message": "Bad credentials"}]}), "o").unwrap_err().contains("Bad credentials"));
        assert!(parse(&json!({"data": {"repository": null}}), "o").is_err());
    }

    #[test]
    fn the_issue_number_comes_from_the_branch_name() {
        let n = crate::pr_draft::issue_number;
        assert_eq!(n("feat/6-board"), Some(6));
        assert_eq!(n("fix/12-empty-stash"), Some(12));
        assert_eq!(n("6-board"), Some(6));
        assert_eq!(n("feature/search"), None);
    }

    #[test]
    fn the_sample_shows_each_verdict() {
        let (list, report, pulls) = sample();
        let goes: Vec<bool> = list.iter().map(|w| verdict(w, false, report.facts.get(&w.path), w.branch.as_deref().and_then(|b| pulls.for_branch(b)), &report).goes()).collect();
        assert_eq!(goes, vec![false, false, false, true, true, false, false, false]);
    }

    /// A real repository: reading the state, then tidying a merged worktree and Undo.
    #[test]
    fn reads_tidies_and_undoes() {
        let dir = std::env::temp_dir().join(format!("fourchette-cockpit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(dir.join("a.txt"), "1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "first"]);
        let (done, wip, fresh) = (dir.join("wt-done"), dir.join("wt-wip"), dir.join("wt-fresh"));
        for (p, b) in [(&done, "feat/1-done"), (&wip, "feat/2-wip"), (&fresh, "feat/3-fresh")] {
            g(&["worktree", "add", "-q", "-b", b, p.to_str().unwrap()]);
        }
        // feat/1-done gets a commit, merged into main; feat/2-wip has uncommitted work.
        std::fs::write(done.join("b.txt"), "2\n").unwrap();
        run_in(&done, &["add", "-A"]).unwrap();
        run_in(&done, &["commit", "-q", "-m", "done"]).unwrap();
        g(&["merge", "-q", "--no-ff", "-m", "merge", "feat/1-done"]);
        std::fs::write(wip.join("c.txt"), "draft\n").unwrap();

        let repo = Repo::open(&dir).unwrap();
        let list = repo.worktrees();
        let report = read(&repo, &list);
        assert_eq!(report.base.as_deref(), Some("main"));
        let find = |p: &Path| list.iter().find(|w| w.path.canonicalize().unwrap() == p.canonicalize().unwrap()).unwrap().clone();
        let (w_done, w_wip, w_fresh) = (find(&done), find(&wip), find(&fresh));
        assert_eq!(report.facts[&w_wip.path].changes, 1);
        assert!(report.facts[&w_done.path].last_commit.is_some());
        assert!(report.fresh.contains("feat/3-fresh") && !report.fresh.contains("feat/1-done"));
        let v = |w: &Worktree| verdict(w, false, report.facts.get(&w.path), None, &report);
        assert!(v(&w_done).goes());
        assert!(!v(&w_wip).goes() && !v(&w_fresh).goes());

        // Tidy up refuses the worktree with work even when asked, and removes the merged one.
        let before = repo.state();
        let mut snap = repo.snapshot("Tidy up worktrees", false, false);
        let t = tidy(&repo, &[w_done.path.clone(), w_wip.path.clone()]);
        assert_eq!(t.branches, vec!["feat/1-done".to_string()]);
        assert_eq!(t.errors.len(), 1);
        assert!(!done.exists() && wip.exists());
        snap.worktrees.extend(t.removed);

        // Undo puts the worktree and its branch back.
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert!(repo.worktree_of("feat/1-done").is_some() && done.join("b.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A real repository on GitHub: the pull requests of two of its branches.
    #[test]
    #[ignore]
    fn reads_the_pulls_of_real_branches() {
        let pulls = load_pulls("cli/cli", "origin", &["trunk".into(), "this-branch-does-not-exist".into()]).expect("gh signed in");
        assert!(pulls.for_branch("this-branch-does-not-exist").is_none());
    }
}
