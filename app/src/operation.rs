//! A merge, rebase, cherry-pick or revert stopped on conflicts: what the banner says about it, and
//! its Abort, Continue and Skip (the original's `abortButton`, "All conflicts resolved"). Each of them
//! first saves the stopped operation (`Paused`), so Undo (⌘Z) puts it back as it was: its files in
//! the Git folder, the index with its conflicts, and the files as the person left them.
use crate::git::{file_list, Repo};
use crate::undo::{Snapshot, State};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Abort,
    Continue,
    Skip,
}

impl Action {
    pub fn flag(self) -> &'static str {
        match self {
            Action::Abort => "--abort",
            Action::Continue => "--continue",
            Action::Skip => "--skip",
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            Action::Abort => "Abort",
            Action::Continue => "Continue",
            Action::Skip => "Skip",
        }
    }
}

/// An operation stopped half-way, as the banner shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct InProgress {
    /// As `Repo::operation` names it: merge, rebase, cherry-pick, revert or am.
    pub op: String,
    /// A rebase's step, (current, total).
    pub step: Option<(usize, usize)>,
    /// What it does, in a few words: "Merge branch 'feature'", "feature onto main",
    /// "a1b2c3d Fix the login".
    pub what: String,
    /// Files with unresolved conflicts.
    pub conflicts: Vec<String>,
}

impl InProgress {
    /// "Merge in progress", "Rebase in progress"…
    pub fn title(&self) -> String {
        let n = noun(&self.op);
        format!("{}{} in progress", n[..1].to_uppercase(), &n[1..])
    }

    /// A rebase (or a series of patches) can leave out the commit it stopped on.
    pub fn can_skip(&self) -> bool {
        matches!(self.op.as_str(), "rebase" | "am")
    }

    pub fn resolved(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// The operation in words: "merge", "rebase", "cherry-pick", "revert", "patch series".
pub fn noun(op: &str) -> &str {
    match op {
        "am" => "patch series",
        other => other,
    }
}

/// The Abort confirmation, worded like the original's.
pub fn abort_question(op: &str) -> String {
    format!("Do you want to abort the {}?", noun(op))
}

/// What the person reads once an action ran. `now`: the operation still in progress after it.
pub fn outcome(op: &str, action: Action, res: Result<String, String>, now: Option<&InProgress>) -> Result<String, String> {
    let n = noun(op);
    let at = |p: &InProgress| p.step.map(|(i, total)| format!(" at step {i}/{total}")).unwrap_or_default();
    match (action, res, now) {
        (Action::Abort, Ok(_), None) => Ok(format!("Aborted the {n}")),
        (_, _, Some(p)) if p.op == op && !p.conflicts.is_empty() && action != Action::Abort => {
            Err(format!("The {n} stopped on conflicts{} in {}. Resolve and stage them, then Continue.", at(p), file_list(&p.conflicts)))
        }
        (Action::Continue, Ok(_), None) => Ok(format!("{}{} finished", n[..1].to_uppercase(), &n[1..])),
        (Action::Skip, Ok(_), None) => Ok(format!("Skipped the commit; the {n} finished")),
        (_, Ok(_), Some(p)) => Ok(format!("The {n} stopped{}", at(p))),
        (_, Err(e), _) => Err(e),
    }
}

/// The operation's own files in the Git folder, saved and put back as they are.
const OP_FILES: &[&str] = &[
    "rebase-merge",
    "rebase-apply",
    "sequencer",
    "MERGE_HEAD",
    "MERGE_MSG",
    "MERGE_MODE",
    "MERGE_RR",
    "MERGE_AUTOSTASH",
    "AUTO_MERGE",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "SQUASH_MSG",
    "ORIG_HEAD",
];

/// A stopped operation saved before Abort, Continue or Skip, for Undo.
#[derive(Clone, Debug)]
pub struct Paused {
    /// The copies: `git/` (the `OP_FILES` there were), `index`, `files/` (the files below).
    dir: PathBuf,
    /// Files that differed from HEAD, and whether each existed.
    files: Vec<(String, bool)>,
    /// `Repo::worktree_fingerprint` before the action.
    before: u64,
    /// The state and the files right after the action: Undo refuses once either changed, so it
    /// never overwrites what was done since.
    pub after: Option<(State, u64)>,
}

impl Repo {
    fn git_dir(&self) -> Result<PathBuf, String> {
        self.git(&["rev-parse", "--absolute-git-dir"]).map(|d| PathBuf::from(d.trim()))
    }

    /// The operation stopped half-way, if any, with what the banner says about it.
    pub fn in_progress(&self) -> Option<InProgress> {
        let op = self.operation()?;
        let gd = self.git_dir().ok()?;
        let read = |p: &str| std::fs::read_to_string(gd.join(p)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let rebase_dir = if gd.join("rebase-merge").exists() { "rebase-merge" } else { "rebase-apply" };
        let step = match op.as_str() {
            "rebase" | "am" => {
                let (num, end) = if rebase_dir == "rebase-merge" { ("msgnum", "end") } else { ("next", "last") };
                let n = |f: &str| read(&format!("{rebase_dir}/{f}")).and_then(|s| s.parse::<usize>().ok());
                n(num).zip(n(end))
            }
            _ => None,
        };
        let commit = |sha: String| self.git(&["log", "-1", "--format=%h %s", &sha]).ok().map(|s| s.trim().to_string());
        let what = match op.as_str() {
            "merge" => read("MERGE_MSG").and_then(|m| m.lines().next().map(String::from)),
            "rebase" => {
                let head = read(&format!("{rebase_dir}/head-name")).map(|h| h.trim_start_matches("refs/heads/").to_string());
                let onto = read(&format!("{rebase_dir}/onto")).map(|o| self.name_of(&o));
                match (head, onto) {
                    (Some(h), Some(o)) => Some(format!("{h} onto {o}")),
                    (None, Some(o)) => Some(format!("onto {o}")),
                    _ => None,
                }
            }
            "cherry-pick" => read("CHERRY_PICK_HEAD").and_then(commit),
            "revert" => read("REVERT_HEAD").and_then(commit),
            _ => None,
        };
        Some(InProgress { op, step, what: what.unwrap_or_default(), conflicts: self.conflicted_files() })
    }

    /// A branch at `sha`, else its short hash.
    fn name_of(&self, sha: &str) -> String {
        let names = self.git(&["for-each-ref", "--points-at", sha, "--format=%(refname:short)", "refs/heads", "refs/remotes"]).unwrap_or_default();
        names.lines().next().map(String::from).unwrap_or_else(|| sha[..sha.len().min(7)].to_string())
    }

    /// Abort, Continue or Skip the operation in progress, under a snapshot that keeps it whole for
    /// Undo. Returns that snapshot when anything changed, and what to tell the person.
    pub fn operation_step(&self, action: Action) -> (Option<Snapshot>, Result<String, String>) {
        let Some(before) = self.in_progress() else {
            return (None, Err("No merge, rebase, cherry-pick or revert is in progress.".into()));
        };
        let op = before.op.clone();
        let mut label = format!("{} {}", action.verb(), noun(&op));
        if let (Action::Skip, Some((i, n))) = (action, before.step) {
            label.push_str(&format!(" step {i}/{n}"));
        }
        let mut snap = self.snapshot(&label, false, false);
        match self.pause_save() {
            Ok(p) => snap.paused = Some(p),
            Err(e) => return (None, Err(format!("Couldn't save the {} for Undo, so nothing was done: {e}", noun(&op)))),
        }
        let res = self.git(&[&op, action.flag()]);
        self.after(&mut snap);
        let now = self.in_progress();
        let fingerprint = snap.paused.as_ref().map(|p| p.before);
        let changed = self.state() != snap.state || now.as_ref() != Some(&before) || fingerprint != Some(self.worktree_fingerprint());
        if !changed {
            if let Some(p) = &snap.paused {
                let _ = std::fs::remove_dir_all(&p.dir);
            }
        }
        (changed.then_some(snap), outcome(&op, action, res, now.as_ref()))
    }

    /// Copies what makes the stopped operation: its files in the Git folder, the index (with the
    /// conflicts) and every file that differs from HEAD.
    pub fn pause_save(&self) -> Result<Paused, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let gd = self.git_dir()?;
        let dir = std::env::temp_dir().join(format!("fourchette-paused-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        let io = |e: std::io::Error| e.to_string();
        std::fs::create_dir_all(dir.join("git")).map_err(io)?;
        std::fs::create_dir_all(dir.join("files")).map_err(io)?;
        for f in OP_FILES {
            if gd.join(f).exists() {
                copy_any(&gd.join(f), &dir.join("git").join(f)).map_err(io)?;
            }
        }
        std::fs::copy(gd.join("index"), dir.join("index")).map_err(io)?;
        let changed = self.git(&["diff", "HEAD", "--name-only", "--no-renames", "-z"])?;
        let mut files = Vec::new();
        for path in changed.split('\0').filter(|p| !p.is_empty()) {
            let src = self.root.join(path);
            let exists = src.symlink_metadata().is_ok();
            if exists {
                copy_any(&src, &dir.join("files").join(path)).map_err(io)?;
            }
            files.push((path.to_string(), exists));
        }
        Ok(Paused { dir, files, before: self.worktree_fingerprint(), after: None })
    }

    /// Puts back the operation `s` saved (the undo of its Abort, Continue or Skip): the branches
    /// and HEAD, then the files, the index and the operation's own files.
    pub fn pause_restore(&self, s: &Snapshot, p: &Paused) -> Result<(), String> {
        let gd = self.git_dir()?;
        let op = s.state.operation.as_deref().map(noun).unwrap_or("operation");
        if !p.dir.join("index").exists() {
            return Err(format!("The saved copy of the {op} is gone."));
        }
        match &p.after {
            Some((st, fp)) if *st == self.state() && *fp == self.worktree_fingerprint() => {}
            _ => return Err(format!("The repository changed after “{}”, so Undo leaves it as it is.", s.label)),
        }
        let old = &s.state;
        // 1. Branches and tags where they were (a Continue that finished moved the branch).
        let now = self.state();
        for (r, sha) in &now.refs {
            match old.refs.iter().find(|(o, _)| o == r) {
                None => {
                    self.git(&["update-ref", "-d", r, sha])?;
                }
                Some((_, o)) if o != sha => {
                    self.git(&["update-ref", r, o])?;
                }
                _ => {}
            }
        }
        for (r, sha) in &old.refs {
            if !now.refs.iter().any(|(n, _)| n == r) {
                self.git(&["update-ref", r, sha])?;
            }
        }
        // 2. HEAD on the branch, or detached on the commit, as during the operation.
        match &old.head_branch {
            Some(b) => self.git(&["symbolic-ref", "HEAD", &format!("refs/heads/{b}")])?,
            None => self.git(&["update-ref", "--no-deref", "HEAD", &old.head_sha])?,
        };
        // 3. The files of that commit, then the ones that differed from it.
        self.git(&["reset", "-q", "--hard"])?;
        let io = |e: std::io::Error| e.to_string();
        for (path, existed) in &p.files {
            let dst = self.root.join(path);
            if *existed {
                if dst.symlink_metadata().is_ok() {
                    std::fs::remove_file(&dst).map_err(io)?;
                }
                copy_any(&p.dir.join("files").join(path), &dst).map_err(io)?;
            } else if dst.symlink_metadata().is_ok() {
                std::fs::remove_file(&dst).map_err(io)?;
            }
        }
        // 4. The index with its conflicts, and the operation's files.
        std::fs::copy(p.dir.join("index"), gd.join("index")).map_err(io)?;
        let _ = self.git(&["update-index", "-q", "--refresh"]);
        for f in OP_FILES {
            let (here, saved) = (gd.join(f), p.dir.join("git").join(f));
            if here.is_dir() {
                std::fs::remove_dir_all(&here).map_err(io)?;
            } else if here.exists() {
                std::fs::remove_file(&here).map_err(io)?;
            }
            if saved.exists() {
                copy_any(&saved, &here).map_err(io)?;
            }
        }
        // 5. A rebase that finished re-applied its autostash, and put it in the stash list when
        //    that conflicted: the restored rebase holds it again, so that copy goes.
        let autostash = ["rebase-merge/autostash", "rebase-apply/autostash", "MERGE_AUTOSTASH"]
            .iter()
            .find_map(|f| std::fs::read_to_string(gd.join(f)).ok())
            .map(|s| s.trim().to_string());
        let now = self.state();
        if now.stashes.len() == old.stashes.len() + 1 && now.stashes[1..] == old.stashes[..] && autostash.as_deref() == Some(now.stashes[0].as_str()) {
            self.git(&["stash", "drop", "-q"])?;
        }
        Ok(())
    }
}

/// Copies a file, a symbolic link or a folder, creating the parent folders.
fn copy_any(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let meta = src.symlink_metadata()?;
    if meta.file_type().is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(src)?, dst)
    } else if meta.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_any(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}

/// The sample operation of `FOURCHETTE_OPERATION=<merge|rebase|cherry-pick|revert>[,resolved]`.
pub fn sample(v: &str) -> InProgress {
    let (op, resolved) = v.split_once(',').map_or((v, false), |(o, r)| (o, r.trim() == "resolved"));
    let op = match op.trim() {
        "merge" | "rebase" | "cherry-pick" | "revert" | "am" => op.trim(),
        _ => "merge",
    };
    let what = match op {
        "merge" => "Merge branch 'feature/graph-colours' into develop",
        "rebase" => "feature/search onto develop",
        "cherry-pick" => "ca2a04c Fix the search field's focus ring",
        "revert" => "9e1f3b2 Lazy-load the avatars",
        _ => "",
    };
    let conflicts = if resolved { vec![] } else { vec!["src/graph.rs".to_string(), "src/theme.rs".to_string()] };
    InProgress { op: op.into(), step: (op == "rebase").then_some((2, 5)), what: what.into(), conflicts }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::git::run_in;

    /// `main` and `topic` both changed line 2 of f.txt; `topic` has a second commit that adds
    /// g.txt. HEAD is on `main`.
    pub(crate) fn fixture(name: &str) -> (PathBuf, Repo) {
        let dir = std::env::temp_dir().join(format!("fourchette-op-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(dir.join("f.txt"), "1\n2\n3\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "base"]);
        g(&["checkout", "-q", "-b", "topic"]);
        std::fs::write(dir.join("f.txt"), "1\nTOPIC\n3\n").unwrap();
        g(&["commit", "-q", "-am", "topic edit"]);
        std::fs::write(dir.join("g.txt"), "g\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "topic adds g"]);
        g(&["checkout", "-q", "main"]);
        std::fs::write(dir.join("f.txt"), "1\nMAIN\n3\n").unwrap();
        g(&["commit", "-q", "-am", "main edit"]);
        let repo = Repo::open(&dir).unwrap();
        (dir, repo)
    }

    fn read(r: &Repo, f: &str) -> String {
        std::fs::read_to_string(r.root.join(f)).unwrap_or_default()
    }

    #[test]
    fn the_banner_words() {
        let p = sample("rebase");
        assert_eq!((p.title().as_str(), p.step, p.can_skip(), p.resolved()), ("Rebase in progress", Some((2, 5)), true, false));
        assert_eq!(sample("cherry-pick,resolved").title(), "Cherry-pick in progress");
        assert!(sample("merge,resolved").resolved() && !sample("merge").can_skip());
        assert_eq!(abort_question("merge"), "Do you want to abort the merge?");
        assert_eq!(abort_question("rebase"), "Do you want to abort the rebase?");
        let stopped = InProgress { op: "rebase".into(), step: Some((3, 5)), what: String::new(), conflicts: vec!["a".into()] };
        assert_eq!(outcome("merge", Action::Abort, Ok(String::new()), None), Ok("Aborted the merge".into()));
        assert_eq!(outcome("merge", Action::Continue, Ok(String::new()), None), Ok("Merge finished".into()));
        assert_eq!(outcome("rebase", Action::Skip, Ok(String::new()), None), Ok("Skipped the commit; the rebase finished".into()));
        assert_eq!(
            outcome("rebase", Action::Continue, Err("CONFLICT".into()), Some(&stopped)),
            Err("The rebase stopped on conflicts at step 3/5 in a. Resolve and stage them, then Continue.".into())
        );
        assert_eq!(outcome("merge", Action::Abort, Err("fatal: x".into()), None), Err("fatal: x".into()));
    }

    #[test]
    fn a_merge_aborted_comes_back_with_undo_as_it_was_left() {
        let (dir, repo) = fixture("merge-abort");
        assert!(repo.git(&["merge", "topic"]).is_err());
        let p = repo.in_progress().unwrap();
        assert_eq!((p.op.as_str(), p.what.as_str(), p.conflicts.clone()), ("merge", "Merge branch 'topic'", vec!["f.txt".to_string()]));
        // Half done: g.txt (from topic) edited by hand and staged, f.txt still has its markers.
        std::fs::write(repo.root.join("g.txt"), "g edited\n").unwrap();
        repo.git(&["add", "g.txt"]).unwrap();
        let conflicted = read(&repo, "f.txt");
        assert!(conflicted.contains("<<<<<<<"));
        let during = repo.state();

        let (snap, res) = repo.operation_step(Action::Abort);
        assert_eq!(res, Ok("Aborted the merge".into()));
        let snap = snap.expect("an undo step");
        assert_eq!(snap.label, "Abort merge");
        assert!(repo.in_progress().is_none());
        assert_eq!(read(&repo, "f.txt"), "1\nMAIN\n3\n");
        assert!(!repo.root.join("g.txt").exists());

        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), during);
        let p = repo.in_progress().unwrap();
        assert_eq!((p.op.as_str(), p.conflicts.clone()), ("merge", vec!["f.txt".to_string()]));
        assert_eq!(read(&repo, "f.txt"), conflicted);
        assert_eq!(read(&repo, "g.txt"), "g edited\n");
        assert!(repo.status().staged.iter().any(|f| f.path == "g.txt"), "still staged");
        // And it can still be finished.
        std::fs::write(repo.root.join("f.txt"), "1\nBOTH\n3\n").unwrap();
        repo.git(&["add", "-A"]).unwrap();
        let (_, res) = repo.operation_step(Action::Continue);
        assert_eq!(res, Ok("Merge finished".into()));
        assert_eq!(repo.git(&["log", "-1", "--format=%P"]).unwrap().split_whitespace().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn continue_needs_every_conflict_resolved_and_undo_takes_it_back() {
        let (dir, repo) = fixture("merge-continue");
        assert!(repo.git(&["merge", "topic"]).is_err());
        // Not resolved yet: Git refuses, nothing moves and there is no undo step.
        let before = repo.state();
        let (snap, res) = repo.operation_step(Action::Continue);
        assert!(snap.is_none() && res.is_err(), "{res:?}");
        assert_eq!(repo.state(), before);

        std::fs::write(repo.root.join("f.txt"), "1\nBOTH\n3\n").unwrap();
        repo.git(&["add", "f.txt"]).unwrap();
        assert!(repo.in_progress().unwrap().resolved());
        let during = repo.state();
        let (snap, res) = repo.operation_step(Action::Continue);
        assert_eq!(res, Ok("Merge finished".into()));
        assert!(repo.in_progress().is_none());
        assert_ne!(repo.state().head_sha, during.head_sha, "the merge commit");

        repo.undo(&snap.unwrap()).unwrap();
        assert_eq!(repo.state(), during);
        let p = repo.in_progress().unwrap();
        assert!(p.op == "merge" && p.resolved());
        assert_eq!(read(&repo, "f.txt"), "1\nBOTH\n3\n");
        assert!(repo.status().staged.iter().any(|f| f.path == "f.txt"), "the resolution is still staged");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rebase_skips_a_step_aborts_and_undo_brings_each_back() {
        let (dir, repo) = fixture("rebase");
        repo.git(&["checkout", "-q", "topic"]).unwrap();
        let topic = repo.state();
        assert!(repo.git(&["rebase", "main"]).is_err());
        let p = repo.in_progress().unwrap();
        assert_eq!((p.op.as_str(), p.step, p.what.as_str()), ("rebase", Some((1, 2)), "topic onto main"));
        assert_eq!(p.conflicts, ["f.txt"]);
        let during = repo.state();
        assert_eq!(during.head_branch, None, "detached while rebasing");

        // Skip the conflicting commit: the other one applies and the rebase ends.
        let (snap, res) = repo.operation_step(Action::Skip);
        assert_eq!(res, Ok("Skipped the commit; the rebase finished".into()));
        let snap = snap.unwrap();
        assert_eq!(snap.label, "Skip rebase step 1/2");
        assert!(repo.in_progress().is_none());
        assert_eq!(repo.current_branch().as_deref(), Some("topic"));
        assert_eq!(repo.git(&["log", "--format=%s", "-3"]).unwrap(), "topic adds g\nmain edit\nbase\n");

        // Undo: back at step 1/2, on the conflict, the branch where it was.
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), during);
        let p = repo.in_progress().unwrap();
        assert_eq!((p.step, p.conflicts.clone()), (Some((1, 2)), vec!["f.txt".to_string()]));

        // Abort: the branch as before the rebase. Undo of that: the rebase again.
        let (snap, res) = repo.operation_step(Action::Abort);
        assert_eq!(res, Ok("Aborted the rebase".into()));
        assert_eq!(repo.state(), topic);
        repo.undo(&snap.unwrap()).unwrap();
        assert_eq!(repo.state(), during);
        assert_eq!(repo.in_progress().unwrap().step, Some((1, 2)));
        assert!(read(&repo, "f.txt").contains("<<<<<<<"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_refuses_once_the_files_changed_after_an_abort() {
        let (dir, repo) = fixture("refuse");
        assert!(repo.git(&["merge", "topic"]).is_err());
        let (snap, _) = repo.operation_step(Action::Abort);
        std::fs::write(repo.root.join("f.txt"), "new work\n").unwrap();
        let e = repo.undo(&snap.unwrap()).unwrap_err();
        assert!(e.contains("changed after “Abort merge”"), "{e}");
        assert_eq!(read(&repo, "f.txt"), "new work\n");
        assert!(repo.in_progress().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cherry_pick_says_which_commit_and_aborts() {
        let (dir, repo) = fixture("pick");
        let sha = repo.git(&["rev-parse", "--short", "topic~1"]).unwrap();
        assert!(repo.git(&["cherry-pick", "topic~1"]).is_err());
        let p = repo.in_progress().unwrap();
        assert_eq!(p.op, "cherry-pick");
        assert_eq!(p.what, format!("{} topic edit", sha.trim()));
        assert!(!p.can_skip());
        let (snap, res) = repo.operation_step(Action::Abort);
        assert_eq!(res, Ok("Aborted the cherry-pick".into()));
        repo.undo(&snap.unwrap()).unwrap();
        assert_eq!(repo.in_progress().unwrap().op, "cherry-pick");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
