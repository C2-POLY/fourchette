mod assistant;
mod board;
mod issue;
mod changelog;
mod palette;
mod pr_draft;
mod operation;
mod project;
mod repos;
mod resolve;
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
    CheckoutSync { sync: crate::checkout::Sync, action: crate::checkout::SyncAction, merge: crate::checkout::MergeStyle, stash: bool, dirty: bool },
    /// "Do you want to abort the merge?" for the operation in progress (`Repo::operation`).
    Abort { op: String },
    /// The original's PushWindowController: the remote branch, Force push and Push all tags.
    Push(crate::push::Push),
    /// The original's MergeWindowController: merge a branch into the checked-out one, as a merge
    /// commit or squashed, `no_ff` its "No fast-forward" box; `check` foresees conflicts.
    Merge { merge: crate::merge::Merge, style: crate::checkout::MergeStyle, no_ff: bool, check: crate::merge::Check },
    /// The original's PullWindowController: the remote branch to pull, Merge or Rebase, and
    /// "Stash and reapply" when there are local changes (`dirty`).
    Pull { pull: crate::pull::Pull, mode: crate::pull::Mode, stash: bool, dirty: bool },
}

pub enum Cmd {
    Git { label: String, args: Vec<String>, network: bool },
    /// Check out a local branch, or (`remote`) a remote one: at once, or through the Check out
    /// dialog when it is behind its upstream (`checkout::plan`).
    Checkout { name: String, remote: bool },
    /// The Check out dialog's choice, run as one undo step.
    /// `merge`: how Merge joins the upstream (ignored by the other actions).
    CheckoutSync { sync: crate::checkout::Sync, action: crate::checkout::SyncAction, merge: crate::checkout::MergeStyle, stash: bool },
    /// Abort, Continue or Skip the merge, rebase, cherry-pick or revert in progress.
    Operation(crate::operation::Action),
    /// Open the Merge dialog for a branch (`remote`: a remote one) into the checked-out one.
    AskMerge { name: String, remote: bool },
    /// The Merge dialog's choice, run as one undo step.
    Merge { merge: crate::merge::Merge, way: crate::merge::Way },
    Fetch { all: bool, quiet: bool },
    /// Pull the upstream branch, as the Settings window's Git section says.
    Pull,
    /// Repository › Pull… (⇧⌘L): open the Pull dialog.
    PullDialog,
    /// The Pull dialog's choice: `git pull` with these arguments, from `source`.
    PullFrom { source: crate::pull::Source, args: Vec<String> },
    /// A stash's Apply: `stash apply`, or `stash pop` when "Delete stash after Apply" is on.
    ApplyStash(String),
    Push,
    /// Push… (⇧⌘P, the palette): open the Push dialog on the current branch.
    AskPush,
    /// The Push dialog's choice.
    PushTo(crate::push::Push),
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
    /// Claude writes the changelog, for the audience chosen in its window.
    AiChangelog,
    /// Claude drafts the pull request of the current branch, in its window.
    AiDraftPr,
    /// Claude proposes a resolution for these conflicted files (all of them when empty).
    AiResolve(Vec<String>),
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
    /// Move a card of the project's board: at once on screen, then on GitHub.
    ProjectMove(crate::github::Move),
    /// Go to the Project view, or back to the history (⌘B).
    Board,
    /// The toolbar's issue chip: the Project view with that item's details.
    IssueDetail(String),
    /// The offer to move the branch's issue: Move, Not now, and the confirmation's Undo.
    IssueMove,
    IssueDecline,
    IssueUndo,
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
    /// Settings' "Check now": check for an update at once, unless a check is running.
    UpdateCheckNow,
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
    toast: Option<tab::Notice>,
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
    /// FOURCHETTE_MENU=<x>,<y>: where a screenshot right-clicks, to show a context menu.
    shot_menu: Option<egui::Pos2>,
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
            shot_menu: None,
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
        // FOURCHETTE_CHANGELOG=<developers|product|leadership|users> shows the changelog window.
        if let Ok(v) = std::env::var("FOURCHETTE_CHANGELOG") {
            let a = crate::changelog::Audience::ALL.into_iter().find(|a| a.label().to_lowercase().starts_with(&v.to_lowercase())).unwrap_or_default();
            app.tab_mut().changelog_sample(a);
        }
        // FOURCHETTE_PR=1 shows the pull request window with the sample draft.
        if std::env::var("FOURCHETTE_PR").is_ok() {
            app.tab_mut().pr_draft_sample();
        }
        // FOURCHETTE_RESOLVE=1 shows the conflict window with the sample project's conflicts.
        if std::env::var("FOURCHETTE_RESOLVE").is_ok() {
            app.tab_mut().resolve_sample();
        }
        if let Ok(v) = std::env::var("FOURCHETTE_CLEANUP_PREVIEW") {
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
                loading: v == "loading",
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
        // FOURCHETTE_BOARD_TREE=parents|all|nested: how the board shows sub-issues.
        if let Ok(v) = std::env::var("FOURCHETTE_BOARD_TREE") {
            t.board.open = true;
            t.board.tree = board::Tree::ALL.into_iter().find(|tr| tr.label().eq_ignore_ascii_case(&v)).unwrap_or_default();
        }
        if let Ok(v) = std::env::var("FOURCHETTE_BOARD") {
            t.board.open = true;
            t.board.all = v == "all";
        }
        if let Some(n) = std::env::var("FOURCHETTE_BOARD_DETAIL").ok().and_then(|v| v.trim_start_matches('#').parse::<u64>().ok()) {
            t.board.open = true;
            t.board.detail = t.project.board.as_ref().and_then(|b| b.items.iter().find(|i| i.number == Some(n))).map(|i| i.id.clone());
        }
        // FOURCHETTE_BOARD_DRAG=<n> shows card #n lifted mid-drag over the next column.
        if let Some(n) = std::env::var("FOURCHETTE_BOARD_DRAG").ok().and_then(|v| v.trim_start_matches('#').parse::<u64>().ok()) {
            t.board.open = true;
            t.board.drag_demo = Some(n);
        }
        // FOURCHETTE_ISSUE=<n>[,progress|review|moved]: on a branch of issue #n, with its offer.
        if let Ok(v) = std::env::var("FOURCHETTE_ISSUE") {
            t.issue_sample(&v);
        }
        // FOURCHETTE_DETAILS=right|bottom|hidden shows a layout for a screenshot without saving it.
        if let Some(l) = std::env::var("FOURCHETTE_DETAILS").ok().and_then(|d| DetailsLayout::parse(&d)) {
            t.details_layout = l;
        }
        // FOURCHETTE_CHECKOUT_SYNC=1 opens the Check out dialog on `main`, diverged from
        // origin/main: 2 commits of its own, 3 behind. (Only behind, it would not ask.)
        if std::env::var("FOURCHETTE_CHECKOUT_SYNC").is_ok() {
            if let Some(b) = t.refs.iter_mut().find(|r| r.name == "main" && r.kind == RefKind::Local) {
                (b.upstream, b.behind, b.ahead) = ("origin/main".into(), 3, 2);
            }
            let now = t.checkout("main", false);
            debug_assert!(now.is_none(), "main should ask");
            // FOURCHETTE_CHECKOUT_ACTION=ff|merge|rebase|reset|stay picks the action shown.
            if let (Some(Dialog::CheckoutSync { sync, action, .. }), Ok(a)) = (&mut t.dialog, std::env::var("FOURCHETTE_CHECKOUT_ACTION")) {
                use crate::checkout::SyncAction::*;
                let wanted = match a.as_str() {
                    "ff" => FastForward,
                    "merge" => Merge,
                    "rebase" => Rebase,
                    "reset" => Reset,
                    _ => JustCheckout,
                };
                if sync.actions().contains(&wanted) {
                    *action = wanted;
                }
            }
            // FOURCHETTE_CHECKOUT_CLEAN=1 shows it without local changes (no stash option).
            if let (Some(Dialog::CheckoutSync { dirty, .. }), Ok(_)) = (&mut t.dialog, std::env::var("FOURCHETTE_CHECKOUT_CLEAN")) {
                *dirty = false;
            }
            // FOURCHETTE_CHECKOUT_MERGE=squash picks Merge with that style.
            if let (Some(Dialog::CheckoutSync { action, merge, .. }), Ok(m)) = (&mut t.dialog, std::env::var("FOURCHETTE_CHECKOUT_MERGE")) {
                let wanted = if m == "squash" { crate::checkout::MergeStyle::Squash } else { crate::checkout::MergeStyle::Commit };
                (*action, *merge) = (crate::checkout::SyncAction::Merge, wanted);
            }
        }
        // FOURCHETTE_OPERATION=<merge|rebase|cherry-pick|revert>[,resolved] shows the banner of an
        // operation stopped on conflicts (`,resolved`: all of them resolved).
        if let Ok(v) = std::env::var("FOURCHETTE_OPERATION") {
            t.operation = Some(crate::operation::sample(&v));
        }
        // FOURCHETTE_DIALOG=<name> opens one of the dialogs on the sample project.
        if let Ok(v) = std::env::var("FOURCHETTE_DIALOG") {
            t.dialog = sample_dialog(&v, &t.worktrees);
            if let Some(kind) = v.strip_prefix("push") {
                sample_push(t, kind);
            }
        }
        // FOURCHETTE_DIALOG=pull: the Pull dialog on `develop`, diverged from origin/develop.
        if std::env::var("FOURCHETTE_DIALOG").is_ok_and(|v| v == "pull") {
            let s = app.library.settings;
            app.tab_mut().pull_sample(s);
        }
        // FOURCHETTE_MENU=<x>,<y> right-clicks there, in points, to show a context menu.
        app.shot_menu = std::env::var("FOURCHETTE_MENU").ok().and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some(egui::pos2(x.trim().parse().ok()?, y.trim().parse().ok()?))
        });
        // FOURCHETTE_TOAST=<message> shows a toast, FOURCHETTE_TOAST=error:<message> an error's,
        // FOURCHETTE_TOAST=undo:<message> one that offers Cmd+Z.
        if let Ok(v) = std::env::var("FOURCHETTE_TOAST") {
            let (msg, error) = v.strip_prefix("error:").map_or((v.as_str(), false), |m| (m, true));
            let (msg, undo) = msg.strip_prefix("undo:").map_or((msg, false), |m| (m, true));
            app.notify(msg.to_string(), error);
            if let Some(t) = &mut app.toast {
                t.undo = undo;
            }
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
        t.pulls = project::PullsState::default();
        self.project_load();
    }

    fn notify(&mut self, msg: impl Into<String>, error: bool) {
        self.toast = Some(tab::Notice { msg: msg.into(), error, at: Instant::now(), undo: false });
    }

    /// Moves the tabs' latest messages to the window's toast. A message from a tab in the
    /// background starts with its repository's name, since the toast is the window's.
    fn take_notices(&mut self) {
        let active = self.active;
        for (i, t) in self.tabs.iter_mut().enumerate() {
            if let Some(mut n) = t.notice.take() {
                if i != active {
                    n.msg = format!("{}: {}", t.name(), n.msg);
                }
                self.toast = Some(n);
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
                    // Another channel: check on it at once, so the pill shows what it offers.
                    if s.update_channel != self.library.settings.update_channel {
                        self.update.check_now(s.update_channel);
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
                Cmd::IssueDetail(id) => {
                    if !self.tab().board.open {
                        self.board_toggle();
                    }
                    self.tab_mut().board.detail = Some(id);
                }
                Cmd::UpdateDialog(open) => self.update_dialog_open(open),
                Cmd::InstallUpdate => self.update_install(ctx),
                Cmd::UpdateCheckNow => self.update.check_now(self.library.settings.update_channel),
                Cmd::Pull => {
                    let s = self.library.settings;
                    self.tab_mut().pull(ctx, s);
                }
                Cmd::PullDialog => {
                    let s = self.library.settings;
                    self.tab_mut().pull_dialog(s);
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

/// The dialog `FOURCHETTE_DIALOG` names, filled in as on the sample project: `confirm` (delete a
/// tag), `rebase`, `branch`, `tag`, `delete-branch` (checked out in a worktree with changes, not
/// merged), `delete-worktree`, `group`, and `merge` (feature/search into develop, diverged, with
/// conflicts; `merge-clean` without, `merge-ff` where a fast-forward is possible, `merge-noff` with "No fast-forward" ticked, `merge-checking`
/// while the check runs, `merge-old` with a Git too old to check), and `abort` (of
/// `FOURCHETTE_OPERATION`'s, else a merge).
fn sample_dialog(name: &str, worktrees: &[crate::worktree::Worktree]) -> Option<Dialog> {
    let wt = worktrees.iter().find(|w| !w.is_main).cloned();
    let confirm = |title: &str, message: &str, args: &[&str]| Dialog::Confirm {
        title: title.into(),
        message: message.into(),
        label: title.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        network: false,
    };
    Some(match name {
        "confirm" => confirm("Delete tag", "Do you want to delete the tag “v2.3.0”?", &["tag", "-d", "v2.3.0"]),
        "rebase" => confirm("Rebase", "Do you want to rebase “develop” onto “main”?", &["rebase", "main"]),
        "branch" => Dialog::NewRef { at: "develop".into(), label: "develop".into(), name: "feature/".into(), checkout: true, tag: false },
        "tag" => Dialog::NewRef { at: "ca2a04c".into(), label: "commit ca2a04c".into(), name: String::new(), checkout: false, tag: true },
        "delete-branch" => {
            let name = wt.as_ref().and_then(|w| w.branch.clone()).unwrap_or_else(|| "feature/search".into());
            Dialog::DeleteBranch { name, merged: false, force: false, worktree: wt, dirty: true, also_worktree: true }
        }
        "delete-worktree" => Dialog::DeleteWorktree { wt: wt?, dirty: true },
        "abort" => Dialog::Abort { op: std::env::var("FOURCHETTE_OPERATION").map(|v| crate::operation::sample(&v).op).unwrap_or_else(|_| "merge".into()) },
        "group" => Dialog::NewGroup { path: PathBuf::from("/Users/me/dev/fourchette"), name: "Side projects".into() },
        m if m.starts_with("merge") => {
            use crate::merge::{Check, Merge, Test};
            let (ours, theirs) = if m == "merge-ff" || m == "merge-noff" { (0, 3) } else { (2, 3) };
            let test = match m {
                "merge-clean" | "merge-ff" | "merge-noff" => Test::Clean,
                "merge-checking" => Test::Checking,
                "merge-old" => Test::Unknown("Checking for conflicts beforehand needs Git 2.38 or later.".into()),
                _ => Test::Conflicts(vec!["src/search/index.rs".into(), "src/app.rs".into(), "Cargo.lock".into()]),
            };
            let merge = Merge { branch: "feature/search".into(), remote: false, head: "develop".into(), ours, theirs };
            Dialog::Merge { merge, style: Default::default(), no_ff: m == "merge-noff", check: Check::done(test) }
        }
        _ => return None,
    })
}

/// The Push dialog on the sample project's `develop`: `push` 3 commits ahead of origin/develop,
/// `-diverged` 2 ahead and 3 behind, `-force` the same with Force push asking to confirm, `-new`
/// to a new remote branch with all tags.
fn sample_push(t: &mut Tab, kind: &str) {
    let (ahead, behind, upstream) = match kind {
        "-diverged" | "-force" => (2, 3, "origin/develop"),
        "-new" => (0, 0, ""),
        _ => (3, 0, "origin/develop"),
    };
    if let Some(b) = t.refs.iter_mut().find(|r| r.name == "develop" && r.kind == RefKind::Local) {
        (b.upstream, b.ahead, b.behind) = (upstream.into(), ahead, behind);
    }
    t.ask_push();
    if let Some(Dialog::Push(p)) = &mut t.dialog {
        match kind {
            "-force" => (p.force, p.confirm) = (true, true),
            "-new" => {
                p.target = "feature/lighter-toolbar".into();
                p.tags = true;
            }
            _ => {}
        }
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
