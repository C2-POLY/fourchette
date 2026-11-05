//! The Push dialog, the original's PushWindowController (`forcePushCheckBox`,
//! `pushAllTagsCheckBox`): the remote and the remote branch to push to (the tracked one by
//! default, or a new one, then tracked with `--set-upstream`), "Force push", always as
//! `--force-with-lease`, and "Push all tags" (`--tags`, off by default: "pushSheetPushAllTags").
//! The toolbar's Push still pushes at once (`Tab::push`). A push is not undoable.
use crate::git::{self, GitRef, RefKind};

/// What the Push dialog shows and what the person chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Push {
    /// The checked-out branch.
    pub branch: String,
    /// The repository's remotes, and the one chosen.
    pub remotes: Vec<String>,
    pub remote: String,
    /// The branch on the remote, as typed ("main", "feature/x").
    pub target: String,
    /// The branch's upstream as (remote, branch), when it has one.
    pub upstream: Option<(String, String)>,
    /// The remote branches the last fetch saw ("origin/main").
    pub known: Vec<String>,
    /// Commits only the branch has, and only the remote branch has, for `counted`.
    pub ahead: usize,
    pub behind: usize,
    /// The (remote, target) the counts are for.
    pub counted: (String, String),
    pub force: bool,
    pub tags: bool,
    /// The person clicked Force push once, and is asked to confirm what it replaces.
    pub confirm: bool,
}

fn commits(n: usize) -> String {
    if n == 1 {
        "1 commit".into()
    } else {
        format!("{n} commits")
    }
}

/// Characters Git refuses in a branch name (`git check-ref-format`).
fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && !n.starts_with(['-', '/', '.'])
        && !n.ends_with(['/', '.'])
        && !n.ends_with(".lock")
        && !n.contains("..")
        && !n.contains("//")
        && !n.contains("@{")
        && !n.chars().any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
}

impl Push {
    /// The dialog for `branch`, from the remotes and the refs: to its upstream when it has one,
    /// else to a branch of the same name on `default_remote` (or the first remote). `None`
    /// without a remote.
    pub fn new(branch: &str, remotes: Vec<String>, refs: &[GitRef], default_remote: Option<String>) -> Option<Push> {
        let local = refs.iter().find(|r| r.kind == RefKind::Local && r.name == branch);
        let upstream = local.and_then(|r| r.upstream.split_once('/')).map(|(r, b)| (r.to_string(), b.to_string())).filter(|(r, _)| remotes.contains(r));
        let remote = match &upstream {
            Some((r, _)) => r.clone(),
            None => default_remote.filter(|r| remotes.contains(r)).or_else(|| remotes.first().cloned())?,
        };
        let target = upstream.as_ref().map_or_else(|| branch.to_string(), |(_, b)| b.clone());
        let known = refs.iter().filter(|r| r.kind == RefKind::Remote && !r.name.ends_with("/HEAD")).map(|r| r.name.clone()).collect();
        let mut p = Push {
            branch: branch.into(),
            remotes,
            remote,
            target,
            upstream,
            known,
            ahead: 0,
            behind: 0,
            counted: (String::new(), String::new()),
            force: false,
            tags: false,
            confirm: false,
        };
        if p.is_upstream() {
            if let Some(l) = local {
                (p.ahead, p.behind) = (l.ahead, l.behind);
                p.counted = p.key();
            }
        }
        Some(p)
    }

    /// The remotes in `git remote`'s output, else those the refs name ("origin/main").
    pub fn remotes_of(out: Option<&str>, refs: &[GitRef]) -> Vec<String> {
        let mut v: Vec<String> = match out {
            Some(o) => o.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect(),
            None => refs.iter().filter(|r| r.kind == RefKind::Remote).filter_map(|r| r.name.split_once('/')).map(|(r, _)| r.to_string()).collect(),
        };
        v.dedup();
        let mut seen = std::collections::HashSet::new();
        v.retain(|r| seen.insert(r.clone()));
        v
    }

    pub fn key(&self) -> (String, String) {
        (self.remote.clone(), self.target.trim().to_string())
    }

    /// "origin/main".
    pub fn remote_ref(&self) -> String {
        format!("{}/{}", self.remote, self.target.trim())
    }

    /// The remote branch exists, as far as the last fetch knows.
    pub fn exists(&self) -> bool {
        self.known.contains(&self.remote_ref())
    }

    pub fn is_upstream(&self) -> bool {
        self.upstream.as_ref() == Some(&self.key())
    }

    /// The counts are for another remote branch than the one chosen.
    pub fn stale(&self) -> bool {
        self.counted != self.key()
    }

    /// Takes the counts for the chosen remote branch: the branch's own ones for its upstream,
    /// `count` (Git's `rev-list --left-right --count`) for another existing one, none for a new one.
    pub fn recount(&mut self, refs: &[GitRef], count: impl FnOnce(&str) -> Option<(usize, usize)>) {
        (self.ahead, self.behind) = if !self.exists() {
            (0, 0)
        } else if self.is_upstream() {
            refs.iter().find(|r| r.kind == RefKind::Local && r.name == self.branch).map_or((0, 0), |r| (r.ahead, r.behind))
        } else {
            count(&self.remote_ref()).unwrap_or((0, 0))
        };
        self.counted = self.key();
        self.confirm = false;
    }

    pub fn valid(&self) -> bool {
        valid_name(self.target.trim())
    }

    /// The remote branch is new or not the upstream yet: the push makes it the upstream. A push
    /// to another existing branch leaves the tracking as it is.
    pub fn set_upstream(&self) -> bool {
        self.upstream.is_none() || !self.exists()
    }

    /// The remote branch has commits the branch doesn't: a plain push is refused.
    pub fn diverged(&self) -> bool {
        self.exists() && self.behind > 0
    }

    /// A push that replaces commits on the remote, which is asked twice.
    pub fn replaces(&self) -> bool {
        self.force && self.diverged()
    }

    /// The primary button can push: a valid name, and the remote would take it.
    pub fn can_push(&self) -> bool {
        self.valid() && (!self.diverged() || self.force)
    }

    pub fn args(&self) -> Vec<String> {
        git::push_args(&self.remote, &self.branch, self.target.trim(), self.set_upstream(), self.force, self.tags)
    }

    /// The panel's title: what the push does.
    pub fn title(&self) -> String {
        let verb = if self.force { "Force push" } else { "Push" };
        format!("{verb} {} to {}", self.branch, self.remote_ref())
    }

    /// The primary button, which asks before replacing commits.
    pub fn button(&self) -> String {
        match (self.replaces(), self.confirm) {
            (true, true) => format!("Replace {} on {}", commits(self.behind), self.remote),
            (true, false) => "Force push…".into(),
            (false, _) if self.force => "Force push".into(),
            _ => "Push".into(),
        }
    }

    /// Next to the remote branch: whether it exists and how the branch stands against it.
    pub fn counts(&self) -> String {
        if !self.exists() {
            return format!("New on {}", self.remote);
        }
        match (self.ahead, self.behind) {
            (0, 0) => "Up to date".into(),
            (a, 0) => format!("{a} ahead"),
            (0, b) => format!("{b} behind"),
            (a, b) => format!("{a} ahead, {b} behind"),
        }
    }

    /// Under the remote branch: whether the push makes it the upstream.
    pub fn tracking(&self) -> String {
        match (self.exists(), self.is_upstream(), self.set_upstream()) {
            (false, _, _) => format!("{} creates it and tracks it", self.remote),
            (true, true, _) => format!("Tracked by {}", self.branch),
            (true, false, true) => format!("{} will track it", self.branch),
            (true, false, false) => format!("Not tracked by {}", self.branch),
        }
    }

    /// Where things stand: the sentence in the "Now" view.
    pub fn summary(&self) -> String {
        let (b, r) = (&self.branch, self.remote_ref());
        if !self.exists() {
            return format!("{r} doesn't exist yet: the push creates it with {b}'s commits.");
        }
        match (self.ahead, self.behind) {
            (0, 0) => format!("{b} and {r} are on the same commit."),
            (a, 0) => format!("{b} has {} that {r} doesn't.", commits(a)),
            (0, n) => format!("{r} has {} that {b} doesn't.", commits(n)),
            (a, n) => format!("{b} and {r} have diverged: {} here, {} there.", commits(a), commits(n)),
        }
    }

    /// What the push does: the sentence in the "After" view.
    pub fn explain(&self) -> String {
        let (b, r) = (&self.branch, self.remote_ref());
        let tags = if self.tags { " Every tag goes too." } else { "" };
        let s = if !self.exists() {
            format!("{r} is created at {b}'s last commit, and {b} tracks it.")
        } else if self.diverged() && !self.force {
            format!("Git refuses the push: {r} stays where it is.")
        } else if self.replaces() {
            format!("{r} is moved to {b}: its {} that {b} doesn't have leave {}.", commits(self.behind), self.remote)
        } else if self.ahead == 0 {
            format!("Nothing to send: {r} already has {b}'s commits.")
        } else {
            format!("{r} moves up to {b}: {} sent, nothing replaced.", commits(self.ahead))
        };
        format!("{s}{tags}")
    }

    /// The red note of the confirmation step.
    pub fn warning(&self) -> String {
        format!(
            "{} will lose {} that only it has. --force-with-lease refuses if it changed since the last fetch, but a push can't be undone with ⌘Z.",
            self.remote_ref(),
            commits(self.behind)
        )
    }

    /// The background job's label.
    pub fn label(&self) -> String {
        let verb = if self.force { "Force push" } else { "Push" };
        format!("{verb} to '{}'", self.remote)
    }

    /// The toast once Git has answered.
    pub fn done(&self, out: &str) -> String {
        if out.contains("Everything up-to-date") {
            return "Everything is up-to-date".into();
        }
        let verb = if self.force { "Force pushed" } else { "Pushed" };
        let tags = if self.tags { " with its tags" } else { "" };
        format!("{verb} {}{tags} to {}", self.branch, self.remote_ref())
    }

    /// The scenes of the preview.
    pub fn preview(&self) -> crate::preview::Preview {
        crate::preview::Preview::push(&self.branch, &self.remote_ref(), self.exists(), self.ahead, self.behind, self.force)
    }
}

/// Git's refusal in words: a remote with commits the branch doesn't have, or one that moved since
/// the last fetch (the lease).
pub fn refused(e: &str, remote_ref: &str) -> String {
    if e.contains("stale info") {
        format!("{remote_ref} changed since the last fetch, so the force push was refused. Fetch, look at what's new, then try again.")
    } else if e.contains("[rejected]") || e.contains("non-fast-forward") || e.contains("fetch first") {
        format!("{remote_ref} has commits that aren't here. Pull them first, or force push from Push… (⇧⌘P).")
    } else {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &str, kind: RefKind, upstream: &str, ahead: usize, behind: usize) -> GitRef {
        GitRef { name: name.into(), full: name.into(), kind, target: String::new(), is_head: false, upstream: upstream.into(), ahead, behind }
    }

    fn refs(ahead: usize, behind: usize) -> Vec<GitRef> {
        vec![
            r("main", RefKind::Local, "origin/main", ahead, behind),
            r("feature", RefKind::Local, "", 0, 0),
            r("origin/main", RefKind::Remote, "", 0, 0),
            r("origin/HEAD", RefKind::Remote, "", 0, 0),
            r("origin/dev", RefKind::Remote, "", 0, 0),
            r("fork/main", RefKind::Remote, "", 0, 0),
        ]
    }

    fn remotes() -> Vec<String> {
        vec!["origin".into(), "fork".into()]
    }

    #[test]
    fn it_pushes_to_the_upstream_by_default_without_force_or_tags() {
        let p = Push::new("main", remotes(), &refs(2, 0), Some("fork".into())).unwrap();
        assert_eq!((p.remote.as_str(), p.target.as_str()), ("origin", "main"));
        assert!(p.is_upstream() && p.exists() && !p.stale());
        assert_eq!((p.ahead, p.behind), (2, 0));
        assert!(!p.force && !p.tags, "pushSheetPushAllTags is false");
        assert!(!p.set_upstream() && !p.diverged() && p.can_push());
        assert_eq!(p.args(), ["-c", "push.default=upstream", "push", "origin", "main:refs/heads/main", "--verbose", "--progress"]);
        assert_eq!(p.button(), "Push");
        assert_eq!(p.counts(), "2 ahead");
        assert_eq!(p.tracking(), "Tracked by main");
        assert!(!p.known.contains(&"origin/HEAD".to_string()));
    }

    #[test]
    fn without_an_upstream_it_creates_the_branch_and_tracks_it() {
        let p = Push::new("feature", remotes(), &refs(0, 0), Some("origin".into())).unwrap();
        assert_eq!((p.remote.as_str(), p.target.as_str(), p.upstream.clone()), ("origin", "feature", None));
        assert!(!p.exists() && p.set_upstream());
        assert_eq!(p.counts(), "New on origin");
        assert_eq!(p.args(), ["-c", "push.default=upstream", "push", "origin", "feature:refs/heads/feature", "--set-upstream", "--verbose", "--progress"]);
        // The default remote when it is one of them, else the first.
        assert_eq!(Push::new("feature", remotes(), &refs(0, 0), Some("gone".into())).unwrap().remote, "origin");
        assert_eq!(Push::new("feature", vec![], &refs(0, 0), None), None);
    }

    #[test]
    fn a_new_name_is_tracked_another_existing_branch_is_not() {
        let mut p = Push::new("main", remotes(), &refs(1, 0), None).unwrap();
        p.target = "release".into();
        assert!(p.stale());
        p.recount(&refs(1, 0), |_| panic!("no count for a new branch"));
        assert!(!p.exists() && p.set_upstream() && (p.ahead, p.behind) == (0, 0));
        assert!(p.args().contains(&"main:refs/heads/release".to_string()));
        p.target = "dev".into();
        p.recount(&refs(1, 0), |r| {
            assert_eq!(r, "origin/dev");
            Some((4, 1))
        });
        assert!(p.exists() && !p.set_upstream() && p.diverged());
        assert_eq!(p.tracking(), "Not tracked by main");
        p.remote = "fork".into();
        p.target = "main".into();
        p.recount(&refs(1, 0), |_| Some((0, 0)));
        assert_eq!(p.counts(), "Up to date");
    }

    #[test]
    fn force_is_always_with_lease_and_tags_come_before_set_upstream() {
        let mut p = Push::new("feature", remotes(), &refs(0, 0), None).unwrap();
        (p.force, p.tags) = (true, true);
        assert_eq!(
            p.args(),
            ["-c", "push.default=upstream", "push", "origin", "feature:refs/heads/feature", "--force-with-lease", "--tags", "--set-upstream", "--verbose", "--progress"]
        );
        assert!(!p.args().contains(&"--force".to_string()));
        assert_eq!(p.done("To x\n   1..2  feature -> feature\n"), "Force pushed feature with its tags to origin/feature");
        assert_eq!(p.done("Everything up-to-date\n"), "Everything is up-to-date");
    }

    #[test]
    fn a_diverged_branch_needs_force_and_a_confirmation() {
        let mut p = Push::new("main", remotes(), &refs(2, 3), None).unwrap();
        assert!(p.diverged() && !p.can_push());
        assert!(p.explain().starts_with("Git refuses"));
        assert_eq!(p.summary(), "main and origin/main have diverged: 2 commits here, 3 commits there.");
        p.force = true;
        assert!(p.can_push() && p.replaces());
        assert_eq!(p.button(), "Force push…");
        p.confirm = true;
        assert_eq!(p.button(), "Replace 3 commits on origin");
        assert!(p.warning().contains("3 commits") && p.warning().contains("can't be undone"));
        // Another target asks again.
        p.recount(&refs(2, 3), |_| None);
        assert!(!p.confirm);
        // Force with nothing to replace pushes at once.
        let mut q = Push::new("main", remotes(), &refs(2, 0), None).unwrap();
        q.force = true;
        assert!(!q.replaces());
        assert_eq!(q.button(), "Force push");
    }

    #[test]
    fn names_git_refuses_are_refused() {
        for ok in ["main", "feature/x", "v1.2-fix"] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in ["", "a b", "-x", "a..b", "x/", "x.lock", "a:b", "a~1", "a^", "a*", "@{x"] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn remotes_come_from_git_or_from_the_refs() {
        assert_eq!(Push::remotes_of(Some("origin\nfork\n"), &[]), ["origin", "fork"]);
        assert_eq!(Push::remotes_of(None, &refs(0, 0)), ["origin", "fork"]);
    }

    #[test]
    fn refusals_say_what_to_do() {
        assert!(refused(" ! [rejected]        main -> main (fetch first)", "origin/main").contains("Pull them first"));
        assert!(refused(" ! [rejected]        main -> main (stale info)", "origin/main").contains("changed since the last fetch"));
        assert_eq!(refused("fatal: boom", "origin/main"), "fatal: boom");
    }

    /// Against a local bare remote: a diverged push is refused, a forced one goes through with
    /// the lease, tags go with --tags, and a new branch becomes the upstream.
    #[test]
    fn it_pushes_to_a_local_bare_remote() {
        let tmp = std::env::temp_dir().join(format!("fourchette-push-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (bare, a, b) = (tmp.join("remote.git"), tmp.join("a"), tmp.join("b"));
        std::fs::create_dir_all(&tmp).unwrap();
        let git = |dir: &std::path::Path, args: &[&str]| -> Result<String, String> { git::exec(dir, args).map(|(o, e)| format!("{o}{e}")) };
        let cfg = ["-c", "user.name=T", "-c", "user.email=t@example.com"];
        let commit = |dir: &std::path::Path, msg: &str| {
            let mut args = cfg.to_vec();
            args.extend(["commit", "-q", "--allow-empty", "-m", msg]);
            git(dir, &args).unwrap();
        };
        git(&tmp, &["init", "-q", "--bare", bare.to_str().unwrap()]).unwrap();
        git(&tmp, &["init", "-q", "-b", "main", a.to_str().unwrap()]).unwrap();
        git(&a, &["remote", "add", "origin", bare.to_str().unwrap()]).unwrap();
        commit(&a, "one");
        let run = |p: &Push| -> Result<String, String> {
            let args = p.args();
            let v: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            git(&a, &v).map(|o| p.done(&o)).map_err(|e| refused(&e, &p.remote_ref()))
        };
        let refs_of = |ahead, behind, up: &str| vec![r("main", RefKind::Local, up, ahead, behind), r("origin/main", RefKind::Remote, "", 0, 0)];
        // A first push creates main on origin and tracks it.
        let mut p = Push::new("main", vec!["origin".into()], &[r("main", RefKind::Local, "", 0, 0)], None).unwrap();
        assert!(p.set_upstream());
        assert_eq!(run(&p).unwrap(), "Pushed main to origin/main");
        assert_eq!(git(&a, &["rev-parse", "--abbrev-ref", "main@{upstream}"]).unwrap().trim(), "origin/main");
        // Someone else pushes; this clone commits too and fetches: diverged.
        git(&tmp, &["clone", "-q", "-b", "main", bare.to_str().unwrap(), b.to_str().unwrap()]).unwrap();
        commit(&b, "theirs");
        git(&b, &["push", "-q", "origin", "main"]).unwrap();
        commit(&a, "mine");
        git(&a, &["fetch", "-q", "origin"]).unwrap();
        p = Push::new("main", vec!["origin".into()], &refs_of(1, 1, "origin/main"), None).unwrap();
        assert!(p.diverged() && !p.can_push());
        let e = run(&p).unwrap_err();
        assert!(e.contains("Pull them first"), "{e}");
        // Forced, with a tag: origin/main is replaced and the tag goes too.
        git(&a, &["tag", "v1"]).unwrap();
        (p.force, p.tags) = (true, true);
        assert_eq!(run(&p).unwrap(), "Force pushed main with its tags to origin/main");
        let head = git(&a, &["rev-parse", "main"]).unwrap();
        assert_eq!(git(&bare, &["rev-parse", "main"]).unwrap(), head);
        assert_eq!(git(&bare, &["rev-parse", "v1"]).unwrap(), head);
        // The lease: origin moved since this clone's fetch, so a force push is refused.
        git(&b, &["fetch", "-q", "origin"]).unwrap();
        git(&b, &["reset", "-q", "--hard", "origin/main"]).unwrap();
        commit(&b, "theirs again");
        git(&b, &["push", "-q", "origin", "main"]).unwrap();
        commit(&a, "mine again");
        let e = run(&p).unwrap_err();
        assert!(e.contains("changed since the last fetch"), "{e}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
