//! Merging a branch into the current one, the original's MergeWindowController (its "No
//! fast-forward" box, `noFastForwardCheckBox`) and MergeTestGitCommand: what the merge would do,
//! whether it would conflict, found beforehand with `git merge-tree` (no branch, index or file
//! is touched), and the Git sequence, which `Tab::merge` runs under one undo snapshot.
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use crate::checkout::MergeStyle;
use crate::git::{self, Repo};
use crate::preview::{Outcome, Preview};

/// A branch to merge into the checked-out one, as the repository is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merge {
    /// The branch to merge, as Git names it ("feature", "origin/feature"), and whether it is a
    /// remote one.
    pub branch: String,
    pub remote: bool,
    /// The checked-out branch.
    pub head: String,
    /// Commits only `head` has, and commits only `branch` has (the ones the merge brings in).
    pub ours: usize,
    pub theirs: usize,
}

/// What the merge does, from the style chosen and the "No fast-forward" box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    FastForward,
    Commit,
    Squash,
}

fn commits(n: usize) -> String {
    if n == 1 {
        "1 commit".into()
    } else {
        format!("{n} commits")
    }
}

/// (ours, theirs) from `rev-list --left-right --count HEAD...<branch>`: "2\t3".
pub fn parse_counts(out: &str) -> Option<(usize, usize)> {
    let mut it = out.split_whitespace().map(|n| n.parse::<usize>().ok());
    Some((it.next()??, it.next()??))
}

impl Merge {
    /// `branch` against the checked-out branch. Refused on a detached HEAD: Fork merges into a
    /// branch only.
    pub fn read(repo: &Repo, branch: &str, remote: bool) -> Result<Merge, String> {
        let head = repo.current_branch().ok_or_else(|| "Check out the branch to merge into first: HEAD is detached.".to_string())?;
        let out = repo.git(&["rev-list", "--left-right", "--count", &format!("HEAD...{branch}"), "--"])?;
        let (ours, theirs) = parse_counts(&out).ok_or_else(|| format!("Couldn't compare {head} with {branch}."))?;
        Ok(Merge { branch: branch.to_string(), remote, head, ours, theirs })
    }

    /// `head` has every commit of `branch`: nothing to merge.
    pub fn up_to_date(&self) -> bool {
        self.theirs == 0
    }

    /// `head` has no commits of its own: Git would just move it up to `branch`.
    pub fn can_fast_forward(&self) -> bool {
        self.ours == 0 && self.theirs > 0
    }

    pub fn way(&self, style: MergeStyle, no_ff: bool) -> Way {
        match style {
            MergeStyle::Squash => Way::Squash,
            MergeStyle::Commit if self.can_fast_forward() && !no_ff => Way::FastForward,
            MergeStyle::Commit => Way::Commit,
        }
    }

    /// Git's arguments. A merge commit is asked for with `--no-ff` whenever there is one to make,
    /// so a `merge.ff` setting does not turn the merge into something the dialog did not show; a
    /// fast-forward with `--ff-only`, for the same reason.
    pub fn args(&self, way: Way) -> Vec<String> {
        let b = self.branch.as_str();
        let v: &[&str] = match way {
            Way::FastForward => &["merge", "--ff-only", b],
            Way::Commit => &["merge", "--no-ff", "--no-edit", b],
            Way::Squash => &["merge", "--squash", b],
        };
        v.iter().map(|s| s.to_string()).collect()
    }

    /// The result's title in the dialog, and the name of the step in the Undo menu.
    pub fn title(&self, way: Way) -> String {
        let (b, h) = (&self.branch, &self.head);
        match way {
            Way::FastForward => format!("Fast-forward {h} to {b}"),
            Way::Commit => format!("Merge {b} into {h}"),
            Way::Squash => format!("Squash {b} into {h}"),
        }
    }

    /// The main button.
    pub fn button(way: Way) -> &'static str {
        match way {
            Way::FastForward => "Fast-forward",
            Way::Commit => "Merge",
            Way::Squash => "Squash",
        }
    }

    /// The status line under the dialog's title.
    pub fn counts(&self) -> String {
        match (self.theirs, self.ours) {
            (0, _) => "nothing to merge".into(),
            (t, 0) => format!("{} to bring in, a fast-forward is possible", commits(t)),
            (t, o) => format!("{} to bring in, {} has {} of its own", commits(t), self.head, commits(o)),
        }
    }

    /// Where the two branches stand: the dialog's sentence in its "Now" view.
    pub fn summary(&self) -> String {
        let (b, h) = (&self.branch, &self.head);
        match (self.theirs, self.ours) {
            (0, _) => format!("{h} already has every commit of {b}: there is nothing to merge."),
            (t, 0) => format!("{b} is {} ahead of {h}, which has no commits of its own.", commits(t)),
            (t, o) => format!("{h} and {b} have diverged: {} here, {} there.", commits(o), commits(t)),
        }
    }

    /// What the merge does to `head`, in a sentence or two, in the "After" view.
    pub fn explain(&self, style: MergeStyle, no_ff: bool) -> String {
        let (b, h) = (&self.branch, &self.head);
        let theirs = commits(self.theirs);
        let they = if self.theirs == 1 { "lands" } else { "land" };
        if self.up_to_date() {
            return self.summary();
        }
        match self.way(style, no_ff) {
            Way::FastForward => format!("{h} slides up to {b}. No new commit; tick “No fast-forward” to record the merge with one."),
            Way::Commit if self.can_fast_forward() => format!("A new merge commit on {h} brings in the {theirs} of {b}, though a fast-forward was possible: the history keeps the branch visible."),
            Way::Commit => format!("A new merge commit on {h} joins its {} with the {theirs} of {b}. Nothing is rewritten.", commits(self.ours)),
            Way::Squash => format!("The {theirs} of {b} {they} on {h} as one new commit with a single parent. The history shows no merge."),
        }
    }

    /// What the toast says once it went through.
    fn done(&self, way: Way) -> String {
        let (b, h) = (&self.branch, &self.head);
        match way {
            Way::FastForward => format!("Fast-forwarded {h} to {b}"),
            Way::Commit => format!("Merged {b} into {h}"),
            Way::Squash => format!("Squashed {b} into {h}"),
        }
    }

    /// The preview's outcome: a merge commit even when a fast-forward is possible, with "No
    /// fast-forward".
    pub fn outcome(&self, style: MergeStyle, no_ff: bool) -> Outcome {
        match self.way(style, no_ff) {
            Way::FastForward => Outcome::FastForward,
            Way::Commit if self.ours == 0 => Outcome::NoFastForward,
            Way::Commit => Outcome::Merge,
            Way::Squash => Outcome::Squash,
        }
    }

    pub fn preview(&self, style: MergeStyle, no_ff: bool) -> Preview {
        Preview::merge(&self.head, &self.branch, self.ours, self.theirs, self.outcome(style, no_ff))
    }
}

// ------------------------------------------------------------------ conflicts, beforehand

/// Whether the merge would stop on conflicts, as `git merge-tree` foresees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Test {
    Checking,
    Clean,
    /// The files that would conflict.
    Conflicts(Vec<String>),
    /// It could not tell, and why (Git older than 2.38, unrelated histories…).
    Unknown(String),
}

/// The merge Git would make, in memory: `merge-tree --write-tree` (Git 2.38) writes the merged
/// tree's objects to the object database and moves no branch, index or file. Names only, and
/// unquoted, so a name with accents reads as it is.
pub fn test_args(branch: &str) -> Vec<String> {
    ["-c", "core.quotePath=false", "merge-tree", "--write-tree", "--name-only", "--no-messages", "HEAD", branch].iter().map(|s| s.to_string()).collect()
}

/// Reads `git merge-tree`'s answer: exit 0, a clean merge (its tree's id); exit 1, conflicts (the
/// tree's id, then one conflicted file a line); 129, an option this Git does not know.
pub fn parse_test(code: Option<i32>, out: &str, err: &str) -> Test {
    let mut lines = out.lines();
    let tree = lines.next().unwrap_or("").trim();
    let is_oid = tree.len() >= 40 && tree.chars().all(|c| c.is_ascii_hexdigit());
    match code {
        Some(0) => Test::Clean,
        Some(1) if is_oid => {
            let mut files: Vec<String> = Vec::new();
            for l in lines.map(str::trim).filter(|l| !l.is_empty()) {
                if !files.iter().any(|f| f == l) {
                    files.push(l.to_string());
                }
            }
            Test::Conflicts(files)
        }
        Some(129) => Test::Unknown("Checking for conflicts beforehand needs Git 2.38 or later.".into()),
        _ => {
            let why = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git merge-tree failed");
            Test::Unknown(format!("Couldn't check for conflicts beforehand: {}", why.trim_start_matches("fatal: ")))
        }
    }
}

/// Runs the check in `root` (blocking: call it from a thread).
pub fn test(root: &Path, branch: &str) -> Test {
    let out = std::process::Command::new("git").args(test_args(branch)).current_dir(root).env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C").output();
    match out {
        Ok(o) => parse_test(o.status.code(), &String::from_utf8_lossy(&o.stdout), &String::from_utf8_lossy(&o.stderr)),
        Err(e) => Test::Unknown(format!("Couldn't start git: {e}")),
    }
}

/// The check, run in a thread and polled each frame by the dialog.
pub struct Check {
    pub test: Test,
    rx: Option<Receiver<Test>>,
}

impl Check {
    pub fn start(root: PathBuf, branch: String) -> Check {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(test(&root, &branch));
        });
        Check { test: Test::Checking, rx: Some(rx) }
    }

    /// A result known already (the sample project, a fast-forward).
    pub fn done(test: Test) -> Check {
        Check { test, rx: None }
    }

    /// Takes the thread's answer when it came. True while it is still checking.
    pub fn poll(&mut self) -> bool {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(t) => {
                    self.test = t;
                    self.rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.test = Test::Unknown("Couldn't check for conflicts beforehand.".into());
                    self.rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        self.rx.is_some()
    }
}

// ------------------------------------------------------------------ the merge

/// Merges `m.branch` into the checked-out branch the `way` chosen: `merge --ff-only <branch>` |
/// `merge --no-ff --no-edit <branch>` | `merge --squash <branch>` then `commit` (when it brought
/// any change). A merge or squash stopped on conflicts stays where Git stopped; Undo takes it back.
pub fn run(repo: &Repo, m: &Merge, way: Way) -> Result<String, String> {
    let args = m.args(way);
    if let Err(e) = repo.git(&args.iter().map(|x| x.as_str()).collect::<Vec<_>>()) {
        // A squash stopped on conflicts leaves no MERGE_HEAD, only conflicted files.
        let conflicts = repo.conflicted_files();
        if repo.operation().is_some() || (way == Way::Squash && !conflicts.is_empty()) {
            let what = if way == Way::Squash { "squash" } else { "merge" };
            return Err(format!(
                "The {what} of {} into {} stopped on conflicts in {}. Resolve them in a terminal, or Undo (⌘Z) to go back to before the {what}.",
                m.branch,
                m.head,
                git::file_list(&conflicts)
            ));
        }
        let local = ["would be overwritten", "uncommitted changes", "not uptodate", "Your local changes"].iter().any(|s| e.contains(s));
        return Err(if local { format!("{e}\nCommit or stash your changes first.") } else { e });
    }
    if way == Way::Squash {
        // A squash only stages the branch's changes: commit them, when there are any.
        if repo.git(&["diff", "--cached", "--quiet"]).is_ok() {
            return Ok(format!("Nothing to squash: {} already has the changes of {}", m.head, m.branch));
        }
        repo.git(&["commit", "-q", "-m", &m.title(Way::Squash)])
            .map_err(|e| format!("{} was squashed into the index, but the commit failed: {e}. Undo (⌘Z) goes back to before the squash.", m.branch))?;
    }
    Ok(m.done(way))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merge(ours: usize, theirs: usize) -> Merge {
        Merge { branch: "feature".into(), remote: false, head: "main".into(), ours, theirs }
    }

    fn args(m: &Merge, s: MergeStyle, no_ff: bool) -> String {
        m.args(m.way(s, no_ff)).join(" ")
    }

    #[test]
    fn counts_come_from_rev_list() {
        assert_eq!(parse_counts("2\t3\n"), Some((2, 3)));
        assert_eq!(parse_counts("0 0"), Some((0, 0)));
        assert_eq!(parse_counts("x"), None);
        assert_eq!(parse_counts(""), None);
    }

    #[test]
    fn a_fast_forward_is_possible_only_without_commits_of_its_own() {
        assert!(merge(0, 3).can_fast_forward());
        assert!(!merge(2, 3).can_fast_forward());
        assert!(!merge(0, 0).can_fast_forward(), "nothing to bring in");
        assert!(merge(0, 0).up_to_date() && merge(4, 0).up_to_date());
        assert!(!merge(0, 1).up_to_date());
    }

    #[test]
    fn the_way_follows_the_style_and_the_no_fast_forward_box() {
        use MergeStyle::*;
        let ff = merge(0, 3);
        assert_eq!(ff.way(Commit, false), Way::FastForward);
        assert_eq!(ff.way(Commit, true), Way::Commit);
        assert_eq!(ff.way(Squash, false), Way::Squash);
        assert_eq!(ff.way(Squash, true), Way::Squash, "the box does not matter to a squash");
        // Diverged: a merge commit, box or not.
        let d = merge(2, 3);
        assert_eq!(d.way(Commit, false), Way::Commit);
        assert_eq!(d.way(Commit, true), Way::Commit);
    }

    #[test]
    fn git_arguments_of_each_way() {
        use MergeStyle::*;
        assert_eq!(args(&merge(0, 3), Commit, false), "merge --ff-only feature");
        assert_eq!(args(&merge(0, 3), Commit, true), "merge --no-ff --no-edit feature");
        assert_eq!(args(&merge(2, 3), Commit, false), "merge --no-ff --no-edit feature");
        assert_eq!(args(&merge(2, 3), Squash, false), "merge --squash feature");
        let remote = Merge { branch: "origin/feature".into(), remote: true, ..merge(1, 1) };
        assert_eq!(args(&remote, Commit, false), "merge --no-ff --no-edit origin/feature");
    }

    #[test]
    fn titles_buttons_and_sentences() {
        use MergeStyle::*;
        let ff = merge(0, 3);
        assert_eq!(ff.title(Way::FastForward), "Fast-forward main to feature");
        assert_eq!(ff.title(Way::Commit), "Merge feature into main");
        assert_eq!(ff.title(Way::Squash), "Squash feature into main");
        assert_eq!(Merge::button(Way::FastForward), "Fast-forward");
        assert_eq!(ff.counts(), "3 commits to bring in, a fast-forward is possible");
        assert_eq!(merge(2, 1).counts(), "1 commit to bring in, main has 2 commits of its own");
        assert_eq!(merge(2, 0).counts(), "nothing to merge");
        assert_eq!(merge(2, 1).summary(), "main and feature have diverged: 2 commits here, 1 commit there.");
        assert_eq!(merge(0, 0).explain(Commit, false), merge(0, 0).summary());
        assert!(ff.explain(Commit, false).contains("No fast-forward"));
        assert!(ff.explain(Commit, true).contains("though a fast-forward was possible"));
        assert!(merge(1, 1).explain(Squash, false).starts_with("The 1 commit of feature lands on main"));
    }

    #[test]
    fn the_preview_shows_a_merge_commit_with_no_fast_forward() {
        use MergeStyle::*;
        let ff = merge(0, 3);
        assert_eq!(ff.outcome(Commit, false), Outcome::FastForward);
        assert_eq!(ff.outcome(Commit, true), Outcome::NoFastForward);
        assert_eq!(ff.outcome(Squash, true), Outcome::Squash);
        assert_eq!(merge(2, 3).outcome(Commit, false), Outcome::Merge);
    }

    #[test]
    fn merge_tree_output_is_read_as_clean_conflicts_or_unknown() {
        let oid = "e916b57b73b8114e1879dda4078c3cc73b5846ba";
        assert_eq!(parse_test(Some(0), &format!("{oid}\n"), ""), Test::Clean);
        assert_eq!(parse_test(Some(1), &format!("{oid}\nf.txt\nsrc/é n.rs\nf.txt\n\n"), ""), Test::Conflicts(vec!["f.txt".into(), "src/é n.rs".into()]));
        // Git older than 2.38 does not know --write-tree: usage, exit 129.
        assert_eq!(parse_test(Some(129), "", "error: unknown option `write-tree'\nusage: git merge-tree"), Test::Unknown("Checking for conflicts beforehand needs Git 2.38 or later.".into()));
        assert_eq!(parse_test(Some(128), "", "fatal: refusing to merge unrelated histories\n"), Test::Unknown("Couldn't check for conflicts beforehand: refusing to merge unrelated histories".into()));
        // Exit 1 without a tree id is an error, not conflicts.
        assert!(matches!(parse_test(Some(1), "", "fatal: bad"), Test::Unknown(_)));
        assert!(matches!(parse_test(None, "", ""), Test::Unknown(_)));
        assert_eq!(test_args("feature")[2..].join(" "), "merge-tree --write-tree --name-only --no-messages HEAD feature");
    }
}

/// The merge on real repositories: the check beforehand, each way, and Undo taking it back.
#[cfg(test)]
mod repo_tests {
    use super::*;
    use crate::git::run_in;
    use crate::undo::Snapshot;
    use std::path::PathBuf;

    /// A repository on `main` (f.txt line 2 "main"), with `clean` (a new file, from main's
    /// parent), `clash` (f.txt line 2 "clash", from main's parent) and `ahead` (one commit on top
    /// of main: a fast-forward is possible).
    fn fixture(name: &str) -> (PathBuf, Repo) {
        let dir = std::env::temp_dir().join(format!("fourchette-merge-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let g = |args: &[&str]| run_in(&dir, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "A"]);
        g(&["config", "user.email", "a@example.com"]);
        std::fs::write(dir.join("f.txt"), "1\n2\n3\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "base"]);
        g(&["checkout", "-q", "-b", "clean"]);
        std::fs::write(dir.join("new.txt"), "new\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "clean"]);
        g(&["checkout", "-q", "-b", "clash", "main"]);
        std::fs::write(dir.join("f.txt"), "1\nclash\n3\n").unwrap();
        g(&["commit", "-q", "-am", "clash"]);
        g(&["checkout", "-q", "main"]);
        std::fs::write(dir.join("f.txt"), "1\nmain\n3\n").unwrap();
        g(&["commit", "-q", "-am", "main"]);
        g(&["checkout", "-q", "-b", "ahead"]);
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "ahead"]);
        g(&["checkout", "-q", "main"]);
        (dir.clone(), Repo::open(&dir).unwrap())
    }

    /// What `Tab::merge` does: one snapshot, the merge, `after`.
    fn merge(repo: &Repo, branch: &str, style: MergeStyle, no_ff: bool) -> (Snapshot, Result<String, String>) {
        let m = Merge::read(repo, branch, false).unwrap();
        let way = m.way(style, no_ff);
        let mut snap = repo.snapshot(&m.title(way), false, false);
        let res = run(repo, &m, way);
        repo.after(&mut snap);
        if way == Way::Squash && snap.left.is_none() && !repo.conflicted_files().is_empty() {
            snap.left = Some(crate::undo::SQUASH.into());
        }
        (snap, res)
    }

    fn git_new_enough(repo: &Repo) -> bool {
        !matches!(test(&repo.root, "clean"), Test::Unknown(_))
    }

    #[test]
    fn the_check_foresees_conflicts_without_touching_the_repository() {
        let (dir, repo) = fixture("test");
        if !git_new_enough(&repo) {
            return;
        }
        let before = (repo.state(), repo.git(&["status", "--porcelain"]).unwrap());
        assert_eq!(test(&repo.root, "clean"), Test::Clean);
        assert_eq!(test(&repo.root, "ahead"), Test::Clean);
        assert_eq!(test(&repo.root, "clash"), Test::Conflicts(vec!["f.txt".into()]));
        assert!(matches!(test(&repo.root, "no-such-branch"), Test::Unknown(_)));
        assert_eq!((repo.state(), repo.git(&["status", "--porcelain"]).unwrap()), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn counts_and_fast_forward_from_the_repository() {
        let (dir, repo) = fixture("read");
        let m = Merge::read(&repo, "ahead", false).unwrap();
        assert_eq!((m.head.as_str(), m.ours, m.theirs), ("main", 0, 1));
        assert!(m.can_fast_forward());
        let m = Merge::read(&repo, "clash", false).unwrap();
        assert_eq!((m.ours, m.theirs), (1, 1));
        repo.git(&["checkout", "-q", "ahead"]).unwrap();
        assert!(Merge::read(&repo, "main", false).unwrap().up_to_date());
        repo.git(&["checkout", "-q", "--detach"]).unwrap();
        assert!(Merge::read(&repo, "main", false).is_err(), "no branch to merge into");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fast_forward_and_no_fast_forward_and_undo() {
        let (dir, repo) = fixture("ff");
        let before = repo.state();
        let (snap, res) = merge(&repo, "ahead", MergeStyle::Commit, false);
        assert_eq!(res, Ok("Fast-forwarded main to ahead".into()));
        assert_eq!(repo.git(&["rev-parse", "main"]).unwrap(), repo.git(&["rev-parse", "ahead"]).unwrap());
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);

        let (snap, res) = merge(&repo, "ahead", MergeStyle::Commit, true);
        assert_eq!(res, Ok("Merged ahead into main".into()));
        assert_eq!(repo.git(&["rev-list", "--count", "--merges", "HEAD"]).unwrap().trim(), "1", "a merge commit");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert!(!repo.root.join("a.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn squash_commits_once_and_undo_takes_it_back() {
        let (dir, repo) = fixture("squash");
        let before = repo.state();
        let (snap, res) = merge(&repo, "clean", MergeStyle::Squash, false);
        assert_eq!(res, Ok("Squashed clean into main".into()));
        assert_eq!(repo.git(&["log", "-1", "--format=%s"]).unwrap().trim(), "Squash clean into main");
        assert_eq!(repo.git(&["log", "-1", "--format=%p"]).unwrap().split_whitespace().count(), 1, "a single parent");
        assert!(repo.root.join("new.txt").exists());
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert!(!repo.root.join("new.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_merge_or_squash_stopped_on_conflicts_is_taken_back_whole() {
        let (dir, repo) = fixture("conflicts");
        std::fs::write(repo.root.join("own.txt"), "mine\n").unwrap();
        let before = repo.state();
        for style in [MergeStyle::Commit, MergeStyle::Squash] {
            let (snap, res) = merge(&repo, "clash", style, false);
            let e = res.unwrap_err();
            assert!(e.contains("stopped on conflicts in f.txt"), "{e}");
            assert!(snap.left.is_some(), "{style:?}: Undo knows what to abort");
            repo.undo(&snap).unwrap();
            assert_eq!(repo.state(), before, "{style:?}");
            assert!(repo.conflicted_files().is_empty());
            assert_eq!(std::fs::read_to_string(repo.root.join("f.txt")).unwrap(), "1\nmain\n3\n");
            assert_eq!(std::fs::read_to_string(repo.root.join("own.txt")).unwrap(), "mine\n", "an untracked file stays");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_changes_in_the_way_are_refused() {
        let (dir, repo) = fixture("dirty");
        std::fs::write(repo.root.join("f.txt"), "edited\n").unwrap();
        let before = repo.state();
        let (_, res) = merge(&repo, "clash", MergeStyle::Commit, false);
        let e = res.unwrap_err();
        assert!(e.contains("Commit or stash your changes first"), "{e}");
        assert_eq!(repo.state(), before);
        assert_eq!(std::fs::read_to_string(repo.root.join("f.txt")).unwrap(), "edited\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
