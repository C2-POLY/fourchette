//! Universal undo: a snapshot of branches, tags, stashes and HEAD is taken before each local
//! action, and Undo puts the repository back the way it was. (New feature, not in the original.)
use crate::git::Repo;
use std::path::PathBuf;

/// How many actions can be undone in a row.
pub const UNDO_DEPTH: usize = 30;

/// `Snapshot::left` of a `merge --squash` stopped on conflicts, which `Repo::operation` cannot see.
pub const SQUASH: &str = "squash";

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub head_branch: Option<String>,
    pub head_sha: String,
    /// (full ref name, sha) for refs/heads and refs/tags, sorted.
    pub refs: Vec<(String, String)>,
    /// Stash commit shas, newest first.
    pub stashes: Vec<String>,
    /// A rebase or merge (…) stopped on conflicts, as `Repo::operation` names it.
    pub operation: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub label: String,
    pub state: State,
    /// Undo a commit with `reset --soft` so its changes come back staged.
    pub soft: bool,
    /// Working-copy changes saved with `git stash create` before a discard.
    pub worktree: Option<String>,
    /// A new file that was moved to the Trash: (path in the repository, path in the Trash).
    pub trashed: Option<(String, PathBuf)>,
    /// A new file the action wrote (CHANGELOG.md): Undo moves it to the Trash.
    pub written: Option<String>,
    /// Files the action marked resolved (`git add` during a conflict): Undo puts them back in conflict.
    pub resolved: Vec<String>,
    /// Worktrees removed by the action (with their saved uncommitted work).
    pub worktrees: Vec<crate::worktree::Removed>,
    /// Uncommitted changes before a pull or a checkout-and-sync (`git stash create`). One that
    /// put them aside and back moved them onto the new commits, which `reset --keep` (or the
    /// checkout back) refuses to take back; Undo then resets with `--hard` and re-applies these,
    /// as long as the changes are still as the action left them (`dirty_after`).
    pub dirty: Option<String>,
    /// `Repo::worktree_fingerprint` right after the pull.
    pub dirty_after: Option<u64>,
    /// The operation the action left stopped on conflicts (`Repo::operation`), which Undo aborts.
    pub left: Option<String>,
}

impl Repo {
    pub fn state(&self) -> State {
        let head_branch = self.current_branch();
        let head_sha = self.git(&["rev-parse", "-q", "--verify", "HEAD"]).unwrap_or_default().trim().to_string();
        let mut refs: Vec<(String, String)> = self
            .git(&["for-each-ref", "--format=%(refname) %(objectname)", "refs/heads", "refs/tags"])
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once(' ').map(|(a, b)| (a.to_string(), b.to_string())))
            .collect();
        refs.sort();
        let stashes = self.git(&["stash", "list", "--format=%H"]).unwrap_or_default().lines().map(String::from).collect();
        State { head_branch, head_sha, refs, stashes, operation: self.operation() }
    }

    pub fn snapshot(&self, label: &str, soft: bool, save_worktree: bool) -> Snapshot {
        let worktree = if save_worktree { self.stash_create() } else { None };
        Snapshot { label: label.to_string(), state: self.state(), soft, worktree, trashed: None, written: None, resolved: Vec::new(), worktrees: Vec::new(), dirty: None, dirty_after: None, left: None }
    }

    /// Notes, right after the action, what Undo needs to know of its result: the operation it
    /// left stopped on conflicts, and the state of the uncommitted changes the snapshot saved.
    pub fn after(&self, s: &mut Snapshot) {
        s.left = self.operation().filter(|_| s.state.operation.is_none());
        if s.dirty.is_some() {
            s.dirty_after = Some(self.worktree_fingerprint());
        }
    }

    /// Put the repository back to the snapshot. Returns what was undone.
    pub fn undo(&self, s: &Snapshot) -> Result<String, String> {
        let old = &s.state;
        // A file the action wrote goes to the Trash, then one it replaced comes back.
        if let Some(path) = &s.written {
            if self.root.join(path).exists() {
                crate::git::trash(&self.root, path)?;
            }
        }
        // Files marked resolved go back in conflict (from Git's resolve-undo record).
        for path in &s.resolved {
            self.git(&["update-index", "--unresolve", "--", path])?;
        }
        // A new file sent to the Trash comes back first.
        if let Some((path, in_trash)) = &s.trashed {
            let dst = self.root.join(path);
            if let Some(dir) = dst.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::fs::rename(in_trash, &dst).map_err(|e| format!("Couldn't bring {path} back from the Trash: {e}"))?;
        }
        // 0. An operation the action left stopped on conflicts (a pull's rebase or merge) is
        //    aborted: Git puts back the branch, and the changes it put aside (`--autostash`).
        //    Only that one: a rebase started in a terminal since is the person's.
        if let Some(op) = s.left.as_ref().filter(|op| self.operation().as_ref() == Some(*op)) {
            self.git(&[op, "--abort"])?;
        }
        // A squash stopped on conflicts leaves no MERGE_HEAD to abort, only conflicted files:
        // `reset --merge` takes them back as `merge --abort` would, other local edits kept.
        if s.left.as_deref() == Some(SQUASH) && self.operation().is_none() && !self.conflicted_files().is_empty() {
            self.git(&["reset", "-q", "--merge"])?;
        }
        let now = self.state();

        // 1. Go back to the branch (or detached commit) that was checked out. A checkout that put
        //    the uncommitted changes back on the other branch's new commits blocks it: see
        //    `reset_dirty`.
        let mut put_back = None;
        if now.head_branch != old.head_branch {
            let switch: Vec<&str> = match &old.head_branch {
                Some(b) => {
                    if let Some((_, sha)) = old.refs.iter().find(|(r, _)| r == &format!("refs/heads/{b}")) {
                        self.git(&["update-ref", &format!("refs/heads/{b}"), sha])?;
                    }
                    vec!["checkout", b.as_str()]
                }
                None => vec!["checkout", "--detach", old.head_sha.as_str()],
            };
            if let Err(e) = self.git(&switch) {
                put_back = Some(self.reset_dirty(s, e, &switch)?);
            }
        }
        // 2. Move the checked-out branch back to where it was.
        let head_now = self.git(&["rev-parse", "-q", "--verify", "HEAD"]).unwrap_or_default().trim().to_string();
        if !old.head_sha.is_empty() && head_now != old.head_sha {
            let mode = if s.soft { "--soft" } else { "--keep" };
            if let Err(e) = self.git(&["reset", "-q", mode, &old.head_sha]) {
                put_back = Some(self.reset_dirty(s, e, &[])?);
            }
        }
        // 3. Every other branch and tag back to its old place; new ones removed, removed ones restored.
        let current = self.current_branch().map(|b| format!("refs/heads/{b}"));
        let now = self.state();
        for (r, sha) in &now.refs {
            if Some(r) == current.as_ref() {
                continue;
            }
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
            if Some(r) != current.as_ref() && !now.refs.iter().any(|(n, _)| n == r) {
                self.git(&["update-ref", r, sha])?;
            }
        }
        // 3b. Removed worktrees come back on their branch, with their uncommitted work.
        for w in &s.worktrees {
            crate::worktree::restore(self, w)?;
        }
        // 4. Stashes: a new stash is popped back into the files, a removed one is stored again
        //    under its own message. After `reset_dirty` the pull's autostash holds the changes
        //    just re-applied, so it is dropped instead.
        let now = self.state();
        if now.stashes.len() > old.stashes.len() && now.stashes.get(1..) == Some(&old.stashes[..]) {
            if put_back.as_deref().is_some_and(|w| self.same_tree(&now.stashes[0], w)) {
                self.git(&["stash", "drop", "-q"])?;
            } else {
                self.git(&["stash", "pop", "-q"])?;
            }
        } else {
            for sha in old.stashes.iter().rev() {
                if !now.stashes.contains(sha) {
                    let msg = self.git(&["log", "-1", "--format=%s", sha]).map(|m| m.trim().to_string()).unwrap_or_default();
                    let msg = if msg.is_empty() { "Restored by Undo".to_string() } else { msg };
                    self.git(&["stash", "store", "-m", &msg, sha])?;
                }
            }
        }
        // 5. Discarded changes come back.
        if let Some(w) = &s.worktree {
            self.git(&["stash", "apply", "-q", w])?;
        }
        Ok(format!("Undid: {}", s.label))
    }

    /// Takes the branch back when `reset --keep` refused (`err`) because a pull moved the
    /// uncommitted changes onto its commits: `reset --hard`, then the changes as they were before
    /// the pull. Only when they are still as the pull left them, so no later edit is lost.
    /// With a `switch` (the checkout back to the branch a checkout-and-sync left), which refused
    /// for the same reason: `reset --hard` where it is, then that checkout, then the changes.
    /// Returns the stash commit re-applied.
    fn reset_dirty(&self, s: &Snapshot, err: String, switch: &[&str]) -> Result<String, String> {
        let Some(dirty) = s.dirty.as_ref().filter(|_| !s.soft) else { return Err(err) };
        if s.dirty_after != Some(self.worktree_fingerprint()) {
            return Err(format!("Your uncommitted changes were edited after “{}”. Commit or stash them, then Undo again.", s.label));
        }
        if switch.is_empty() {
            self.git(&["reset", "-q", "--hard", &s.state.head_sha])?;
        } else {
            self.git(&["reset", "-q", "--hard"])?;
            self.git(switch)?;
        }
        if let Err(e) = self.git(&["stash", "apply", "-q", dirty]) {
            // Never leave them only in a commit nobody points at.
            let _ = self.git(&["stash", "store", "-m", &format!("Changes before {}", s.label), dirty]);
            return Err(format!("{e}\nYour changes from before “{}” are in the stash list.", s.label));
        }
        Ok(dirty.clone())
    }

    fn same_tree(&self, a: &str, b: &str) -> bool {
        let tree = |c: &str| self.git(&["rev-parse", &format!("{c}^{{tree}}")]).ok().map(|t| t.trim().to_string());
        tree(a).is_some() && tree(a) == tree(b)
    }
}

#[cfg(test)]
mod tests {
    use crate::git::{run_in, Repo};

    #[test]
    fn undo_puts_things_back() {
        let dir = std::env::temp_dir().join(format!("fourchette-undo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "first"]);
        let repo = Repo::open(&dir).unwrap();
        let first = repo.state();

        // Commit -> undo brings the change back, staged.
        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        g(&["add", "-A"]);
        let s = repo.snapshot("Commit", true, false);
        repo.commit("second\n", false, 1).unwrap();
        repo.undo(&s).unwrap();
        assert_eq!(repo.state(), first);
        assert!(repo.status().staged.iter().any(|f| f.path == "a.txt"));
        g(&["commit", "-q", "-m", "second"]);
        let second = repo.state();

        // New branch + checkout -> undo returns to main and removes it.
        let s = repo.snapshot("Created dev", false, false);
        g(&["checkout", "-q", "-b", "dev"]);
        repo.undo(&s).unwrap();
        assert_eq!(repo.state(), second);

        // Tag delete -> undo restores it.
        g(&["tag", "v1"]);
        let tagged = repo.state();
        let s = repo.snapshot("Deleted tag v1", false, false);
        g(&["tag", "-d", "v1"]);
        repo.undo(&s).unwrap();
        assert_eq!(repo.state(), tagged);

        // Hard reset -> undo moves the branch forward again.
        let s = repo.snapshot("Reset", false, false);
        g(&["reset", "-q", "--hard", "HEAD~1"]);
        repo.undo(&s).unwrap();
        assert_eq!(repo.state(), tagged);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "two\n");

        // Stash -> undo pops the changes back.
        std::fs::write(dir.join("a.txt"), "three\n").unwrap();
        let s = repo.snapshot("Stash", false, false);
        g(&["stash", "push", "-q", "-u"]);
        repo.undo(&s).unwrap();
        assert_eq!(repo.state(), tagged);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "three\n");

        // Discard -> undo restores the edit.
        let s = repo.snapshot("Discard a.txt", false, true);
        g(&["checkout", "--", "a.txt"]);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "two\n");
        repo.undo(&s).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "three\n");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod worktree_tests {
    use crate::git::{run_in, Repo};

    #[test]
    fn undo_brings_back_branch_and_its_worktree() {
        let dir = std::env::temp_dir().join(format!("fourchette-undo-wt-{}", std::process::id()));
        let wt_dir = dir.with_extension("task");
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(dir.join("a.txt"), "1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "first"]);
        g(&["worktree", "add", "-q", "-b", "claude/task", wt_dir.to_str().unwrap()]);
        std::fs::write(wt_dir.join("draft.txt"), "wip\n").unwrap();
        let repo = Repo::open(&dir).unwrap();
        let before = repo.state();

        let mut snap = repo.snapshot("Delete branch claude/task", false, false);
        let (removed, res) = crate::worktree::delete_branch(&repo, "claude/task", false, true);
        res.unwrap();
        snap.worktrees.extend(removed);
        assert!(!wt_dir.exists());
        assert!(repo.state().refs.iter().all(|(r, _)| r != "refs/heads/claude/task"));

        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert_eq!(std::fs::read_to_string(wt_dir.join("draft.txt")).unwrap(), "wip\n");
        assert!(repo.worktree_of("claude/task").is_some());
        let _ = std::fs::remove_dir_all(&wt_dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod pull_tests {
    use super::Snapshot;
    use crate::git::{exec, pull_args, pull_outcome, run_in, Repo};
    use std::path::{Path, PathBuf};

    /// A clone `a` with a commit of its own (g.txt), behind `origin/main`, which added line 4
    /// ("B") to f.txt. Returns the folder holding everything, and `a`.
    fn fixture(name: &str) -> (PathBuf, Repo) {
        let base = std::env::temp_dir().join(format!("fourchette-pull-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let (a, b, remote) = (base.join("a"), base.join("b"), base.join("remote.git"));
        let g = |dir: &Path, args: &[&str]| run_in(dir, args).unwrap();
        g(&base, &["init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
        g(&base, &["init", "-q", "-b", "main", a.to_str().unwrap()]);
        g(&a, &["config", "user.name", "A"]);
        g(&a, &["config", "user.email", "a@example.com"]);
        std::fs::write(a.join("f.txt"), "1\n2\n3\n").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "base"]);
        g(&a, &["remote", "add", "origin", remote.to_str().unwrap()]);
        g(&a, &["push", "-q", "-u", "origin", "main"]);
        g(&base, &["clone", "-q", remote.to_str().unwrap(), b.to_str().unwrap()]);
        g(&b, &["config", "user.name", "B"]);
        g(&b, &["config", "user.email", "b@example.com"]);
        std::fs::write(b.join("f.txt"), "1\n2\n3\nB\n").unwrap();
        g(&b, &["commit", "-q", "-am", "remote change"]);
        g(&b, &["push", "-q"]);
        std::fs::write(a.join("g.txt"), "local\n").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "local commit"]);
        (base, Repo::open(&a).unwrap())
    }

    /// What `Tab::pull` and `poll_busy` do, without the thread.
    fn pull(repo: &Repo, rebase: bool, autostash: bool) -> (Snapshot, Result<String, String>) {
        let mut snap = Snapshot { dirty: repo.stash_create(), ..repo.snapshot("Pull", false, false) };
        let args = pull_args("origin", "main", rebase, autostash);
        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let res = exec(&repo.root, &a).map(|(o, e)| format!("{o}{e}"));
        let res = pull_outcome(res, repo.operation().as_deref(), &repo.conflicted_files(), "origin", "main");
        repo.after(&mut snap);
        (snap, res)
    }

    fn read(repo: &Repo, f: &str) -> String {
        std::fs::read_to_string(repo.root.join(f)).unwrap()
    }

    fn mode(rebase: bool) -> &'static str {
        if rebase { "rebase" } else { "merge" }
    }

    #[test]
    fn undo_takes_back_a_clean_pull_in_both_modes() {
        for rebase in [false, true] {
            let (base, repo) = fixture(&format!("clean-{}", mode(rebase)));
            let before = repo.state();
            let (snap, res) = pull(&repo, rebase, false);
            assert_eq!(res, Ok("Pulled origin/main".into()), "{}", mode(rebase));
            assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nB\n");
            let log = repo.git(&["log", "--format=%s", "-3"]).unwrap();
            if rebase {
                assert_eq!(log, "local commit\nremote change\nbase\n", "rebased on top, no merge commit");
            } else {
                assert!(log.starts_with("Merge branch 'main'"), "{log}");
            }
            repo.undo(&snap).unwrap();
            assert_eq!(repo.state(), before, "{}", mode(rebase));
            assert_eq!(read(&repo, "f.txt"), "1\n2\n3\n");
            assert!(repo.status().is_empty());
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn without_stash_and_reapply_uncommitted_changes_stop_a_rebase_pull() {
        let (base, repo) = fixture("dirty-off");
        std::fs::write(repo.root.join("f.txt"), "X\n2\n3\n").unwrap();
        let before = repo.state();
        let (_, res) = pull(&repo, true, false);
        let e = res.unwrap_err();
        assert!(e.contains("Stash and reapply"), "{e}");
        assert_eq!(repo.state(), before, "nothing moved");
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn undo_after_stash_and_reapply_brings_back_the_changes_as_they_were() {
        for rebase in [false, true] {
            // The changed file is also changed by the pull: `reset --keep` alone would refuse.
            let (base, repo) = fixture(&format!("autostash-{}", mode(rebase)));
            std::fs::write(repo.root.join("f.txt"), "X\n2\n3\n").unwrap();
            let before = repo.state();
            let (snap, res) = pull(&repo, rebase, true);
            assert_eq!(res, Ok("Pulled origin/main".into()), "{}", mode(rebase));
            assert_eq!(read(&repo, "f.txt"), "X\n2\n3\nB\n", "changes reapplied on top of the pull");
            assert!(repo.state().stashes.is_empty());
            repo.undo(&snap).unwrap();
            assert_eq!(repo.state(), before, "{}", mode(rebase));
            assert_eq!(read(&repo, "f.txt"), "X\n2\n3\n");
            assert_eq!(repo.status().unstaged.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["f.txt"]);
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn undo_refuses_rather_than_lose_edits_made_after_the_pull() {
        let (base, repo) = fixture("edited-after");
        std::fs::write(repo.root.join("f.txt"), "X\n2\n3\n").unwrap();
        let (snap, res) = pull(&repo, true, true);
        res.unwrap();
        std::fs::write(repo.root.join("f.txt"), "X\n2\n3\nB\nlater\n").unwrap();
        let after = repo.state();
        let e = repo.undo(&snap).unwrap_err();
        assert!(e.contains("edited after “Pull”"), "{e}");
        assert_eq!(repo.state(), after, "nothing moved");
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\nB\nlater\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn changes_that_conflict_on_reapply_stay_in_the_stash_and_undo_restores_them() {
        for rebase in [false, true] {
            let (base, repo) = fixture(&format!("reapply-conflict-{}", mode(rebase)));
            std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nLOCAL\n").unwrap();
            let before = repo.state();
            let (snap, res) = pull(&repo, rebase, true);
            let e = res.unwrap_err();
            assert!(e.contains("conflict with it in f.txt") && e.contains("stash list"), "{e}");
            assert_eq!(repo.state().stashes.len(), 1, "Git keeps the changes in the stash");
            assert_eq!(repo.conflicted_files(), ["f.txt"]);
            repo.undo(&snap).unwrap();
            assert_eq!(repo.state(), before, "{}: no stash left, branch back", mode(rebase));
            assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nLOCAL\n");
            assert!(repo.conflicted_files().is_empty());
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn a_pull_stopped_on_conflicts_is_aborted_by_undo() {
        for rebase in [false, true] {
            let (base, repo) = fixture(&format!("pull-conflict-{}", mode(rebase)));
            std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nCOMMIT\n").unwrap();
            repo.git(&["commit", "-q", "-am", "edit line 4"]).unwrap();
            std::fs::write(repo.root.join("g.txt"), "dirty\n").unwrap();
            let before = repo.state();
            let (snap, res) = pull(&repo, rebase, true);
            let e = res.unwrap_err();
            assert!(e.starts_with(&format!("Pull stopped on conflicts in f.txt (a {} is in progress)", mode(rebase))), "{e}");
            assert_eq!(repo.state().operation.as_deref(), Some(mode(rebase)));
            repo.undo(&snap).unwrap();
            assert_eq!(repo.state(), before, "{}", mode(rebase));
            assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nCOMMIT\n");
            assert_eq!(read(&repo, "g.txt"), "dirty\n", "the change put aside is back");
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn undo_leaves_alone_a_rebase_started_in_a_terminal_since() {
        let (base, repo) = fixture("terminal-rebase");
        std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nCOMMIT\n").unwrap();
        let mut snap = repo.snapshot("Commit", true, false);
        repo.git(&["commit", "-q", "-am", "edit line 4"]).unwrap();
        repo.after(&mut snap);
        assert_eq!(snap.left, None);
        // Then, in a terminal: a rebase that stops on conflicts, half resolved.
        repo.git(&["fetch", "-q"]).unwrap();
        assert!(repo.git(&["rebase", "origin/main"]).is_err());
        std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nresolved by hand\n").unwrap();
        let _ = repo.undo(&snap);
        assert_eq!(repo.operation().as_deref(), Some("rebase"), "not aborted");
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nresolved by hand\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// What the branch menu's fast-forward entry runs, through `Tab::git` (snapshot, git, undo).
    fn fast_forward(repo: &Repo, name: &str) -> Result<(Snapshot, Result<String, String>), String> {
        let refs = repo.refs();
        let b = refs.iter().find(|r| r.name == name).unwrap();
        let ff = crate::ui::fast_forward(b, &refs, &repo.worktrees()).ok_or("no entry")?;
        let crate::app::Cmd::Git { label, args, network: false } = ff.action? else { panic!("not a local Git command") };
        let snap = repo.snapshot(&label, false, false);
        let res = repo.git(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        Ok((snap, res))
    }

    #[test]
    fn fast_forward_moves_a_branch_and_undo_puts_it_back() {
        // `behind` tracks origin/main from before the fetch: 1 behind. `main` has diverged.
        let (base, repo) = fixture("fast-forward");
        repo.git(&["branch", "-q", "--track", "behind", "origin/main"]).unwrap();
        repo.git(&["fetch", "-q"]).unwrap();
        let refs = repo.refs();
        let get = |n: &str| refs.iter().find(|r| r.name == n).unwrap().clone();
        assert_eq!((get("behind").ahead, get("behind").behind), (0, 1));
        assert_eq!((get("main").ahead, get("main").behind), (1, 1));
        assert_eq!(fast_forward(&repo, "main").err().as_deref(), Some("no entry"), "diverged: no entry");
        let remote = repo.git(&["rev-parse", "origin/main"]).unwrap();
        let sha = |r: &str| repo.git(&["rev-parse", r]).unwrap();

        // Not checked out: the branch moves, nothing else does (no checkout, files untouched).
        let before = repo.state();
        std::fs::remove_file(repo.root.join(".git/FETCH_HEAD")).unwrap();
        let (snap, res) = fast_forward(&repo, "behind").unwrap();
        res.unwrap();
        assert_eq!(sha("behind"), remote);
        assert_eq!(repo.current_branch().as_deref(), Some("main"));
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\n");
        assert!(repo.status().is_empty());
        assert!(!repo.root.join(".git/FETCH_HEAD").exists(), "FETCH_HEAD left alone");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before, "the branch is back");

        // Checked out in another worktree: greyed out, and Git refuses it anyway.
        let wt = base.join("wt");
        repo.git(&["worktree", "add", "-q", wt.to_str().unwrap(), "behind"]).unwrap();
        let e = fast_forward(&repo, "behind").err().unwrap();
        assert!(e.contains("checked out in the worktree “wt”"), "{e}");
        let args = crate::git::fast_forward_args("behind", "refs/remotes/origin/main", false);
        assert!(repo.git(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()).is_err());
        repo.git(&["worktree", "remove", wt.to_str().unwrap()]).unwrap();

        // The current branch: `merge --ff-only`. Local changes in the way: refused, nothing moves.
        repo.git(&["checkout", "-q", "behind"]).unwrap();
        std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nmine\n").unwrap();
        let before = repo.state();
        let (_, res) = fast_forward(&repo, "behind").unwrap();
        assert!(res.unwrap_err().contains("would be overwritten"));
        assert_eq!(repo.state(), before);
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nmine\n");
        // Clean: the branch and the files move; Undo takes both back.
        repo.git(&["checkout", "--", "f.txt"]).unwrap();
        let (snap, res) = fast_forward(&repo, "behind").unwrap();
        res.unwrap();
        assert_eq!(sha("HEAD"), remote);
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nB\n");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn undo_of_apply_and_delete_stores_the_stash_again_under_its_name() {
        let (base, repo) = fixture("apply-delete");
        std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nwip\n").unwrap();
        repo.git(&["stash", "push", "-q", "-m", "my work"]).unwrap();
        std::fs::write(repo.root.join("h.txt"), "other\n").unwrap();
        repo.git(&["add", "h.txt"]).unwrap();
        repo.git(&["stash", "push", "-q", "-m", "older"]).unwrap();
        let before = repo.state();
        let list = |r: &Repo| r.git(&["stash", "list", "--format=%s"]).unwrap();
        let names_before = list(&repo);
        // What the Apply button runs with "Delete stash after Apply" on, through `Tab::git`.
        let crate::app::Cmd::Git { args, .. } = crate::app::Cmd::apply_stash("stash@{1}", true) else { panic!() };
        let snap = repo.snapshot("Stash applied and deleted", false, false);
        repo.git(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()).unwrap();
        assert_eq!(repo.state().stashes.len(), 1);
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nwip\n");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state().stashes.len(), 2);
        assert!(before.stashes.iter().all(|s| repo.state().stashes.contains(s)), "the same stash commits");
        assert!(list(&repo).lines().any(|l| l == "On main: my work"), "{}\nwas\n{names_before}", list(&repo));
        let _ = std::fs::remove_dir_all(&base);
    }
}
