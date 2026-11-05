//! The Pull dialog (#101), the original's PullWindowController (`showPullSheet:`, its
//! `rebaseCheckBox` and `stashAndReapplyCheckBox`): the remote branch to pull into the current
//! branch (its upstream by default), Merge or Rebase (from Settings › Git), and "Stash and
//! reapply" when there are local changes. The toolbar's Pull still pulls at once with the global
//! settings (`Tab::pull`); both run under the same kind of undo snapshot.
use crate::git::{self, GitRef, RefKind, Repo};
use crate::preview::{Outcome, Preview};

/// How the pulled commits join the branch when it has commits of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Merge,
    Rebase,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Merge, Mode::Rebase];

    /// Settings › Git's "Pull" choice.
    pub fn from_setting(rebase: bool) -> Self {
        if rebase {
            Mode::Rebase
        } else {
            Mode::Merge
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Mode::Merge => "Merge",
            Mode::Rebase => "Rebase",
        }
    }
}

/// A remote branch: its remote ("origin") and its name there ("feature/x").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub remote: String,
    pub branch: String,
}

impl Source {
    /// "origin/feature/x", as the history shows it.
    pub fn name(&self) -> String {
        format!("{}/{}", self.remote, self.branch)
    }
}

/// The remote branches of `refs`, split into remote and branch by the longest of `remotes` that
/// starts the name (a remote's name may hold a slash); at the first slash when `remotes` says
/// nothing. A remote's HEAD is left out.
pub fn sources(refs: &[GitRef], remotes: &[String]) -> Vec<Source> {
    refs.iter()
        .filter(|r| r.kind == RefKind::Remote && !r.name.ends_with("/HEAD"))
        .filter_map(|r| {
            let by_remote = remotes.iter().filter(|m| r.name.len() > m.len() + 1 && r.name.starts_with(m.as_str()) && r.name.as_bytes()[m.len()] == b'/').max_by_key(|m| m.len());
            let (remote, branch) = match by_remote {
                Some(m) => (m.as_str(), &r.name[m.len() + 1..]),
                None => r.name.split_once('/')?,
            };
            Some(Source { remote: remote.into(), branch: branch.into() })
        })
        .collect()
}

/// What the Pull dialog is about: the current branch and the remote branches it can pull.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pull {
    pub branch: String,
    pub sources: Vec<Source>,
    /// The chosen one, an index of `sources`.
    pub sel: usize,
    /// Commits only the branch has and only the chosen remote branch has, as the last fetch saw
    /// them; None until counted.
    pub counts: Option<(usize, usize)>,
}

impl Pull {
    /// The dialog for `branch`, on its upstream when it has one among `sources`, else the branch
    /// of the same name on `default_remote`, else on any remote, else the first. None without any
    /// remote branch.
    pub fn new(branch: &str, upstream: Option<(String, String)>, sources: Vec<Source>, default_remote: Option<&str>) -> Option<Self> {
        if sources.is_empty() {
            return None;
        }
        let up = upstream.and_then(|(r, b)| sources.iter().position(|s| s.remote == r && s.branch == b));
        let same = || {
            let named = |s: &Source| s.branch == branch;
            sources.iter().position(|s| named(s) && Some(s.remote.as_str()) == default_remote).or_else(|| sources.iter().position(named))
        };
        let sel = up.or_else(same).unwrap_or(0);
        Some(Pull { branch: branch.into(), sources, sel, counts: None })
    }

    pub fn source(&self) -> &Source {
        &self.sources[self.sel]
    }

    pub fn ahead(&self) -> usize {
        self.counts.map_or(0, |c| c.0)
    }

    pub fn behind(&self) -> usize {
        self.counts.map_or(0, |c| c.1)
    }

    /// The branch has nothing of its own, so the pull only moves it up: Merge and Rebase both
    /// come to a fast-forward.
    pub fn fast_forward(&self) -> bool {
        self.ahead() == 0 && self.behind() > 0
    }

    /// What the preview shows: nothing new, a fast-forward when possible, else the mode.
    pub fn outcome(&self, mode: Mode) -> Outcome {
        match (self.behind(), self.ahead(), mode) {
            (0, _, _) => Outcome::Stay,
            (_, 0, _) => Outcome::FastForward,
            (_, _, Mode::Merge) => Outcome::Merge,
            (_, _, Mode::Rebase) => Outcome::Rebase,
        }
    }

    pub fn preview(&self, mode: Mode) -> Preview {
        Preview::new(&self.branch, &self.source().name(), self.ahead(), self.behind(), self.outcome(mode))
    }

    /// "3 behind", "2 ahead, 3 behind" or "up to date", next to the remote branch's name.
    pub fn counts_text(&self) -> String {
        match (self.ahead(), self.behind()) {
            (0, 0) => "up to date".into(),
            (0, b) => format!("{b} behind"),
            (a, 0) => format!("{a} ahead"),
            (a, b) => format!("{a} ahead, {b} behind"),
        }
    }

    /// The result's title above the preview.
    pub fn title(&self, mode: Mode) -> String {
        let (b, up) = (&self.branch, self.source().name());
        match self.outcome(mode) {
            Outcome::Stay => format!("Bring in what's new on {up}"),
            Outcome::FastForward => format!("Fast-forward {b} to {up}"),
            Outcome::Rebase => format!("Rebase {b} onto {up}"),
            _ => format!("Merge {up} into {b}"),
        }
    }

    /// Where the branch stands: the sentence of the "Now" view.
    pub fn summary(&self) -> String {
        let (b, up) = (&self.branch, self.source().name());
        match (self.ahead(), self.behind()) {
            (0, 0) => format!("{b} has every commit of {up} the last fetch saw."),
            (0, n) => format!("{b} is {} behind {up}.", commits(n)),
            (a, 0) => format!("{b} is {} ahead of {up}, with nothing to bring in at the last fetch.", commits(a)),
            (a, n) => format!("{b} and {up} have diverged: {} here, {} there.", commits(a), commits(n)),
        }
    }

    /// What the pull does to the branch: the sentence of the "After" view.
    pub fn explain(&self, mode: Mode) -> String {
        let (b, up) = (&self.branch, self.source().name());
        let (mine, theirs) = (commits(self.ahead()), commits(self.behind()));
        match self.outcome(mode) {
            Outcome::Stay => format!("The pull fetches {up} first, then brings in any commit that arrived since the last fetch."),
            Outcome::FastForward => format!("{b} slides up to {up}: a fast-forward. No new commit, nothing rewritten."),
            Outcome::Rebase => format!("Your {mine} are replayed on top of {up} as new commits. The old ones leave the branch."),
            _ => format!("A new merge commit on {b} joins its {mine} with the {theirs} of {up}. Nothing is rewritten."),
        }
    }

    /// Every sentence the dialog can show, so its room keeps one height.
    pub fn sentences(&self) -> Vec<String> {
        let mut v = vec![self.summary()];
        v.extend(Mode::ALL.iter().map(|m| self.explain(*m)));
        v
    }

    /// Git's arguments. Merge says `--ff` too, so the fast-forward is made whenever possible
    /// whatever `pull.ff` or `merge.ff` say; Rebase fast-forwards by itself.
    pub fn args(&self, mode: Mode, stash: bool) -> Vec<String> {
        let s = self.source();
        let mut a = git::pull_args(&s.remote, &s.branch, mode == Mode::Rebase, stash);
        if mode == Mode::Merge {
            a.insert(3, "--ff".into());
        }
        a
    }
}

fn commits(n: usize) -> String {
    if n == 1 {
        "1 commit".into()
    } else {
        format!("{n} commits")
    }
}

/// The repository's remotes, for `sources`.
pub fn remotes(repo: &Repo) -> Vec<String> {
    repo.git(&["remote"]).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// Commits only HEAD has and only the remote branch `name` has ("origin/main").
pub fn count(repo: &Repo, name: &str) -> Option<(usize, usize)> {
    let out = repo.git(&["rev-list", "--left-right", "--count", &format!("HEAD...refs/remotes/{name}")]).ok()?;
    let mut n = out.split_whitespace().map(|x| x.parse::<usize>().ok());
    Some((n.next()??, n.next()??))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &str, kind: RefKind) -> GitRef {
        GitRef { name: name.into(), full: name.into(), kind, target: "a".into(), is_head: false, upstream: String::new(), ahead: 0, behind: 0 }
    }

    fn src(remote: &str, branch: &str) -> Source {
        Source { remote: remote.into(), branch: branch.into() }
    }

    fn pull(ahead: usize, behind: usize) -> Pull {
        Pull { branch: "main".into(), sources: vec![src("origin", "main")], sel: 0, counts: Some((ahead, behind)) }
    }

    #[test]
    fn remote_branches_split_on_the_longest_remote_and_skip_head() {
        let refs = vec![r("main", RefKind::Local), r("origin/HEAD", RefKind::Remote), r("origin/feature/x", RefKind::Remote), r("my/fork/main", RefKind::Remote), r("v1", RefKind::Tag)];
        let remotes = ["origin".to_string(), "my".into(), "my/fork".into()];
        assert_eq!(sources(&refs, &remotes), [src("origin", "feature/x"), src("my/fork", "main")]);
        // Without the remotes: at the first slash.
        assert_eq!(sources(&refs, &[]), [src("origin", "feature/x"), src("my", "fork/main")]);
    }

    #[test]
    fn the_upstream_is_chosen_first_then_the_same_name() {
        let s = vec![src("upstream", "main"), src("origin", "develop"), src("origin", "main")];
        let p = |up: Option<(&str, &str)>, branch: &str, def: Option<&str>| Pull::new(branch, up.map(|(a, b)| (a.into(), b.into())), s.clone(), def).unwrap().sel;
        assert_eq!(p(Some(("origin", "develop")), "main", Some("origin")), 1, "the upstream wins");
        assert_eq!(p(None, "main", Some("origin")), 2, "the same name on the default remote");
        assert_eq!(p(None, "main", None), 0, "else on any remote");
        assert_eq!(p(Some(("gone", "x")), "topic", None), 0, "else the first");
        assert_eq!(Pull::new("main", None, vec![], None), None, "nothing to pull from");
    }

    #[test]
    fn a_fast_forward_is_shown_whenever_possible() {
        use Outcome::*;
        let behind = pull(0, 3);
        assert!(behind.fast_forward());
        assert_eq!((behind.outcome(Mode::Merge), behind.outcome(Mode::Rebase)), (FastForward, FastForward));
        assert_eq!(behind.title(Mode::Rebase), "Fast-forward main to origin/main");
        assert_eq!(behind.counts_text(), "3 behind");
        let diverged = pull(2, 1);
        assert!(!diverged.fast_forward());
        assert_eq!((diverged.outcome(Mode::Merge), diverged.outcome(Mode::Rebase)), (Merge, Rebase));
        assert_eq!(diverged.title(Mode::Merge), "Merge origin/main into main");
        assert_eq!(diverged.summary(), "main and origin/main have diverged: 2 commits here, 1 commit there.");
        assert_eq!(diverged.counts_text(), "2 ahead, 1 behind");
        // Nothing new at the last fetch: nothing moves on the preview, the pull still fetches.
        let same = pull(0, 0);
        assert_eq!(same.outcome(Mode::Merge), Stay);
        assert_eq!(same.counts_text(), "up to date");
        assert_eq!(pull(1, 0).outcome(Mode::Rebase), Stay);
        // Not counted yet: as if up to date.
        assert_eq!(Pull { counts: None, ..pull(1, 1) }.outcome(Mode::Merge), Stay);
    }

    #[test]
    fn args_name_the_remote_branch_the_mode_and_the_stash() {
        let mut p = pull(1, 1);
        p.sources.push(src("my/fork", "feature/x"));
        assert_eq!(p.args(Mode::Merge, false).join(" "), "pull --no-edit --no-rebase --ff origin main");
        assert_eq!(p.args(Mode::Rebase, true).join(" "), "pull --no-edit --rebase --autostash origin main");
        p.sel = 1;
        assert_eq!(p.args(Mode::Merge, true).join(" "), "pull --no-edit --no-rebase --ff --autostash my/fork feature/x");
        assert_eq!(Mode::from_setting(true), Mode::Rebase);
        assert_eq!(Mode::from_setting(false), Mode::Merge);
    }

    /// The pull on a real repository from a remote branch that is not the upstream, then Undo:
    /// the branch and the changes come back, the fetched remote branch stays.
    #[test]
    fn a_pull_from_another_remote_branch_fast_forwards_and_undo_takes_it_back() {
        use crate::undo::Snapshot;
        let (base, repo) = crate::checkout::repo_tests::fixture("pull-dialog");
        // On `side` (no upstream, at the common commit); origin/main is 1 ahead of it.
        std::fs::write(repo.root.join("new.txt"), "dirty\n").unwrap();
        let before = repo.state();
        let up = repo.upstream();
        let p = Pull::new("side", up, sources(&repo.refs(), &remotes(&repo)), repo.default_remote().as_deref()).unwrap();
        let p = Pull { sel: p.sources.iter().position(|s| s.name() == "origin/main").unwrap(), ..p };
        let p = Pull { counts: count(&repo, &p.source().name()), ..p };
        assert_eq!(p.counts, Some((0, 1)));
        assert!(p.fast_forward());
        let mut snap = Snapshot { dirty: repo.stash_create(), ..repo.snapshot("Pull", false, false) };
        let args = p.args(Mode::Merge, true);
        repo.git(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>()).unwrap();
        repo.after(&mut snap);
        assert_eq!(repo.git(&["rev-parse", "side"]).unwrap(), repo.git(&["rev-parse", "origin/main"]).unwrap(), "fast-forwarded");
        assert_eq!(repo.git(&["rev-list", "--count", "--merges", "HEAD"]).unwrap().trim(), "0", "no merge commit");
        assert_eq!(std::fs::read_to_string(repo.root.join("new.txt")).unwrap(), "dirty\n");
        repo.undo(&snap).unwrap();
        assert_eq!(repo.state(), before);
        assert_eq!(std::fs::read_to_string(repo.root.join("new.txt")).unwrap(), "dirty\n");
        let _ = std::fs::remove_dir_all(&base);
    }
}
