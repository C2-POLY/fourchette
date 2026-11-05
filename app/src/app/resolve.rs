//! The conflict window: Claude proposes a resolution for each conflicted file (see
//! crate::resolve), a few at a time in the background, and the person applies them one by one.
use super::*;
use crate::modal::{self, pill, text, Modal, Width};
use crate::resolve::{self, Conflict, Proposal};
use egui::{CornerRadius, Margin, RichText, Stroke};
use std::sync::{Arc, Mutex};

pub enum FileState {
    /// Claude is on it.
    Waiting,
    Ready(Proposal),
    Failed(String),
    /// Written ("applied") or written and staged ("marked resolved").
    Done { resolved: bool },
}

pub struct ResolveView {
    /// The operation stopped on conflicts ("merge", "rebase"…).
    pub op: String,
    pub files: Vec<(String, FileState)>,
    pub current: usize,
    pub rx: Option<Receiver<(String, Result<Proposal, String>)>>,
}

impl ResolveView {
    fn waiting(&self) -> usize {
        self.files.iter().filter(|(_, s)| matches!(s, FileState::Waiting)).count()
    }
}

/// Files Git still has in conflict, as the status lists them.
pub fn conflicted(status: &Status) -> Vec<String> {
    status.unstaged.iter().filter(|f| f.code == 'U').map(|f| f.path.clone()).collect()
}

impl Tab {
    /// Opens the window on `paths` (all the conflicted files when empty) and asks Claude for each.
    pub(super) fn resolve_open(&mut self, paths: Vec<String>, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories first.", true);
            return;
        };
        let all = r.conflicted_files();
        let paths: Vec<String> = if paths.is_empty() { all } else { paths.into_iter().filter(|p| all.contains(p)).collect() };
        if paths.is_empty() {
            self.notify("There is no conflict to resolve.", false);
            return;
        }
        let op = r.operation().unwrap_or_else(|| "operation".into());
        // What Claude can't help with is said at once; the rest goes to the workers.
        let mut files = Vec::new();
        let mut queue = Vec::new();
        for p in &paths {
            match resolve::read(r, p) {
                Ok(c) => {
                    files.push((p.clone(), FileState::Waiting));
                    queue.push(c);
                }
                Err(e) => files.push((p.clone(), FileState::Failed(e))),
            }
        }
        let (tx, rx) = channel();
        let queue: Arc<Mutex<Vec<Conflict>>> = Arc::new(Mutex::new(queue.into_iter().rev().collect()));
        let model = self.claude_model;
        for _ in 0..resolve::WORKERS {
            let (tx, queue, ctx) = (tx.clone(), queue.clone(), ctx.clone());
            std::thread::spawn(move || loop {
                let next = queue.lock().ok().and_then(|mut q| q.pop());
                let Some(c) = next else { break };
                let _ = tx.send((c.path.clone(), resolve::propose(&c, model)));
                ctx.request_repaint();
            });
        }
        self.resolve = Some(ResolveView { op, files, current: 0, rx: Some(rx) });
    }

    pub(super) fn resolve_poll(&mut self) {
        let Some(view) = self.resolve.as_mut() else { return };
        let Some(rx) = &view.rx else { return };
        while let Ok((path, res)) = rx.try_recv() {
            if let Some((_, s)) = view.files.iter_mut().find(|(p, _)| *p == path) {
                *s = match res {
                    Ok(p) => FileState::Ready(p),
                    Err(e) => FileState::Failed(e),
                };
            }
        }
        if view.waiting() == 0 {
            view.rx = None;
        }
    }

    /// Writes the proposal of the current file, and with `resolved` stages it. One Undo step.
    fn resolve_apply(&mut self, resolved: bool) {
        let Some(view) = self.resolve.as_mut() else { return };
        let Some((path, FileState::Ready(p))) = view.files.get(view.current) else { return };
        let (path, text) = (path.clone(), p.text.clone());
        let Some(r) = &self.repo else { return };
        match resolve::apply(r, &path, &text, resolved) {
            Ok(snap) => {
                self.push_undo(snap);
                let view = self.resolve.as_mut().expect("open");
                view.files[view.current].1 = FileState::Done { resolved };
                // On to the next file that has a proposal waiting.
                if let Some(i) = view.files.iter().position(|(_, s)| matches!(s, FileState::Ready(_))) {
                    view.current = i;
                }
                self.reload();
                let what = if resolved { "marked resolved" } else { "written — stage it once checked" };
                self.notify(format!("{path} {what}. Undo (⌘Z) puts the conflict back"), false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    pub(super) fn resolve_window(&mut self, ctx: &egui::Context) {
        let Some(view) = self.resolve.as_mut() else { return };
        let k = theme::c();
        let n = view.files.len();
        let ready = matches!(view.files.get(view.current), Some((_, FileState::Ready(_))));
        let status = vec![text("the"), pill(view.op.as_str()), text(format!("stopped on {n} conflicted file{}", if n == 1 { "" } else { "s" }))];
        let out = Modal::new("resolve", Width::Large, "ai", vec![text("Resolve conflicts with Claude")])
            .status(status)
            .hint(modal::undo_hint())
            .cancel(Some("Close"))
            .extra("Apply")
            .primary("Apply and mark resolved", false, ready)
            .show(ctx, |ui| {
                // One tab per file, with its state.
                if n > 1 {
                    ui.horizontal_wrapped(|ui| {
                        for (i, (path, s)) in view.files.iter().enumerate() {
                            // The file's state as a drawn mark: a ring while Claude works, a dot
                            // when a proposal waits, a check once applied, red when it failed.
                            let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            let c = r.center();
                            match s {
                                FileState::Waiting => {
                                    ui.painter().circle_stroke(c, 4.0, Stroke::new(1.5_f32, k.faint));
                                }
                                FileState::Ready(_) => {
                                    ui.painter().circle_filled(c, 4.0, k.accent);
                                }
                                FileState::Failed(_) => {
                                    ui.painter().circle_filled(c, 4.0, k.deleted);
                                }
                                FileState::Done { .. } => crate::ui::check_mark(ui.painter(), c, k.added),
                            }
                            let name = path.rsplit('/').next().unwrap_or(path);
                            let label = RichText::new(name).color(if i == view.current { k.strong } else { k.dim });
                            if ui.selectable_label(i == view.current, label).on_hover_text(path.as_str()).clicked() {
                                view.current = i;
                            }
                            ui.add_space(6.0);
                        }
                    });
                    ui.add_space(8.0);
                }
                let Some((path, state)) = view.files.get(view.current) else { return };
                ui.label(RichText::new(path).font(theme::mono()).color(k.strong));
                ui.add_space(6.0);
                match state {
                    FileState::Waiting => {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.add_space(8.0);
                            ui.label(RichText::new("Claude is reading both sides of this file…").color(k.dim));
                            ui.label(RichText::new("Nothing is written until you apply its proposal.").size(theme::FS_SECONDARY).color(k.faint));
                        });
                        ui.add_space(30.0);
                    }
                    FileState::Failed(e) => {
                        modal::note(ui, k.deleted, |ui| {
                            ui.label(RichText::new(e).color(k.text));
                        });
                    }
                    FileState::Done { resolved } => {
                        let msg = if *resolved { "Applied and marked resolved." } else { "Applied. Stage the file once you've checked it, to mark it resolved." };
                        ui.label(RichText::new(msg).color(k.added));
                    }
                    FileState::Ready(p) => {
                        if !p.explanation.is_empty() {
                            egui::Frame::new().fill(k.accent_soft).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(12)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(RichText::new(&p.explanation).color(k.text));
                            });
                            ui.add_space(8.0);
                        }
                        patch_view(ui, &p.patch);
                    }
                }
            });
        if out.close {
            self.resolve = None;
            return;
        }
        if out.primary && ready {
            self.resolve_apply(true);
        } else if out.extra == Some(0) && ready {
            self.resolve_apply(false);
        }
    }

    /// The screenshot mode's window, with the sample project's conflicts.
    pub(super) fn resolve_sample(&mut self) {
        let files = resolve::sample()
            .into_iter()
            .map(|(p, s)| {
                let s = match s {
                    None => FileState::Waiting,
                    Some(Ok(p)) => FileState::Ready(p),
                    Some(Err(e)) => FileState::Failed(e),
                };
                (p, s)
            })
            .collect();
        self.resolve = Some(ResolveView { op: "merge".into(), files, current: 0, rx: None });
    }
}

/// Claude's proposal against the conflicted file: removed lines in red, added ones in green.
fn patch_view(ui: &mut egui::Ui, patch: &str) {
    let k = theme::c();
    egui::Frame::new().fill(k.input).stroke(Stroke::new(1.0_f32, k.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::both().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            if patch.trim().is_empty() {
                ui.label(RichText::new("Claude's proposal is the file as it is.").color(k.faint));
            }
            for l in patch.lines() {
                let (col, bg) = if l.starts_with("@@") {
                    (k.faint, None)
                } else if l.starts_with('+') {
                    (k.added, Some(theme::tint(k.added, k.input, 0.12)))
                } else if l.starts_with('-') {
                    (k.deleted, Some(theme::tint(k.deleted, k.input, 0.12)))
                } else {
                    (k.text, None)
                };
                let label = ui.add(egui::Label::new(RichText::new(l).font(theme::mono()).color(col)).extend());
                if let Some(bg) = bg {
                    let r = label.rect.with_min_x(ui.min_rect().left()).with_max_x(ui.max_rect().right());
                    ui.painter().rect_filled(r, CornerRadius::ZERO, bg);
                    ui.painter().text(label.rect.left_top(), egui::Align2::LEFT_TOP, l, theme::mono(), col);
                }
            }
        });
    });
}
