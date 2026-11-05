//! The banner over the history while a merge, rebase, cherry-pick or revert is stopped on
//! conflicts, its Abort, Skip and Continue, and the Abort confirmation (see crate::operation).
use super::*;
use crate::modal::{self, text, Kind, Modal, Width};
use crate::operation::{self, Action};
use egui::{vec2, Align, CornerRadius, Layout, Margin, RichText, Sense, Stroke, StrokeKind, Ui};

/// How many conflicted files the banner names before "and n more".
const SHOWN_FILES: usize = 6;

impl Tab {
    /// Reads the operation in progress again (after a reload or a status refresh). The sample
    /// project keeps the one `FOURCHETTE_OPERATION` gave it.
    pub(super) fn read_operation(&mut self) {
        if let Some(r) = &self.repo {
            self.operation = r.in_progress();
        }
    }

    /// Abort, Continue or Skip, as one Undo step that puts the operation back as it was.
    pub(super) fn operation_run(&mut self, action: Action) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to do that.", true);
            return;
        };
        if self.busy.is_some() {
            self.notify("Please wait for the current operation to finish.", true);
            return;
        }
        let (snap, res) = r.operation_step(action);
        self.reload();
        let undoable = snap.is_some();
        if let Some(s) = snap {
            self.push_undo(s);
        }
        match res {
            Ok(msg) => self.notify(msg, false),
            Err(e) => self.notify(e, true),
        }
        if let Some(n) = &mut self.notice {
            n.undo = undoable;
        }
    }

    /// The banner: what is in progress, its conflicts or "All conflicts resolved", and the buttons.
    pub(super) fn operation_banner(&self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let Some(p) = &self.operation else { return };
        let k = theme::c();
        egui::Frame::new()
            .fill(theme::tint(k.modified, k.bg, 0.1))
            .stroke(Stroke::new(1.0_f32, theme::tint(k.modified, k.bg, 0.3)))
            .inner_margin(Margin { left: 38, right: 12, top: 8, bottom: 8 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let top = ui.min_rect().left_top();
                crate::ui::glyph(ui.painter(), top + vec2(-19.0, 15.0), "warning", 16.0, 1.5, k.modified);
                ui.horizontal(|ui| {
                    ui.set_min_height(30.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let tip = if p.resolved() { format!("Finish the {}", operation::noun(&p.op)) } else { "Resolve and stage every conflicted file first".into() };
                        if modal::button(ui, "Continue", Kind::Primary, p.resolved()).on_hover_text(tip).clicked() {
                            cmds.push(Cmd::Operation(Action::Continue));
                        }
                        if p.can_skip() && modal::button(ui, "Skip", Kind::Plain, true).on_hover_text("Leave out the commit it stopped on").clicked() {
                            cmds.push(Cmd::Operation(Action::Skip));
                        }
                        if !p.resolved() && modal::button(ui, "Resolve with Claude…", Kind::Plain, true).on_hover_text("Claude proposes a resolution for each conflicted file").clicked() {
                            cmds.push(Cmd::AiResolve(Vec::new()));
                        }
                        if modal::button(ui, "Abort", Kind::Plain, true).on_hover_text(format!("Go back to before the {}", operation::noun(&p.op))).clicked() {
                            cmds.push(Cmd::Dialog(Dialog::Abort { op: p.op.clone() }));
                        }
                        ui.add_space(4.0);
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.label(RichText::new(p.title()).font(theme::body()).strong().color(k.strong));
                            if let Some((i, n)) = p.step {
                                ui.label(RichText::new(format!("{i}/{n}")).size(theme::FS_SECONDARY).color(k.dim)).on_hover_text(format!("Step {i} of {n}"));
                            }
                            if !p.what.is_empty() {
                                ui.add(egui::Label::new(RichText::new(&p.what).color(k.dim)).truncate());
                            }
                        });
                    });
                });
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                    if p.resolved() {
                        let (r, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
                        crate::ui::check_mark(ui.painter(), r.center(), k.added);
                        ui.label(RichText::new("All conflicts resolved").color(k.added));
                        ui.label(RichText::new(format!("— Continue finishes the {}.", operation::noun(&p.op))).color(k.dim));
                        return;
                    }
                    ui.label(RichText::new("Conflicts in").color(k.text));
                    for f in p.conflicts.iter().take(SHOWN_FILES) {
                        if file_chip(ui, f).on_hover_text("Show its changes").clicked() {
                            cmds.push(Cmd::Select(Sel::Wip));
                            cmds.push(Cmd::Diff(DiffReq::Worktree { path: f.clone(), staged: false, untracked: false }));
                        }
                    }
                    if p.conflicts.len() > SHOWN_FILES {
                        ui.label(RichText::new(format!("and {} more", p.conflicts.len() - SHOWN_FILES)).color(k.dim));
                    }
                    ui.label(RichText::new("— resolve each file, then stage it.").color(k.dim));
                });
            });
    }
}

/// A conflicted file's name in a small rounded box, clickable.
fn file_chip(ui: &mut Ui, path: &str) -> egui::Response {
    let k = theme::c();
    let g = ui.painter().layout_no_wrap(path.to_string(), theme::mono(), k.text);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 20.0), Sense::click());
    let hot = resp.hovered();
    let fill = if hot { k.widget_active } else { k.widget_hover };
    ui.painter().rect(rect, CornerRadius::same(5), fill, Stroke::new(1.0_f32, theme::tint(k.deleted, fill, 0.45)), StrokeKind::Inside);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, k.text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The Abort confirmation (Fork: "Do you want to abort the merge?"), red. Returns true to close.
pub(super) fn abort_dialog(ctx: &egui::Context, op: &str, cmds: &mut Vec<Cmd>) -> bool {
    let n = operation::noun(op);
    let title = format!("Abort {n}");
    let out = Modal::new("abort_operation", Width::Small, "undo", vec![text(title.as_str())])
        .danger(true)
        .status(vec![text(operation::abort_question(op))])
        .hint(modal::undo_hint())
        .primary(title.as_str(), true, true)
        .show(ctx, |ui| {
            let k = theme::c();
            let before = if op == "merge" { "The branch goes back to how it was before the merge" } else { "The branch goes back to how it was before it started" };
            ui.label(RichText::new(format!("{before}, and the conflicts you resolved are dropped.")).color(k.text));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("Undo (⌘Z) brings the {n} back as you left it.")).size(theme::FS_SECONDARY).color(k.dim));
        });
    if out.primary {
        cmds.push(Cmd::Operation(Action::Abort));
    }
    out.primary || out.close
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_follows_the_repository_and_cmd_z_takes_an_abort_back() {
        let (dir, repo) = crate::operation::tests::fixture("tab");
        assert!(repo.git(&["merge", "topic"]).is_err());
        let mut t = Tab::new();
        t.set_repo(repo);
        let p = t.operation.clone().expect("the banner shows");
        assert_eq!((p.op.as_str(), p.conflicts.clone()), ("merge", vec!["f.txt".to_string()]));
        let ctx = egui::Context::default();

        t.run(Cmd::Operation(Action::Abort), &ctx);
        assert!(t.operation.is_none(), "the banner goes");
        let n = t.notice.clone().unwrap();
        assert!(n.msg == "Aborted the merge" && !n.error && n.undo, "{n:?}");
        assert_eq!(t.undo_label(), Some("Abort merge"));

        t.run(Cmd::Undo, &ctx);
        assert_eq!(t.notice.as_ref().map(|n| n.msg.as_str()), Some("Undid: Abort merge"));
        assert_eq!(t.operation.as_ref().map(|p| p.conflicts.clone()), Some(vec!["f.txt".to_string()]), "back with its conflict");

        // Staging the resolution: "All conflicts resolved", then Continue.
        std::fs::write(dir.join("f.txt"), "1\nBOTH\n3\n").unwrap();
        t.run(Cmd::Git { label: "Staged f.txt".into(), args: vec!["add".into(), "--".into(), "f.txt".into()], network: false }, &ctx);
        assert!(t.operation.as_ref().unwrap().resolved());
        t.run(Cmd::Operation(Action::Continue), &ctx);
        assert!(t.operation.is_none());
        assert_eq!(t.notice.as_ref().map(|n| n.msg.as_str()), Some("Merge finished"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
