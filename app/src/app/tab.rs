//! One open repository: its history, selection, commit message, undo steps and background
//! work. `App` holds the tabs and the app-wide parts; the views and commands act on the active
//! tab, and every tab polls its own jobs, so a result always lands in the tab that started it.
use super::*;

pub struct Tab {
    /// Never reused: salts this tab's egui ids, so tabs keep their own scroll and text fields.
    pub(super) key: u64,
    /// A repository restored at launch and not read yet: the tab reads it, and starts its
    /// timers (status refresh, auto-fetch), the first time it is shown (`App::activate`).
    pub(super) pending: Option<PathBuf>,
    /// The name a sample-project tab shows (`FOURCHETTE_TABS` screenshots).
    pub(super) sample_name: Option<String>,
    /// The home page shows in this tab instead of a repository (issue #109).
    pub(super) home: bool,
    pub(super) repo: Option<Repo>,
    pub(super) commits: Vec<Commit>,
    pub(super) rows: Vec<Row>,
    pub(super) max_lanes: usize,
    pub(super) refs: Vec<GitRef>,
    pub(super) by_target: HashMap<String, Vec<GitRef>>,
    pub(super) index: HashMap<String, usize>,
    pub(super) stashes: Vec<Stash>,
    pub(super) status: Status,
    pub(super) branch: Option<String>,
    pub(super) head: String,
    pub(super) ahead_behind: Option<(usize, usize)>,
    pub(super) sel: Sel,
    pub(super) details: Option<CommitDetails>,
    pub(super) details_for: String,
    pub(super) diff: Option<DiffView>,
    pub(super) summary: String,
    pub(super) description: String,
    pub(super) amend: bool,
    pub(super) search: String,
    pub(super) filter: String,
    pub(super) collapsed: HashSet<String>,
    /// The folders folded in the Tree view of the local changes, with whether it is the staged list.
    pub(super) collapsed_dirs: HashSet<(bool, String)>,
    /// The local changes show a tree (the settings' `files_tree`, copied each frame).
    pub(super) files_tree: bool,
    pub(super) busy: Option<(String, Receiver<JobResult>, bool)>,
    pub(super) last_status: Instant,
    pub(super) last_fetch: Instant,
    /// A dialog is about this repository (its branches, worktrees, files), so it stays with it.
    pub(super) dialog: Option<Dialog>,
    pub(super) scroll_to: Option<usize>,
    pub(super) project: project::ProjectState,
    pub(super) board: board::BoardState,
    pub(super) undo: Vec<Snapshot>,
    /// Snapshot taken before a background job that changes the repository (Pull).
    pub(super) pending_undo: Option<Snapshot>,
    pub(super) commit_limit: usize,
    /// The diff context and the subject limits of the settings, copied here each frame by `poll_tabs`.
    pub(super) diff_context: u32,
    /// The diff shows two columns / the whole file (the settings' `diff_split` and `diff_whole`).
    pub(super) diff_split: bool,
    pub(super) diff_whole: bool,
    pub(super) subject_limits: (usize, usize),
    /// The Claude model and the commit-message language of the settings, copied the same way.
    pub(super) claude_model: crate::settings::ClaudeModel,
    pub(super) commit_language: crate::settings::CommitLanguage,
    /// What the open diff shows, to draw it again when the context changes.
    pub(super) diff_req: Option<DiffReq>,
    pub(super) ai: assistant::AiState,
    pub(super) changelog: Option<super::changelog::ChangelogView>,
    pub(super) pr_draft: Option<super::pr_draft::PrDraftView>,
    pub(super) cockpit: Option<super::cockpit::CockpitView>,
    pub(super) resolve: Option<super::resolve::ResolveView>,
    pub(super) review: Option<super::review::ReviewView>,
    pub(super) worktrees: Vec<crate::worktree::Worktree>,
    /// The remotes and their URLs, for the sidebar's Remote section.
    pub(super) remotes: Vec<(String, String)>,
    /// Where the details panel sits in this repository.
    pub(super) details_layout: DetailsLayout,
    /// The tab's latest message for the person; `App` moves it to the window's toast.
    pub(super) notice: Option<Notice>,
    /// A merge, rebase, cherry-pick or revert stopped on conflicts: the banner shows it.
    pub(super) operation: Option<crate::operation::InProgress>,
    /// The branch's issue on the project, and the offer to move it.
    pub(super) issue: super::issue::IssueFlow,
    /// The repository's pull requests and their CI, shown as badges on the branches.
    pub(super) pulls: project::PullsState,
    /// The card of a pull request's checks, opened from its badge.
    pub(super) checks: super::checks::ChecksCard,
    /// The skin of the settings, copied here each frame: Runner shows the board's city as a
    /// stadium, the Big eater as a food court.
    pub(super) skin: crate::settings::Skin,
    /// What moved the Runner skin's runner, or fed the Big eater, since the app last looked,
    /// with the toast's message (`App::take_strides` takes them).
    pub(super) strides: Vec<(crate::course::Stride, String)>,
    /// The Ultra-trail's km, copied here each frame for the Mountain view's "You" (#207).
    pub(super) trail_km: f32,
    /// The screen x of the Mountain view's km 0 and km 160 this frame, if it is drawn: the
    /// course bar aligns its profile on them.
    pub(super) mountain_x: Option<(f32, f32)>,
}

/// A message for the person, shown as the window's toast.
#[derive(Clone, Debug)]
pub struct Notice {
    pub msg: String,
    pub error: bool,
    pub at: Instant,
    /// What it reports can be taken back with Cmd+Z: the toast says so, and stays `UNDO_SECS`.
    pub undo: bool,
    /// A button on the toast and the command it runs, such as the Undo of "Remove N missing",
    /// which is the repository list's and not Git's (Cmd+Z). The toast then stays `UNDO_SECS`.
    pub action: Option<ToastAction>,
    /// The Runner skin's coach, or the Big eater's announcer, says it instead (#206, #212): a
    /// title, the metres or kcal and its colour.
    pub coach: Option<super::course::Coach>,
}

/// A toast's button: its label, and the command it runs.
pub type ToastAction = (&'static str, fn() -> Cmd);

/// How long a toast stays, and one that offers Cmd+Z.
pub const TOAST_SECS: f32 = 4.0;
pub const UNDO_SECS: f32 = 12.0;

/// Polls every tab's background work once per frame (jobs, Claude, the GitHub project), then
/// its timers unless `timers` is off (screenshots).
pub(super) fn poll_tabs(tabs: &mut [Tab], ctx: &egui::Context, settings: crate::settings::Settings, timers: bool) {
    for t in tabs {
        t.apply_settings(settings);
        t.poll_busy();
        t.ai_poll();
        t.changelog_poll();
        t.pr_draft_poll();
        t.cockpit_poll();
        t.resolve_poll();
        t.review_poll();
        t.project_poll();
        t.pulls_poll();
        t.checks_poll(ctx);
        t.issue_poll(settings.issue_status_auto);
        if timers {
            t.timers(ctx, settings);
        }
    }
}

impl Tab {
    pub(super) fn new() -> Tab {
        static NEXT_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Tab {
            key: NEXT_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            pending: None,
            sample_name: None,
            home: false,
            repo: None,
            commits: vec![],
            rows: vec![],
            max_lanes: 1,
            refs: vec![],
            by_target: HashMap::new(),
            index: HashMap::new(),
            stashes: vec![],
            status: Status::default(),
            branch: None,
            head: String::new(),
            ahead_behind: None,
            sel: Sel::None,
            details: None,
            details_for: String::new(),
            diff: None,
            summary: String::new(),
            description: String::new(),
            amend: false,
            search: String::new(),
            filter: String::new(),
            collapsed: crate::sidebar::initial_collapsed(),
            collapsed_dirs: HashSet::new(),
            files_tree: false,
            busy: None,
            last_status: Instant::now(),
            last_fetch: Instant::now(),
            dialog: None,
            scroll_to: None,
            project: project::ProjectState::default(),
            board: board::BoardState::default(),
            undo: Vec::new(),
            pending_undo: None,
            commit_limit: COMMIT_LIMIT,
            diff_context: git::DIFF_CONTEXT,
            diff_split: false,
            diff_whole: false,
            subject_limits: (SUBJECT_LIMIT_LOW, SUBJECT_LIMIT_HIGH),
            claude_model: Default::default(),
            commit_language: Default::default(),
            diff_req: None,
            ai: assistant::AiState::default(),
            changelog: None,
            pr_draft: None,
            cockpit: None,
            resolve: None,
            review: None,
            worktrees: Vec::new(),
            remotes: Vec::new(),
            details_layout: DetailsLayout::default(),
            notice: None,
            operation: None,
            issue: Default::default(),
            pulls: Default::default(),
            checks: Default::default(),
            skin: Default::default(),
            trail_km: 0.0,
            mountain_x: None,
            strides: Vec::new(),
        }
    }

    /// A tab showing the home page.
    pub(super) fn home() -> Tab {
        Tab { home: true, ..Tab::new() }
    }

    /// A tab for `path`, read when it is first shown.
    pub(super) fn unread(path: PathBuf) -> Tab {
        Tab { pending: Some(path), ..Tab::new() }
    }

    pub(super) fn is_demo(&self) -> bool {
        self.repo.is_none()
    }

    /// The repository this tab is about, read or not; None for the sample project.
    pub(super) fn root(&self) -> Option<&std::path::Path> {
        self.repo.as_ref().map(|r| r.root.as_path()).or(self.pending.as_deref())
    }

    /// The name on the tab: the repository's folder.
    pub(super) fn name(&self) -> String {
        match (&self.repo, &self.pending) {
            (Some(r), _) => r.name(),
            (None, Some(p)) => p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string()),
            (None, None) if self.home => "Home".into(),
            (None, None) => self.sample_name.clone().unwrap_or_else(|| "sample-project".into()),
        }
    }

    pub(super) fn load_demo(&mut self) {
        let d = demo::build();
        self.repo = None;
        self.home = false;
        self.commits = d.commits;
        self.refs = d.refs;
        self.remotes = vec![("origin".into(), "https://github.com/example/sample-project.git".into()), ("upstream".into(), "git@github.com:acme/sample-project.git".into())];
        self.stashes = d.stashes;
        self.status = d.status;
        self.branch = Some("develop".into());
        self.ahead_behind = Some((3, 0));
        let wt = |folder: &str, branch: Option<&str>, missing: bool| crate::worktree::Worktree {
            path: PathBuf::from(format!("/Users/me/sample-project/.claude/worktrees/{folder}")),
            branch: branch.map(String::from),
            head: String::new(),
            is_main: false,
            missing,
            locked: false,
        };
        self.worktrees = vec![wt("graph-colours", Some("feature/graph-colours"), false), wt("search-fix", Some("feature/search"), false), wt("old-try", None, true)];
        self.project = project::ProjectState { board: Some(std::sync::Arc::new(crate::github::sample())), ..Default::default() };
        self.pulls = project::PullsState { pulls: Some(std::sync::Arc::new(crate::pulls::sample())), loading: None };
        self.after_load();
        self.sel = Sel::Commit(0);
        self.details_layout = DetailsLayout::default();
    }

    /// Shows `r` in this tab: reads it and selects the local changes, else the newest commit.
    /// `App::set_repo` adds what comes from the repository list.
    pub(super) fn set_repo(&mut self, r: Repo) {
        self.repo = Some(r);
        self.home = false;
        self.sel = Sel::None;
        self.diff = None;
        self.details = None;
        self.details_for.clear();
        self.reload();
        // A repository opened while an operation is stopped: no cramp, it was there before.
        self.strides.retain(|(s, _)| !matches!(s, crate::course::Stride::Cramp { .. }));
        self.sel = if !self.status.is_empty() {
            Sel::Wip
        } else if self.commits.is_empty() {
            Sel::None
        } else {
            Sel::Commit(0)
        };
    }

    /// Takes the settings that shape a tab: a new diff context draws the open diff again, a new
    /// commit limit reads the history again.
    fn apply_settings(&mut self, s: crate::settings::Settings) {
        self.skin = s.skin;
        self.subject_limits = (s.subject_low, s.subject_high);
        self.files_tree = s.files_tree;
        self.claude_model = s.claude_model;
        self.commit_language = s.commit_language;
        self.diff_split = s.diff_split;
        if self.diff_context != s.diff_context || self.diff_whole != s.diff_whole {
            self.diff_context = s.diff_context;
            self.diff_whole = s.diff_whole;
            if let Some(req) = self.diff_req.clone().filter(|_| self.diff.is_some()) {
                self.open_diff(req);
            }
        }
        if self.commit_limit != s.max_commits {
            self.commit_limit = s.max_commits;
            self.reload();
        }
    }

    pub(super) fn reload(&mut self) {
        let Some(r) = &self.repo else { return };
        let selected_hash = match &self.sel {
            Sel::Commit(i) => self.commits.get(*i).map(|c| c.hash.clone()),
            _ => None,
        };
        self.commits = r.commits(self.commit_limit);
        self.worktrees = r.worktrees();
        self.refs = r.refs();
        self.remotes = r.remotes();
        self.stashes = r.stashes();
        self.status = r.status();
        self.branch = r.current_branch();
        self.ahead_behind = r.ahead_behind();
        self.read_operation();
        self.after_load();
        if let Some(h) = selected_hash {
            self.sel = self.index.get(&h).map(|&i| Sel::Commit(i)).unwrap_or(Sel::None);
        }
        if self.sel == Sel::Wip && self.status.is_empty() {
            self.sel = if self.commits.is_empty() { Sel::None } else { Sel::Commit(0) };
        }
        self.details_for.clear();
    }

    pub(super) fn refresh_status(&mut self) {
        if self.pending.is_some() {
            return;
        }
        if let Some(r) = &self.repo {
            self.status = r.status();
        }
        self.read_operation();
        self.last_status = Instant::now();
    }

    /// The original refreshes the working copy when more than the update interval has passed
    /// (sub_10048580c), and fetches remotes automatically when that setting is on (here the
    /// person's choice in the Settings window, with its interval).
    fn timers(&mut self, ctx: &egui::Context, s: crate::settings::Settings) {
        // The sample project, and a tab not read yet, have nothing to refresh or fetch.
        if self.repo.is_none() || self.pending.is_some() {
            return;
        }
        if self.last_status.elapsed().as_secs_f32() > STATUS_UPDATE_SECS && self.busy.is_none() {
            self.refresh_status();
        }
        if s.auto_fetch && self.last_fetch.elapsed().as_secs_f32() > s.fetch_secs() && self.busy.is_none() {
            self.last_fetch = Instant::now();
            self.fetch(false, true, ctx);
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(5));
    }

    /// Runs `job` in the background in this repository; `poll_busy` picks up its result.
    fn spawn(&mut self, label: &str, quiet: bool, ctx: &egui::Context, job: impl FnOnce(&std::path::Path) -> JobResult + Send + 'static) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        if self.busy.is_some() {
            if !quiet {
                self.notify("Please wait for the current operation to finish.", true);
            }
            return;
        }
        let (tx, rx) = channel();
        let root = r.root.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job(&root));
            ctx.request_repaint();
        });
        self.busy = Some((label.to_string(), rx, quiet));
    }

    fn fetch(&mut self, all: bool, quiet: bool, ctx: &egui::Context) {
        let remote = if all { None } else { self.repo.as_ref().and_then(|r| r.default_remote()) };
        if !all && remote.is_none() {
            if !quiet {
                self.notify("This repository has no remote to fetch from.", true);
            }
            return;
        }
        let label = match &remote {
            Some(r) => format!("Fetch '{r}'"),
            None => "Fetch all remotes".to_string(),
        };
        self.last_fetch = Instant::now();
        self.spawn(&label, quiet, ctx, move |root| {
            let args = git::fetch_args(remote.as_deref(), false);
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            git::exec(root, &a).map(|(_, err)| git::fetch_summary(&err))
        });
    }

    fn push(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        let Some(branch) = r.current_branch() else {
            self.notify("Check out a branch before pushing.", true);
            return;
        };
        let (remote, remote_branch, set_up) = match r.upstream() {
            Some((rem, b)) => (rem, b, false),
            None => match r.default_remote() {
                Some(rem) => (rem, branch.clone(), true),
                None => {
                    self.notify("This repository has no remote to push to.", true);
                    return;
                }
            },
        };
        self.spawn(&format!("Push to '{remote}'"), false, ctx, move |root| {
            let args = git::push_args(&remote, &branch, &remote_branch, set_up, false, false);
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            git::exec(root, &a).map(|(o, e)| git::push_summary(&format!("{o}{e}"), &branch, &remote))
        });
    }

    /// Opens the Delete branch dialog on what was clicked, with the other side of the branch
    /// (its remote branch, or its local one) offered unticked.
    fn ask_delete_branch(&mut self, local: Option<String>, remote: Option<String>) {
        use crate::delete::{self, Delete, Local, Remote};
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        let find = |name: &str, kind: RefKind| self.refs.iter().find(|x| x.kind == kind && x.name == name);
        let (clicked_local, clicked_remote) = (local.is_some(), remote.is_some());
        let local_ref = local.as_deref().and_then(|n| find(n, RefKind::Local)).or_else(|| remote.as_deref().and_then(|n| find(n, RefKind::Remote)).and_then(|x| delete::local_of(x, &self.refs)));
        let remote_ref = remote.as_deref().and_then(|n| find(n, RefKind::Remote)).or_else(|| local_ref.and_then(|x| delete::remote_of(x, &self.refs)));
        // The checked-out branch can't be deleted here, only its remote branch.
        let local = local_ref.filter(|b| !b.is_head).map(|b| {
            let merged = r.git(&["branch", "--format=%(refname:short)", "--merged", "HEAD"]).unwrap_or_default().lines().any(|l| l.trim() == b.name);
            let worktree = r.worktree_of(&b.name);
            let dirty = worktree.as_ref().map(|w| crate::worktree::is_dirty(&w.path)).unwrap_or(false);
            Local { name: b.name.clone(), merged, force: false, worktree, dirty, also_worktree: true }
        });
        let remote = remote_ref.and_then(|x| {
            let mut rm = Remote::of(&x.name)?;
            rm.open_pull = self.pull_of(x).filter(|p| p.state.is_open()).map(|p| format!("#{} {}", p.number, p.title));
            Some(rm)
        });
        if local.is_none() && remote.is_none() {
            self.notify("The checked-out branch can't be deleted. Check out another branch first.", true);
            return;
        }
        self.dialog = Some(Dialog::DeleteBranch(Delete::new(local, remote, clicked_local, clicked_remote)));
    }

    /// Deletes the local branch (one Undo step, its worktree too), then the remote one in the
    /// background, which no Undo takes back. A failed local deletion leaves the remote alone.
    fn delete_branch(&mut self, d: crate::delete::Delete, ctx: &egui::Context) {
        let Some(r) = &self.repo else { return };
        if let Some(l) = d.local.as_ref().filter(|_| d.del_local) {
            let name = &l.name;
            let mut snap = r.snapshot(&format!("Delete branch {name}"), false, false);
            let (removed, res) = crate::worktree::delete_branch(r, name, l.force, l.also_worktree);
            let had_wt = removed.is_some();
            snap.worktrees.extend(removed);
            self.reload();
            if had_wt {
                self.push_undo(snap);
            } else {
                self.record(Some(snap));
            }
            match res {
                Ok(_) if had_wt => self.notify(format!("Deleted {name} and its worktree"), false),
                Ok(_) => self.notify(format!("Deleted {name}"), false),
                Err(e) => {
                    self.notify(e, true);
                    return;
                }
            }
        }
        let Some(rm) = d.remote.filter(|_| d.del_remote) else { return };
        self.spawn(&format!("Delete {}", rm.name()), false, ctx, move |root| {
            let args = rm.args();
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let (res, gone) = crate::delete::pushed(&rm, git::exec(root, &a).map(|(o, e)| format!("{o}{e}")));
            if gone {
                // Already deleted on the remote: drop the stale remote-tracking branch here.
                let _ = git::exec(root, &["branch", "-dr", &rm.name()]);
            }
            res
        });
    }

    /// Opens the Push dialog on the current branch (the sample project's too, for a look).
    pub(super) fn ask_push(&mut self) {
        let Some(branch) = self.branch.clone() else {
            self.notify("Check out a branch before pushing.", true);
            return;
        };
        let out = self.repo.as_ref().map(|r| r.git(&["remote"]).unwrap_or_default());
        let remotes = crate::push::Push::remotes_of(out.as_deref(), &self.refs);
        let default = self.repo.as_ref().and_then(|r| r.default_remote()).or_else(|| remotes.iter().find(|r| *r == "origin").cloned());
        match crate::push::Push::new(&branch, remotes, &self.refs, default) {
            Some(p) => self.dialog = Some(Dialog::Push(p)),
            None => self.notify("This repository has no remote to push to.", true),
        }
    }

    /// The Push dialog's push. Not undoable, like the toolbar's.
    fn push_to(&mut self, p: crate::push::Push, ctx: &egui::Context) {
        self.spawn(&p.label(), false, ctx, move |root| {
            let args = p.args();
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            git::exec(root, &a).map(|(o, e)| p.done(&format!("{o}{e}"))).map_err(|e| crate::push::refused(&e, &p.remote_ref()))
        });
    }

    /// Pulls the upstream branch, merging or rebasing and putting uncommitted changes aside as
    /// the Settings window says ("pullSheetRebase", "pullSheetStashAndReapply", both off by
    /// default, sub_10031b7cc). The snapshot keeps the uncommitted changes too, so Undo can take
    /// the pull back even once they were moved onto the pulled commits.
    pub(super) fn pull(&mut self, ctx: &egui::Context, s: crate::settings::Settings) {
        let Some(up) = self.repo.as_ref().and_then(|r| r.upstream()) else {
            self.notify("The current branch has no upstream branch to pull from.", true);
            return;
        };
        let args = git::pull_args(&up.0, &up.1, s.pull_rebase, s.pull_autostash);
        self.pull_with(ctx, crate::pull::Source { remote: up.0, branch: up.1 }, args);
    }

    /// Runs `git <args>`, a pull from `source`, in the background. The snapshot keeps the
    /// uncommitted changes too, so one Undo takes the whole pull back (the fetch stays).
    pub(super) fn pull_with(&mut self, ctx: &egui::Context, source: crate::pull::Source, args: Vec<String>) {
        if self.busy.is_none() {
            self.pending_undo = self.repo.as_ref().map(|r| Snapshot { dirty: r.stash_create(), ..r.snapshot("Pull", false, false) });
        }
        self.spawn(&format!("Pull from '{}'", source.remote), false, ctx, move |root| {
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let res = git::exec(root, &a).map(|(o, e)| format!("{o}{e}"));
            let repo = Repo { root: root.to_path_buf() };
            git::pull_outcome(res, repo.operation().as_deref(), &repo.conflicted_files(), &source.remote, &source.branch)
        });
    }

    /// Repository › Pull…: the Pull dialog on the current branch, its upstream chosen, Merge or
    /// Rebase and the stash as Settings › Git says, with fresh local changes and counts.
    pub(super) fn pull_dialog(&mut self, s: crate::settings::Settings) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        let Some(branch) = r.current_branch() else {
            self.notify("Check out a branch to pull into first.", true);
            return;
        };
        let sources = crate::pull::sources(&self.refs, &crate::pull::remotes(r));
        let Some(mut pull) = crate::pull::Pull::new(&branch, r.upstream(), sources, r.default_remote().as_deref()) else {
            self.notify("This repository has no remote branch to pull from.", true);
            return;
        };
        pull.counts = crate::pull::count(r, &pull.source().name());
        self.refresh_status();
        let mode = crate::pull::Mode::from_setting(s.pull_rebase);
        self.dialog = Some(Dialog::Pull { pull, mode, stash: s.pull_autostash, dirty: !self.status.is_empty() });
    }

    /// FOURCHETTE_DIALOG=pull: the Pull dialog on the sample project's `develop`, 2 commits ahead
    /// of origin/develop and 3 behind.
    pub(super) fn pull_sample(&mut self, s: crate::settings::Settings) {
        let branch = self.branch.clone().unwrap_or_else(|| "develop".into());
        let up = Some(("origin".to_string(), branch.clone()));
        if let Some(mut pull) = crate::pull::Pull::new(&branch, up, crate::pull::sources(&self.refs, &[]), Some("origin")) {
            pull.counts = Some((2, 3));
            let mode = crate::pull::Mode::from_setting(s.pull_rebase);
            self.dialog = Some(Dialog::Pull { pull, mode, stash: true, dirty: !self.status.is_empty() });
        }
    }

    fn after_load(&mut self) {
        let (rows, w) = graph::layout(&self.commits);
        self.rows = rows;
        self.max_lanes = w;
        self.by_target = git::refs_by_target(&self.refs);
        self.index = self.commits.iter().enumerate().map(|(i, c)| (c.hash.clone(), i)).collect();
        self.head = self.refs.iter().find(|r| r.is_head).map(|r| r.target.clone()).unwrap_or_default();
    }

    pub(super) fn notify(&mut self, msg: impl Into<String>, error: bool) {
        self.notice = Some(Notice { msg: msg.into(), error, at: Instant::now(), undo: false, action: None, coach: None });
    }

    pub(super) fn ensure_details(&mut self) {
        let Sel::Commit(i) = self.sel else { return };
        let Some(c) = self.commits.get(i) else { return };
        if self.details_for == c.hash {
            return;
        }
        self.details_for = c.hash.clone();
        self.details = match &self.repo {
            Some(r) => r.details(&c.hash),
            None => Some(demo::details(c)),
        };
    }

    // ---------------------------------------------------------------- actions

    /// Runs a command about this repository. `App::run` keeps the app-wide ones.
    pub(super) fn run(&mut self, c: Cmd, ctx: &egui::Context) {
        match c {
            Cmd::Select(s) => {
                self.sel = s;
                self.diff = None;
                self.board.open = false;
            }
            Cmd::SelectHash(h) => {
                if let Some(&i) = self.index.get(&h) {
                    self.sel = Sel::Commit(i);
                    self.scroll_to = Some(i);
                    self.diff = None;
                }
                self.board.open = false;
            }
            Cmd::Diff(req) => self.open_diff(req),
            Cmd::DiffAt(req, line) => {
                self.open_diff(req);
                if let Some(d) = &mut self.diff {
                    d.goto = line;
                }
            }
            Cmd::CloseDiff => self.diff = None,
            Cmd::Checks(pull, anchor) => self.checks_open(pull, anchor),
            Cmd::Dialog(d) => self.dialog = Some(d),
            Cmd::Reload => {
                self.reload();
                self.notify("Refreshed", false);
            }
            Cmd::Commit => {
                let label = if self.amend { "Amend commit" } else { "Commit" };
                let snap = self.repo.as_ref().map(|r| r.snapshot(label, true, false));
                self.do_commit();
                self.record(snap);
            }
            Cmd::Git { label, args, network } => self.git(label, args, network, ctx),
            Cmd::Checkout { name, remote } => {
                if let Some(c) = self.checkout(&name, remote) {
                    self.run(c, ctx);
                }
            }
            Cmd::CheckoutSync { sync, action, merge, stash } => self.checkout_sync(&sync, action, merge, stash),
            Cmd::Operation(a) => self.operation_run(a),
            Cmd::AskMerge { name, remote } => self.ask_merge(&name, remote),
            Cmd::Merge { merge, way } => self.merge(&merge, way),
            Cmd::PullFrom { source, args } => self.pull_with(ctx, source, args),
            Cmd::Fetch { all, quiet } => self.fetch(all, quiet, ctx),
            Cmd::Push => self.push(ctx),
            Cmd::AskPush => self.ask_push(),
            Cmd::PushTo(p) => self.push_to(p, ctx),
            Cmd::Discard { path, untracked } => {
                let Some(r) = &self.repo else { return };
                let mut snap = r.snapshot(&format!("Discard {path}"), false, !untracked);
                let res = if untracked {
                    git::trash(&r.root, &path).map(|dst| snap.trashed = Some((path.clone(), dst)))
                } else {
                    r.git(&["checkout", "--", &path]).map(|_| ())
                };
                self.reload();
                match res {
                    Ok(_) => {
                        self.push_undo(snap);
                        self.notify(if untracked { format!("Moved {path} to the Trash") } else { format!("Discarded changes to {path}") }, false)
                    }
                    Err(e) => self.notify(e, true),
                }
            }
            Cmd::Undo => self.do_undo(),
            Cmd::AiCleanup => self.ai_cleanup(ctx),
            Cmd::AiCommitMessage => self.ai_commit_message(ctx),
            Cmd::AiExplain(h) => self.ai_explain(h, ctx),
            Cmd::AiDeleteSelected => self.ai_delete_selected(),
            Cmd::AiChangelog => self.changelog_open(ctx),
            Cmd::AiDraftPr => self.pr_draft_open(ctx),
            Cmd::Cockpit => self.cockpit_open(ctx),
            Cmd::AiResolve(paths) => self.resolve_open(paths, ctx),
            Cmd::AiReview => self.review_open(ctx),
            Cmd::AskDeleteBranch { local, remote } => self.ask_delete_branch(local, remote),
            Cmd::DeleteBranch(d) => self.delete_branch(d, ctx),
            Cmd::AskDeleteWorktree(path) => {
                if let Some(wt) = self.worktrees.iter().find(|w| w.path == path).cloned() {
                    let dirty = crate::worktree::is_dirty(&wt.path);
                    self.dialog = Some(Dialog::DeleteWorktree { wt, dirty });
                }
            }
            Cmd::DeleteWorktree(path) => {
                let Some(r) = &self.repo else { return };
                let Some(wt) = self.worktrees.iter().find(|w| w.path == path).cloned() else { return };
                let mut snap = r.snapshot(&format!("Delete worktree {}", wt.name()), false, false);
                match crate::worktree::remove_for_undo(r, &wt) {
                    Ok(removed) => {
                        snap.worktrees.push(removed);
                        self.reload();
                        self.push_undo(snap);
                        self.notify(format!("Deleted worktree “{}” — Undo (⌘Z) brings it back", wt.name()), false);
                    }
                    Err(e) => self.notify(e, true),
                }
            }
            Cmd::PruneWorktrees => {
                let Some(r) = &self.repo else { return };
                let res = r.git(&["worktree", "prune"]);
                self.reload();
                match res {
                    Ok(_) => self.notify("Cleaned up worktrees whose folder is gone", false),
                    Err(e) => self.notify(e, true),
                }
            }
            Cmd::ToggleDir(staged, path) => {
                if !self.collapsed_dirs.remove(&(staged, path.clone())) {
                    self.collapsed_dirs.insert((staged, path));
                }
            }
            // App-wide: `App::run` handles them and never passes them here.
            Cmd::OpenDialog
            | Cmd::Open(_)
            | Cmd::Copy(_)
            | Cmd::Palette
            | Cmd::PaletteCheckout
            | Cmd::SetAppearance(_)
            | Cmd::SetSkin(_)
            | Cmd::Finisher { .. }
            | Cmd::GoldenFork { .. }
            | Cmd::SetDetails(_)
            | Cmd::ToggleDetails
            | Cmd::Settings
            | Cmd::SetSettings(_)
            | Cmd::PreviewIcon(_)
            | Cmd::SetFilesTree(_)
            | Cmd::SetDiffView { .. }
            | Cmd::Pull
            | Cmd::PullDialog
            | Cmd::ApplyStash(_)
            | Cmd::SetCommitLimit(_)
            | Cmd::Switcher
            | Cmd::RepoFavorite(_)
            | Cmd::RepoGroup(..)
            | Cmd::RepoForget(_)
            | Cmd::RepoRemoveMissing(_)
            | Cmd::RepoPutBack
            | Cmd::ScanFolder
            | Cmd::FetchAllRepos
            | Cmd::Reveal(_)
            | Cmd::OpenTerminal(_)
            | Cmd::OpenUrl(_)
            | Cmd::ProjectReload
            | Cmd::ProjectPick(_)
            | Cmd::Board
            | Cmd::BoardFull
            | Cmd::IssueDetail(_)
            | Cmd::NewTab
            | Cmd::OpenTab(_)
            | Cmd::SelectTab(_)
            | Cmd::CloseTab(_)
            | Cmd::ShowTab { .. }
            | Cmd::Home
            | Cmd::CloneDialog
            | Cmd::ClonePickFolder
            | Cmd::CloneOpen(_)
            | Cmd::HomeKeep { .. }
            | Cmd::HomeDismiss(_)
            | Cmd::HomeSample
            | Cmd::UpdateDialog(_)
            | Cmd::InstallUpdate
            | Cmd::UpdateCheckNow
            | Cmd::CloseWhatsNew => {}
            Cmd::ProjectMove(m) => self.project_move(m),
            Cmd::IssueMove => self.issue_accept(),
            Cmd::IssueDecline => self.issue_decline(),
            Cmd::IssueUndo => self.issue_undo(),
        }
    }

    fn git(&mut self, label: String, args: Vec<String>, network: bool, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        if network {
            let done = format!("{label} finished");
            self.spawn(&label, false, ctx, move |root| {
                let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                git::run_in(root, &a).map(|_| done)
            });
        } else {
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let snap = r.snapshot(&label, false, false);
            let res = r.git(&a);
            self.reload();
            self.record(Some(snap));
            match res {
                Ok(_) => self.notify(label, false),
                Err(e) => self.notify(e, true),
            }
        }
    }

    /// Checks out `name` (`remote`: a remote branch). Returns the plain checkout to run, or opens
    /// the Check out dialog, with fresh local changes, when the branch is behind its upstream.
    pub(super) fn checkout(&mut self, name: &str, remote: bool) -> Option<Cmd> {
        match crate::checkout::plan(name, remote, &self.refs, &self.worktrees) {
            crate::checkout::Plan::Now { label, args } => Some(Cmd::Git { label, args, network: false }),
            // Nothing to choose: check out and fast-forward, and the toast offers Cmd+Z.
            crate::checkout::Plan::FastForward(sync) => {
                self.checkout_sync(&sync, crate::checkout::SyncAction::FastForward, Default::default(), false);
                None
            }
            crate::checkout::Plan::Ask(sync) => {
                self.refresh_status();
                let action = sync.default_action();
                self.dialog = Some(Dialog::CheckoutSync { sync, action, merge: Default::default(), stash: false, dirty: !self.status.is_empty() });
                None
            }
        }
    }

    /// The Check out dialog's choice: the whole sequence under one snapshot, which also keeps the
    /// uncommitted changes, so one Undo takes back the branch, HEAD and the changes.
    fn checkout_sync(&mut self, sync: &crate::checkout::Sync, action: crate::checkout::SyncAction, merge: crate::checkout::MergeStyle, stash: bool) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        if self.busy.is_some() {
            self.notify("Please wait for the current operation to finish.", true);
            return;
        }
        let snap = Snapshot { dirty: r.stash_create(), ..r.snapshot(&sync.label(action, merge), false, false) };
        let res = crate::checkout::run(r, sync, action, merge, stash);
        self.reload();
        let steps = self.undo.len();
        self.record(Some(snap));
        match res {
            Ok(msg) => self.notify(msg, false),
            Err(e) => self.notify(e, true),
        }
        // Several Git steps ran as one: the toast offers to take them all back while it shows.
        if let Some(n) = &mut self.notice {
            n.undo = self.undo.len() > steps;
        }
    }

    /// Opens the Merge dialog for `name`, and starts the conflict check in a thread (none for a
    /// fast-forward, which cannot conflict).
    fn ask_merge(&mut self, name: &str, remote: bool) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        match crate::merge::Merge::read(r, name, remote) {
            Ok(merge) => {
                use crate::merge::{Check, Test};
                let check = if merge.up_to_date() || merge.can_fast_forward() { Check::done(Test::Clean) } else { Check::start(r.root.clone(), name.to_string()) };
                self.dialog = Some(Dialog::Merge { merge, style: Default::default(), no_ff: false, check });
            }
            Err(e) => self.notify(e, true),
        }
    }

    /// The Merge dialog's choice under one snapshot. A merge stopped on conflicts is kept too
    /// (Undo aborts it); a squash stopped on conflicts as well, though `record` cannot see it.
    fn merge(&mut self, merge: &crate::merge::Merge, way: crate::merge::Way) {
        let Some(r) = &self.repo else { return };
        if self.busy.is_some() {
            self.notify("Please wait for the current operation to finish.", true);
            return;
        }
        let snap = r.snapshot(&merge.title(way), false, false);
        let res = crate::merge::run(r, merge, way);
        let squash_stopped = way == crate::merge::Way::Squash && r.operation().is_none() && !r.conflicted_files().is_empty();
        let steps = self.undo.len();
        if squash_stopped {
            let mut snap = snap;
            r.after(&mut snap);
            snap.left = Some(crate::undo::SQUASH.into());
            self.push_undo(snap);
        } else {
            self.record(Some(snap));
        }
        self.reload();
        match res {
            Ok(msg) => {
                // Merged for good, not stopped on conflicts.
                if self.operation.is_none() && !squash_stopped {
                    self.strides.push((crate::course::Stride::Merge, msg.clone()));
                }
                self.notify(msg, false)
            }
            Err(e) => self.notify(e, true),
        }
        if let Some(n) = &mut self.notice {
            n.undo = self.undo.len() > steps;
        }
    }

    // ---------------------------------------------------------------- undo

    /// Keep the snapshot only if the action really changed something.
    pub(super) fn record(&mut self, snap: Option<Snapshot>) {
        let (Some(mut s), Some(r)) = (snap, &self.repo) else { return };
        r.after(&mut s);
        if r.state() != s.state {
            self.push_undo(s);
        }
    }

    pub(super) fn push_undo(&mut self, s: Snapshot) {
        self.undo.push(s);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    fn do_undo(&mut self) {
        if self.busy.is_some() {
            self.notify("Please wait for the current operation to finish.", true);
            return;
        }
        let (Some(r), Some(s)) = (&self.repo, self.undo.last().cloned()) else {
            self.notify("Nothing to undo.", true);
            return;
        };
        let res = r.undo(&s);
        self.reload();
        match res {
            Ok(msg) => {
                self.undo.pop();
                self.strides.push((crate::course::Stride::Undo, msg.clone()));
                self.notify(msg, false);
            }
            Err(e) => self.notify(format!("Couldn't undo “{}”: {e}", s.label), true),
        }
    }

    fn poll_busy(&mut self) {
        let done = match &self.busy {
            Some((_, rx, quiet)) => rx.try_recv().ok().map(|r| (r, *quiet)),
            None => None,
        };
        if let Some((res, quiet)) = done {
            // A fetch or a pull may bring news of the pull requests too.
            let fetched = self.busy.as_ref().is_some_and(|(label, _, _)| label.starts_with("Fetch") || label.starts_with("Pull") || label.starts_with("Delete "));
            let pushed = self.busy.as_ref().is_some_and(|(label, _, _)| label.starts_with("Push") || label.starts_with("Force push"));
            // A push that sent something moves the Runner skin's runner on.
            if let (true, Ok(msg)) = (pushed, &res) {
                if !msg.contains("up-to-date") {
                    self.strides.push((crate::course::Stride::Push, msg.clone()));
                }
            }
            self.busy = None;
            if fetched && res.is_ok() {
                self.pulls_load();
            }
            self.reload();
            let snap = self.pending_undo.take();
            self.record(snap);
            self.last_status = Instant::now();
            match res {
                Ok(msg) if !quiet => self.notify(msg, false),
                Ok(_) => {}
                Err(e) if !quiet => self.notify(e, true),
                Err(_) => {}
            }
        }
    }

    fn do_commit(&mut self) {
        if self.summary.trim().is_empty() {
            self.notify("Write a commit subject first.", true);
            return;
        }
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to commit.", true);
            return;
        };
        let mut message = self.summary.trim().to_string();
        if !self.description.trim().is_empty() {
            message.push_str("\n\n");
            message.push_str(self.description.trim());
        }
        message.push('\n');
        let files = self.status.staged.len();
        match r.commit(&message, self.amend, files) {
            Ok(done) => {
                self.summary.clear();
                self.description.clear();
                self.amend = false;
                self.reload();
                self.sel = if self.status.is_empty() { Sel::Commit(0) } else { Sel::Wip };
                self.strides.push((crate::course::Stride::Commit, done.clone()));
                self.notify(done, false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    /// Opens the sample project's diff in the given mode (the FOURCHETTE_DIFF screenshot view).
    pub(super) fn show_demo_diff(&mut self, split: bool, whole: bool) {
        self.diff_split = split;
        self.diff_whole = whole;
        self.open_diff(DiffReq::Commit { hash: String::new(), path: "src/lanes.rs".into() });
    }

    pub(super) fn open_diff(&mut self, req: DiffReq) {
        let ctx = if self.diff_whole { git::WHOLE_CONTEXT } else { self.diff_context };
        let (title, text) = match (&self.repo, &req) {
            (None, DiffReq::Commit { path, .. }) | (None, DiffReq::Worktree { path, .. }) => (path.clone(), demo::diff(path, self.diff_whole)),
            (Some(r), DiffReq::Commit { hash, path }) => (path.clone(), r.diff_commit_file(hash, path, ctx)),
            (Some(r), DiffReq::Worktree { path, staged, untracked }) => (path.clone(), r.diff_worktree_file(path, *staged, *untracked, ctx)),
        };
        self.diff = Some(DiffView { title, text, goto: None, scrolled: Default::default() });
        self.diff_req = Some(req);
    }

    pub(super) fn head_has_upstream(&self) -> bool {
        self.refs.iter().any(|r| r.is_head && r.kind == RefKind::Local && !r.upstream.is_empty())
    }

    pub(super) fn ref_color(&self, target: &str) -> Color32 {
        self.index.get(target).and_then(|&i| self.rows.get(i)).map(|r| theme::lane_color(r.color)).unwrap_or(theme::c().faint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::Sender;

    fn tabs(n: usize) -> Vec<Tab> {
        (0..n).map(|_| Tab::new()).collect()
    }

    fn poll(tabs: &mut [Tab]) {
        poll_tabs(tabs, &egui::Context::default(), crate::settings::Settings::default(), false);
    }

    /// A job that has not answered yet; keep the sender alive to keep it running.
    fn job(t: &mut Tab, label: &str, quiet: bool) -> Sender<JobResult> {
        let (tx, rx) = channel();
        t.busy = Some((label.into(), rx, quiet));
        tx
    }

    fn snapshot(label: &str) -> Snapshot {
        let state = crate::undo::State { head_branch: None, head_sha: String::new(), refs: vec![], stashes: vec![], operation: None };
        Snapshot { label: label.into(), state, soft: false, worktree: None, trashed: None, written: None, resolved: vec![], worktrees: vec![], dirty: None, dirty_after: None, left: None, paused: None }
    }

    fn notice(t: &Tab) -> Option<(&str, bool)> {
        t.notice.as_ref().map(|n| (n.msg.as_str(), n.error))
    }

    #[test]
    fn a_branch_only_behind_is_fast_forwarded_at_once_and_offers_cmd_z() {
        use crate::checkout::{plan, MergeStyle, Plan, SyncAction};
        let (base, repo) = crate::checkout::repo_tests::fixture("toast");
        let Plan::FastForward(s) = plan("behind", false, &repo.refs(), &repo.worktrees()) else { panic!("behind is only behind") };
        let mut t = Tab::new();
        t.repo = Some(repo);
        t.reload();
        // Only behind: no dialog, the checkout and the fast-forward happen at once.
        assert!(t.checkout("behind", false).is_none());
        assert!(t.dialog.is_none());
        let n = t.notice.clone().unwrap();
        assert!(!n.error && n.undo, "{n:?}");
        assert_eq!(t.undo_label(), Some(s.label(SyncAction::FastForward, MergeStyle::Commit).as_str()));
        // An ordinary message does not offer it.
        t.notify("Fetched", false);
        assert!(!t.notice.as_ref().unwrap().undo);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_job_result_lands_in_the_tab_that_started_it() {
        let mut t = tabs(3);
        let _running = job(&mut t[0], "Fetch 'origin'", false);
        let done = job(&mut t[1], "Push to 'origin'", false);
        done.send(Ok("Pushed main to origin".into())).unwrap();
        poll(&mut t);
        assert!(t[0].busy.is_some(), "the other job is still running");
        assert_eq!(notice(&t[0]), None);
        assert!(t[1].busy.is_none());
        assert_eq!(notice(&t[1]), Some(("Pushed main to origin", false)));
        assert_eq!(notice(&t[2]), None);
    }

    #[test]
    fn a_quiet_job_ends_without_a_message_and_an_error_says_so() {
        let mut t = tabs(2);
        job(&mut t[0], "Fetch 'origin'", true).send(Ok("Fetched".into())).unwrap();
        job(&mut t[1], "Pull from 'origin'", false).send(Err("No network".into())).unwrap();
        poll(&mut t);
        assert!(t[0].busy.is_none() && t[0].notice.is_none());
        assert_eq!(notice(&t[1]), Some(("No network", true)));
    }

    #[test]
    fn claude_answers_the_tab_that_asked() {
        let mut t = tabs(2);
        let (tx, rx) = channel();
        t[1].ai.job = Some((assistant::AiJob::CommitMessage, rx));
        tx.send(Ok(serde_json::Value::String("Fix the login\n\nIt timed out.".into()))).unwrap();
        poll(&mut t);
        assert!(!t[1].ai.busy());
        assert_eq!((t[1].summary.as_str(), t[1].description.as_str()), ("Fix the login", "It timed out."));
        assert!(t[0].summary.is_empty() && t[0].notice.is_none());
    }

    #[test]
    fn the_github_project_goes_to_its_tab() {
        let mut t = tabs(2);
        let (tx, rx) = channel();
        t[0].project.loading = Some(rx);
        tx.send(Ok(crate::github::sample())).unwrap();
        poll(&mut t);
        assert!(t[0].project.loading.is_none() && t[0].project.board.is_some());
        assert!(t[1].project.board.is_none());
    }

    #[test]
    fn a_card_moves_at_once_and_comes_back_when_github_refuses() {
        use crate::github::Move;
        let mut t = tabs(1);
        t[0].project.board = Some(std::sync::Arc::new(crate::github::sample()));
        let status = |t: &Tab| t.project.board.as_ref().unwrap().items.iter().find(|i| i.number == Some(44)).unwrap().status.clone();
        // The sample project has no ids: the move stays on screen and nothing is written.
        t[0].project_move(Move { item: "item-44".into(), status: Some("Ready".into()), place: true, after: None });
        assert_eq!(status(&t[0]), "Ready");
        assert!(t[0].project.writes.is_none());
        // A refused write puts the card back, with the reason.
        let undo = std::sync::Arc::make_mut(t[0].project.board.as_mut().unwrap()).apply(&Move { item: "item-44".into(), status: Some("Done".into()), place: false, after: None }).unwrap();
        let (jobs, _) = channel();
        let (tx, rx) = channel();
        t[0].project.writes = Some((jobs, rx));
        tx.send((undo, Err("GitHub needs your OK to change projects.".into()))).unwrap();
        assert_eq!(status(&t[0]), "Done");
        poll(&mut t);
        assert_eq!(status(&t[0]), "Ready");
        assert_eq!(notice(&t[0]), Some(("Couldn't move #44, so it went back: GitHub needs your OK to change projects.", true)));
    }

    #[test]
    fn undo_steps_stay_with_their_tab() {
        let mut t = tabs(2);
        t[0].push_undo(snapshot("Commit"));
        assert_eq!(t[0].undo_label(), Some("Commit"));
        assert_eq!(t[1].undo_label(), None);
        // ⌘Z in the other tab finds nothing to undo and leaves the first tab's step alone.
        t[1].run(Cmd::Undo, &egui::Context::default());
        assert_eq!(notice(&t[1]), Some(("Nothing to undo.", true)));
        assert_eq!(t[0].undo.len(), 1);
        for i in 0..UNDO_DEPTH + 5 {
            t[1].push_undo(snapshot(&format!("Step {i}")));
        }
        assert_eq!(t[1].undo.len(), UNDO_DEPTH);
        assert_eq!(t[0].undo.len(), 1);
    }

    #[test]
    fn the_sample_project_is_a_tab_of_its_own() {
        let mut t = tabs(2);
        t[0].load_demo();
        assert!(t[0].is_demo() && !t[0].commits.is_empty() && t[0].project.board.is_some());
        assert!(t[1].commits.is_empty() && t[1].project.board.is_none());
    }

    #[test]
    fn stash_apply_deletes_it_only_when_asked() {
        let Cmd::Git { label, args, network } = Cmd::apply_stash("stash@{2}", false) else { panic!() };
        assert_eq!((label.as_str(), args, network), ("Stash applied", vec!["stash".to_string(), "apply".into(), "stash@{2}".into()], false));
        let Cmd::Git { label, args, network } = Cmd::apply_stash("stash@{2}", true) else { panic!() };
        // `pop` keeps the stash when applying it conflicts.
        assert_eq!((label.as_str(), args, network), ("Stash applied and deleted", vec!["stash".to_string(), "pop".into(), "stash@{2}".into()], false));
    }

    /// A repository with a 20-line file whose line 10 is changed and not committed.
    fn changed_repo(name: &str) -> (std::path::PathBuf, Repo) {
        let d = std::env::temp_dir().join(format!("fourchette-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let g = |args: &[&str]| assert!(std::process::Command::new("git").args(args).current_dir(&d).output().unwrap().status.success(), "git {args:?}");
        g(&["init", "-q", "-b", "main"]);
        let lines = |x: &str| (1..=20).map(|n| if n == 10 { x.to_string() } else { format!("line {n}") }).collect::<Vec<_>>().join("\n") + "\n";
        std::fs::write(d.join("f.txt"), lines("old")).unwrap();
        g(&["add", "."]);
        g(&["-c", "user.name=T", "-c", "user.email=t@t", "commit", "-q", "-m", "first"]);
        std::fs::write(d.join("f.txt"), lines("new")).unwrap();
        let r = Repo::open(&crate::git::find_root(&d).unwrap()).unwrap();
        (d, r)
    }

    #[test]
    fn a_new_diff_context_draws_the_open_diff_again() {
        let (d, r) = changed_repo("context");
        let mut t = Tab::new();
        t.set_repo(r);
        let hunk_lines = |t: &Tab| t.diff.as_ref().unwrap().text.lines().filter(|l| l.starts_with(' ')).count();
        t.open_diff(DiffReq::Worktree { path: "f.txt".into(), staged: false, untracked: false });
        assert_eq!(hunk_lines(&t), 6, "3 lines of context on each side by default");
        let s = crate::settings::Settings { diff_context: 1, ..Default::default() };
        t.apply_settings(s);
        assert_eq!(hunk_lines(&t), 2);
        // A tab with no diff open has nothing to draw.
        t.diff = None;
        t.apply_settings(crate::settings::Settings { diff_context: 10, ..Default::default() });
        assert!(t.diff.is_none() && t.diff_context == 10);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn settings_reach_every_tab_the_limit_included() {
        let mut t = tabs(2);
        use crate::settings::{ClaudeModel, CommitLanguage};
        let s = crate::settings::Settings { max_commits: 5_000, subject_low: 40, subject_high: 60, claude_model: ClaudeModel::Opus, commit_language: CommitLanguage::French, ..Default::default() };
        poll_tabs(&mut t, &egui::Context::default(), s, false);
        for tab in &t {
            assert_eq!((tab.commit_limit, tab.subject_limits), (5_000, (40, 60)));
            assert_eq!((tab.claude_model, tab.commit_language), (ClaudeModel::Opus, CommitLanguage::French));
        }
    }
}
