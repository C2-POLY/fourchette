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
    pub(super) subject_limits: (usize, usize),
    /// The Claude model and the commit-message language of the settings, copied the same way.
    pub(super) claude_model: crate::settings::ClaudeModel,
    pub(super) commit_language: crate::settings::CommitLanguage,
    /// What the open diff shows, to draw it again when the context changes.
    pub(super) diff_req: Option<DiffReq>,
    pub(super) ai: assistant::AiState,
    pub(super) changelog: Option<super::changelog::ChangelogView>,
    pub(super) worktrees: Vec<crate::worktree::Worktree>,
    /// Where the details panel sits in this repository.
    pub(super) details_layout: DetailsLayout,
    /// The tab's latest message for the person; `App` moves it to the window's toast.
    pub(super) notice: Option<(String, bool, Instant)>,
}

/// Polls every tab's background work once per frame (jobs, Claude, the GitHub project), then
/// its timers unless `timers` is off (screenshots).
pub(super) fn poll_tabs(tabs: &mut [Tab], ctx: &egui::Context, settings: crate::settings::Settings, timers: bool) {
    for t in tabs {
        t.apply_settings(settings);
        t.poll_busy();
        t.ai_poll();
        t.changelog_poll();
        t.project_poll();
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
            collapsed: HashSet::new(),
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
            subject_limits: (SUBJECT_LIMIT_LOW, SUBJECT_LIMIT_HIGH),
            claude_model: Default::default(),
            commit_language: Default::default(),
            diff_req: None,
            ai: assistant::AiState::default(),
            changelog: None,
            worktrees: Vec::new(),
            details_layout: DetailsLayout::default(),
            notice: None,
        }
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
            (None, None) => self.sample_name.clone().unwrap_or_else(|| "sample-project".into()),
        }
    }

    pub(super) fn load_demo(&mut self) {
        let d = demo::build();
        self.repo = None;
        self.commits = d.commits;
        self.refs = d.refs;
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
        self.after_load();
        self.sel = Sel::Commit(0);
        self.details_layout = DetailsLayout::default();
    }

    /// Shows `r` in this tab: reads it and selects the local changes, else the newest commit.
    /// `App::set_repo` adds what comes from the repository list.
    pub(super) fn set_repo(&mut self, r: Repo) {
        self.repo = Some(r);
        self.sel = Sel::None;
        self.diff = None;
        self.details = None;
        self.details_for.clear();
        self.reload();
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
        self.subject_limits = (s.subject_low, s.subject_high);
        self.claude_model = s.claude_model;
        self.commit_language = s.commit_language;
        if self.diff_context != s.diff_context {
            self.diff_context = s.diff_context;
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
        self.stashes = r.stashes();
        self.status = r.status();
        self.branch = r.current_branch();
        self.ahead_behind = r.ahead_behind();
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
            let args = git::push_args(&remote, &branch, &remote_branch, set_up, false);
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            git::exec(root, &a).map(|(o, e)| git::push_summary(&format!("{o}{e}"), &branch, &remote))
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
        if self.busy.is_none() {
            self.pending_undo = self.repo.as_ref().map(|r| Snapshot { dirty: r.stash_create(), ..r.snapshot("Pull", false, false) });
        }
        let args = git::pull_args(&up.0, &up.1, s.pull_rebase, s.pull_autostash);
        self.spawn(&format!("Pull from '{}'", up.0), false, ctx, move |root| {
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let res = git::exec(root, &a).map(|(o, e)| format!("{o}{e}"));
            let repo = Repo { root: root.to_path_buf() };
            git::pull_outcome(res, repo.operation().as_deref(), &repo.conflicted_files(), &up.0, &up.1)
        });
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
        self.notice = Some((msg.into(), error, Instant::now()));
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
            Cmd::CloseDiff => self.diff = None,
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
            Cmd::CheckoutSync { sync, action, stash } => self.checkout_sync(&sync, action, stash),
            Cmd::Fetch { all, quiet } => self.fetch(all, quiet, ctx),
            Cmd::Push => self.push(ctx),
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
            Cmd::AskDeleteBranch(name) => {
                let Some(r) = &self.repo else {
                    self.notify("This is the sample project — open one of your repositories to do that.", true);
                    return;
                };
                let merged = r.git(&["branch", "--format=%(refname:short)", "--merged", "HEAD"]).unwrap_or_default().lines().any(|l| l.trim() == name);
                let worktree = r.worktree_of(&name);
                let dirty = worktree.as_ref().map(|w| crate::worktree::is_dirty(&w.path)).unwrap_or(false);
                self.dialog = Some(Dialog::DeleteBranch { name, merged, force: false, worktree, dirty, also_worktree: true });
            }
            Cmd::DeleteBranch { name, force, also_worktree } => {
                let Some(r) = &self.repo else { return };
                let mut snap = r.snapshot(&format!("Delete branch {name}"), false, false);
                let (removed, res) = crate::worktree::delete_branch(r, &name, force, also_worktree);
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
                    Err(e) => self.notify(e, true),
                }
            }
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
            // App-wide: `App::run` handles them and never passes them here.
            Cmd::OpenDialog
            | Cmd::Open(_)
            | Cmd::Copy(_)
            | Cmd::Palette
            | Cmd::PaletteCheckout
            | Cmd::SetAppearance(_)
            | Cmd::SetDetails(_)
            | Cmd::ToggleDetails
            | Cmd::SetVariant(_)
            | Cmd::Settings
            | Cmd::SetSettings(_)
            | Cmd::Pull
            | Cmd::ApplyStash(_)
            | Cmd::SetCommitLimit(_)
            | Cmd::Switcher
            | Cmd::RepoFavorite(_)
            | Cmd::RepoGroup(..)
            | Cmd::RepoForget(_)
            | Cmd::ScanFolder
            | Cmd::Reveal(_)
            | Cmd::OpenTerminal(_)
            | Cmd::OpenUrl(_)
            | Cmd::ProjectReload
            | Cmd::ProjectPick(_)
            | Cmd::Board
            | Cmd::NewTab
            | Cmd::OpenTab(_)
            | Cmd::SelectTab(_)
            | Cmd::CloseTab(_)
            | Cmd::ShowTab { .. }
            | Cmd::UpdateDialog(_)
            | Cmd::InstallUpdate => {}
            Cmd::ProjectMove(m) => self.project_move(m),
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
            crate::checkout::Plan::Ask(sync) => {
                self.refresh_status();
                let action = sync.default_action();
                self.dialog = Some(Dialog::CheckoutSync { sync, action, stash: false, dirty: !self.status.is_empty() });
                None
            }
        }
    }

    /// The Check out dialog's choice: the whole sequence under one snapshot, which also keeps the
    /// uncommitted changes, so one Undo takes back the branch, HEAD and the changes.
    fn checkout_sync(&mut self, sync: &crate::checkout::Sync, action: crate::checkout::SyncAction, stash: bool) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        if self.busy.is_some() {
            self.notify("Please wait for the current operation to finish.", true);
            return;
        }
        let snap = Snapshot { dirty: r.stash_create(), ..r.snapshot(&sync.label(action), false, false) };
        let res = crate::checkout::run(r, sync, action, stash);
        self.reload();
        self.record(Some(snap));
        match res {
            Ok(msg) => self.notify(msg, false),
            Err(e) => self.notify(e, true),
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
            self.busy = None;
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
                self.notify(done, false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    fn open_diff(&mut self, req: DiffReq) {
        let (title, text) = match (&self.repo, &req) {
            (None, DiffReq::Commit { path, .. }) | (None, DiffReq::Worktree { path, .. }) => (path.clone(), demo::diff(path)),
            (Some(r), DiffReq::Commit { hash, path }) => (path.clone(), r.diff_commit_file(hash, path, self.diff_context)),
            (Some(r), DiffReq::Worktree { path, staged, untracked }) => (path.clone(), r.diff_worktree_file(path, *staged, *untracked, self.diff_context)),
        };
        self.diff = Some(DiffView { title, text });
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
        Snapshot { label: label.into(), state, soft: false, worktree: None, trashed: None, written: None, worktrees: vec![], dirty: None, dirty_after: None, left: None }
    }

    fn notice(t: &Tab) -> Option<(&str, bool)> {
        t.notice.as_ref().map(|(m, e, _)| (m.as_str(), *e))
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
