//! Actions done outside the app (#241): commits, pushes and merges made with Claude Code or in
//! a terminal, counted from the reflogs of the branches (read only) for the skins' bars.

use crate::course::Stride;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::Path;

/// A cheap fingerprint of the reflogs' files (`<common_dir>/logs/refs`): each file's relative
/// path, length and modification time, with no `git` run. An append, a new reflog or a deleted
/// branch's vanished one changes it; a missing folder gives a stable value.
pub fn signature(common_dir: &Path) -> u64 {
    fn walk(root: &Path, dir: &Path, h: &mut std::collections::hash_map::DefaultHasher) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                walk(root, &path, h);
            } else {
                path.strip_prefix(root).unwrap_or(&path).hash(h);
                meta.len().hash(h);
                meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).hash(h);
            }
        }
    }
    let root = common_dir.join("logs").join("refs");
    let mut h = std::collections::hash_map::DefaultHasher::new();
    walk(&root, &root, &mut h);
    h.finish()
}

/// The pull requests merged on GitHub among `hash\tcommitter_email\tsubject` lines (from
/// `Repo::first_parent_log`): the commits GitHub itself made (committer `noreply@github.com`)
/// whose subject ends with `(#n)` (squash, rebase) or starts with `Merge pull request #n`.
/// Each number once, with its title.
pub fn merged_prs(log: &str) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = Vec::new();
    for line in log.lines() {
        let mut f = line.splitn(3, '\t');
        let (Some(_), Some(email), Some(subject)) = (f.next(), f.next(), f.next()) else { continue };
        if email != "noreply@github.com" {
            continue;
        }
        let found = if let Some(rest) = subject.strip_prefix("Merge pull request #") {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let title = rest[digits.len()..].trim().strip_prefix("from ").unwrap_or("").to_string();
            digits.parse::<u32>().ok().map(|n| (n, title))
        } else if let Some(body) = subject.trim_end().strip_suffix(')') {
            body.rsplit_once("(#").and_then(|(title, n)| n.parse::<u32>().ok().map(|n| (n, title.trim().to_string())))
        } else {
            None
        };
        if let Some((n, title)) = found {
            if !out.iter().any(|(m, _)| *m == n) {
                out.push((n, title));
            }
        }
    }
    out
}

/// The reflogs of the local and remote branches: per ref, `(hash, subject)` newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reflogs {
    pub refs: BTreeMap<String, Vec<(String, String)>>,
}

impl Reflogs {
    /// Parses `git reflog show --all --format=%gD%x09%H%x09%gs`. Only `refs/heads/*` and
    /// `refs/remotes/*` are kept; each line's `@{n}` gives its place, the newest being 0.
    pub fn parse(text: &str) -> Reflogs {
        let mut found: BTreeMap<String, Vec<(usize, String, String)>> = BTreeMap::new();
        for line in text.lines() {
            let mut f = line.splitn(3, '\t');
            let (Some(sel), Some(hash)) = (f.next(), f.next()) else { continue };
            let subject = f.next().unwrap_or("");
            let Some((name, rest)) = sel.rsplit_once("@{") else { continue };
            let Some(n) = rest.strip_suffix('}').and_then(|n| n.parse::<usize>().ok()) else { continue };
            if !(name.starts_with("refs/heads/") || name.starts_with("refs/remotes/")) {
                continue;
            }
            found.entry(name.to_string()).or_default().push((n, hash.to_string(), subject.to_string()));
        }
        let refs = found
            .into_iter()
            .map(|(name, mut v)| {
                v.sort_by_key(|e| e.0);
                (name, v.into_iter().map(|(_, h, s)| (h, s)).collect())
            })
            .collect();
        Reflogs { refs }
    }
}

/// What one reflog entry counts as, if anything.
pub fn classify(refname: &str, subject: &str, remotes: &[String]) -> Option<Stride> {
    if refname.starts_with("refs/remotes/") {
        return (subject == "update by push").then_some(Stride::Push);
    }
    if !refname.starts_with("refs/heads/") {
        return None;
    }
    if subject.starts_with("commit (merge):") {
        return Some(Stride::Merge);
    }
    if subject.starts_with("commit:") || subject.starts_with("commit (initial):") || subject.starts_with("commit (amend):") {
        return Some(Stride::Commit);
    }
    let rest = subject.strip_prefix("merge ")?;
    let (from, what) = rest.split_once(": ")?;
    if remotes.iter().any(|r| from.starts_with(&format!("{r}/"))) {
        return None;
    }
    (what.starts_with("Merge made by") || what == "Fast-forward").then_some(Stride::Merge)
}

/// How many entries of each ref were already seen, per repository.
#[derive(Debug, Default)]
pub struct Ledger {
    seen: BTreeMap<String, usize>,
    started: bool,
    /// The pull requests already counted as merged on GitHub.
    prs: BTreeSet<u32>,
}

/// What `Ledger::advance` found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Advance {
    pub strides: Vec<(Stride, String)>,
    /// `(refname, old, new)` of the remote refs moved by entries other than a push (a fetch):
    /// the commits in `old..new` may be pull requests merged on GitHub.
    pub ranges: Vec<(String, String, String)>,
}

impl Ledger {
    /// The actions that left new entries since the last read, minus those the app did itself
    /// (`own`, which is cleared), each with a short message. The first read only sets the counts.
    pub fn advance(&mut self, read: &Reflogs, own: &mut Vec<Stride>) -> Advance {
        let remotes = remote_names(read);
        let mut out = Advance::default();
        if self.started {
            for (name, entries) in &read.refs {
                let before = self.seen.get(name).copied().unwrap_or(0);
                let new = entries.len().saturating_sub(before);
                // Oldest first, so the messages come in the order things happened.
                if name.starts_with("refs/remotes/") && entries[..new].iter().any(|(_, s)| s != "update by push") {
                    if let Some((old, _)) = entries.get(new) {
                        out.ranges.push((name.clone(), old.clone(), entries[0].0.clone()));
                    }
                }
                for (_, subject) in entries[..new].iter().rev() {
                    let Some(stride) = classify(name, subject, &remotes) else { continue };
                    if let Some(i) = own.iter().position(|o| *o == stride) {
                        own.remove(i);
                        continue;
                    }
                    out.strides.push((stride.clone(), message(&stride, name, subject)));
                }
            }
        }
        own.clear();
        self.seen = read.refs.iter().map(|(k, v)| (k.clone(), v.len())).collect();
        self.started = true;
        out
    }

    /// One merge per pull request not counted yet, from `merged_prs`.
    pub fn take_prs(&mut self, prs: Vec<(u32, String)>) -> Vec<(Stride, String)> {
        if !self.started {
            return Vec::new();
        }
        prs.into_iter().filter(|(n, _)| self.prs.insert(*n)).map(|(n, t)| (Stride::Merge, format!("Merged on GitHub: #{n} {t}"))).collect()
    }
}

/// The remotes named by the remote refs read.
fn remote_names(read: &Reflogs) -> Vec<String> {
    let mut v: Vec<String> = read
        .refs
        .keys()
        .filter_map(|k| k.strip_prefix("refs/remotes/")?.split('/').next().map(String::from))
        .collect();
    v.dedup();
    v
}

fn message(stride: &Stride, refname: &str, subject: &str) -> String {
    let what = subject.split_once(": ").map(|(_, s)| s).unwrap_or(subject);
    match stride {
        Stride::Push => {
            let branch = refname.strip_prefix("refs/remotes/").and_then(|r| r.split_once('/')).map(|(_, b)| b).unwrap_or(refname);
            format!("Pushed {branch} outside Fourchette")
        }
        Stride::Merge => format!("Merged outside Fourchette: {what}"),
        _ => format!("Committed outside Fourchette: {what}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reads(lines: &[(&str, &[&str])]) -> Reflogs {
        let mut text = String::new();
        for (name, subjects) in lines {
            for (i, s) in subjects.iter().enumerate() {
                text.push_str(&format!("{name}@{{{i}}}\tabc{i}\t{s}\n"));
            }
        }
        Reflogs::parse(&text)
    }

    const MAIN: &str = "refs/heads/main";

    #[test]
    fn parse_keeps_branches_and_remotes_newest_first() {
        let r = Reflogs::parse("HEAD@{0}\ta\tcommit: x\nrefs/heads/main@{1}\tb\tcommit: old\nrefs/heads/main@{0}\tc\tcommit: new\nrefs/stash@{0}\td\tWIP\nrefs/remotes/origin/main@{0}\te\tupdate by push\n");
        assert_eq!(r.refs.len(), 2);
        assert_eq!(r.refs[MAIN][0], ("c".into(), "commit: new".into()));
        assert_eq!(r.refs[MAIN][1].1, "commit: old");
        assert!(Reflogs::parse("").refs.is_empty());
    }

    #[test]
    fn classify_every_subject() {
        let rem = vec!["origin".to_string()];
        let c = |r: &str, s: &str| classify(r, s, &rem);
        assert_eq!(c(MAIN, "commit: fix"), Some(Stride::Commit));
        assert_eq!(c(MAIN, "commit (initial): first"), Some(Stride::Commit));
        assert_eq!(c(MAIN, "commit (amend): fix"), Some(Stride::Commit));
        assert_eq!(c(MAIN, "commit (merge): Merge branch 'x'"), Some(Stride::Merge));
        assert_eq!(c(MAIN, "merge feat/x: Merge made by the 'ort' strategy."), Some(Stride::Merge));
        assert_eq!(c(MAIN, "merge feat/x: Fast-forward"), Some(Stride::Merge));
        assert_eq!(c(MAIN, "merge origin/main: Fast-forward"), None);
        assert_eq!(c(MAIN, "merge origin/main: Merge made by the 'ort' strategy."), None);
        assert_eq!(c("refs/remotes/origin/main", "update by push"), Some(Stride::Push));
        assert_eq!(c("refs/remotes/origin/main", "fetch origin: fast-forward"), None);
        assert_eq!(c(MAIN, "update by push"), None);
        assert_eq!(c("refs/remotes/origin/main", "commit: x"), None);
        for s in ["pull: Fast-forward", "reset: moving to HEAD~1", "rebase (finish): refs/heads/x onto y", "cherry-pick: x", "checkout: moving", "branch: Created from HEAD", "fetch: x"] {
            assert_eq!(c(MAIN, s), None, "{s}");
        }
    }

    #[test]
    fn first_read_counts_nothing() {
        let mut l = Ledger::default();
        let r = reads(&[(MAIN, &["commit: b", "commit: a"])]);
        assert!(l.advance(&r, &mut vec![]).strides.is_empty());
        assert!(l.advance(&r, &mut vec![]).strides.is_empty());
    }

    #[test]
    fn outside_commit_counts_once() {
        let mut l = Ledger::default();
        l.advance(&reads(&[(MAIN, &["commit: a"])]), &mut vec![]);
        let r = reads(&[(MAIN, &["commit: b", "commit: a"])]);
        let got = l.advance(&r, &mut vec![]).strides;
        assert_eq!(got, vec![(Stride::Commit, "Committed outside Fourchette: b".to_string())]);
        assert!(l.advance(&r, &mut vec![]).strides.is_empty());
    }

    #[test]
    fn push_and_merge_count() {
        let mut l = Ledger::default();
        l.advance(&reads(&[(MAIN, &["commit: a"]), ("refs/remotes/origin/main", &["fetch: x"])]), &mut vec![]);
        let r = reads(&[(MAIN, &["merge feat: Fast-forward", "commit: a"]), ("refs/remotes/origin/main", &["update by push", "fetch: x"])]);
        let got: Vec<Stride> = l.advance(&r, &mut vec![]).strides.into_iter().map(|g| g.0).collect();
        assert_eq!(got.len(), 2);
        assert!(got.contains(&Stride::Merge) && got.contains(&Stride::Push));
    }

    #[test]
    fn app_actions_are_not_counted_again() {
        let mut l = Ledger::default();
        l.advance(&reads(&[(MAIN, &["commit: a"])]), &mut vec![]);
        let r = reads(&[(MAIN, &["commit: c", "commit: b", "commit: a"])]);
        let mut own = vec![Stride::Commit];
        let got = l.advance(&r, &mut own).strides;
        assert_eq!(got.len(), 1, "one of the two is the app's");
        assert!(own.is_empty());
    }

    #[test]
    fn two_reads_of_the_same_state_count_once() {
        let mut l = Ledger::default();
        l.advance(&reads(&[(MAIN, &["commit: a"])]), &mut vec![]);
        let r = reads(&[(MAIN, &["commit: b", "commit: a"])]);
        assert_eq!(l.advance(&r, &mut vec![]).strides.len(), 1);
        assert_eq!(l.advance(&r, &mut vec![]).strides.len(), 0);
    }

    #[test]
    fn a_new_branch_counts_its_commits() {
        let mut l = Ledger::default();
        l.advance(&reads(&[(MAIN, &["commit: a"])]), &mut vec![]);
        let r = reads(&[(MAIN, &["commit: a"]), ("refs/heads/x", &["commit: two", "commit: one", "branch: Created from main"])]);
        assert_eq!(l.advance(&r, &mut vec![]).strides.len(), 2);
    }

    #[test]
    fn updates_from_the_remote_do_not_count() {
        let mut l = Ledger::default();
        let remote = ("refs/remotes/origin/main", &["fetch: fast-forward"][..]);
        l.advance(&reads(&[(MAIN, &["commit: a"]), remote]), &mut vec![]);
        let r = reads(&[(MAIN, &["merge origin/main: Fast-forward", "commit: a"]), remote]);
        assert!(l.advance(&r, &mut vec![]).strides.is_empty());
    }

    #[test]
    fn a_shrinking_reflog_counts_nothing() {
        let mut l = Ledger::default();
        l.advance(&reads(&[(MAIN, &["commit: b", "commit: a"])]), &mut vec![]);
        assert!(l.advance(&reads(&[(MAIN, &["commit: a"])]), &mut vec![]).strides.is_empty());
        assert!(l.advance(&reads(&[]), &mut vec![]).strides.is_empty());
    }

    #[test]
    fn signature_follows_the_reflog_files() {
        let dir = std::env::temp_dir().join(format!("fourchette-sig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let missing = signature(&dir);
        assert_eq!(missing, signature(&dir), "a missing folder is stable");
        let logs = dir.join("logs/refs/heads");
        std::fs::create_dir_all(&logs).unwrap();
        let empty = signature(&dir);
        std::fs::write(logs.join("x"), "one\n").unwrap();
        let one = signature(&dir);
        assert_ne!(empty, one);
        assert_eq!(one, signature(&dir));
        std::fs::write(logs.join("x"), "one\ntwo\n").unwrap();
        let two = signature(&dir);
        assert_ne!(one, two, "an append");
        std::fs::remove_file(logs.join("x")).unwrap();
        assert_ne!(two, signature(&dir), "a deleted file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn advance_gives_the_ranges_of_remote_refs_moved_by_a_fetch() {
        let origin = "refs/remotes/origin/main";
        let mut l = Ledger::default();
        l.advance(&reads(&[(origin, &["fetch: x"])]), &mut vec![]);
        // reads() gives the hash abc<i>: the newest is abc0, the one below abc1.
        let got = l.advance(&reads(&[(origin, &["fetch origin: fast-forward", "fetch: x"])]), &mut vec![]);
        assert_eq!(got.ranges, vec![(origin.to_string(), "abc1".to_string(), "abc0".to_string())]);
        // A push alone gives no range, and neither does a ref without an older entry.
        let got = l.advance(&reads(&[(origin, &["update by push", "fetch origin: fast-forward", "fetch: x"])]), &mut vec![]);
        assert!(got.ranges.is_empty());
        let got = l.advance(&reads(&[(origin, &["update by push", "fetch origin: fast-forward", "fetch: x"]), ("refs/remotes/origin/new", &["fetch: storing head"])]), &mut vec![]);
        assert!(got.ranges.is_empty());
    }

    const GH: &str = "noreply@github.com";

    #[test]
    fn merged_prs_reads_github_merges() {
        let log = format!(
            "a1\t{GH}\tfeat(x): thing (#12)\nb2\t{GH}\tMerge pull request #13 from c2/feat-y\nc3\tme@x.io\tfix: local (#14)\nd4\t{GH}\tplain commit\n"
        );
        assert_eq!(merged_prs(&log), vec![(12, "feat(x): thing".to_string()), (13, "c2/feat-y".to_string())]);
        assert!(merged_prs("").is_empty());
    }

    #[test]
    fn merged_prs_counts_a_rebase_merge_once() {
        let log = format!("a\t{GH}\tfirst (#20)\nb\t{GH}\tsecond (#20)\nc\t{GH}\tother (#21)\n");
        let got: Vec<u32> = merged_prs(&log).into_iter().map(|p| p.0).collect();
        assert_eq!(got, vec![20, 21]);
    }

    #[test]
    fn take_prs_counts_each_pull_request_once() {
        let mut l = Ledger::default();
        assert!(l.take_prs(vec![(1, "a".into())]).is_empty(), "baseline");
        l.advance(&reads(&[]), &mut vec![]);
        let got = l.take_prs(vec![(1, "a".into()), (2, "b".into())]);
        assert_eq!(got, vec![(Stride::Merge, "Merged on GitHub: #1 a".to_string()), (Stride::Merge, "Merged on GitHub: #2 b".to_string())]);
        assert_eq!(l.take_prs(vec![(2, "b".into()), (3, "c".into())]).len(), 1);
    }
}
