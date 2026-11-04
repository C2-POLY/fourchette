//! Universal undo: a snapshot of branches, tags, stashes and HEAD is taken before each local
//! action, and Undo puts the repository back the way it was. (New feature, not in the original.)
use crate::git::Repo;
use std::path::PathBuf;

/// How many actions can be undone in a row.
pub const UNDO_DEPTH: usize = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub head_branch: Option<String>,
    pub head_sha: String,
    /// (full ref name, sha) for refs/heads and refs/tags, sorted.
    pub refs: Vec<(String, String)>,
    /// Stash commit shas, newest first.
    pub stashes: Vec<String>,
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
    /// Worktrees removed by the action (with their saved uncommitted work).
    pub worktrees: Vec<crate::worktree::Removed>,
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
        State { head_branch, head_sha, refs, stashes }
    }

    pub fn snapshot(&self, label: &str, soft: bool, save_worktree: bool) -> Snapshot {
        let worktree = if save_worktree {
            self.git(&["stash", "create"]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
        } else {
            None
        };
        Snapshot { label: label.to_string(), state: self.state(), soft, worktree, trashed: None, worktrees: Vec::new() }
    }

    /// Put the repository back to the snapshot. Returns what was undone.
    pub fn undo(&self, s: &Snapshot) -> Result<String, String> {
        let old = &s.state;
        // A new file sent to the Trash comes back first.
        if let Some((path, in_trash)) = &s.trashed {
            let dst = self.root.join(path);
            if let Some(dir) = dst.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::fs::rename(in_trash, &dst).map_err(|e| format!("Couldn't bring {path} back from the Trash: {e}"))?;
        }
        let now = self.state();

        // 1. Go back to the branch (or detached commit) that was checked out.
        if now.head_branch != old.head_branch {
            match &old.head_branch {
                Some(b) => {
                    if let Some((_, sha)) = old.refs.iter().find(|(r, _)| r == &format!("refs/heads/{b}")) {
                        self.git(&["update-ref", &format!("refs/heads/{b}"), sha])?;
                    }
                    self.git(&["checkout", b])?;
                }
                None => {
                    self.git(&["checkout", "--detach", &old.head_sha])?;
                }
            }
        }
        // 2. Move the checked-out branch back to where it was.
        let head_now = self.git(&["rev-parse", "-q", "--verify", "HEAD"]).unwrap_or_default().trim().to_string();
        if !old.head_sha.is_empty() && head_now != old.head_sha {
            let mode = if s.soft { "--soft" } else { "--keep" };
            self.git(&["reset", "-q", mode, &old.head_sha])?;
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
        // 4. Stashes: a new stash is popped back into the files, a removed one is stored again.
        let now = self.state();
        if now.stashes.len() > old.stashes.len() && now.stashes.get(1..) == Some(&old.stashes[..]) {
            self.git(&["stash", "pop", "-q"])?;
        } else {
            for sha in old.stashes.iter().rev() {
                if !now.stashes.contains(sha) {
                    self.git(&["stash", "store", "-m", "Restored by Undo", sha])?;
                }
            }
        }
        // 5. Discarded changes come back.
        if let Some(w) = &s.worktree {
            self.git(&["stash", "apply", "-q", w])?;
        }
        Ok(format!("Undid: {}", s.label))
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
