mod app_icon;
mod assistant;
mod board;
mod city;
mod board_full;
mod issue;
mod mess;
mod changelog;
mod checks;
mod clone;
mod cockpit;
mod home;
mod palette;
mod pr_draft;
mod operation;
mod project;
mod repos;
mod resolve;
mod review;
mod settings;
mod tab;
mod tabs;
mod track;
mod course;
mod updater;
mod view_tabs;
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
    /// A line of the new side to scroll to and highlight (a review remark's).
    pub goto: Option<u32>,
    /// The scroll to `goto` is done, so the person can scroll away.
    pub scrolled: std::cell::Cell<bool>,
}

pub enum Dialog {
    NewRef { at: String, label: String, name: String, checkout: bool, tag: bool },
    /// The original's RemoveLocalBranchController (sub_100070d94): "Delete local branch from your
    /// repository", with "Also delete worktree" when the branch is checked out in one, and its
    /// remote branch as a second check row when it has one.
    DeleteBranch(crate::delete::Delete),
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
    /// A diff scrolled to a line of its new side, highlighted (a review remark's).
    DiffAt(DiffReq, Option<u32>),
    CloseDiff,
    OpenDialog,
    Open(PathBuf),
    /// The clone dialog (#204): home page, File › Clone Repository… and the palette.
    CloneDialog,
    /// The clone dialog's Change…: another folder for clones.
    ClonePickFolder,
    /// A repository just cloned, or already in the list: it joins the list and opens.
    CloneOpen(PathBuf),
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
    /// The skin over the interface (#202).
    SetSkin(crate::settings::Skin),
    /// The Runner skin's Finisher dialog: Run it again (`again`) or Recover (#206).
    Finisher { again: bool },
    /// Details panel on the right, at the bottom or hidden, kept per repository in repos.json.
    SetDetails(DetailsLayout),
    /// ⌘J: hide the details panel, or show it again where it was.
    ToggleDetails,
    /// Open or close the Settings window (⌘,).
    Settings,
    /// Keep the settings chosen in the Settings window; they apply at once.
    SetSettings(crate::settings::Settings),
    /// The icon colour picker is being dragged: the Dock shows the colour, nothing is saved.
    PreviewIcon(crate::icon::IconColour),
    /// The List / Tree switch of the local changes (a setting, shared by every repository).
    SetFilesTree(bool),
    /// The diff's Unified / Side by side and Whole file choices, kept in the settings.
    SetDiffView { split: bool, whole: bool },
    /// Folds or unfolds a folder of the Tree view (`true` for the staged list).
    ToggleDir(bool, String),
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
    /// Claude reviews the staged changes, in its window.
    AiReview,
    /// Worktrees.
    /// Delete a branch: the one clicked, local or remote (both for a merged pill); the dialog
    /// offers the other side too.
    AskDeleteBranch { local: Option<String>, remote: Option<String> },
    DeleteBranch(crate::delete::Delete),
    AskDeleteWorktree(PathBuf),
    DeleteWorktree(PathBuf),
    PruneWorktrees,
    /// The worktree cockpit: every worktree with its state, issue and pull request.
    Cockpit,
    /// Repository switcher (⌘O) and the repository list.
    Switcher,
    RepoFavorite(PathBuf),
    RepoGroup(PathBuf, Option<String>),
    RepoForget(PathBuf),
    /// "Remove N missing": these repositories, whose folder is gone, come off the list.
    RepoRemoveMissing(Vec<PathBuf>),
    /// The Undo of that toast: they go back where they were.
    RepoPutBack,
    ScanFolder,
    /// "Fetch all" of the switcher and the palette: every repository of the list.
    FetchAllRepos,
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
    /// Show the board alone in the window, or bring the rest back (⌃⌘B).
    BoardFull,
    /// The toolbar's issue chip: the Project view with that item's details.
    IssueDetail(String),
    /// A click on a branch's pull request badge: the card of its checks, under the badge.
    Checks(crate::pulls::Pull, Option<egui::Pos2>),
    /// The offer to move the branch's issue: Move, Not now, and the confirmation's Undo.
    IssueMove,
    IssueDecline,
    IssueUndo,
    /// Tabs: a new tab on the home page (⌘T, "+").
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
    /// The "What's new in Fourchette" dialog's OK.
    CloseWhatsNew,
    /// ⇧⌘] / ⌃Tab (next) and ⇧⌘[ / ⌃⇧Tab (previous), going round.
    ShowTab { next: bool },
    /// The home page (⇧⌘O, File menu, palette): its tab, else a new one (issue #109).
    Home,
    /// The home page's "Keep as a group": these repositories go to that group.
    HomeKeep { name: String, paths: Vec<PathBuf> },
    /// The home page's "Dismiss" of a proposal, by its key.
    HomeDismiss(String),
    /// The home page's "Explore the sample project", in its tab.
    HomeSample,
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
    /// What the switcher shows of each repository: changes, ahead/behind, pull requests, Fetch all.
    states: crate::repo_state::States,
    /// The Settings window (⌘,) is open.
    settings_open: bool,
    /// The messy gamer skin's litter and vacuum (#202).
    mess: mess::Mess,
    /// The Runner skin's race of the day (#206), in memory only.
    race: course::CourseState,
    /// The fonts of the Settings menus are loaded, each in its own family (`theme::preview_family`).
    font_previews: bool,
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
    /// The app icon in the colour chosen in Settings (#159).
    icon: app_icon::IconState,
    /// FOURCHETTE_MENU=<x>,<y>: where a screenshot right-clicks, to show a context menu.
    shot_menu: Option<egui::Pos2>,
    /// The home page's search and what it read from each repository's folder.
    home: home::HomeState,
    /// The full board (⌃⌘B, issue #153): the board alone, under one thin bar.
    full: board_full::FullBoard,
    /// The repositories "Remove N missing" took off last, which its toast's Undo puts back.
    removed: crate::library::Removed,
    /// Folders whose repositories are opened without being kept in the list or the tabs
    /// (`library::temp_roots`); none in tests, whose repositories live there.
    temp_roots: Vec<PathBuf>,
    /// The clone dialog (#204), open over any tab.
    clone: Option<clone::CloneView>,
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
            states: Default::default(),
            settings_open: false,
            mess: mess::Mess::default(),
            race: course::CourseState::default(),
            font_previews: false,
            settings_section: 0,
            screenshot,
            frame: 0,
            shot_requested: false,
            was_focused: true,
            palette: None,
            menu_cmds: Vec::new(),
            update: updater::UpdateState::default(),
            icon: app_icon::IconState::default(),
            shot_menu: None,
            home: home::HomeState::default(),
            full: Default::default(),
            removed: Vec::new(),
            temp_roots: crate::library::temp_roots(),
            clone: None,
        }
    }

    pub fn new(cc: &eframe::CreationContext, repo_arg: Option<PathBuf>, screenshot: Option<PathBuf>) -> Self {
        let library = crate::library::Library::load();
        // FOURCHETTE_APPEARANCE=light|dark|system shows a mode for a screenshot without saving it.
        let appearance = std::env::var("FOURCHETTE_APPEARANCE").ok().and_then(|a| theme::Appearance::parse(&a)).unwrap_or(library.appearance);
        theme::apply(&cc.egui_ctx, appearance);
        // FOURCHETTE_FONTS=<ui>,<code> (for example `sf,menlo`) shows fonts for a screenshot without saving them.
        let (ui_font, code_font) = fonts_from_env(&library.settings);
        theme::set_fonts(&cc.egui_ctx, ui_font, code_font, false);
        let mut app = App::with_library(library, screenshot);
        app.update = updater::UpdateState::new(app.screenshot.is_some());
        // FOURCHETTE_UPDATE=1|dialog shows the pill, and the dialog, with a sample release;
        // FOURCHETTE_WHATS_NEW=1 the dialog shown once after an update.
        app.update.apply_env();
        // The saved icon colour, or FOURCHETTE_ICON=<preset|#rrggbb>'s without saving it.
        app.icon_start();
        let cwd = std::env::current_dir().ok();
        let explicit = repo_arg.or_else(|| cwd.filter(|d| git::find_root(d).is_some()));
        // The tabs open at the last quit come back (not in a screenshot); without any, the
        // repository given or the current folder, else the home page (the sample project in a
        // screenshot, unless FOURCHETTE_HOME=1).
        if app.screenshot.is_some() || !app.restore_tabs(explicit.clone()) {
            let home = app.screenshot.is_none() || std::env::var("FOURCHETTE_HOME").is_ok();
            match explicit.map(|p| Repo::open(&p)) {
                Some(Ok(r)) => app.set_repo(r),
                Some(Err(e)) => {
                    app.tab_mut().home = true;
                    app.notify(e, true);
                }
                None if home => app.tab_mut().home = true,
                None => app.tab_mut().load_demo(),
            }
        }
        // FOURCHETTE_HOME=1 shows the home page with invented repositories, never the person's;
        // FOURCHETTE_HOME=missing with three of them gone (so "Remove 3 missing" shows);
        // FOURCHETTE_HOME=<text> with that search typed.
        if let (Ok(v), Some(_)) = (std::env::var("FOURCHETTE_HOME"), &app.screenshot) {
            app.home.sample(&mut app.library, v == "missing");
            if v != "1" && v != "missing" {
                app.home.query = v;
            }
        }
        // FOURCHETTE_FILES=tree|list shows the local changes (selected) as a tree or a list for a screenshot without saving it.
        if app.screenshot.is_some() {
            if let Ok(v) = std::env::var("FOURCHETTE_FILES") {
                app.library.settings.files_tree = v == "tree";
                app.tab_mut().sel = Sel::Wip;
            }
        }
        // FOURCHETTE_DIFF=<unified|split>[,whole] opens the sample diff in that mode for a screenshot, without saving it.
        if app.screenshot.is_some() {
            if let Ok(v) = std::env::var("FOURCHETTE_DIFF") {
                app.library.settings.diff_split = v.split(',').any(|w| w == "split");
                app.library.settings.diff_whole = v.split(',').any(|w| w == "whole");
                let (split, whole) = (app.library.settings.diff_split, app.library.settings.diff_whole);
                app.tab_mut().show_demo_diff(split, whole);
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
        // FOURCHETTE_COCKPIT=1 shows the worktree cockpit of the sample project (`=tidy` its Tidy up list).
        if let Ok(v) = std::env::var("FOURCHETTE_COCKPIT") {
            app.tab_mut().cockpit_sample(v == "tidy");
        }
        // FOURCHETTE_REVIEW=1 shows the review window with the sample remarks (`loading` while
        // Claude reads, `diff` the diff a click on the first remark opens).
        if let Ok(v) = std::env::var("FOURCHETTE_REVIEW") {
            app.tab_mut().review_sample(v == "loading", v == "diff");
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
        // FOURCHETTE_SWITCHER=1|fetching|fetched: the switcher, on a sample list in a screenshot.
        if let Ok(v) = std::env::var("FOURCHETTE_SWITCHER") {
            app.switcher = Some(repos::Switcher::default());
            if app.screenshot.is_some() {
                app.switcher_sample(&v);
            }
        }
        let t = app.tab_mut();
        if std::env::var("FOURCHETTE_BOARD_LIST").is_ok() {
            t.board.open = true;
            t.board.list = true;
        }
        // FOURCHETTE_BOARD_CITY=1: the board as a city (#190), `=all` with the whole project, `=town`
        // a bigger sample city with every district style (#200).
        if let Ok(v) = std::env::var("FOURCHETTE_BOARD_CITY") {
            t.board.open = true;
            t.board.city_on = true;
            t.board.all = v == "all" || v == "town";
            if v == "town" {
                if let Some(b) = t.project.board.as_mut() {
                    std::sync::Arc::make_mut(b).items = crate::city::sample();
                }
            }
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
        // FOURCHETTE_BOARD_FULL=1: the board alone in the window (#153).
        let full = std::env::var("FOURCHETTE_BOARD_FULL").is_ok();
        if full {
            t.board.open = true;
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
        // FOURCHETTE_CHECKS=<n> opens the checks of sample pull request #n under its badge.
        if let Some(n) = std::env::var("FOURCHETTE_CHECKS").ok().and_then(|v| v.trim_start_matches('#').parse::<u64>().ok()) {
            if let Some(p) = t.pulls.pulls.as_deref().and_then(|p| p.list.iter().find(|q| q.number == n)).cloned() {
                t.checks_open(p, None);
            }
        }
        // FOURCHETTE_ISSUE=<n>[,progress|review|moved]: on a branch of issue #n, with its offer.
        if let Ok(v) = std::env::var("FOURCHETTE_ISSUE") {
            t.issue_sample(&v);
        }
        if full {
            let key = t.key;
            app.full.enter(key);
        }
        let t = app.tab_mut();
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
        // FOURCHETTE_SKIN=gamer: the messy gamer skin with litter all over (#202), `=carried`
        // with the vacuum in hand, sucking (not saved). FOURCHETTE_SKIN=runner: the Runner skin
        // at 2,300 m (#206), `=runner,split` at the 3,000 m split, `=runner,finish` finished.
        if let Some(v) = std::env::var("FOURCHETTE_SKIN").ok().filter(|v| v.starts_with("runner")) {
            app.library.settings.skin = crate::settings::Skin::Runner;
            app.race.sample(&v);
            app.coach_sample();
        } else if let Ok(v) = std::env::var("FOURCHETTE_SKIN") {
            app.library.settings.skin = crate::settings::Skin::Gamer;
            app.mess.sample = true;
            if v == "carried" {
                app.mess.carried = true;
                app.mess.cleaned = 7;
                app.mess.pin = Some(egui::pos2(640.0, 420.0));
            }
        }
        // FOURCHETTE_TOAST=<message> shows a toast, FOURCHETTE_TOAST=error:<message> an error's,
        // FOURCHETTE_TOAST=undo:<message> one that offers Cmd+Z, FOURCHETTE_TOAST=button:<message>
        // one with an Undo button (as after "Remove N missing").
        if let Ok(v) = std::env::var("FOURCHETTE_TOAST") {
            let (msg, error) = v.strip_prefix("error:").map_or((v.as_str(), false), |m| (m, true));
            let (msg, undo) = msg.strip_prefix("undo:").map_or((msg, false), |m| (m, true));
            let (msg, button) = msg.strip_prefix("button:").map_or((msg, false), |m| (m, true));
            app.notify(msg.to_string(), error);
            if let Some(t) = &mut app.toast {
                t.undo = undo;
                if button {
                    t.action = Some(("Undo", || Cmd::RepoPutBack));
                }
            }
        }
        // FOURCHETTE_CLONE=1 shows the clone dialog with sample repositories (`=cloning` with a
        // clone under way, `=error` after a failed one, `=signed-out` without `gh`).
        if let (Ok(v), Some(_)) = (std::env::var("FOURCHETTE_CLONE"), &app.screenshot) {
            app.clone_sample(&v);
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
        self.tab().dialog.is_some() || self.update.dialog || self.update.whats_new.is_some() || self.clone.is_some() || self.race.finisher
    }

    /// Opens `r` in the active tab, in place of the repository it showed.
    fn set_repo(&mut self, r: Repo) {
        let root = r.root.clone();
        self.tab_mut().set_repo(r);
        if self.screenshot.is_none() && !crate::library::is_temporary(&root, &self.temp_roots) {
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
        self.toast = Some(tab::Notice { msg: msg.into(), error, at: Instant::now(), undo: false, action: None, coach: None });
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
        self.course_strides();
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
                Cmd::CloneDialog => self.clone_open(ctx),
                Cmd::ClonePickFolder => self.clone_pick_folder(),
                Cmd::CloneOpen(p) => self.clone_done(p),
                Cmd::OpenTab(p) => self.open_path(p, true),
                Cmd::NewTab => {
                    self.switcher = None;
                    self.tabs.push(Tab::home());
                    self.activate(self.tabs.len() - 1);
                }
                Cmd::Home => self.show_home(),
                Cmd::HomeKeep { name, paths } => {
                    self.library.keep_group(&paths, &name);
                    self.library.save();
                    self.notify(format!("{} repositories moved to the group “{name}”", paths.len()), false);
                }
                Cmd::HomeDismiss(key) => {
                    self.library.dismiss(&key);
                    self.library.save();
                }
                Cmd::HomeSample => self.tab_mut().load_demo(),
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
                Cmd::SetSkin(skin) => {
                    self.library.settings.skin = skin;
                    self.library.save();
                }
                Cmd::Finisher { again } => self.finisher_done(again),
                Cmd::Settings => {
                    self.settings_open = !self.settings_open;
                    if self.settings_open {
                        self.palette = None;
                        self.switcher = None;
                    }
                }
                Cmd::SetSettings(s) => {
                    if (s.ui_font, s.code_font) != (self.library.settings.ui_font, self.library.settings.code_font) {
                        theme::set_fonts(ctx, s.ui_font, s.code_font, self.font_previews);
                    }
                    // Another channel: check on it at once, so the pill shows what it offers.
                    if s.update_channel != self.library.settings.update_channel {
                        self.update.check_now(s.update_channel);
                    }
                    if s.app_icon != self.library.settings.app_icon {
                        self.icon_chosen(s.app_icon);
                    }
                    self.library.settings = s;
                    self.library.save();
                }
                Cmd::PreviewIcon(c) => self.icon_preview(c),
                Cmd::SetFilesTree(on) => {
                    self.library.settings.files_tree = on;
                    self.library.save();
                }
                Cmd::SetDiffView { split, whole } => {
                    self.library.settings.diff_split = split;
                    self.library.settings.diff_whole = whole;
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
                Cmd::RepoRemoveMissing(paths) => self.remove_missing(&paths),
                Cmd::RepoPutBack => self.put_back(),
                Cmd::ScanFolder => self.scan_folder(),
                Cmd::FetchAllRepos => self.fetch_all_repos(ctx),
                Cmd::Reveal(p) => {
                    let _ = std::process::Command::new("open").arg("-R").arg(&p).spawn();
                }
                Cmd::OpenTerminal(p) => {
                    let _ = std::process::Command::new("open").args(["-a", "Terminal"]).arg(&p).spawn();
                }
                Cmd::OpenUrl(u) => ctx.open_url(egui::OpenUrl::new_tab(u)),
                Cmd::ProjectReload => self.project_load(),
                Cmd::ProjectPick(i) => self.project_pick(i),
                // The home page has no board.
                Cmd::Board if self.tab().home => {}
                Cmd::Board => self.board_toggle(),
                Cmd::BoardFull if self.tab().home => {}
                Cmd::BoardFull => self.board_full_toggle(),
                Cmd::IssueDetail(id) => {
                    if !self.tab().board.open {
                        self.board_toggle();
                    }
                    self.tab_mut().board.detail = Some(id);
                }
                Cmd::UpdateDialog(open) => self.update_dialog_open(open),
                Cmd::InstallUpdate => self.update_install(ctx),
                Cmd::UpdateCheckNow => self.update.check_now(self.library.settings.update_channel),
                Cmd::CloseWhatsNew => self.update.whats_new = None,
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

    /// "Remove N missing": takes these repositories off the list (nothing on disk), saves it once,
    /// and says so in a toast whose Undo puts them back.
    fn remove_missing(&mut self, paths: &[PathBuf]) {
        let removed = self.library.remove(paths);
        if removed.is_empty() {
            return;
        }
        self.library.save();
        let n = removed.len();
        self.removed = removed;
        self.notify(if n == 1 { "1 repository removed".to_string() } else { format!("{n} repositories removed") }, false);
        if let Some(t) = &mut self.toast {
            t.action = Some(("Undo", || Cmd::RepoPutBack));
        }
    }

    /// The Undo of "Remove N missing": the repositories it took off go back as they were.
    fn put_back(&mut self) {
        let removed = std::mem::take(&mut self.removed);
        if removed.is_empty() {
            return;
        }
        let n = removed.len();
        self.library.put_back(removed);
        self.library.save();
        self.notify(if n == 1 { "1 repository put back".to_string() } else { format!("{n} repositories put back") }, false);
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
            let local = crate::delete::Local { name: name.clone(), merged: false, force: false, worktree: wt, dirty: true, also_worktree: true };
            let mut remote = crate::delete::Remote::of(&format!("origin/{name}"));
            if let Some(r) = remote.as_mut() {
                r.open_pull = Some("#52 Search across branches".into());
            }
            let both = std::env::var("FOURCHETTE_DELETE").ok();
            let mut d = crate::delete::Delete::new(Some(local), remote, true, both.is_some());
            d.confirm = both.as_deref() == Some("confirm");
            Dialog::DeleteBranch(d)
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
