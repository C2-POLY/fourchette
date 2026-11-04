//! Git worktrees (Claude Code and others create many of them). A branch that is checked out in
//! a worktree can't be deleted until that worktree is removed, so the app removes it first.
use crate::git::{run_in, Repo};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    /// Short branch name, or None when detached.
    pub branch: Option<String>,
    pub head: String,
    /// The repository's own folder (the original's MainWorktreeSidebarItem).
    pub is_main: bool,
    /// Its folder no longer exists; `git worktree prune` cleans it up.
    pub missing: bool,
    pub locked: bool,
}

impl Worktree {
    /// The folder name, as the original shows it (`lastPathComponent`, sub_100070d94 / sub_1002008ec).
    pub fn name(&self) -> String {
        self.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.display().to_string())
    }
}

/// Parse `git worktree list --porcelain`.
pub fn parse(out: &str) -> Vec<Worktree> {
    let mut list = Vec::new();
    for block in out.split("\n\n") {
        let mut wt = Worktree { path: PathBuf::new(), branch: None, head: String::new(), is_main: list.is_empty(), missing: false, locked: false };
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                wt.path = PathBuf::from(p);
            } else if let Some(h) = line.strip_prefix("HEAD ") {
                wt.head = h.to_string();
            } else if let Some(b) = line.strip_prefix("branch ") {
                wt.branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
            } else if line.starts_with("prunable") {
                wt.missing = true;
            } else if line.starts_with("locked") {
                wt.locked = true;
            }
        }
        if !wt.path.as_os_str().is_empty() {
            if !wt.is_main && !wt.path.exists() {
                wt.missing = true;
            }
            list.push(wt);
        }
    }
    list
}

impl Repo {
    pub fn worktrees(&self) -> Vec<Worktree> {
        parse(&self.git(&["worktree", "list", "--porcelain"]).unwrap_or_default())
    }

    /// The linked worktree (not the main folder) where this branch is checked out.
    pub fn worktree_of(&self, branch: &str) -> Option<Worktree> {
        self.worktrees().into_iter().find(|w| !w.is_main && w.branch.as_deref() == Some(branch))
    }
}

/// Uncommitted work (including new files) in a worktree folder.
pub fn is_dirty(path: &Path) -> bool {
    path.exists() && run_in(path, &["status", "--porcelain", "--untracked-files=all"]).map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// Save a worktree's uncommitted work as a commit object (not on any branch) so Undo can bring
/// it back after the worktree is removed. Returns its id, or None when there was nothing.
pub fn save_changes(path: &Path) -> Option<String> {
    if !is_dirty(path) {
        return None;
    }
    run_in(path, &["add", "-A"]).ok()?;
    run_in(path, &["stash", "create", "Saved by Fourchette before removing this worktree"]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Remove a worktree. A missing folder is cleaned up with `prune`; uncommitted work needs `force`.
pub fn remove(repo: &Repo, wt: &Worktree, force: bool) -> Result<(), String> {
    if wt.is_main {
        return Err("The repository's main folder can't be removed.".into());
    }
    if wt.missing || !wt.path.exists() {
        repo.git(&["worktree", "prune"]).map(|_| ())
    } else {
        let p = wt.path.to_string_lossy().into_owned();
        let mut args = vec!["worktree", "remove"];
        if force {
            // Twice also removes a locked worktree.
            args.push("--force");
            if wt.locked {
                args.push("--force");
            }
        }
        args.push(&p);
        repo.git(&args).map(|_| ())
    }
}

/// What Undo needs to put a removed worktree back.
#[derive(Clone, Debug)]
pub struct Removed {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: String,
    pub saved: Option<String>,
}

/// Remove the worktree, keeping what Undo needs (its uncommitted work is saved first).
pub fn remove_for_undo(repo: &Repo, wt: &Worktree) -> Result<Removed, String> {
    let saved = if wt.missing { None } else { save_changes(&wt.path) };
    remove(repo, wt, saved.is_some() || wt.locked)?;
    Ok(Removed { path: wt.path.clone(), branch: wt.branch.clone(), head: wt.head.clone(), saved })
}

/// Delete a local branch, removing the worktree it is checked out in first (Git refuses to
/// delete a branch used by a worktree). `-d` for merged branches, `-D` when `force`.
/// Returns the removed worktree (for Undo) and the result of the branch deletion.
pub fn delete_branch(repo: &Repo, name: &str, force: bool, remove_worktree: bool) -> (Option<Removed>, Result<(), String>) {
    let mut removed = None;
    if let Some(wt) = repo.worktree_of(name) {
        if !remove_worktree {
            return (None, Err(format!("“{name}” is checked out in the worktree “{}”. Remove the worktree first.", wt.name())));
        }
        match remove_for_undo(repo, &wt) {
            Ok(r) => removed = Some(r),
            Err(e) => return (None, Err(format!("Couldn't remove the worktree “{}”: {e}", wt.name()))),
        }
    }
    let res = repo.git(&["branch", if force { "-D" } else { "-d" }, name]).map(|_| ());
    (removed, res)
}

/// Undo: re-create the worktree on its branch and re-apply its saved work.
pub fn restore(repo: &Repo, r: &Removed) -> Result<(), String> {
    if r.path.exists() {
        return Err(format!("{} already exists", r.path.display()));
    }
    let p = r.path.to_string_lossy().into_owned();
    match &r.branch {
        Some(b) => repo.git(&["worktree", "add", &p, b])?,
        None => repo.git(&["worktree", "add", "--detach", &p, &r.head])?,
    };
    if let Some(sha) = &r.saved {
        run_in(&r.path, &["stash", "apply", "-q", sha])?;
        let _ = run_in(&r.path, &["reset", "-q"]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain() {
        let out = "worktree /repo\nHEAD aaa\nbranch refs/heads/main\n\nworktree /repo/.claude/worktrees/fix\nHEAD bbb\nbranch refs/heads/claude/fix\nlocked\n\nworktree /gone/away\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n";
        let w = parse(out);
        assert_eq!(w.len(), 3);
        assert!(w[0].is_main && w[0].branch.as_deref() == Some("main"));
        assert_eq!(w[1].branch.as_deref(), Some("claude/fix"));
        assert!(w[1].locked && !w[1].is_main);
        assert_eq!(w[1].name(), "fix");
        assert!(w[2].missing && w[2].branch.is_none());
    }

    #[test]
    fn delete_branch_in_worktree_and_undo() {
        let dir = std::env::temp_dir().join(format!("fourchette-wt-{}", std::process::id()));
        let wt_dir = dir.with_extension("wt");
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(dir.join("a.txt"), "1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "first"]);
        g(&["worktree", "add", "-q", "-b", "claude/task", wt_dir.to_str().unwrap()]);
        // Uncommitted work inside the worktree.
        std::fs::write(wt_dir.join("note.txt"), "draft\n").unwrap();
        let repo = Repo::open(&dir).unwrap();

        // Git alone refuses: the branch is used by a worktree.
        assert!(repo.git(&["branch", "-d", "claude/task"]).is_err());
        let wt = repo.worktree_of("claude/task").expect("found the worktree");
        assert!(is_dirty(&wt.path));

        // Remove the worktree first (saving its work), then the branch goes.
        let removed = remove_for_undo(&repo, &wt).unwrap();
        assert!(removed.saved.is_some());
        assert!(!wt_dir.exists());
        repo.git(&["branch", "-d", "claude/task"]).unwrap();
        assert!(repo.worktree_of("claude/task").is_none());

        // Undo: branch back, worktree back, its draft back.
        repo.git(&["branch", "claude/task", "main"]).unwrap();
        restore(&repo, &removed).unwrap();
        assert_eq!(std::fs::read_to_string(wt_dir.join("note.txt")).unwrap(), "draft\n");
        assert_eq!(repo.worktree_of("claude/task").map(|w| w.path.canonicalize().unwrap()), Some(wt_dir.canonicalize().unwrap()));

        // A worktree whose folder was deleted by hand is cleaned up with prune.
        std::fs::remove_dir_all(&wt_dir).unwrap();
        let gone = repo.worktree_of("claude/task").unwrap();
        assert!(gone.missing);
        remove(&repo, &gone, false).unwrap();
        assert!(repo.worktree_of("claude/task").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
