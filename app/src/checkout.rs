//! Checking out a branch that is behind its upstream, the original's CheckoutAndSyncController
//! ("Checkout and Fast-Forward", "Checkout and Merge", "Checkout and Rebase", "Checkout and Reset",
//! "Stash and reapply local changes"): whether to ask, what to offer, and the Git sequence of
//! each choice. `Tab::checkout_sync` runs it under one undo snapshot.
use crate::git::{self, GitRef, RefKind, Repo};
use crate::worktree::Worktree;

/// What to do with the branch once checked out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncAction {
    FastForward,
    Merge,
    Rebase,
    Reset,
    JustCheckout,
}

/// How Merge joins the upstream: a merge commit (a fast-forward when the branch has nothing of
/// its own), or one squashed commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MergeStyle {
    #[default]
    Commit,
    Squash,
}

impl MergeStyle {
    pub const ALL: [MergeStyle; 2] = [MergeStyle::Commit, MergeStyle::Squash];

    /// Its name on the Merge card.
    pub fn short(self) -> &'static str {
        match self {
            MergeStyle::Commit => "Merge commit",
            MergeStyle::Squash => "Squash",
        }
    }
}

/// A local branch behind its upstream, as the last fetch saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sync {
    pub branch: String,
    /// The upstream's short name ("origin/main") and full ref ("refs/remotes/origin/main").
    pub upstream: String,
    pub upstream_ref: String,
    pub ahead: usize,
    pub behind: usize,
}

/// What a checkout does: run at once; check out and fast-forward at once (a branch only behind,
/// where every choice would come to that); or ask first in the Check out dialog (one that has
/// diverged).
#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    Now { label: String, args: Vec<String> },
    FastForward(Sync),
    Ask(Sync),
}

/// The checkout of `name`, a local branch or (`remote`) a remote one such as "origin/main". A
/// remote branch with no local one is checked out with `--track`; one whose local branch exists
/// checks out that local branch. A local branch only behind its upstream is fast-forwarded to it;
/// one diverged from it asks; an up-to-date one, one without an upstream (or with one gone), the current one, and one checked
/// out in another worktree (Git refuses it, and says why) are checked out at once, as before.
pub fn plan(name: &str, remote: bool, refs: &[GitRef], worktrees: &[Worktree]) -> Plan {
    let local = if remote { name.split_once('/').map(|x| x.1).unwrap_or(name) } else { name };
    let now = |args: &[&str]| Plan::Now { label: format!("Switched to {local}"), args: args.iter().map(|s| s.to_string()).collect() };
    let Some(b) = refs.iter().find(|r| r.kind == RefKind::Local && r.name == local) else {
        return if remote { now(&["checkout", "--track", name]) } else { now(&["checkout", local]) };
    };
    let elsewhere = !b.is_head && worktrees.iter().any(|w| w.branch.as_deref() == Some(local));
    let up = refs.iter().filter(|r| !b.upstream.is_empty() && r.name == b.upstream).min_by_key(|r| r.kind != RefKind::Remote);
    match up {
        Some(up) if b.behind > 0 && !b.is_head && !elsewhere => {
            let s = Sync { branch: local.to_string(), upstream: b.upstream.clone(), upstream_ref: up.full.clone(), ahead: b.ahead, behind: b.behind };
            if s.diverged() {
                Plan::Ask(s)
            } else {
                Plan::FastForward(s)
            }
        }
        _ => now(&["checkout", local]),
    }
}

fn commits(n: usize) -> String {
    if n == 1 {
        "1 commit".into()
    } else {
        format!("{n} commits")
    }
}

impl Sync {
    /// It has commits of its own as well: a fast-forward is impossible.
    pub fn diverged(&self) -> bool {
        self.ahead > 0
    }

    /// The choices the dialog offers, in its order.
    pub fn actions(&self) -> Vec<SyncAction> {
        use SyncAction::*;
        if self.diverged() {
            vec![Merge, Rebase, Reset, JustCheckout]
        } else {
            vec![FastForward, Merge, Rebase, Reset, JustCheckout]
        }
    }

    /// Fast-forward when possible, else Merge.
    pub fn default_action(&self) -> SyncAction {
        if self.diverged() {
            SyncAction::Merge
        } else {
            SyncAction::FastForward
        }
    }

    /// Where the branch stands: the dialog's sentence in its "Now" view.
    pub fn summary(&self) -> String {
        if self.diverged() {
            format!("{} and {} have diverged: {} here, {} there.", self.branch, self.upstream, commits(self.ahead), commits(self.behind))
        } else {
            format!("{} is {} behind {}.", self.branch, commits(self.behind), self.upstream)
        }
    }

    /// "3 behind" or "2 ahead, 3 behind", next to the upstream's name.
    pub fn counts(&self) -> String {
        if self.diverged() {
            format!("{} ahead, {} behind", self.ahead, self.behind)
        } else {
            format!("{} behind", self.behind)
        }
    }

    /// A choice as the dialog words it.
    pub fn text(&self, a: SyncAction, m: MergeStyle) -> String {
        let (b, up) = (&self.branch, &self.upstream);
        match a {
            SyncAction::FastForward => format!("Fast-forward to {up}"),
            SyncAction::Merge => match m {
                MergeStyle::Commit => format!("Merge {up} into {b}"),
                MergeStyle::Squash => format!("Squash {up} into {b}"),
            },
            SyncAction::Rebase => format!("Rebase {b} onto {up}"),
            SyncAction::Reset => format!("Reset {b} to {up}"),
            SyncAction::JustCheckout => "Just check out".into(),
        }
    }

    /// Every choice the dialog shows as a card, in its order: Fast-forward too when the branch has
    /// diverged, greyed out with `unavailable`'s reason.
    pub fn cards() -> [SyncAction; 5] {
        use SyncAction::*;
        [FastForward, Merge, Rebase, Reset, JustCheckout]
    }

    /// Why a card is greyed out, when it is.
    pub fn unavailable(&self, a: SyncAction) -> Option<String> {
        (!self.actions().contains(&a)).then(|| format!("Not possible: {} has commits of its own", self.branch))
    }

    /// A card's title.
    pub fn short(a: SyncAction) -> &'static str {
        match a {
            SyncAction::FastForward => "Fast-forward",
            SyncAction::Merge => "Merge",
            SyncAction::Rebase => "Rebase",
            SyncAction::Reset => "Reset",
            SyncAction::JustCheckout => "Just check out",
        }
    }

    /// A card's line under its title.
    pub fn hint(&self, a: SyncAction) -> String {
        let (b, up) = (&self.branch, &self.upstream);
        match a {
            SyncAction::FastForward => format!("Move {b} up to {up}"),
            SyncAction::Merge => "One merge commit joins both".into(),
            SyncAction::Rebase => "Replay your commits on top".into(),
            SyncAction::Reset => format!("Match {up} exactly"),
            SyncAction::JustCheckout => format!("Leave {b} as it is"),
        }
    }

    /// What a choice does to the branch, in a sentence or two, beside the cards.
    pub fn explain(&self, a: SyncAction, m: MergeStyle) -> String {
        let (b, up) = (&self.branch, &self.upstream);
        let (mine, theirs) = (commits(self.ahead), commits(self.behind));
        let they = if self.behind == 1 { "lands" } else { "land" };
        match (a, m, self.diverged()) {
            (SyncAction::Merge, MergeStyle::Squash, true) => return format!("The {theirs} of {up} {they} on {b} as one new commit with a single parent. The history shows no merge."),
            (SyncAction::Merge, MergeStyle::Squash, false) => return format!("The {theirs} of {up} {they} on {b} as one new commit instead of a fast-forward. The history shows no merge."),
            _ => {}
        }
        match (a, self.diverged()) {
            (SyncAction::FastForward, _) => format!("{b} slides up to {up}. No new commit, nothing rewritten."),
            (SyncAction::Merge, true) => format!("A new merge commit on {b} joins its {mine} with the {theirs} of {up}. Nothing is rewritten."),
            (SyncAction::Merge, false) => format!("Nothing to merge: {b} has no commits of its own, so this is a fast-forward too."),
            (SyncAction::Rebase, true) => format!("Your {mine} are replayed on top of {up} as new commits. The old ones leave the branch."),
            (SyncAction::Rebase, false) => format!("Nothing to replay: {b} has no commits of its own, so this is a fast-forward too."),
            (SyncAction::Reset, _) => self.reset_warning(),
            (SyncAction::JustCheckout, true) => format!("You switch to {b} as it is: still {} ahead and {} behind.", self.ahead, self.behind),
            (SyncAction::JustCheckout, false) => format!("You switch to {b} as it is, {theirs} behind."),
        }
    }

    /// The one line under Reset: what it takes away.
    pub fn reset_warning(&self) -> String {
        match self.ahead {
            0 => format!("Moves {} to {}; it has no commits of its own to lose.", self.branch, self.upstream),
            n => format!("Removes the {} that {} has and {} doesn't.", commits(n), self.branch, self.upstream),
        }
    }

    /// The name of the whole sequence in the Undo menu.
    pub fn label(&self, a: SyncAction, m: MergeStyle) -> String {
        let b = &self.branch;
        match a {
            SyncAction::FastForward => format!("Check out {b} and fast-forward"),
            SyncAction::Merge => match m {
                MergeStyle::Squash => format!("Check out {b} and squash"),
                _ => format!("Check out {b} and merge"),
            },
            SyncAction::Rebase => format!("Check out {b} and rebase"),
            SyncAction::Reset => format!("Check out {b} and reset"),
            SyncAction::JustCheckout => format!("Check out {b}"),
        }
    }

    /// What the toast says once it went through.
    fn done(&self, a: SyncAction, m: MergeStyle) -> String {
        let (b, up) = (&self.branch, &self.upstream);
        match a {
            SyncAction::FastForward => format!("Switched to {b} and fast-forwarded it to {up}"),
            SyncAction::Merge if m == MergeStyle::Squash => format!("Switched to {b} and squashed {up} into it"),
            SyncAction::Merge => format!("Switched to {b} and merged {up} into it"),
            SyncAction::Rebase => format!("Switched to {b} and rebased it onto {up}"),
            SyncAction::Reset => format!("Switched to {b} and reset it to {up}"),
            SyncAction::JustCheckout => format!("Switched to {b}"),
        }
    }

    /// Git's arguments for the step after the checkout; none for "Just check out". Merge names
    /// the upstream by its short name, which is what its commit message says. Reset uses `--keep`,
    /// which refuses rather than throw away uncommitted changes.
    pub fn args(&self, a: SyncAction, m: MergeStyle) -> Option<Vec<String>> {
        let v = |a: &[&str]| Some(a.iter().map(|s| s.to_string()).collect());
        let up = self.upstream_ref.as_str();
        match a {
            SyncAction::FastForward => Some(git::fast_forward_args(&self.branch, up, true)),
            SyncAction::Merge => match m {
                MergeStyle::Commit => v(&["merge", "--no-edit", &self.upstream]),
                MergeStyle::Squash => v(&["merge", "--squash", &self.upstream]),
            },
            SyncAction::Rebase => v(&["rebase", up]),
            SyncAction::Reset => v(&["reset", "--keep", up]),
            SyncAction::JustCheckout => None,
        }
    }
}

fn stash_top(repo: &Repo) -> Option<String> {
    repo.git(&["rev-parse", "-q", "--verify", "refs/stash"]).ok().map(|s| s.trim().to_string())
}

/// Git's refusal, with what to do about it when local changes were in the way.
fn refused(e: String, stashed: bool) -> String {
    let local = ["would be overwritten", "unstaged changes", "uncommitted changes", "not uptodate", "Your local changes"].iter().any(|m| e.contains(m));
    if local && !stashed {
        format!("{e}\nCommit or stash your changes first, or tick “Stash and reapply local changes”.")
    } else {
        e
    }
}

/// Checks `s.branch` out and does `a`, putting uncommitted changes (untracked files included)
/// aside and back when `stash`:
///
/// `[stash push -u]`, `checkout <branch> --`, then `merge --ff-only <upstream>` | `merge --no-edit
/// <upstream>` (`--no-ff` too, or `--squash` then `commit`) | `rebase <upstream>` | `reset --keep
/// <upstream>` | nothing, then `[stash pop]`.
///
/// A refusal (local changes in the way) goes back to the branch it started on, with the changes,
/// so nothing has changed. A merge or rebase stopped on conflicts stays where Git stopped, the
/// changes left in the stash list; so do changes whose reapply conflicts.
pub fn run(repo: &Repo, s: &Sync, a: SyncAction, m: MergeStyle, stash: bool) -> Result<String, String> {
    let back: Vec<String> = match repo.current_branch() {
        Some(b) => vec!["checkout".into(), "-q".into(), b, "--".into()],
        None => vec!["checkout".into(), "-q".into(), "--detach".into(), repo.git(&["rev-parse", "HEAD"])?.trim().to_string()],
    };
    let top = stash_top(repo);
    let stashed = stash && {
        repo.git(&["stash", "push", "-q", "-u", "-m", &format!("Changes before checking out {}", s.branch)])?;
        stash_top(repo) != top
    };
    // Back where it started after a refusal, the changes put back too.
    let undo = |e: String| -> String {
        let mut msg = refused(e, stashed);
        if let Err(e2) = repo.git(&back.iter().map(|x| x.as_str()).collect::<Vec<_>>()) {
            msg.push_str(&format!("\nCouldn't go back to the branch you were on: {e2}"));
        }
        if stashed && repo.git(&["stash", "pop", "-q"]).is_err() {
            msg.push_str("\nYour local changes are in the stash list.");
        }
        msg
    };
    if let Err(e) = repo.git(&["checkout", "-q", &s.branch, "--"]) {
        return Err(undo(e));
    }
    let squash = a == SyncAction::Merge && m == MergeStyle::Squash;
    let mut done = s.done(a, m);
    if let Some(args) = s.args(a, m) {
        if let Err(e) = repo.git(&args.iter().map(|x| x.as_str()).collect::<Vec<_>>()) {
            // A squash stopped on conflicts leaves no MERGE_HEAD, only conflicted files.
            let op = repo.operation().or_else(|| (squash && !repo.conflicted_files().is_empty()).then(|| "squash".to_string()));
            let Some(op) = op else { return Err(undo(e)) };
            let mut msg = format!(
                "Switched to {}, but the {op} stopped on conflicts in {}. Resolve them, then Continue in the banner above the history, or Undo (⌘Z) to go back to before the checkout.",
                s.branch,
                git::file_list(&repo.conflicted_files())
            );
            if stashed {
                msg.push_str(" Your local changes are in the stash list.");
            }
            return Err(msg);
        }
        // A squash only stages the upstream's changes: commit them, when there are any.
        if squash {
            if repo.git(&["diff", "--cached", "--quiet"]).is_ok() {
                done = format!("Switched to {}; nothing to squash, it already has the changes of {}", s.branch, s.upstream);
            } else if let Err(e) = repo.git(&["commit", "-q", "--no-edit", "-m", &format!("Squash {} into {}", s.upstream, s.branch)]) {
                return Err(format!("Switched to {} and squashed {} into the index, but the commit failed: {e}. Undo (⌘Z) goes back to before the checkout.", s.branch, s.upstream));
            }
        }
    }
    if stashed {
        if let Err(e) = repo.git(&["stash", "pop", "-q"]) {
            let files = repo.conflicted_files();
            let why = if files.is_empty() { e } else { format!("they conflict with it in {}", git::file_list(&files)) };
            return Err(format!(
                "{done}, but your local changes couldn't be put back: {why}. They are safe in the stash list; Undo (⌘Z) goes back to before the checkout."
            ));
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(name: &str, up: &str, ahead: usize, behind: usize, is_head: bool) -> GitRef {
        GitRef { name: name.into(), full: format!("refs/heads/{name}"), kind: RefKind::Local, target: "a".into(), is_head, upstream: up.into(), ahead, behind }
    }

    fn remote(name: &str) -> GitRef {
        GitRef { name: name.into(), full: format!("refs/remotes/{name}"), kind: RefKind::Remote, target: "b".into(), is_head: false, upstream: String::new(), ahead: 0, behind: 0 }
    }

    fn now(args: &[&str]) -> Plan {
        let name = args.last().unwrap().rsplit_once('/').map(|x| x.1).unwrap_or(args.last().unwrap());
        Plan::Now { label: format!("Switched to {name}"), args: args.iter().map(|s| s.to_string()).collect() }
    }

    fn wt(branch: &str) -> Worktree {
        Worktree { path: format!("/tmp/wt/{branch}").into(), branch: Some(branch.into()), head: String::new(), is_main: false, missing: false, locked: false }
    }

    #[test]
    fn a_branch_behind_its_upstream_asks_first_and_any_other_is_checked_out_at_once() {
        let refs = vec![
            local("main", "origin/main", 0, 0, true),
            local("behind", "origin/behind", 0, 3, false),
            local("diverged", "origin/diverged", 2, 1, false),
            local("ahead", "origin/ahead", 2, 0, false),
            local("uptodate", "origin/uptodate", 0, 0, false),
            local("untracked", "", 0, 0, false),
            local("gone", "origin/gone", 0, 4, false),
            remote("origin/main"),
            remote("origin/behind"),
            remote("origin/diverged"),
            remote("origin/ahead"),
            remote("origin/uptodate"),
            remote("origin/new"),
        ];
        let sync = |b: &str, ahead, behind| Sync { branch: b.into(), upstream: format!("origin/{b}"), upstream_ref: format!("refs/remotes/origin/{b}"), ahead, behind };
        // Only behind: every choice would be a fast-forward, so it is done without asking.
        assert_eq!(plan("behind", false, &refs, &[]), Plan::FastForward(sync("behind", 0, 3)));
        assert_eq!(plan("diverged", false, &refs, &[]), Plan::Ask(sync("diverged", 2, 1)));
        // From the remote branch: its local branch, the same way.
        assert_eq!(plan("origin/behind", true, &refs, &[]), Plan::FastForward(sync("behind", 0, 3)));
        assert_eq!(plan("origin/diverged", true, &refs, &[]), Plan::Ask(sync("diverged", 2, 1)));
        // At once: ahead, up to date, no upstream, an upstream not among the refs, the current one.
        assert_eq!(plan("ahead", false, &refs, &[]), now(&["checkout", "ahead"]));
        assert_eq!(plan("uptodate", false, &refs, &[]), now(&["checkout", "uptodate"]));
        assert_eq!(plan("origin/uptodate", true, &refs, &[]), now(&["checkout", "uptodate"]));
        assert_eq!(plan("untracked", false, &refs, &[]), now(&["checkout", "untracked"]));
        assert_eq!(plan("gone", false, &refs, &[]), now(&["checkout", "gone"]));
        assert_eq!(plan("origin/main", true, &refs, &[]), now(&["checkout", "main"]));
        // A remote branch with no local one: a new tracking branch, as before.
        assert_eq!(plan("origin/new", true, &refs, &[]), now(&["checkout", "--track", "origin/new"]));
        // Checked out in another worktree: Git refuses, and says where.
        assert_eq!(plan("behind", false, &refs, &[wt("behind")]), now(&["checkout", "behind"]));
    }

    #[test]
    fn fast_forward_is_offered_and_chosen_only_when_possible() {
        use SyncAction::*;
        let behind = Sync { branch: "main".into(), upstream: "origin/main".into(), upstream_ref: "refs/remotes/origin/main".into(), ahead: 0, behind: 3 };
        assert_eq!(behind.actions(), [FastForward, Merge, Rebase, Reset, JustCheckout]);
        assert_eq!(behind.default_action(), FastForward);
        assert_eq!(behind.summary(), "main is 3 commits behind origin/main.");
        assert_eq!(behind.counts(), "3 behind");
        let diverged = Sync { ahead: 1, ..behind.clone() };
        assert_eq!(diverged.actions(), [Merge, Rebase, Reset, JustCheckout]);
        assert_eq!(diverged.default_action(), Merge);
        assert_eq!(diverged.summary(), "main and origin/main have diverged: 1 commit here, 3 commits there.");
        assert_eq!(diverged.reset_warning(), "Removes the 1 commit that main has and origin/main doesn't.");
        // The dialog shows every choice as a card; the impossible fast-forward greyed out.
        assert_eq!(Sync::cards().iter().map(|a| behind.unavailable(*a).is_none()).collect::<Vec<_>>(), [true; 5]);
        assert_eq!(diverged.unavailable(FastForward).as_deref(), Some("Not possible: main has commits of its own"));
        assert_eq!(Sync::cards().iter().filter(|a| diverged.unavailable(**a).is_none()).copied().collect::<Vec<_>>(), diverged.actions());
        assert_eq!(diverged.hint(Reset), "Match origin/main exactly");
        assert_eq!(diverged.explain(Merge, MergeStyle::Commit), "A new merge commit on main joins its 1 commit with the 3 commits of origin/main. Nothing is rewritten.");
        assert_eq!(behind.explain(JustCheckout, MergeStyle::Commit), "You switch to main as it is, 3 commits behind.");
        let args = |a| diverged.args(a, MergeStyle::Commit).map(|v| v.join(" "));
        assert_eq!(args(FastForward).as_deref(), Some("merge --ff-only refs/remotes/origin/main"));
        assert_eq!(args(Merge).as_deref(), Some("merge --no-edit origin/main"));
        assert_eq!(args(Rebase).as_deref(), Some("rebase refs/remotes/origin/main"));
        assert_eq!(args(Reset).as_deref(), Some("reset --keep refs/remotes/origin/main"), "never --hard");
        assert_eq!(args(JustCheckout), None);
    }

    #[test]
    fn merge_styles_have_their_own_git_words() {
        let behind = Sync { branch: "main".into(), upstream: "origin/main".into(), upstream_ref: "refs/remotes/origin/main".into(), ahead: 0, behind: 3 };
        let args = |m| behind.args(SyncAction::Merge, m).unwrap().join(" ");
        assert_eq!(args(MergeStyle::Commit), "merge --no-edit origin/main");
        assert_eq!(args(MergeStyle::Squash), "merge --squash origin/main");
        assert_eq!(behind.text(SyncAction::Merge, MergeStyle::Squash), "Squash origin/main into main");
        assert_eq!(behind.label(SyncAction::Merge, MergeStyle::Squash), "Check out main and squash");
        // Other actions ignore the style.
        assert_eq!(behind.args(SyncAction::Rebase, MergeStyle::Squash), behind.args(SyncAction::Rebase, MergeStyle::Commit));
    }
}

/// The sequences on real repositories, and Undo taking each back in one step.
#[cfg(test)]
pub(crate) mod repo_tests {
    use super::*;
    use crate::git::run_in;
    use crate::undo::Snapshot;
    use std::path::{Path, PathBuf};

    /// A clone `a` on `side`, where `main` has a commit of its own (g.txt) and `origin/main` one
    /// more (line 4 "B" in f.txt): `main` has diverged, and `behind` (at the base) is 1 behind.
    pub(crate) fn fixture(name: &str) -> (PathBuf, Repo) {
        let base = std::env::temp_dir().join(format!("fourchette-checkout-{name}-{}", std::process::id()));
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
        g(&a, &["branch", "-q", "--track", "behind", "origin/main"]);
        g(&a, &["branch", "-q", "side"]);
        g(&base, &["clone", "-q", remote.to_str().unwrap(), b.to_str().unwrap()]);
        g(&b, &["config", "user.name", "B"]);
        g(&b, &["config", "user.email", "b@example.com"]);
        std::fs::write(b.join("f.txt"), "1\n2\n3\nB\n").unwrap();
        g(&b, &["commit", "-q", "-am", "remote change"]);
        g(&b, &["push", "-q"]);
        std::fs::write(a.join("g.txt"), "local\n").unwrap();
        g(&a, &["add", "-A"]);
        g(&a, &["commit", "-q", "-m", "local commit"]);
        g(&a, &["checkout", "-q", "side"]);
        g(&a, &["fetch", "-q"]);
        (base, Repo::open(&a).unwrap())
    }

    /// What `Tab::checkout_sync` does: the plan from the refs, one snapshot, the sequence, `record`.
    fn checkout(repo: &Repo, branch: &str, a: SyncAction, stash: bool) -> (Snapshot, Result<String, String>) {
        checkout_with(repo, branch, a, MergeStyle::Commit, stash)
    }

    fn checkout_with(repo: &Repo, branch: &str, a: SyncAction, m: MergeStyle, stash: bool) -> (Snapshot, Result<String, String>) {
        // A branch only behind is fast-forwarded without asking; `run` does every action all the same.
        let (Plan::Ask(s) | Plan::FastForward(s)) = plan(branch, false, &repo.refs(), &repo.worktrees()) else { panic!("{branch} should sync") };
        assert!(s.actions().contains(&a), "{a:?} not offered for {branch}");
        let mut snap = Snapshot { dirty: repo.stash_create(), ..repo.snapshot(&s.label(a, m), false, false) };
        let res = run(repo, &s, a, m, stash);
        repo.after(&mut snap);
        (snap, res)
    }

    fn read(repo: &Repo, f: &str) -> String {
        std::fs::read_to_string(repo.root.join(f)).unwrap()
    }

    fn sha(repo: &Repo, r: &str) -> String {
        repo.git(&["rev-parse", r]).unwrap().trim().to_string()
    }

    fn log(repo: &Repo) -> String {
        repo.git(&["log", "--format=%s", "-4"]).unwrap()
    }

    #[test]
    fn fast_forward_checks_out_and_moves_the_branch_and_undo_takes_both_back() {
        let (base, repo) = fixture("ff");
        let before = repo.state();
        let (snap, res) = checkout(&repo, "behind", SyncAction::FastForward, false);
        assert_eq!(res, Ok("Switched to behind and fast-forwarded it to origin/main".into()));
        assert_eq!(repo.current_branch().as_deref(), Some("behind"));
        assert_eq!(sha(&repo, "behind"), sha(&repo, "origin/main"));
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nB\n");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before, "back on side, behind where it was");
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn local_changes_in_the_way_are_refused_without_stash_and_carried_with_it() {
        let (base, repo) = fixture("ff-stash");
        std::fs::write(repo.root.join("f.txt"), "X\n2\n3\n").unwrap();
        std::fs::write(repo.root.join("new.txt"), "untracked\n").unwrap();
        let before = repo.state();

        // Without the box: the checkout carries the change, the fast-forward refuses, and the
        // branch it started on comes back with the change: nothing moved.
        let (_, res) = checkout(&repo, "behind", SyncAction::FastForward, false);
        let e = res.unwrap_err();
        assert!(e.contains("would be overwritten") && e.contains("Stash and reapply local changes"), "{e}");
        assert_eq!(repo.state(), before);
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\n");

        // With it: put aside, checked out, moved, put back; the stash list as it was.
        let (snap, res) = checkout(&repo, "behind", SyncAction::FastForward, true);
        res.unwrap();
        assert_eq!(repo.current_branch().as_deref(), Some("behind"));
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\nB\n");
        assert_eq!(read(&repo, "new.txt"), "untracked\n");
        assert!(repo.state().stashes.is_empty());
        // One Undo: back on side, behind back, the change as it was (a plain checkout of side
        // would refuse, since f.txt differs there).
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\n");
        assert_eq!(read(&repo, "new.txt"), "untracked\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rebase_of_a_diverged_branch_with_its_changes_stashed_is_one_undo_step() {
        let (base, repo) = fixture("rebase");
        let (snap, res) = checkout(&repo, "main", SyncAction::Rebase, false);
        assert_eq!(res, Ok("Switched to main and rebased it onto origin/main".into()));
        assert_eq!(log(&repo), "local commit\nremote change\nbase\n");
        let before_clean = snap.state.clone();
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before_clean);

        std::fs::write(repo.root.join("f.txt"), "X\n2\n3\n").unwrap();
        let before = repo.state();
        let (_, res) = checkout(&repo, "main", SyncAction::Rebase, false);
        assert!(res.unwrap_err().contains("Stash and reapply"));
        assert_eq!(repo.state(), before, "refused, back on side");
        let (snap, res) = checkout(&repo, "main", SyncAction::Rebase, true);
        res.unwrap();
        assert_eq!(log(&repo), "local commit\nremote change\nbase\n");
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\nB\n");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert_eq!(repo.current_branch().as_deref(), Some("side"));
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn reset_drops_the_local_commits_keeps_uncommitted_changes_and_undo_brings_them_back() {
        let (base, repo) = fixture("reset");
        let main = sha(&repo, "main");
        // An uncommitted change the reset would overwrite: `--keep` refuses, nothing moves.
        std::fs::write(repo.root.join("f.txt"), "X\n2\n3\n").unwrap();
        let before = repo.state();
        let (_, res) = checkout(&repo, "main", SyncAction::Reset, false);
        assert!(res.unwrap_err().contains("Stash and reapply"));
        assert_eq!(repo.state(), before);
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\n");
        // With the box: reset, the change put back on top.
        let (snap, res) = checkout(&repo, "main", SyncAction::Reset, true);
        assert_eq!(res, Ok("Switched to main and reset it to origin/main".into()));
        assert_eq!(sha(&repo, "main"), sha(&repo, "origin/main"));
        assert!(!repo.root.join("g.txt").exists(), "the local commit is gone");
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\nB\n");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert_eq!(sha(&repo, "main"), main);
        assert_eq!(read(&repo, "f.txt"), "X\n2\n3\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn merge_and_just_check_out_are_undone_too() {
        let (base, repo) = fixture("merge");
        let before = repo.state();
        let (snap, res) = checkout(&repo, "main", SyncAction::Merge, false);
        assert_eq!(res, Ok("Switched to main and merged origin/main into it".into()));
        assert!(log(&repo).starts_with("Merge remote-tracking branch 'origin/main'\n"), "{}", log(&repo));
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);

        let (snap, res) = checkout(&repo, "main", SyncAction::JustCheckout, false);
        assert_eq!(res, Ok("Switched to main".into()));
        assert_eq!(sha(&repo, "HEAD"), sha(&repo, "main"));
        assert_eq!(repo.state().refs, before.refs, "no branch moved");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The parents of HEAD.
    fn parents(repo: &Repo) -> Vec<String> {
        repo.git(&["rev-list", "--parents", "-n", "1", "HEAD"]).unwrap().split_whitespace().skip(1).map(String::from).collect()
    }

    #[test]
    fn squash_makes_one_single_parent_commit_and_undo_takes_it_back() {
        let (base, repo) = fixture("squash");
        let before = repo.state();
        // Diverged: one commit on top of the branch's own, with the upstream's change, no merge.
        let main = sha(&repo, "main");
        let (snap, res) = checkout_with(&repo, "main", SyncAction::Merge, MergeStyle::Squash, false);
        assert_eq!(res, Ok("Switched to main and squashed origin/main into it".into()));
        assert_eq!(log(&repo), "Squash origin/main into main\nlocal commit\nbase\n");
        assert_eq!(parents(&repo), [main]);
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nB\n");
        assert_eq!(repo.operation(), None);
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);

        // Only behind: a new commit instead of a fast-forward, the stash taken along and back.
        std::fs::write(repo.root.join("g2.txt"), "dirty\n").unwrap();
        let before = repo.state();
        let old = sha(&repo, "behind");
        let (snap, res) = checkout_with(&repo, "behind", SyncAction::Merge, MergeStyle::Squash, true);
        res.unwrap();
        assert_eq!(repo.current_branch().as_deref(), Some("behind"));
        assert_eq!(parents(&repo), [old]);
        assert_ne!(sha(&repo, "behind"), sha(&repo, "origin/main"), "not a fast-forward");
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nB\n");
        assert_eq!(read(&repo, "g2.txt"), "dirty\n");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert_eq!(read(&repo, "g2.txt"), "dirty\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_merge_or_rebase_stopped_on_conflicts_stays_there_and_undo_takes_it_all_back() {
        for a in [SyncAction::Merge, SyncAction::Rebase] {
            let op = if a == SyncAction::Merge { "merge" } else { "rebase" };
            let (base, repo) = fixture(&format!("conflict-{op}"));
            repo.git(&["checkout", "-q", "main"]).unwrap();
            std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nCOMMIT\n").unwrap();
            repo.git(&["commit", "-q", "-am", "edit line 4"]).unwrap();
            repo.git(&["checkout", "-q", "side"]).unwrap();
            std::fs::write(repo.root.join("g.txt"), "dirty\n").unwrap();
            let before = repo.state();
            let (snap, res) = checkout(&repo, "main", a, true);
            let e = res.unwrap_err();
            assert!(e.starts_with(&format!("Switched to main, but the {op} stopped on conflicts in f.txt.")) && e.contains("stash list"), "{e}");
            assert_eq!(repo.operation().as_deref(), Some(op), "left where Git stopped");
            assert_eq!(repo.state().stashes.len(), 1, "the changes wait in the stash list");
            repo.undo(&snap).unwrap();
            assert_eq!(repo.state(), before, "{op}");
            assert_eq!(read(&repo, "g.txt"), "dirty\n");
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn changes_that_conflict_on_reapply_stay_in_the_stash_and_undo_restores_them() {
        let (base, repo) = fixture("reapply-conflict");
        std::fs::write(repo.root.join("f.txt"), "1\n2\n3\nLOCAL\n").unwrap();
        let before = repo.state();
        let (snap, res) = checkout(&repo, "behind", SyncAction::FastForward, true);
        let e = res.unwrap_err();
        assert!(e.contains("conflict with it in f.txt") && e.contains("stash list"), "{e}");
        assert_eq!(repo.current_branch().as_deref(), Some("behind"));
        assert_eq!(repo.state().stashes.len(), 1);
        assert_eq!(repo.conflicted_files(), ["f.txt"]);
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before, "on side, no stash left");
        assert_eq!(read(&repo, "f.txt"), "1\n2\n3\nLOCAL\n");
        assert!(repo.conflicted_files().is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
