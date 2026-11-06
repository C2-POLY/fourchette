//! The worktree cockpit's window (#26): every worktree of the repository with its state, the
//! issue its branch name carries and its pull request, read in the background (see
//! crate::cockpit). Tidy up lists what would go and what stays, with the reason, and removes the
//! ticked worktrees and their local branches as one undo step.
use super::*;
use crate::cockpit::{self, Report, Verdict};
use crate::modal::{self, pill, text, Hint, Modal, Width};
use crate::pulls::{Pull, Pulls};
use crate::worktree::Worktree;
use egui::{vec2, Margin, RichText, Sense};
use std::path::Path;

pub enum CockpitMsg {
    Read(Report),
    Pulls(Result<Pulls, String>),
}

#[derive(Default)]
pub struct CockpitView {
    pub list: Vec<Worktree>,
    /// None while the worktrees are being read.
    pub report: Option<Report>,
    /// The pull requests of their branches, once GitHub answered; until then the repository's.
    pub pulls: Option<Pulls>,
    pub pulls_error: Option<String>,
    pub jobs: Vec<Receiver<CockpitMsg>>,
    /// The folder open in this window, which Tidy up never removes.
    pub here: PathBuf,
    /// "owner/name" on GitHub, for the issue links without a project.
    pub slug: Option<String>,
    /// Tidy up's list: each worktree that can go, ticked or not.
    pub tidy: Option<Vec<(PathBuf, bool)>>,
}

impl CockpitView {
    fn loading(&self) -> bool {
        !self.jobs.is_empty()
    }
}

impl Tab {
    /// Opens the cockpit and reads the worktrees. The sample project shows its sample.
    pub(super) fn cockpit_open(&mut self, ctx: &egui::Context) {
        if self.repo.is_none() {
            self.cockpit_sample(false);
            return;
        }
        self.cockpit = Some(CockpitView::default());
        self.cockpit_read(ctx);
    }

    /// Reads the worktrees' state, and the pull requests of their branches, in the background.
    fn cockpit_read(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.repo else { return };
        let list = r.worktrees();
        let root = r.root.clone();
        let remote = r.default_remote();
        let slug = remote.as_ref().and_then(|rem| r.git(&["remote", "get-url", rem]).ok()).and_then(|u| crate::github::slug(&u));
        self.worktrees = list.clone();
        let Some(view) = self.cockpit.as_mut() else { return };
        if view.loading() {
            return;
        }
        view.here = root.clone();
        view.slug = slug.clone();
        view.list = list.clone();
        view.tidy = None;
        let (tx, rx) = channel();
        {
            let (tx, ctx, list) = (tx.clone(), ctx.clone(), list.clone());
            std::thread::spawn(move || {
                let _ = tx.send(CockpitMsg::Read(cockpit::read(&Repo { root }, &list)));
                ctx.request_repaint();
            });
        }
        if let (Some(slug), Some(remote)) = (slug, remote) {
            let branches: Vec<String> = list.iter().filter_map(|w| w.branch.clone()).collect();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(CockpitMsg::Pulls(cockpit::load_pulls(&slug, &remote, &branches)));
                ctx.request_repaint();
            });
        }
        view.jobs.push(rx);
    }

    pub(super) fn cockpit_poll(&mut self) {
        let Some(view) = self.cockpit.as_mut() else { return };
        let mut msgs = Vec::new();
        view.jobs.retain(|rx| loop {
            match rx.try_recv() {
                Ok(m) => msgs.push(m),
                Err(std::sync::mpsc::TryRecvError::Empty) => break true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break false,
            }
        });
        for m in msgs {
            match m {
                CockpitMsg::Read(r) => view.report = Some(r),
                CockpitMsg::Pulls(Ok(p)) => {
                    view.pulls = Some(p);
                    view.pulls_error = None;
                }
                CockpitMsg::Pulls(Err(e)) => view.pulls_error = Some(e),
            }
        }
    }

    /// The pull request a branch shows: from the cockpit's own read, else the repository's.
    fn cockpit_pull<'a>(&'a self, view: &'a CockpitView, branch: &str) -> Option<&'a Pull> {
        match &view.pulls {
            Some(p) => p.for_branch(branch),
            None => self.pulls.pulls.as_deref()?.for_branch(branch),
        }
    }

    fn cockpit_verdicts(&self, view: &CockpitView) -> Vec<Verdict> {
        let empty = Report::default();
        let report = view.report.as_ref().unwrap_or(&empty);
        view.list
            .iter()
            .map(|w| {
                let pull = w.branch.as_deref().and_then(|b| self.cockpit_pull(view, b));
                let facts = view.report.as_ref().and_then(|r| r.facts.get(&w.path));
                cockpit::verdict(w, same_folder(&w.path, &view.here), facts, pull, report)
            })
            .collect()
    }

    /// Removes the ticked worktrees and their local branches, as one undo step.
    fn cockpit_tidy(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to tidy its worktrees.", true);
            return;
        };
        let mut snap = r.snapshot(&format!("Tidy up {}", plural(paths.len(), "worktree")), false, false);
        let done = cockpit::tidy(r, &paths);
        let n = done.removed.len();
        snap.worktrees.extend(done.removed);
        self.reload();
        if n > 0 {
            self.push_undo(snap);
        }
        match (n, done.errors.first()) {
            (0, Some(e)) => self.notify(e.clone(), true),
            (_, Some(e)) => self.notify(format!("Removed {} and {}, but {e}", plural(n, "worktree"), plural(done.branches.len(), "branch")), true),
            _ => self.notify(format!("Removed {} and {} — Undo (⌘Z) brings them back", plural(n, "worktree"), plural(done.branches.len(), "local branch")), false),
        }
        self.cockpit_read(ctx);
    }

    /// The cockpit window.
    pub(super) fn cockpit_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let Some(view) = self.cockpit.take() else { return };
        let mut view = view;
        let verdicts = self.cockpit_verdicts(&view);
        let keep = if view.tidy.is_some() { self.cockpit_tidy_ui(ctx, &mut view, &verdicts) } else { self.cockpit_list_ui(ctx, &mut view, &verdicts, cmds) };
        match keep {
            Next::Stay => self.cockpit = Some(view),
            Next::Close => {}
            Next::Reload => {
                view.report = None;
                self.cockpit = Some(view);
                self.cockpit_read(ctx);
            }
            Next::Prune => {
                self.cockpit = Some(view);
                self.run(Cmd::PruneWorktrees, ctx);
                self.cockpit_read(ctx);
            }
            Next::Tidy(paths) => {
                self.cockpit = Some(view);
                self.cockpit_tidy(ctx, paths);
            }
        }
    }

    fn cockpit_list_ui(&self, ctx: &egui::Context, view: &mut CockpitView, verdicts: &[Verdict], cmds: &mut Vec<Cmd>) -> Next {
        let k = theme::c();
        let loading = view.loading();
        let missing = view.list.iter().any(|w| w.missing);
        let can_go = verdicts.iter().filter(|v| v.goes()).count();
        let mut status = vec![text(plural(view.list.len(), "worktree"))];
        if let Some(b) = view.report.as_ref().and_then(|r| r.base.clone()) {
            status.extend([text("· merged into"), pill(b)]);
        }
        let mut frame = Modal::new("cockpit", Width::Large, "folder", vec![text("Worktrees")]).status(status).cancel(Some("Close")).extra("Reload");
        if missing {
            frame = frame.extra("Clean up missing");
        }
        let hint = if loading {
            "Reading the worktrees and their pull requests…".to_string()
        } else if can_go == 0 {
            "Nothing to tidy up: no worktree is merged and clean".to_string()
        } else {
            format!("{} merged and clean, ready to tidy up", plural(can_go, "worktree"))
        };
        frame = frame.hint(vec![Hint::Text(hint)]).primary("Tidy up…", false, view.report.is_some() && can_go > 0);
        let board = self.project.board.clone();
        let out = frame.show(ctx, |ui| {
            if let Some(e) = &view.pulls_error {
                modal::note(ui, k.modified, |ui| {
                    ui.label(RichText::new(format!("Pull requests not read: {e}")).color(k.text));
                });
                ui.add_space(8.0);
            }
            egui::ScrollArea::vertical().max_height(470.0).auto_shrink([false, true]).show(ui, |ui| {
                for (w, v) in view.list.iter().zip(verdicts) {
                    let facts = view.report.as_ref().and_then(|r| r.facts.get(&w.path));
                    let pull = w.branch.as_deref().and_then(|b| self.cockpit_pull(view, b));
                    row(ui, w, v, facts, pull, board.as_deref(), view.slug.as_deref(), view.report.is_none(), cmds);
                    ui.add_space(6.0);
                }
            });
        });
        if out.close {
            return Next::Close;
        }
        match out.extra {
            Some(0) if !loading => return Next::Reload,
            Some(1) => return Next::Prune,
            _ => {}
        }
        if out.primary && view.report.is_some() && can_go > 0 {
            view.tidy = Some(view.list.iter().zip(verdicts).filter(|(_, v)| v.goes()).map(|(w, _)| (w.path.clone(), true)).collect());
        }
        // A link to the board closes the cockpit, so the board shows.
        if cmds.iter().any(|c| matches!(c, Cmd::IssueDetail(_) | Cmd::OpenTab(_))) {
            return Next::Close;
        }
        Next::Stay
    }

    fn cockpit_tidy_ui(&self, ctx: &egui::Context, view: &mut CockpitView, verdicts: &[Verdict]) -> Next {
        let k = theme::c();
        let Some(ticks) = view.tidy.as_mut() else { return Next::Stay };
        let chosen = ticks.iter().filter(|t| t.1).count();
        let mut hint = modal::undo_hint();
        hint.push(Hint::Text("· remote branches are never deleted".into()));
        let frame = Modal::new("cockpit_tidy", Width::Large, "trash", vec![text("Tidy up worktrees")])
            .danger(true)
            .status(vec![text(format!("{} of {} can go", plural(ticks.len(), "worktree"), view.list.len()))])
            .cancel(Some("Back"))
            .hint(hint)
            .primary(if chosen == 0 { "Remove".to_string() } else { format!("Remove {}", plural(chosen, "worktree")) }, true, chosen > 0);
        // Enter is not enough here: removing takes a click.
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let out = frame.show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(480.0).auto_shrink([false, true]).show(ui, |ui| {
                modal::caption(ui, "Removed: the worktree's folder and its local branch");
                ui.add_space(4.0);
                for (path, on) in ticks.iter_mut() {
                    let Some((w, v)) = view.list.iter().zip(verdicts).find(|(w, _)| &w.path == path) else { continue };
                    let label = format!("{} · {}", w.name(), w.branch.clone().unwrap_or_default());
                    modal::check_row(ui, on, &label, Some(v.reason())).on_hover_text(w.path.display().to_string());
                }
                ui.add_space(12.0);
                modal::caption(ui, "Stays");
                ui.add_space(4.0);
                for (w, v) in view.list.iter().zip(verdicts).filter(|(_, v)| !v.goes()) {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(w.name()).color(k.text)).on_hover_text(w.path.display().to_string());
                        ui.label(RichText::new(v.reason()).size(theme::FS_SECONDARY).color(k.dim));
                    });
                }
            });
        });
        if out.close {
            view.tidy = None;
            return Next::Stay;
        }
        if out.primary && !enter && chosen > 0 {
            let paths = ticks.iter().filter(|t| t.1).map(|t| t.0.clone()).collect();
            view.tidy = None;
            return Next::Tidy(paths);
        }
        Next::Stay
    }

    /// The screenshot mode's cockpit (`FOURCHETTE_COCKPIT=1`, `=tidy` with Tidy up's list).
    pub(super) fn cockpit_sample(&mut self, tidy: bool) {
        let (list, report, pulls) = cockpit::sample();
        let mut view = CockpitView { list, report: Some(report), pulls: Some(pulls), slug: Some("example/sample-project".into()), ..Default::default() };
        if tidy {
            let verdicts = self.cockpit_verdicts(&view);
            view.tidy = Some(view.list.iter().zip(&verdicts).filter(|(_, v)| v.goes()).map(|(w, _)| (w.path.clone(), true)).collect());
        }
        self.cockpit = Some(view);
    }
}

/// What the window asks for after this frame.
enum Next {
    Stay,
    Close,
    Reload,
    /// Clean up missing: `git worktree prune`, then read again.
    Prune,
    Tidy(Vec<PathBuf>),
}

fn plural(n: usize, one: &str) -> String {
    let many = if one.ends_with("ch") { format!("{one}es") } else { format!("{one}s") };
    format!("{n} {}", if n == 1 { one.to_string() } else { many })
}

fn same_folder(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

/// "2 changes · 3 ahead, 1 behind · 2 hours ago".
fn facts_line(f: &cockpit::Facts) -> String {
    let mut parts = vec![if f.changes == 0 { "No changes".to_string() } else { plural(f.changes, "change") }];
    parts.push(match f.track {
        None => "no upstream".into(),
        Some((0, 0)) => "up to date".into(),
        Some((a, 0)) => format!("{a} ahead"),
        Some((0, b)) => format!("{b} behind"),
        Some((a, b)) => format!("{a} ahead, {b} behind"),
    });
    if let Some(t) = f.last_commit {
        parts.push(format!("last commit {}", git::relative_time(t)));
    }
    parts.join(" · ")
}

/// One worktree: its folder and branch with the actions, its state, then its issue and pull
/// request, and whether Tidy up would remove it.
#[allow(clippy::too_many_arguments)]
fn row(ui: &mut egui::Ui, w: &Worktree, v: &Verdict, facts: Option<&cockpit::Facts>, pull: Option<&Pull>, board: Option<&crate::github::Board>, slug: Option<&str>, reading: bool, cmds: &mut Vec<Cmd>) {
    let k = theme::c();
    egui::Frame::new().fill(k.bg).stroke(egui::Stroke::new(1.0_f32, k.border)).corner_radius(8).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(w.name()).strong().color(k.strong)).on_hover_text(w.path.display().to_string());
            match &w.branch {
                Some(b) => {
                    modal::pill_ui(ui, b, false, 0.0);
                }
                None => {
                    ui.label(RichText::new("detached").size(theme::FS_SECONDARY).color(k.dim));
                }
            }
            if w.is_main {
                ui.label(RichText::new("main folder").size(theme::FS_SECONDARY).color(k.faint));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if w.missing {
                    ui.label(RichText::new("Folder gone").color(k.deleted));
                    return;
                }
                if ui.small_button("Copy path").clicked() {
                    cmds.push(Cmd::Copy(w.path.display().to_string()));
                }
                if ui.small_button("Terminal").on_hover_text("Open a terminal in this folder").clicked() {
                    cmds.push(Cmd::OpenTerminal(w.path.clone()));
                }
                if ui.small_button("Finder").on_hover_text("Show this folder in the Finder").clicked() {
                    cmds.push(Cmd::Reveal(w.path.clone()));
                }
                if ui.small_button("Open").on_hover_text("Open as a repository, in a new tab").clicked() {
                    cmds.push(Cmd::OpenTab(w.path.clone()));
                }
            });
        });
        let line = match (facts, w.missing) {
            (_, true) => "Its folder no longer exists: Clean up missing removes it from Git's list".to_string(),
            (Some(f), _) => facts_line(f),
            (None, _) if reading => "Reading…".to_string(),
            (None, _) => String::new(),
        };
        let dirty = facts.is_some_and(|f| f.changes > 0);
        ui.label(RichText::new(line).size(theme::FS_SECONDARY).color(if dirty { k.modified } else { k.dim }));
        ui.horizontal(|ui| {
            if let Some(b) = &w.branch {
                issue_link(ui, b, board, slug, cmds);
                if let Some(p) = pull {
                    let (rect, _) = ui.allocate_exact_size(vec2(crate::ui::pr_badge_width(p), 14.0), Sense::hover());
                    crate::ui::pr_badge(ui.painter(), rect.left(), rect.center().y, p);
                    let r = ui.add(egui::Label::new(RichText::new(format!("#{} {}", p.number, p.state.label())).size(theme::FS_SECONDARY).color(k.text)).sense(Sense::click()));
                    if r.on_hover_text(format!("{}\nClick to open it on GitHub", p.describe())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        cmds.push(Cmd::OpenUrl(p.url.clone()));
                    }
                } else if !w.is_main {
                    ui.label(RichText::new("No pull request").size(theme::FS_SECONDARY).color(k.faint));
                }
            }
            if !w.is_main {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let col = if v.goes() { k.added } else { k.faint };
                    let tag = if v.goes() { format!("Can go: {}", v.reason()) } else { v.reason().to_string() };
                    ui.label(RichText::new(tag).size(theme::FS_SMALL).color(col));
                });
            }
        });
    });
}

/// The issue the branch name carries: on the project, `#n`, its title and status (a click opens
/// its detail on the board, ⌘-click on GitHub); without the project, `#n` linked to GitHub.
fn issue_link(ui: &mut egui::Ui, branch: &str, board: Option<&crate::github::Board>, slug: Option<&str>, cmds: &mut Vec<Cmd>) {
    let k = theme::c();
    if let Some(it) = board.and_then(|b| super::issue::linked_issue(b, branch)) {
        let title: String = if it.title.chars().count() > 46 { format!("{}…", it.title.chars().take(45).collect::<String>()) } else { it.title.clone() };
        let r = ui.add(egui::Label::new(RichText::new(format!("#{} {title}", it.number.unwrap_or(0))).size(theme::FS_SECONDARY).color(k.accent_text)).sense(Sense::click()));
        let r = r.on_hover_text(format!("{}\nClick: its detail on the board · ⌘-click: on GitHub", it.title)).on_hover_cursor(egui::CursorIcon::PointingHand);
        if r.clicked() {
            if ui.input(|i| i.modifiers.command) {
                cmds.push(Cmd::OpenUrl(it.url.clone()));
            } else {
                cmds.push(Cmd::IssueDetail(it.id.clone()));
            }
        }
        if !it.status.is_empty() {
            let col = board.map(|b| super::project::status_color(b, &it.status)).unwrap_or(k.dim);
            ui.label(RichText::new(&it.status).size(theme::FS_SMALL).color(col));
        }
        ui.add_space(8.0);
    } else if let (Some(n), Some(slug)) = (crate::pr_draft::issue_number(branch), slug) {
        let r = ui.add(egui::Label::new(RichText::new(format!("#{n}")).size(theme::FS_SECONDARY).color(k.accent_text)).sense(Sense::click()));
        if r.on_hover_text("The issue of this number, on GitHub").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            cmds.push(Cmd::OpenUrl(format!("https://github.com/{slug}/issues/{n}")));
        }
        ui.add_space(8.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_read_as_a_line() {
        let f = |changes, track| cockpit::Facts { changes, track, last_commit: None };
        assert_eq!(facts_line(&f(0, Some((0, 0)))), "No changes · up to date");
        assert_eq!(facts_line(&f(1, Some((2, 0)))), "1 change · 2 ahead");
        assert_eq!(facts_line(&f(3, Some((2, 1)))), "3 changes · 2 ahead, 1 behind");
        assert_eq!(facts_line(&f(0, None)), "No changes · no upstream");
        assert_eq!(plural(2, "branch"), "2 branches");
        assert_eq!(plural(1, "worktree"), "1 worktree");
    }
}
