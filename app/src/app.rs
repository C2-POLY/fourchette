mod assistant;
mod board;
mod palette;
mod project;
mod repos;
mod settings;
mod tab;
mod tabs;
mod updater;
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
use tab::Tab;

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
    /// The original's CheckoutAndSyncController: check out a branch behind its upstream and bring
    /// it up to date. `dirty`: there are local changes, so "Stash and reapply" shows.
    CheckoutSync { sync: crate::checkout::Sync, action: crate::checkout::SyncAction, stash: bool, dirty: bool },
}

pub enum Cmd {
    Git { label: String, args: Vec<String>, network: bool },
    /// Check out a local branch, or (`remote`) a remote one: at once, or through the Check out
    /// dialog when it is behind its upstream (`checkout::plan`).
    Checkout { name: String, remote: bool },
    /// The Check out dialog's choice, run as one undo step.
    CheckoutSync { sync: crate::checkout::Sync, action: crate::checkout::SyncAction, stash: bool },
    Fetch { all: bool, quiet: bool },
    /// Pull the upstream branch, as the Settings window's Git section says.
    Pull,
    /// A stash's Apply: `stash apply`, or `stash pop` when "Delete stash after Apply" is on.
    ApplyStash(String),
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
    /// Go to the Project view, or back to the history (⌘B).
    Board,
    /// Tabs: pick a repository for a new tab in the switcher (⌘T, "+").
    NewTab,
    /// Open a repository in a new tab, or go to its tab when one shows it.
    OpenTab(PathBuf),
    /// Show the n-th tab.
    SelectTab(usize),
    /// Close the n-th tab; the last one stays.
    CloseTab(usize),
    /// Open (true) or close the update dialog of the toolbar's "Update available".
    UpdateDialog(bool),
    /// The update dialog's "Update and Restart".
    InstallUpdate,
    /// ⇧⌘] / ⌃Tab (next) and ⇧⌘[ / ⌃⇧Tab (previous), going round.
    ShowTab { next: bool },
}

impl Cmd {
    /// The toolbar's Stash, also in the palette and the Repository menu.
    pub fn stash() -> Cmd {
        Cmd::Git { label: "Changes stashed".into(), args: vec!["stash".into(), "push".into(), "-u".into()], network: false }
    }

    /// The toolbar's Pop.
    /// A stash's Apply, which also deletes it when `delete` ("deleteStashAfterApply"). `pop`
    /// deletes it only once applied without conflicts, and through `Tab::git` it takes a
    /// snapshot, so Undo stores the stash again.
    pub fn apply_stash(name: &str, delete: bool) -> Cmd {
        let (label, verb) = if delete { ("Stash applied and deleted", "pop") } else { ("Stash applied", "apply") };
        Cmd::Git { label: label.into(), args: vec!["stash".into(), verb.into(), name.into()], network: false }
    }

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
    /// The open repositories. There is always at least one; the views show `tabs[active]`.
    tabs: Vec<Tab>,
    active: usize,
    toast: Option<(String, bool, Instant)>,
    library: crate::library::Library,
    switcher: Option<repos::Switcher>,
    /// The Settings window (⌘,) is open.
    settings_open: bool,
    /// The section shown in the Settings window, an index of `settings::SECTIONS`.
    settings_section: usize,
    screenshot: Option<PathBuf>,
    frame: u64,
    shot_requested: bool,
    was_focused: bool,
    palette: Option<Palette>,
    /// Actions picked in the macOS menu bar, run with the next frame's.
    menu_cmds: Vec<Cmd>,
    /// New releases on GitHub: the check, the pill, the dialog and the install.
    update: updater::UpdateState,
}

pub fn short(h: &str) -> &str {
    &h[..h.len().min(7)]
}

fn graph_width(max_lanes: usize) -> f32 {
    (max_lanes as f32 * theme::LANE_W + 2.0 * theme::GRAPH_INSET - theme::LANE_W + 6.0).clamp(60.0, 360.0)
}

impl App {
    /// The app with one blank tab, before anything is open.
    fn with_library(library: crate::library::Library, screenshot: Option<PathBuf>) -> Self {
        App {
            tabs: vec![Tab::new()],
            active: 0,
            toast: None,
            library,
            switcher: None,
            settings_open: false,
            settings_section: 0,
            screenshot,
            frame: 0,
            shot_requested: false,
            was_focused: true,
            palette: None,
            menu_cmds: Vec::new(),
            update: updater::UpdateState::default(),
        }
    }

    pub fn new(cc: &eframe::CreationContext, repo_arg: Option<PathBuf>, screenshot: Option<PathBuf>) -> Self {
        let library = crate::library::Library::load();
        // FOURCHETTE_APPEARANCE=light|dark|system shows a mode for a screenshot without saving it.
        let appearance = std::env::var("FOURCHETTE_APPEARANCE").ok().and_then(|a| theme::Appearance::parse(&a)).unwrap_or(library.appearance);
        // FOURCHETTE_VARIANT=classic|gold|neon|forest does the same for a variant.
        theme::apply(&cc.egui_ctx, appearance, theme::Variant::from_env_or(library.variant));
        // FOURCHETTE_FONTS=<ui>,<code> (for example `sf,menlo`) shows fonts for a screenshot without saving them.
        let (ui_font, code_font) = fonts_from_env(&library.settings);
        theme::set_fonts(&cc.egui_ctx, ui_font, code_font);
        let mut app = App::with_library(library, screenshot);
        app.update = updater::UpdateState::new(app.screenshot.is_some());
        // FOURCHETTE_UPDATE=1|dialog shows the pill, and the dialog, with a sample release.
        app.update.apply_env();
        let cwd = std::env::current_dir().ok();
        let explicit = repo_arg.or_else(|| cwd.filter(|d| git::find_root(d).is_some()));
        // The tabs open at the last quit come back (not in a screenshot); without any, the
        // repository given, the current folder or the last one opened, as before tabs.
        if app.screenshot.is_some() || !app.restore_tabs(explicit.clone()) {
            let candidate = explicit.or_else(|| if app.screenshot.is_none() { app.library.recent().first().map(|e| e.path.clone()) } else { None });
            match candidate.map(|p| Repo::open(&p)) {
                Some(Ok(r)) => app.set_repo(r),
                Some(Err(e)) => {
                    app.tab_mut().load_demo();
                    app.notify(e, true);
                }
                None => app.tab_mut().load_demo(),
            }
        }
        // FOURCHETTE_TABS=<n> shows the sample project in n tabs, so the strip shows.
        if let Some(n) = std::env::var("FOURCHETTE_TABS").ok().and_then(|v| v.trim().parse::<usize>().ok()) {
            app.sample_tabs(n);
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
            app.tab_mut().ai.cleanup = Some(assistant::CleanupView {
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
        let t = app.tab_mut();
        if std::env::var("FOURCHETTE_BOARD_LIST").is_ok() {
            t.board.open = true;
            t.board.list = true;
        }
        if let Ok(v) = std::env::var("FOURCHETTE_BOARD") {
            t.board.open = true;
            t.board.all = v == "all";
        }
        if let Some(n) = std::env::var("FOURCHETTE_BOARD_DETAIL").ok().and_then(|v| v.trim_start_matches('#').parse::<u64>().ok()) {
            t.board.open = true;
            t.board.detail = t.project.board.as_ref().and_then(|b| b.items.iter().find(|i| i.number == Some(n))).map(|i| i.id.clone());
        }
        // FOURCHETTE_DETAILS=right|bottom|hidden shows a layout for a screenshot without saving it.
        if let Some(l) = std::env::var("FOURCHETTE_DETAILS").ok().and_then(|d| DetailsLayout::parse(&d)) {
            t.details_layout = l;
        }
        // FOURCHETTE_CHECKOUT_SYNC=1 opens the Check out dialog on `main`, 3 commits behind
        // origin/main; FOURCHETTE_CHECKOUT_SYNC=diverged with 2 commits of its own as well.
        if let Ok(v) = std::env::var("FOURCHETTE_CHECKOUT_SYNC") {
            if let Some(b) = t.refs.iter_mut().find(|r| r.name == "main" && r.kind == RefKind::Local) {
                (b.upstream, b.behind, b.ahead) = ("origin/main".into(), 3, if v == "diverged" { 2 } else { 0 });
            }
            let now = t.checkout("main", false);
            debug_assert!(now.is_none(), "main should ask");
        }
        // FOURCHETTE_SETTINGS=1 opens the Settings window, FOURCHETTE_SETTINGS=<section> at that section.
        if let Ok(v) = std::env::var("FOURCHETTE_SETTINGS") {
            app.settings_open = true;
            app.settings_section = settings::section_index(&v);
        }
        app
    }

    /// The tab on screen.
    fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// A dialog takes the window: the tab's, or the update dialog.
    fn modal(&self) -> bool {
        self.tab().dialog.is_some() || self.update.dialog
    }

    /// Opens `r` in the active tab, in place of the repository it showed.
    fn set_repo(&mut self, r: Repo) {
        let root = r.root.clone();
        self.tab_mut().set_repo(r);
        if self.screenshot.is_none() {
            self.library.touch(&root);
            self.remember_tabs();
            self.library.save();
        }
        let layout = self.library.details(&root);
        let t = self.tab_mut();
        t.details_layout = layout;
        t.project = project::ProjectState::default();
        self.project_load();
    }

    fn notify(&mut self, msg: impl Into<String>, error: bool) {
        self.toast = Some((msg.into(), error, Instant::now()));
    }

    /// Moves the tabs' latest messages to the window's toast. A message from a tab in the
    /// background starts with its repository's name, since the toast is the window's.
    fn take_notices(&mut self) {
        let active = self.active;
        for (i, t) in self.tabs.iter_mut().enumerate() {
            if let Some((msg, error, at)) = t.notice.take() {
                let msg = if i == active { msg } else { format!("{}: {msg}", t.name()) };
                self.toast = Some((msg, error, at));
            }
        }
    }

    // ---------------------------------------------------------------- actions

    fn run(&mut self, cmds: Vec<Cmd>, ctx: &egui::Context) {
        for c in cmds {
            match c {
                Cmd::OpenDialog => {
                    // From the switcher opened for a new tab, the folder goes to a new tab too.
                    let new_tab = self.switcher.as_ref().is_some_and(|s| s.new_tab);
                    if let Some(p) = rfd::FileDialog::new().set_title("Open a Git repository").pick_folder() {
                        self.open_path(p, new_tab);
                        if new_tab {
                            self.switcher = None;
                        }
                    }
                }
                Cmd::Open(p) => self.open_path(p, false),
                Cmd::OpenTab(p) => self.open_path(p, true),
                Cmd::NewTab => {
                    let open = self.switcher.as_ref().is_some_and(|s| s.new_tab);
                    self.switcher = if open { None } else { Some(repos::Switcher { new_tab: true, ..Default::default() }) };
                }
                Cmd::SelectTab(i) => self.activate(i),
                Cmd::CloseTab(i) => self.close_tab(i),
                Cmd::ShowTab { next } => self.show_next_tab(next),
                Cmd::Copy(s) => {
                    ctx.copy_text(s);
                    self.notify("Copied", false);
                }
                Cmd::Palette => {
                    self.palette = if self.palette.is_some() { None } else { Some(Palette { query: String::new(), sel: 0, checkout_only: false }) };
                }
                Cmd::PaletteCheckout => {
                    self.palette = Some(Palette { query: String::new(), sel: 0, checkout_only: true });
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
                    if (s.ui_font, s.code_font) != (self.library.settings.ui_font, self.library.settings.code_font) {
                        theme::set_fonts(ctx, s.ui_font, s.code_font);
                    }
                    self.library.settings = s;
                    self.library.save();
                }
                // The hidden palette command sets the same setting as the Settings window.
                Cmd::SetCommitLimit(n) => {
                    self.library.settings.max_commits = n;
                    self.library.save();
                    self.notify(format!("maxCommitCount set to {n}"), false);
                }
                Cmd::SetDetails(l) => self.set_details(l),
                Cmd::ToggleDetails => self.set_details(self.tab().details_layout.toggled()),
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
                Cmd::UpdateDialog(open) => self.update_dialog_open(open),
                Cmd::InstallUpdate => self.update_install(ctx),
                Cmd::Pull => {
                    let s = self.library.settings;
                    self.tab_mut().pull(ctx, s);
                }
                Cmd::ApplyStash(name) => {
                    let c = Cmd::apply_stash(&name, self.library.settings.delete_stash_after_apply);
                    self.tab_mut().run(c, ctx);
                }
                c => self.tab_mut().run(c, ctx),
            }
            // Each command's message in turn, so the last one shown is the last one said.
            self.take_notices();
        }
    }

    /// Changes the details panel layout and keeps it for this repository
    /// (not for the sample project, nor while taking a screenshot).
    fn set_details(&mut self, l: DetailsLayout) {
        let t = &mut self.tabs[self.active];
        t.details_layout = l;
        if let (Some(r), None) = (&t.repo, &self.screenshot) {
            self.library.set_details(&r.root, l);
            self.library.save();
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
}

/// The saved fonts, or those of `FOURCHETTE_FONTS=<ui>,<code>` (either part may be left out).
fn fonts_from_env(saved: &crate::settings::Settings) -> (crate::settings::UiFont, crate::settings::CodeFont) {
    use crate::settings::{CodeFont, UiFont};
    let v = std::env::var("FOURCHETTE_FONTS").unwrap_or_default();
    let mut parts = v.split(',');
    let ui = parts.next().and_then(UiFont::parse).unwrap_or(saved.ui_font);
    let code = parts.next().and_then(CodeFont::parse).unwrap_or(saved.code_font);
    (ui, code)
}
