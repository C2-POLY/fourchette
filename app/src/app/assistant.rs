//! The AI assistant inside the app: branch clean-up, commit message suggestion and commit
//! explanation, answered by Claude through the person's subscription (see crate::ai).
use super::*;
use crate::ai::{self, Advice, BranchFacts};
use egui::{vec2, Align, CornerRadius, Layout, Margin, RichText, Stroke};
use serde_json::Value;

pub enum AiJob {
    Cleanup { base: String, facts: Vec<BranchFacts> },
    CommitMessage,
    Explain { hash: String },
}

pub struct CleanupView {
    pub base: String,
    pub advice: Vec<Advice>,
    pub summary: String,
    /// Still waiting for Claude.
    pub loading: bool,
    pub count: usize,
}

#[derive(Default)]
pub struct AiState {
    pub job: Option<(AiJob, Receiver<Result<Value, String>>)>,
    pub cleanup: Option<CleanupView>,
    /// (commit hash, explanation)
    pub explanation: Option<(String, String)>,
}

impl AiState {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
}

impl Tab {
    fn ai_spawn(&mut self, job: AiJob, prompt: String, schema: Option<Value>, ctx: &egui::Context) {
        if self.ai.busy() {
            self.notify("Claude is still working on the last question.", true);
            return;
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(ai::ask(&prompt, ai::SYSTEM, schema.as_ref()));
            ctx.request_repaint();
        });
        self.ai.job = Some((job, rx));
    }

    pub(super) fn ai_cleanup(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to clean up its branches.", true);
            return;
        };
        let Some(base) = ai::base_branch(r) else {
            self.notify("Couldn't find the main branch of this repository.", true);
            return;
        };
        let facts = ai::branch_facts(r, &base);
        if facts.is_empty() {
            self.notify(format!("Nothing to clean up: there are no other branches besides {base}."), false);
            return;
        }
        let prompt = ai::cleanup_prompt(&base, &facts);
        self.ai.cleanup = Some(CleanupView { base: base.clone(), advice: vec![], summary: String::new(), loading: true, count: facts.len() });
        self.ai_spawn(AiJob::Cleanup { base, facts }, prompt, Some(ai::cleanup_schema()), ctx);
    }

    pub(super) fn ai_commit_message(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories first.", true);
            return;
        };
        // When amending, the original describes the staged changes together with the last
        // commit: `git diff --cached HEAD^` (sub_1003e4338).
        let diff = if self.amend {
            r.git(&["diff", "--cached", "-M", "HEAD^"]).unwrap_or_default()
        } else {
            r.git(&["diff", "--cached", "-M"]).unwrap_or_default()
        };
        if diff.trim().is_empty() {
            self.notify("Stage some changes first, then Claude can describe them.", true);
            return;
        }
        let prompt = ai::commit_message_prompt(&diff, self.amend, self.subject_limits.0, self.subject_limits.1, ai::PAGE_GUIDE_COLUMN);
        self.ai_spawn(AiJob::CommitMessage, prompt, None, ctx);
    }

    pub(super) fn ai_explain(&mut self, hash: String, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories first.", true);
            return;
        };
        let d = r.details(&hash).unwrap_or_default();
        let diff = r.git(&["show", "--format=", "--stat", "--patch", "-M", "--first-parent", &hash]).unwrap_or_default();
        let prompt = ai::explain_prompt(&d.subject, &d.body, &diff);
        self.ai.explanation = Some((hash.clone(), String::new()));
        self.ai_spawn(AiJob::Explain { hash }, prompt, None, ctx);
    }

    /// Delete the ticked branches — one Undo step brings them all back.
    pub(super) fn ai_delete_selected(&mut self) {
        let Some(view) = &self.ai.cleanup else { return };
        let chosen: Vec<Advice> = view.advice.iter().filter(|a| a.selected).cloned().collect();
        let Some(r) = &self.repo else { return };
        if chosen.is_empty() {
            return;
        }
        let mut snap = r.snapshot(&format!("Clean up {} branch{}", chosen.len(), if chosen.len() == 1 { "" } else { "es" }), false, false);
        let mut failed = Vec::new();
        for a in &chosen {
            // Its worktree goes first; merged branches use the safe delete, others were ticked
            // by the person on purpose.
            let (removed, res) = crate::worktree::delete_branch(r, &a.name, !a.merged, true);
            snap.worktrees.extend(removed);
            if let Err(e) = res {
                failed.push(format!("{}: {e}", a.name));
            }
        }
        let removed_worktrees = snap.worktrees.len();
        self.reload();
        if removed_worktrees > 0 {
            self.push_undo(snap);
        } else {
            self.record(Some(snap));
        }
        self.ai.cleanup = None;
        let done = chosen.len() - failed.len();
        let wt_note = match removed_worktrees {
            0 => String::new(),
            1 => " and 1 worktree".to_string(),
            n => format!(" and {n} worktrees"),
        };
        if failed.is_empty() {
            self.notify(format!("Deleted {done} branch{}{wt_note} — Undo (⌘Z) brings them back", if done == 1 { "" } else { "es" }), false);
        } else {
            self.notify(format!("Deleted {done}; couldn't delete {}", failed.join(", ")), true);
        }
    }

    pub(super) fn ai_poll(&mut self) {
        let done = match &self.ai.job {
            Some((_, rx)) => rx.try_recv().ok(),
            None => None,
        };
        let Some(res) = done else { return };
        let Some((job, _)) = self.ai.job.take() else { return };
        match (job, res) {
            (AiJob::Cleanup { base, facts }, Ok(v)) => {
                // Only if the window is still open (the person may have cancelled).
                if self.ai.cleanup.is_some() {
                    let (advice, summary) = ai::read_advice(&v, &facts);
                    self.ai.cleanup = Some(CleanupView { base, count: facts.len(), advice, summary, loading: false });
                }
            }
            (AiJob::Cleanup { .. }, Err(e)) => {
                self.ai.cleanup = None;
                self.notify(e, true);
            }
            (AiJob::CommitMessage, Ok(v)) => {
                let (subject, body) = ai::split_message(v.as_str().unwrap_or(""));
                self.summary = subject;
                self.description = body;
                self.notify("Claude suggested a commit message — edit it as you like", false);
            }
            (AiJob::Explain { hash }, Ok(v)) => {
                self.ai.explanation = Some((hash, v.as_str().unwrap_or("").trim().to_string()));
            }
            (AiJob::Explain { .. }, Err(e)) => {
                self.ai.explanation = None;
                self.notify(e, true);
            }
            (_, Err(e)) => self.notify(e, true),
        }
    }

    /// The clean-up review window.
    pub(super) fn cleanup_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let mut close = false;
        let Some(view) = self.ai.cleanup.as_mut() else { return };
        egui::Modal::new(egui::Id::new("ai_cleanup")).show(ctx, |ui| {
            ui.set_width(620.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("✨").size(theme::FS_TITLE).color(theme::c().accent_text));
                ui.label(RichText::new("Branch clean-up").size(theme::FS_TITLE).color(theme::c().strong));
                ui.label(RichText::new(format!("compared with {}", view.base)).color(theme::c().dim));
            });
            ui.add_space(6.0);
            if view.loading {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("Claude is looking at {} branch{}…", view.count, if view.count == 1 { "" } else { "es" })).color(theme::c().dim));
                    ui.label(RichText::new("It only reads the list and gives advice — nothing is deleted until you choose.").small().color(theme::c().faint));
                });
                ui.add_space(20.0);
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                return;
            }
            if !view.summary.is_empty() {
                egui::Frame::new().fill(theme::c().accent_soft).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&view.summary).color(theme::c().text));
                });
                ui.add_space(8.0);
            }
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                for a in view.advice.iter_mut() {
                    let (col, tag) = match a.action.as_str() {
                        "delete" => (theme::c().deleted, "Delete"),
                        "keep" => (theme::c().added, "Keep"),
                        _ => (theme::c().modified, "Review"),
                    };
                    egui::Frame::new().fill(theme::c().input).corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(10, 8)).stroke(Stroke::new(1.0_f32, if a.selected { theme::tint(col, theme::c().input, 0.6) } else { theme::c().border })).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut a.selected, "");
                            ui.label(RichText::new(&a.name).color(theme::c().strong));
                            if !a.merged {
                                ui.label(RichText::new("has unmerged work").small().color(theme::c().modified));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                egui::Frame::new().fill(theme::tint(col, theme::c().input, 0.16)).corner_radius(CornerRadius::same(4)).inner_margin(Margin::symmetric(6, 2)).show(ui, |ui| {
                                    ui.label(RichText::new(tag).size(theme::FS_SMALL).color(col));
                                });
                            });
                        });
                        ui.label(RichText::new(&a.reason).color(theme::c().dim));
                        if let Some(w) = &a.worktree {
                            let (text, col) = if w.missing {
                                (format!("⚠ Its worktree folder “{}” is gone — it will be cleaned up first", w.name()), theme::c().faint)
                            } else if a.worktree_dirty {
                                (format!("⚠ Checked out in worktree “{}”, which has uncommitted changes. It will be removed first; its changes are kept for Undo.", w.name()), theme::c().modified)
                            } else {
                                (format!("⚠ Checked out in worktree “{}” — it will be removed first", w.name()), theme::c().accent_text)
                            };
                            ui.label(RichText::new(text).small().color(col)).on_hover_text(w.path.display().to_string());
                        }
                    });
                    ui.add_space(4.0);
                }
            });
            ui.add_space(8.0);
            let n = view.advice.iter().filter(|a| a.selected).count();
            ui.horizontal(|ui| {
                let label = if n == 0 { "Tick the branches to delete".to_string() } else { format!("Delete {n} branch{}", if n == 1 { "" } else { "es" }) };
                if ui.add_enabled(n > 0, egui::Button::new(RichText::new(label).color(theme::c().on_accent)).fill(theme::tint(theme::c().deleted, theme::c().elevated, 0.8)).stroke(Stroke::new(1.0_f32, theme::tint(theme::c().deleted, theme::c().elevated, 0.8))).min_size(vec2(0.0, 30.0))).clicked() {
                    cmds.push(Cmd::AiDeleteSelected);
                }
                if ui.button("Close").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
                ui.label(RichText::new("You can take it back with Undo (⌘Z).").small().color(theme::c().faint));
            });
        });
        if close {
            self.ai.cleanup = None;
        }
    }
}
