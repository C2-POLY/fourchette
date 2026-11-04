//! The GitHub project linked to the repository, in the sidebar (read only).
use super::*;
use crate::github::{self, Board, Item, Kind};
use crate::ui::{section_header, side_item};
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Rect, RichText, Sense, Ui};
use std::sync::mpsc::Receiver;

#[derive(Default)]
pub struct ProjectState {
    pub board: Option<Board>,
    pub error: Option<String>,
    pub loading: Option<Receiver<Result<Board, String>>>,
    pub mine_only: bool,
    /// Columns opened or folded by hand (finished ones start folded).
    pub flipped: HashSet<String>,
}

fn status_color(status: &str) -> Color32 {
    let s = status.to_lowercase();
    if github::is_done(status) {
        theme::ADDED
    } else if s.contains("review") {
        theme::RENAMED
    } else if s.contains("progress") || s.contains("doing") || s.contains("cours") {
        theme::ACCENT
    } else if s.contains("ready") || s.contains("todo") || s.contains("to do") {
        theme::MODIFIED
    } else if s.contains("block") {
        theme::DELETED
    } else {
        theme::DIM
    }
}

fn item_row(ui: &mut Ui, it: &Item, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::HOVER);
    }
    let y = rect.center().y;
    let mut x = rect.left() + 24.0;
    // Drawn rather than typed: the app's font has no issue / pull request symbols.
    let c = pos2(x + 4.0, y);
    match it.kind {
        Kind::Issue => {
            p.circle_stroke(c, 4.5, egui::Stroke::new(1.5, color));
            p.circle_filled(c, 1.5, color);
        }
        Kind::PullRequest => {
            p.circle_filled(c + vec2(-2.5, -2.5), 1.8, color);
            p.circle_filled(c + vec2(2.5, 2.5), 1.8, color);
            p.line_segment([c + vec2(-2.5, -2.5), c + vec2(2.5, 2.5)], egui::Stroke::new(1.3, color));
        }
        Kind::Draft => {
            p.circle_stroke(c, 4.5, egui::Stroke::new(1.0, color.gamma_multiply(0.6)));
        }
    }
    x += 14.0;
    if let Some(n) = it.number {
        let r = p.text(pos2(x, y), Align2::LEFT_CENTER, format!("#{n}"), FontId::monospace(11.0), theme::FAINT);
        x = r.right() + 6.0;
    }
    p.with_clip_rect(Rect::from_min_max(rect.min, pos2(rect.right() - 6.0, rect.max.y))).text(pos2(x, y), Align2::LEFT_CENTER, &it.title, FontId::proportional(12.5), theme::TEXT);
    resp
}

impl App {
    /// Reads the project board for the open repository, in the background.
    pub(super) fn project_load(&mut self) {
        let Some(r) = &self.repo else { return };
        if self.screenshot.is_some() {
            return;
        }
        let url = r.default_remote().and_then(|rem| r.git(&["remote", "get-url", &rem]).ok()).unwrap_or_default();
        let Some(slug) = github::slug(&url) else {
            self.project = ProjectState::default();
            return;
        };
        let preferred = self.library.get(&r.root).and_then(|e| e.project.clone());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(github::load(&slug, preferred.as_deref()));
        });
        self.project.loading = Some(rx);
    }

    pub(super) fn project_poll(&mut self) {
        let Some(rx) = &self.project.loading else { return };
        let Ok(res) = rx.try_recv() else { return };
        self.project.loading = None;
        match res {
            Ok(b) => {
                self.project.board = Some(b);
                self.project.error = None;
            }
            Err(e) => self.project.error = Some(e),
        }
    }

    /// Remembers the chosen project for this repository and reads it.
    pub(super) fn project_pick(&mut self, i: usize) {
        let (Some(r), Some(b)) = (&self.repo, &self.project.board) else { return };
        let key = b.projects.get(i).map(|p| p.key());
        let root = r.root.clone();
        self.library.set_project(&root, key);
        self.library.save();
        self.project_load();
    }

    pub(super) fn project_section(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let loading = self.project.loading.is_some();
        let Some(board) = self.project.board.clone() else {
            if loading {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.spinner();
                    ui.label(RichText::new("Reading the GitHub project…").small().color(theme::DIM));
                });
            } else if let Some(e) = self.project.error.clone() {
                if self.section(ui, "Project", 0) {
                    ui.label(RichText::new(e).small().color(theme::MODIFIED));
                    if ui.small_button("Try again").clicked() {
                        cmds.push(Cmd::ProjectReload);
                    }
                }
            }
            return;
        };
        let Some(proj) = board.project().cloned() else { return };
        let f = self.filter.to_lowercase();
        let mine_only = self.project.mine_only;
        let mut toggle = mine_only;
        let mine = |it: &Item| !mine_only || it.assignees.iter().any(|a| a == &board.me);
        let keep = |it: &Item| f.is_empty() || it.title.to_lowercase().contains(&f) || it.number.map(|n| n.to_string().contains(&f)).unwrap_or(false);
        let open_count: usize = board.columns.iter().filter(|c| !github::is_done(&c.0)).map(|c| c.1.iter().filter(|i| mine(i) && keep(i)).count()).sum();

        let open = !self.collapsed.contains("Project");
        let header = section_header(ui, "Project", open_count, open);
        if header.clicked() {
            if open {
                self.collapsed.insert("Project".into());
            } else {
                self.collapsed.remove("Project");
            }
        }
        header.context_menu(|ui| {
            if ui.button("Refresh").clicked() {
                cmds.push(Cmd::ProjectReload);
            }
            if ui.button("Open the project on GitHub").clicked() {
                cmds.push(Cmd::OpenUrl(proj.url.clone()));
            }
        });
        if !open {
            return;
        }

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if board.projects.len() > 1 {
                egui::ComboBox::from_id_salt("gh_project").selected_text(RichText::new(&proj.title).small()).width(ui.available_width() - 70.0).show_ui(ui, |ui| {
                    for (i, p) in board.projects.iter().enumerate() {
                        if ui.selectable_label(i == board.chosen, &p.title).clicked() && i != board.chosen {
                            cmds.push(Cmd::ProjectPick(i));
                        }
                    }
                });
            } else if ui.link(RichText::new(&proj.title).small().color(theme::DIM)).on_hover_text("Open the project on GitHub").clicked() {
                cmds.push(Cmd::OpenUrl(proj.url.clone()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if loading {
                    ui.spinner();
                } else if ui.small_button("⟳").on_hover_text("Refresh").clicked() {
                    cmds.push(Cmd::ProjectReload);
                }
                if !board.me.is_empty() {
                    ui.toggle_value(&mut toggle, RichText::new("Mine").small()).on_hover_text(format!("Only items assigned to {}", board.me));
                }
            });
        });
        if toggle != mine_only {
            // Takes effect on the next frame, so this frame's list stays consistent.
            self.project.mine_only = toggle;
            ui.ctx().request_repaint();
        }
        if let Some(e) = &self.project.error {
            ui.label(RichText::new(e).small().color(theme::MODIFIED));
        }
        if board.columns.is_empty() {
            ui.label(RichText::new("  Nothing for this repository in the project yet.").small().color(theme::FAINT));
            return;
        }

        for (status, items) in &board.columns {
            let items: Vec<&Item> = items.iter().filter(|i| mine(i) && keep(i)).collect();
            if items.is_empty() {
                continue;
            }
            let key = format!("gh:{status}");
            let col_open = github::is_done(status) == self.project.flipped.contains(&key);
            let color = status_color(status);
            let label = format!("{}  {status}", if col_open { "⏷" } else { "⏵" });
            if side_item(ui, "remote", &label, Some(items.len()), false, None, false).clicked() {
                if !self.project.flipped.remove(&key) {
                    self.project.flipped.insert(key);
                }
            }
            if !col_open {
                continue;
            }
            for it in items {
                let mut tip = it.title.clone();
                if !it.assignees.is_empty() {
                    tip.push_str(&format!("\nAssigned to {}", it.assignees.join(", ")));
                }
                let resp = item_row(ui, it, color).on_hover_text(tip);
                if resp.clicked() && !it.url.is_empty() {
                    cmds.push(Cmd::OpenUrl(it.url.clone()));
                }
                resp.context_menu(|ui| {
                    if !it.url.is_empty() && ui.button("Open on GitHub").clicked() {
                        cmds.push(Cmd::OpenUrl(it.url.clone()));
                    }
                    if it.kind != Kind::PullRequest && ui.button("Create a branch for it…").clicked() {
                        let name = github::branch_name(it);
                        cmds.push(Cmd::Dialog(Dialog::NewRef { at: "HEAD".into(), label: "the current commit".into(), name, checkout: true, tag: false }));
                    }
                    ui.separator();
                    if let Some(n) = it.number {
                        if ui.button(format!("Copy “#{n}”")).clicked() {
                            cmds.push(Cmd::Copy(format!("#{n}")));
                        }
                    }
                    if !it.url.is_empty() && ui.button("Copy link").clicked() {
                        cmds.push(Cmd::Copy(it.url.clone()));
                    }
                });
            }
        }
    }
}
