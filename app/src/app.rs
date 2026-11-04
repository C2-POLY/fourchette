mod assistant;
mod board;
mod palette;
mod project;
mod repos;
mod settings;
mod views;

use crate::git::{self, Commit, CommitDetails, GitRef, RefKind, Repo, Stash, Status};
use crate::graph::{self, Row};
use crate::library::DetailsLayout;
use crate::undo::{Snapshot, UNDO_DEPTH};
use crate::{demo, theme};
use egui::Color32;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Instant;

const COMMIT_LIMIT: usize = git::MAX_COMMIT_COUNT;
/// "automaticStatusUpdateInterval" default 60 s; 60.0 is also the fallback (sub_10031b7cc, sub_10048580c).
pub const STATUS_UPDATE_SECS: f32 = 60.0;
/// "commitSubjectLimitLow" / "commitSubjectLimitHigh" defaults (sub_10031b7cc).
pub const SUBJECT_LIMIT_LOW: usize = 50;
pub const SUBJECT_LIMIT_HIGH: usize = 70;

/// What a background job reports when it finishes.
type JobResult = Result<String, String>;

#[derive(Clone, PartialEq)]
pub enum Sel {
    None,
    Wip,
    Commit(usize),
}

#[derive(Clone)]
pub enum DiffReq {
    Commit { hash: String, path: String },
    Worktree { path: String, staged: bool, untracked: bool },
}

pub struct DiffView {
    pub title: String,
    pub text: String,
}

pub enum Dialog {
    NewRef { at: String, label: String, name: String, checkout: bool, tag: bool },
    /// The original's RemoveLocalBranchController (sub_100070d94): "Delete local branch from your
    /// repository", with "Also delete worktree" when the branch is checked out in one.
    DeleteBranch { name: String, merged: bool, force: bool, worktree: Option<crate::worktree::Worktree>, dirty: bool, also_worktree: bool },
    /// The original's RemoveWorktreeController (sub_1002008ec).
    DeleteWorktree { wt: crate::worktree::Worktree, dirty: bool },
    Confirm { title: String, message: String, label: String, args: Vec<String>, network: bool },
    /// Put a repository in a group the person names.
    NewGroup { path: PathBuf, name: String },
}

pub enum Cmd {
    Git { label: String, args: Vec<String>, network: bool },
    Fetch { all: bool, quiet: bool },
    Pull,
    Push,
    Discard { path: String, untracked: bool },
    Select(Sel),
    Diff(DiffReq),
    CloseDiff,
    OpenDialog,
    Open(PathBuf),
    Dialog(Dialog),
    Reload,
    Copy(String),
    Commit,
    SelectHash(String),
    Undo,
    Palette,
    /// Open the palette listing branches only (the original's showQuickLaunchCheckout:).
    PaletteCheckout,
    /// The original's hidden Quick Launch command "settings maxCommitCount <n>".
    SetCommitLimit(usize),
    /// System, dark or light, kept in repos.json.
    SetAppearance(theme::Appearance),
    /// Details panel on the right, at the bottom or hidden, kept per repository in repos.json.
    SetDetails(DetailsLayout),
    /// ⌘J: hide the details panel, or show it again where it was.
    ToggleDetails,
    /// The app's colours and icon, chosen in the palette.
    SetVariant(theme::Variant),
    /// Open or close the Settings window (⌘,).
    Settings,
    /// Keep the settings chosen in the Settings window; they apply at once.
    SetSettings(crate::settings::Settings),
    /// AI assistant (Claude through the person's subscription).
    AiCleanup,
    AiCommitMessage,
    AiExplain(String),
    AiDeleteSelected,
    /// Worktrees.
    AskDeleteBranch(String),
    DeleteBranch { name: String, force: bool, also_worktree: bool },
    AskDeleteWorktree(PathBuf),
    DeleteWorktree(PathBuf),
    PruneWorktrees,
    /// Repository switcher (⌘O) and the repository list.
    Switcher,
    RepoFavorite(PathBuf),
    RepoGroup(PathBuf, Option<String>),
    RepoForget(PathBuf),
    ScanFolder,
    Reveal(PathBuf),
    /// Repository > Open in Terminal (⌃⌘T).
    OpenTerminal(PathBuf),
    /// GitHub Projects.
    OpenUrl(String),
    ProjectReload,
    ProjectPick(usize),
    /// Open or close the project board (⌘B).
    Board,
}

impl Cmd {
    /// The toolbar's Stash, also in the palette and the Repository menu.
    pub fn stash() -> Cmd {
        Cmd::Git { label: "Changes stashed".into(), args: vec!["stash".into(), "push".into(), "-u".into()], network: false }
    }

    /// The toolbar's Pop.
    pub fn pop_stash() -> Cmd {
        Cmd::Git { label: "Stash applied".into(), args: vec!["stash".into(), "pop".into()], network: false }
    }

    /// The toolbar's Branch: a new branch at HEAD, or a tag there.
    pub fn new_ref_at_head(tag: bool) -> Cmd {
        Cmd::Dialog(Dialog::NewRef { at: "HEAD".into(), label: "the current commit".into(), name: String::new(), checkout: !tag, tag })
    }
}

/// The command palette (Cmd+K), modelled on the original's Quick Launch window.
pub struct Palette {
    pub query: String,
    pub sel: usize,
    /// Branches only, opened from the branch button (QuickLaunch checkout mode, sub_1001f8330 flag 1).
    pub checkout_only: bool,
}

/// Quick Launch row heights: 25 pt for items, 17 pt for group headers
/// (`tableView:heightOfRow:` → sub_100116548).
pub const PALETTE_ROW_H: f32 = 25.0;
pub const PALETTE_HEADER_H: f32 = 17.0;

pub struct App {
    repo: Option<Repo>,
    commits: Vec<Commit>,
    rows: Vec<Row>,
    max_lanes: usize,
    refs: Vec<GitRef>,
    by_target: HashMap<String, Vec<GitRef>>,
    index: HashMap<String, usize>,
    stashes: Vec<Stash>,
    status: Status,
    branch: Option<String>,
    head: String,
    ahead_behind: Option<(usize, usize)>,
    sel: Sel,
    details: Option<CommitDetails>,
    details_for: String,
    diff: Option<DiffView>,
    summary: String,
    description: String,
    amend: bool,
    search: String,
    filter: String,
    collapsed: HashSet<String>,
    toast: Option<(String, bool, Instant)>,
    busy: Option<(String, Receiver<JobResult>, bool)>,
    last_status: Instant,
    last_fetch: Instant,
    dialog: Option<Dialog>,
    scroll_to: Option<usize>,
    library: crate::library::Library,
    switcher: Option<repos::Switcher>,
    /// The Settings window (⌘,) is open.
    settings_open: bool,
    /// The section shown in the Settings window, an index of `settings::SECTIONS`.
    settings_section: usize,
    project: project::ProjectState,
    board: board::BoardState,
    screenshot: Option<PathBuf>,
    frame: u64,
    shot_requested: bool,
    was_focused: bool,
    undo: Vec<Snapshot>,
    /// Snapshot taken before a background job that changes the repository (Pull).
    pending_undo: Option<Snapshot>,
    palette: Option<Palette>,
    commit_limit: usize,
    ai: assistant::AiState,
    worktrees: Vec<crate::worktree::Worktree>,
    /// Where the details panel sits in this repository.
    details_layout: DetailsLayout,
    /// Actions picked in the macOS menu bar, run with the next frame's.
    menu_cmds: Vec<Cmd>,
}

pub fn short(h: &str) -> &str {
    &h[..h.len().min(7)]
}

fn graph_width(max_lanes: usize) -> f32 {
    (max_lanes as f32 * theme::LANE_W + 2.0 * theme::GRAPH_INSET - theme::LANE_W + 6.0).clamp(60.0, 360.0)
}

impl App {
    pub fn new(cc: &eframe::CreationContext, repo_arg: Option<PathBuf>, screenshot: Option<PathBuf>) -> Self {
        let library = crate::library::Library::load();
        // FOURCHETTE_APPEARANCE=light|dark|system shows a mode for a screenshot without saving it.
        let appearance = std::env::var("FOURCHETTE_APPEARANCE").ok().and_then(|a| theme::Appearance::parse(&a)).unwrap_or(library.appearance);
        // FOURCHETTE_VARIANT=classic|gold|neon|forest does the same for a variant.
        theme::apply(&cc.egui_ctx, appearance, theme::Variant::from_env_or(library.variant));
        let mut app = App {
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
            toast: None,
            busy: None,
            dialog: None,
            scroll_to: None,
            library,
            switcher: None,
            settings_open: false,
            settings_section: 0,
            project: project::ProjectState::default(),
            board: board::BoardState::default(),
            screenshot,
            frame: 0,
            shot_requested: false,
            was_focused: true,
            last_status: Instant::now(),
            last_fetch: Instant::now(),
            undo: Vec::new(),
            pending_undo: None,
            palette: None,
            commit_limit: COMMIT_LIMIT,
            ai: assistant::AiState::default(),
            worktrees: Vec::new(),
            details_layout: DetailsLayout::default(),
            menu_cmds: Vec::new(),
        };
        let cwd = std::env::current_dir().ok();
        let candidate = repo_arg
            .or_else(|| cwd.filter(|d| git::find_root(d).is_some()))
            .or_else(|| if app.screenshot.is_none() { app.library.recent().first().map(|e| e.path.clone()) } else { None });
        match candidate.map(|p| Repo::open(&p)) {
            Some(Ok(r)) => app.set_repo(r),
            Some(Err(e)) => {
                app.load_demo();
                app.notify(e, true);
            }
            None => app.load_demo(),
        }
        if std::env::var("FOURCHETTE_CLEANUP_PREVIEW").is_ok() {
            let a = |name: &str, action: &str, reason: &str, merged: bool| crate::ai::Advice {
                name: name.into(),
                action: action.into(),
                reason: reason.into(),
                merged,
                selected: action == "delete",
                worktree: None,
                worktree_dirty: false,
            };
            let wt = |folder: &str, branch: &str| crate::worktree::Worktree {
                path: PathBuf::from(format!("/Users/me/project/.claude/worktrees/{folder}")),
                branch: Some(branch.into()),
                head: String::new(),
                is_main: false,
                missing: false,
                locked: false,
            };
            let mut with_wt = a("claude/fix-login", "delete", "Merged into develop; its worktree is clean, so both can go.", true);
            with_wt.worktree = Some(wt("fix-login", "claude/fix-login"));
            let mut dirty_wt = a("claude/new-graph", "review", "Its worktree still has uncommitted changes — look at them first.", false);
            dirty_wt.worktree = Some(wt("new-graph", "claude/new-graph"));
            dirty_wt.worktree_dirty = true;
            app.ai.cleanup = Some(assistant::CleanupView {
                base: "develop".into(),
                count: 5,
                loading: false,
                summary: "Three branches are already fully in develop and can go — one of them is in a worktree that will be removed first. claude/new-graph has uncommitted work in its worktree, so check it.".into(),
                advice: vec![
                    with_wt,
                    a("hotfix/login-timeout", "delete", "Its fix is already in develop, so nothing would be lost.", true),
                    a("feature/graph-colours", "delete", "Merged into develop an hour ago; the branch isn't needed any more.", true),
                    dirty_wt,
                    a("feature/search", "review", "It has 1 change that isn't in develop yet — look at it before deleting.", false),
                ],
            });
        }
        if let Ok(q) = std::env::var("FOURCHETTE_PALETTE") {
            app.palette = Some(Palette { query: q, sel: 0, checkout_only: false });
        }
        if std::env::var("FOURCHETTE_SWITCHER").is_ok() {
            app.switcher = Some(repos::Switcher::default());
        }
        if let Ok(v) = std::env::var("FOURCHETTE_BOARD") {
            app.board.open = true;
            app.board.all = v == "all";
        }
        if let Some(n) = std::env::var("FOURCHETTE_BOARD_DETAIL").ok().and_then(|v| v.trim_start_matches('#').parse::<u64>().ok()) {
            app.board.open = true;
            app.board.detail = app.project.board.as_ref().and_then(|b| b.items.iter().find(|i| i.number == Some(n))).map(|i| i.id.clone());
        }
        // FOURCHETTE_SETTINGS=1 opens the Settings window, FOURCHETTE_SETTINGS=<section> at that section.
        if let Ok(v) = std::env::var("FOURCHETTE_SETTINGS") {
            app.settings_open = true;
            app.settings_section = settings::section_index(&v);
        }
        // FOURCHETTE_DETAILS=right|bottom|hidden shows a layout for a screenshot without saving it.
        if let Some(l) = std::env::var("FOURCHETTE_DETAILS").ok().and_then(|d| DetailsLayout::parse(&d)) {
            app.details_layout = l;
        }
        app
    }

    fn is_demo(&self) -> bool {
        self.repo.is_none()
    }

    fn load_demo(&mut self) {
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

    fn set_repo(&mut self, r: Repo) {
        let root = r.root.clone();
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
        if self.screenshot.is_none() {
            self.library.touch(&root);
            self.library.save();
        }
        self.details_layout = self.library.details(&root);
        self.project = project::ProjectState::default();
        self.project_load();
    }

    fn reload(&mut self) {
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

    fn refresh_status(&mut self) {
        if let Some(r) = &self.repo {
            self.status = r.status();
        }
        self.last_status = Instant::now();
    }

    /// The original refreshes the working copy when more than the update interval has passed
    /// (sub_10048580c), and fetches remotes automatically when that setting is on (here the
    /// person's choice in the Settings window, with its interval).
    fn timers(&mut self, ctx: &egui::Context) {
        if self.repo.is_none() || self.screenshot.is_some() {
            return;
        }
        if self.last_status.elapsed().as_secs_f32() > STATUS_UPDATE_SECS && self.busy.is_none() {
            self.refresh_status();
        }
        let s = self.library.settings;
        if s.auto_fetch && self.last_fetch.elapsed().as_secs_f32() > s.fetch_secs() && self.busy.is_none() {
            self.last_fetch = Instant::now();
            self.fetch(false, true, ctx);
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(5));
    }

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

    fn pull(&mut self, ctx: &egui::Context) {
        // Defaults "pullSheetRebase" = false and "pullSheetStashAndReapply" = false (sub_10031b7cc):
        // a plain merge pull of the upstream branch.
        let Some(up) = self.repo.as_ref().and_then(|r| r.upstream()) else {
            self.notify("The current branch has no upstream branch to pull from.", true);
            return;
        };
        if self.busy.is_none() {
            self.pending_undo = self.repo.as_ref().map(|r| r.snapshot("Pull", false, false));
        }
        self.spawn(&format!("Pull from '{}'", up.0), false, ctx, move |root| {
            let (o, _) = git::exec(root, &["pull", "--no-edit", &up.0, &up.1])?;
            Ok(if o.contains("Already up to date") { "Already up to date".into() } else { format!("Pulled {}/{}", up.0, up.1) })
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

    fn notify(&mut self, msg: impl Into<String>, error: bool) {
        self.toast = Some((msg.into(), error, Instant::now()));
    }

    fn ensure_details(&mut self) {
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

    fn run(&mut self, cmds: Vec<Cmd>, ctx: &egui::Context) {
        for c in cmds {
            match c {
                Cmd::Select(s) => {
                    self.sel = s;
                    self.diff = None;
                }
                Cmd::SelectHash(h) => {
                    if let Some(&i) = self.index.get(&h) {
                        self.sel = Sel::Commit(i);
                        self.scroll_to = Some(i);
                        self.diff = None;
                    }
                }
                Cmd::Diff(req) => self.open_diff(req),
                Cmd::CloseDiff => self.diff = None,
                Cmd::OpenDialog => {
                    if let Some(p) = rfd::FileDialog::new().set_title("Open a Git repository").pick_folder() {
                        self.open_path(p);
                    }
                }
                Cmd::Open(p) => self.open_path(p),
                Cmd::Dialog(d) => self.dialog = Some(d),
                Cmd::Reload => {
                    self.reload();
                    self.notify("Refreshed", false);
                }
                Cmd::Copy(s) => {
                    ctx.copy_text(s);
                    self.notify("Copied", false);
                }
                Cmd::Commit => {
                    let label = if self.amend { "Amend commit" } else { "Commit" };
                    let snap = self.repo.as_ref().map(|r| r.snapshot(label, true, false));
                    self.do_commit();
                    self.record(snap);
                }
                Cmd::Git { label, args, network } => self.git(label, args, network, ctx),
                Cmd::Fetch { all, quiet } => self.fetch(all, quiet, ctx),
                Cmd::Pull => self.pull(ctx),
                Cmd::Push => self.push(ctx),
                Cmd::Discard { path, untracked } => {
                    let Some(r) = &self.repo else { continue };
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
                Cmd::Palette => {
                    self.palette = if self.palette.is_some() { None } else { Some(Palette { query: String::new(), sel: 0, checkout_only: false }) };
                }
                Cmd::AiCleanup => self.ai_cleanup(ctx),
                Cmd::AiCommitMessage => self.ai_commit_message(ctx),
                Cmd::AiExplain(h) => self.ai_explain(h, ctx),
                Cmd::AiDeleteSelected => self.ai_delete_selected(),
                Cmd::AskDeleteBranch(name) => {
                    let Some(r) = &self.repo else {
                        self.notify("This is the sample project — open one of your repositories to do that.", true);
                        continue;
                    };
                    let merged = r.git(&["branch", "--format=%(refname:short)", "--merged", "HEAD"]).unwrap_or_default().lines().any(|l| l.trim() == name);
                    let worktree = r.worktree_of(&name);
                    let dirty = worktree.as_ref().map(|w| crate::worktree::is_dirty(&w.path)).unwrap_or(false);
                    self.dialog = Some(Dialog::DeleteBranch { name, merged, force: false, worktree, dirty, also_worktree: true });
                }
                Cmd::DeleteBranch { name, force, also_worktree } => {
                    let Some(r) = &self.repo else { continue };
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
                    let Some(r) = &self.repo else { continue };
                    let Some(wt) = self.worktrees.iter().find(|w| w.path == path).cloned() else { continue };
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
                    let Some(r) = &self.repo else { continue };
                    let res = r.git(&["worktree", "prune"]);
                    self.reload();
                    match res {
                        Ok(_) => self.notify("Cleaned up worktrees whose folder is gone", false),
                        Err(e) => self.notify(e, true),
                    }
                }
                Cmd::PaletteCheckout => {
                    self.palette = Some(Palette { query: String::new(), sel: 0, checkout_only: true });
                }
                Cmd::SetCommitLimit(n) => {
                    self.commit_limit = n;
                    self.reload();
                    self.notify(format!("maxCommitCount set to {n}"), false);
                }
                Cmd::SetAppearance(a) => {
                    self.library.appearance = a;
                    self.library.save();
                    theme::set_appearance(ctx, a);
                }
                Cmd::Settings => {
                    self.settings_open = !self.settings_open;
                    if self.settings_open {
                        self.palette = None;
                        self.switcher = None;
                    }
                }
                Cmd::SetSettings(s) => {
                    self.library.settings = s;
                    self.library.save();
                }
                Cmd::SetDetails(l) => self.set_details(l),
                Cmd::ToggleDetails => self.set_details(self.details_layout.toggled()),
                Cmd::SetVariant(v) => {
                    self.library.variant = v;
                    self.library.save();
                    theme::set_variant(ctx, v);
                    crate::icon::show(ctx, v);
                }
                Cmd::Switcher => {
                    self.switcher = if self.switcher.is_some() { None } else { Some(repos::Switcher::default()) };
                }
                Cmd::RepoFavorite(p) => {
                    self.library.toggle_favorite(&p);
                    self.library.save();
                }
                Cmd::RepoGroup(p, g) => {
                    self.library.set_group(&p, g);
                    self.library.save();
                }
                Cmd::RepoForget(p) => {
                    self.library.forget(&p);
                    self.library.save();
                }
                Cmd::ScanFolder => self.scan_folder(),
                Cmd::Reveal(p) => {
                    let _ = std::process::Command::new("open").arg("-R").arg(&p).spawn();
                }
                Cmd::OpenTerminal(p) => {
                    let _ = std::process::Command::new("open").args(["-a", "Terminal"]).arg(&p).spawn();
                }
                Cmd::OpenUrl(u) => ctx.open_url(egui::OpenUrl::new_tab(u)),
                Cmd::ProjectReload => self.project_load(),
                Cmd::ProjectPick(i) => self.project_pick(i),
                Cmd::Board => self.board_toggle(),
            }
        }
    }

    /// Changes the details panel layout and keeps it for this repository
    /// (not for the sample project, nor while taking a screenshot).
    fn set_details(&mut self, l: DetailsLayout) {
        self.details_layout = l;
        if let (Some(r), None) = (&self.repo, &self.screenshot) {
            self.library.set_details(&r.root, l);
            self.library.save();
        }
    }

    fn open_path(&mut self, p: PathBuf) {
        match Repo::open(&p) {
            Ok(r) => {
                let name = r.name();
                self.set_repo(r);
                self.notify(format!("Opened {name}"), false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    /// Adds every repository found under a folder to the list.
    fn scan_folder(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Add the repositories in a folder").pick_folder() else { return };
        let found = crate::library::scan(&dir, crate::library::SCAN_DEPTH);
        let added = found.iter().filter(|p| self.library.add(p)).count();
        self.library.save();
        self.switcher.get_or_insert_with(repos::Switcher::default);
        self.notify(
            match (found.len(), added) {
                (0, _) => format!("No repositories in {}", dir.display()),
                (n, 0) => format!("The {n} repositories there were already in the list"),
                (_, a) => format!("Added {a} repositories"),
            },
            found.is_empty(),
        );
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

    // ---------------------------------------------------------------- undo

    /// Keep the snapshot only if the action really changed something.
    fn record(&mut self, snap: Option<Snapshot>) {
        let (Some(s), Some(r)) = (snap, &self.repo) else { return };
        if r.state() != s.state {
            self.push_undo(s);
        }
    }

    fn push_undo(&mut self, s: Snapshot) {
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
            (Some(r), DiffReq::Commit { hash, path }) => (path.clone(), r.diff_commit_file(hash, path)),
            (Some(r), DiffReq::Worktree { path, staged, untracked }) => (path.clone(), r.diff_worktree_file(path, *staged, *untracked)),
        };
        self.diff = Some(DiffView { title, text });
    }

    fn head_has_upstream(&self) -> bool {
        self.refs.iter().any(|r| r.is_head && r.kind == RefKind::Local && !r.upstream.is_empty())
    }

    fn ref_color(&self, target: &str) -> Color32 {
        self.index.get(target).and_then(|&i| self.rows.get(i)).map(|r| theme::lane_color(r.color)).unwrap_or(theme::c().faint)
    }
}
