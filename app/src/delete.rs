//! The Delete branch dialog's model: a local branch, its remote branch, or both. The local one
//! is deleted with a Snapshot (⌘Z brings it back); the remote one with `git push <remote>
//! --delete`, which touches the remote for everyone and is asked twice.

use crate::git::{GitRef, RefKind};
use crate::worktree::Worktree;

/// The local side of the dialog: the branch and what Git needs to delete it.
#[derive(Clone, Debug)]
pub struct Local {
    pub name: String,
    /// Merged into the current branch: `-d` is enough.
    pub merged: bool,
    /// Not merged: the person ticked "Delete even though…", so `-D`.
    pub force: bool,
    pub worktree: Option<Worktree>,
    pub dirty: bool,
    pub also_worktree: bool,
}

/// The remote side: `origin/feat/x` is `remote` "origin" and `branch` "feat/x".
#[derive(Clone, Debug, PartialEq)]
pub struct Remote {
    pub remote: String,
    pub branch: String,
    /// Its open pull request, "#n title": GitHub closes it when the branch goes.
    pub open_pull: Option<String>,
}

impl Remote {
    /// `origin/feat/x`, split on its first slash like the rest of the app.
    pub fn of(name: &str) -> Option<Remote> {
        let (remote, branch) = name.split_once('/')?;
        (!branch.is_empty() && branch != "HEAD").then(|| Remote { remote: remote.into(), branch: branch.into(), open_pull: None })
    }

    pub fn name(&self) -> String {
        format!("{}/{}", self.remote, self.branch)
    }

    pub fn args(&self) -> Vec<String> {
        vec!["push".into(), self.remote.clone(), "--delete".into(), self.branch.clone()]
    }
}

#[derive(Clone, Debug)]
pub struct Delete {
    pub local: Option<Local>,
    pub remote: Option<Remote>,
    pub del_local: bool,
    pub del_remote: bool,
    /// The first click on the main action, when it deletes on the remote: the second one does it.
    pub confirm: bool,
}

/// The local branch a remote branch belongs to: the one that tracks it, else the only one of the
/// same name without an upstream.
pub fn local_of<'a>(remote: &GitRef, refs: &'a [GitRef]) -> Option<&'a GitRef> {
    refs.iter().filter(|l| l.kind == RefKind::Local).find(|l| crate::ui::remote_partner(l, refs).is_some_and(|j| refs[j].name == remote.name))
}

/// The remote branch of a local one: its upstream, else the only `<remote>/<name>`.
pub fn remote_of<'a>(local: &GitRef, refs: &'a [GitRef]) -> Option<&'a GitRef> {
    crate::ui::remote_partner(local, refs).map(|j| &refs[j])
}

impl Delete {
    /// The dialog for what was clicked (`clicked_local`, `clicked_remote`, or both for a merged
    /// pill); the other side, when it exists, is offered unticked. The checked-out branch is
    /// never offered locally.
    pub fn new(local: Option<Local>, remote: Option<Remote>, clicked_local: bool, clicked_remote: bool) -> Delete {
        let del_local = local.is_some() && (clicked_local || remote.is_none());
        let del_remote = remote.is_some() && (clicked_remote || local.is_none());
        Delete { local, remote, del_local, del_remote, confirm: false }
    }

    /// Both sides exist, so the dialog shows a check row for each.
    pub fn both(&self) -> bool {
        self.local.is_some() && self.remote.is_some()
    }

    /// The branch the dialog is about, for its title.
    pub fn title(&self) -> String {
        match (&self.local, &self.remote) {
            (Some(l), _) => l.name.clone(),
            (None, Some(r)) => r.name(),
            (None, None) => String::new(),
        }
    }

    /// Something is ticked, and Git would take the local deletion.
    pub fn can_go(&self) -> bool {
        let local_ok = match (&self.local, self.del_local) {
            (Some(l), true) => (l.worktree.is_none() || l.also_worktree) && (l.merged || l.force),
            _ => true,
        };
        (self.del_local || self.del_remote) && local_ok
    }

    /// The main action deletes on the remote and hasn't been confirmed yet.
    pub fn needs_confirm(&self) -> bool {
        self.del_remote && !self.confirm
    }

    pub fn button(&self) -> String {
        let remote = self.remote.as_ref().filter(|_| self.del_remote);
        match remote {
            Some(r) if self.confirm => format!("Delete on {}", r.remote),
            Some(_) => "Delete…".into(),
            None if self.local.as_ref().is_some_and(|l| l.worktree.is_some() && l.also_worktree) => "Delete branch and worktree".into(),
            None => "Delete branch".into(),
        }
    }

    /// The status line under the title.
    pub fn status(&self) -> String {
        match (self.del_local, self.del_remote.then_some(self.remote.as_ref()).flatten()) {
            (true, Some(r)) => format!("Delete the branch from your repository and from {}", r.remote),
            (false, Some(r)) => format!("Delete the branch from {}", r.remote),
            _ => "Delete local branch from your repository".into(),
        }
    }

    /// The red note of the second click.
    pub fn warning(&self) -> String {
        let r = self.remote.as_ref().map(Remote::name).unwrap_or_default();
        let remote = self.remote.as_ref().map(|r| r.remote.as_str()).unwrap_or_default();
        format!("{r} will be deleted on {remote} for everyone who uses it. ⌘Z can't bring it back: push the branch again to restore it.")
    }
}

/// The toast of `git push --delete`, or why it was refused. A remote branch already gone (deleted
/// on GitHub after a merge) is not an error: `gone` tells the caller to drop the stale ref.
pub fn pushed(r: &Remote, res: Result<String, String>) -> (Result<String, String>, bool) {
    match res {
        Ok(_) => (Ok(format!("Deleted {} on {}", r.branch, r.remote)), false),
        Err(e) if e.contains("remote ref does not exist") => (Ok(format!("{} was already gone from {}", r.branch, r.remote)), true),
        Err(e) => (Err(format!("Couldn't delete {} on {}: {}", r.branch, r.remote, e.trim())), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gref(name: &str, kind: RefKind, upstream: &str) -> GitRef {
        let full = match kind {
            RefKind::Remote => format!("refs/remotes/{name}"),
            _ => format!("refs/heads/{name}"),
        };
        GitRef { name: name.into(), full, kind, target: "a".into(), is_head: false, upstream: upstream.into(), ahead: 0, behind: 0 }
    }

    fn local(merged: bool) -> Local {
        Local { name: "feat/x".into(), merged, force: false, worktree: None, dirty: false, also_worktree: true }
    }

    #[test]
    fn splits_a_remote_branch() {
        let r = Remote::of("origin/feat/135-diff").unwrap();
        assert_eq!((r.remote.as_str(), r.branch.as_str()), ("origin", "feat/135-diff"));
        assert_eq!(r.args(), ["push", "origin", "--delete", "feat/135-diff"]);
        assert!(Remote::of("origin/HEAD").is_none());
        assert!(Remote::of("origin").is_none());
    }

    #[test]
    fn finds_the_other_side() {
        let refs = vec![gref("a", RefKind::Local, "origin/a"), gref("origin/a", RefKind::Remote, ""), gref("b", RefKind::Local, ""), gref("origin/b", RefKind::Remote, ""), gref("origin/c", RefKind::Remote, "")];
        assert_eq!(remote_of(&refs[0], &refs).map(|r| r.name.as_str()), Some("origin/a"));
        assert_eq!(local_of(&refs[1], &refs).map(|r| r.name.as_str()), Some("a"));
        // Same name, no upstream.
        assert_eq!(local_of(&refs[3], &refs).map(|r| r.name.as_str()), Some("b"));
        assert!(local_of(&refs[4], &refs).is_none());
    }

    #[test]
    fn ticks_what_was_clicked() {
        let r = Remote::of("origin/feat/x");
        let d = Delete::new(Some(local(true)), r.clone(), true, false);
        assert!(d.both() && d.del_local && !d.del_remote);
        let d = Delete::new(Some(local(true)), r.clone(), false, true);
        assert!(!d.del_local && d.del_remote);
        let d = Delete::new(Some(local(true)), r.clone(), true, true);
        assert!(d.del_local && d.del_remote);
        // Only the remote side exists.
        let d = Delete::new(None, r, false, true);
        assert!(!d.both() && d.del_remote);
    }

    #[test]
    fn the_remote_is_asked_twice() {
        let mut d = Delete::new(Some(local(true)), Remote::of("origin/feat/x"), true, false);
        assert!(!d.needs_confirm());
        assert_eq!(d.button(), "Delete branch");
        d.del_remote = true;
        assert!(d.needs_confirm());
        assert_eq!(d.button(), "Delete…");
        d.confirm = true;
        assert_eq!(d.button(), "Delete on origin");
    }

    #[test]
    fn an_unmerged_local_branch_needs_force() {
        let mut d = Delete::new(Some(local(false)), Remote::of("origin/feat/x"), true, false);
        assert!(!d.can_go());
        // The remote alone doesn't need it.
        d.del_local = false;
        d.del_remote = true;
        assert!(d.can_go());
        d.del_remote = false;
        assert!(!d.can_go());
    }

    #[test]
    fn a_branch_already_gone_is_not_an_error() {
        let r = Remote::of("origin/feat/x").unwrap();
        assert_eq!(pushed(&r, Ok(String::new())), (Ok("Deleted feat/x on origin".into()), false));
        let gone = "error: unable to delete 'feat/x': remote ref does not exist\nerror: failed to push some refs";
        assert_eq!(pushed(&r, Err(gone.into())).1, true);
        assert!(pushed(&r, Err("fatal: Authentication failed".into())).0.is_err());
    }

    #[test]
    fn deletes_a_branch_on_a_remote() {
        let base = std::env::temp_dir().join(format!("fourchette-del-{}", std::process::id()));
        let (bare, dir) = (base.join("remote.git"), base.join("clone"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| crate::git::exec(&dir, a).unwrap();
        crate::git::exec(&base, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@example.com"]);
        g(&["remote", "add", "origin", bare.to_str().unwrap()]);
        std::fs::write(dir.join("a.txt"), "1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "first"]);
        g(&["push", "-q", "origin", "main", "main:feat/x"]);
        g(&["fetch", "-q"]);

        let r = Remote::of("origin/feat/x").unwrap();
        let args = r.args();
        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (res, gone) = pushed(&r, crate::git::exec(&dir, &a).map(|(o, e)| format!("{o}{e}")));
        assert_eq!((res, gone), (Ok("Deleted feat/x on origin".into()), false));
        // Gone from the remote and from the remote-tracking branches.
        assert!(g(&["ls-remote", "--heads", "origin"]).0.lines().all(|l| !l.ends_with("feat/x")));
        assert!(crate::git::exec(&dir, &["rev-parse", "--verify", "-q", "refs/remotes/origin/feat/x"]).is_err());
        // A second time: already gone.
        assert!(pushed(&r, crate::git::exec(&dir, &a).map(|(o, e)| format!("{o}{e}"))).1);
        let _ = std::fs::remove_dir_all(&base);
    }
}
