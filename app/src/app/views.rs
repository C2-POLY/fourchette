//! What the window shows: toolbar, sidebar, history, details, diff, dialogs.
use super::*;
use crate::git::FileChange;
use crate::library::{DetailsLayout, DetailsPlace};
use crate::menu;
use crate::ui::*;
use egui::{pos2, vec2, Align, Align2, CornerRadius, FontId, Key, Layout, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui};

/// The bottom details panel's smallest height.
const DETAILS_BOTTOM_MIN_H: f32 = 160.0;
/// Below this width the bottom panel keeps the one-column layout of the right panel.
const WIDE_DETAILS_MIN_W: f32 = 640.0;

/// Splits the space left in `ui` into two columns with a thin line between them.
fn two_columns(ui: &mut Ui, id: &str) -> (Ui, Ui) {
    const GAP: f32 = 32.0;
    let r = ui.available_rect_before_wrap();
    let w = ((r.width() - GAP) / 2.0).max(0.0);
    let left = Rect::from_min_size(r.min, vec2(w, r.height()));
    let right = Rect::from_min_size(pos2(r.min.x + w + GAP, r.min.y), vec2(w, r.height()));
    let x = r.min.x + w + GAP / 2.0;
    ui.painter().vline(x, r.y_range().shrink(12.0), Stroke::new(1.0_f32, theme::c().border));
    let col = |ui: &mut Ui, side: &str, rect: Rect| ui.new_child(egui::UiBuilder::new().id_salt((id, side)).max_rect(rect).layout(Layout::top_down(Align::Min)));
    let (a, b) = (col(ui, "left", left), col(ui, "right", right));
    ui.allocate_rect(r, Sense::hover());
    (a, b)
}

impl Tab {
    // ---------------------------------------------------------------- toolbar

    fn toolbar(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        // The toolbar is the window's top edge (no title bar): dragging its empty background moves
        // the window and a double-click zooms it. Added first, so the buttons on top keep their clicks.
        let bg = ui.interact(ui.max_rect(), ui.id().with("window_drag"), Sense::click_and_drag());
        if bg.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if bg.double_clicked() {
            let zoomed = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!zoomed));
        }
        let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ui.horizontal_centered(|ui| {
            // Room for the red, yellow and green buttons, which sit on the toolbar.
            ui.add_space(crate::titlebar::inset(fullscreen).max(6.0));
            // The repository is the tab's (⌘O switches it); the branch menu comes first.
            let br = self.branch.clone().unwrap_or_else(|| format!("detached at {}", short(&self.head)));
            // Like the original's status view, the branch button opens Quick Launch in checkout mode.
            if toolbar_pill(ui, "branch", &br).on_hover_text("Check out another branch").clicked() {
                cmds.push(Cmd::PaletteCheckout);
            }

            ui.add_space(28.0);
            let (ahead, behind) = self.ahead_behind.unwrap_or((0, 0));
            let busy = self.busy.as_ref().map(|b| b.0.clone());
            let undo_tip = match self.undo_label() {
                Some(l) => format!("Undo “{l}” (⌘Z)"),
                None => "Nothing to undo yet".to_string(),
            };
            if tool_button(ui, "undo", "Undo", 0, false).on_hover_text(undo_tip).clicked() {
                cmds.push(Cmd::Undo);
            }
            ui.add_space(6.0);
            let tools: [(&str, &str, &str, usize); 6] = [
                ("fetch", "Fetch", "Get the latest changes from all remotes", 0),
                ("pull", "Pull", "Get the upstream branch's commits (merge or rebase: Settings › Git)", behind),
                ("push", "Push", "Send your commits to the remote", ahead),
                ("branch", "Branch", "Create a branch at the current commit", 0),
                ("stash", "Stash", "Put your local changes aside", 0),
                ("pop", "Pop", "Bring back the last stashed changes", 0),
            ];
            for (icon, label, tip, badge) in tools {
                let active = busy.as_deref().map(|b| b.starts_with(label)).unwrap_or(false);
                if tool_button(ui, icon, label, badge, active).on_hover_text(tip).clicked() {
                    match icon {
                        // Option-click fetches every remote (the original's "Fetch all remotes" box).
                        "fetch" => cmds.push(Cmd::Fetch { all: ui.input(|i| i.modifiers.alt), quiet: false }),
                        "pull" => cmds.push(Cmd::Pull),
                        "push" => cmds.push(Cmd::Push),
                        "branch" => cmds.push(Cmd::new_ref_at_head(false)),
                        "stash" => cmds.push(Cmd::stash()),
                        _ => cmds.push(Cmd::pop_stash()),
                    }
                }
            }

            ui.add_space(10.0);
            // AI assistant (Claude through the person's subscription).
            ui.add_space(4.0);
            let ai_btn = tool_button(ui, "ai", "Assistant", 0, self.ai.busy()).on_hover_text("Ask Claude (uses your Claude subscription)");
            egui::Popup::menu(&ai_btn).show(|ui| {
                ui.set_min_width(260.0);
                ui.label(RichText::new("Claude · your subscription").size(theme::FS_SMALL).color(theme::c().faint));
                if ui.button("🧹  Clean up branches…").clicked() {
                    cmds.push(Cmd::AiCleanup);
                }
                if ui.add_enabled(!self.status.staged.is_empty(), egui::Button::new("✏  Suggest a commit message")).clicked() {
                    cmds.push(Cmd::Select(Sel::Wip));
                    cmds.push(Cmd::AiCommitMessage);
                }
                let sel_hash = match self.sel {
                    Sel::Commit(i) => self.commits.get(i).map(|c| c.hash.clone()),
                    _ => None,
                };
                if ui.add_enabled(sel_hash.is_some(), egui::Button::new("💬  Explain the selected commit")).clicked() {
                    if let Some(h) = sel_hash {
                        cmds.push(Cmd::AiExplain(h));
                    }
                }
                if crate::ai::find_claude().is_none() {
                    ui.separator();
                    ui.label(RichText::new("Claude Code isn't installed on this Mac.").small().color(theme::c().modified));
                }
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(faint_hint("Search commits")).desired_width(220.0).margin(Margin::symmetric(10, 6)));
                if let Some(b) = &busy {
                    ui.add_space(8.0);
                    ui.label(RichText::new(format!("{b}…")).color(theme::c().dim));
                    ui.spinner();
                } else if self.ai.busy() {
                    ui.add_space(8.0);
                    // The original's progress text for commit messages (sub_1003e4338).
                    let text = if matches!(self.ai.job, Some((assistant::AiJob::CommitMessage, _))) { crate::ai::GENERATING_WITH_CLAUDE } else { "Claude is thinking…" };
                    ui.label(RichText::new(text).color(theme::c().accent_text));
                    ui.spinner();
                }
            });
        });
    }

    // ---------------------------------------------------------------- sidebar

    pub(super) fn section(&mut self, ui: &mut Ui, title: &str, count: usize) -> bool {
        let open = !self.collapsed.contains(title);
        if section_header(ui, title, count, open).clicked() {
            if open {
                self.collapsed.insert(title.to_string());
            } else {
                self.collapsed.remove(title);
            }
        }
        open
    }

    fn sidebar(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        ui.add_space(8.0);
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text(faint_hint("Filter branches")).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 6)));
        ui.add_space(8.0);
        let f = self.filter.to_lowercase();
        let keep = |n: &str| f.is_empty() || n.to_lowercase().contains(&f);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let n_changes = self.status.staged.len() + self.status.unstaged.len();
            let in_project = self.board.open;
            if side_item(ui, "changes", "Local Changes", Some(n_changes), self.sel == Sel::Wip && !in_project, None, false).clicked() {
                cmds.push(Cmd::Select(Sel::Wip));
            }
            let in_history = matches!(self.sel, Sel::Commit(_)) && !in_project;
            if side_item(ui, "history", "All Commits", Some(self.commits.len()), in_history, None, false).clicked() && !self.commits.is_empty() {
                cmds.push(Cmd::SelectHash(self.head.clone()));
            }
            if side_item(ui, "project", "Project", Some(self.project_open_count()), in_project, None, false).on_hover_text("The GitHub project of this repository (⌘B)").clicked() && !in_project {
                cmds.push(Cmd::Board);
            }
            ui.add_space(6.0);

            let locals: Vec<GitRef> = self.refs.iter().filter(|r| r.kind == RefKind::Local && keep(&r.name)).cloned().collect();
            if self.section(ui, "Branches", locals.len()) {
                let cur = self.branch.clone().unwrap_or_default();
                for b in &locals {
                    let resp = side_item(ui, "branch", &b.name, None, false, Some(self.ref_color(&b.target)), b.is_head);
                    if resp.clicked() {
                        cmds.push(Cmd::SelectHash(b.target.clone()));
                    }
                    if resp.double_clicked() {
                        cmds.extend(label_checkout(b, &self.refs));
                    }
                    resp.context_menu(|ui| branch_menu(ui, b, &cur, cmds));
                }
            }

            let remotes: Vec<GitRef> = self.refs.iter().filter(|r| r.kind == RefKind::Remote && keep(&r.name)).cloned().collect();
            if self.section(ui, "Remotes", remotes.len()) {
                let mut groups: Vec<(String, Vec<GitRef>)> = Vec::new();
                for r in remotes {
                    let remote = r.name.split_once('/').map(|x| x.0).unwrap_or(&r.name).to_string();
                    match groups.iter_mut().find(|g| g.0 == remote) {
                        Some(g) => g.1.push(r),
                        None => groups.push((remote, vec![r])),
                    }
                }
                for (remote, items) in groups {
                    let key = format!("remote:{remote}");
                    let open = !self.collapsed.contains(&key);
                    if group_row(ui, &remote, items.len(), open).clicked() {
                        if open {
                            self.collapsed.insert(key);
                        } else {
                            self.collapsed.remove(&key);
                        }
                    }
                    if !open {
                        continue;
                    }
                    for r in &items {
                        let name = r.name.split_once('/').map(|x| x.1).unwrap_or(&r.name);
                        let resp = side_item_indent(ui, name, self.ref_color(&r.target), 14.0);
                        if resp.clicked() {
                            cmds.push(Cmd::SelectHash(r.target.clone()));
                        }
                        if resp.double_clicked() {
                            cmds.push(checkout_remote(r, &self.refs));
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Check out").clicked() {
                                cmds.push(checkout_remote(r, &self.refs));
                            }
                            if ui.button(format!("Merge {} into current branch", r.name)).clicked() {
                                cmds.push(Cmd::Git { label: format!("Merged {}", r.name), args: vec!["merge".into(), "--no-edit".into(), r.name.clone()], network: false });
                            }
                            if ui.button("Copy name").clicked() {
                                cmds.push(Cmd::Copy(r.name.clone()));
                            }
                        });
                    }
                }
            }

            let tags: Vec<GitRef> = self.refs.iter().filter(|r| r.kind == RefKind::Tag && keep(&r.name)).cloned().collect();
            if self.section(ui, "Tags", tags.len()) {
                for t in &tags {
                    let resp = side_item(ui, "tag", &t.name, None, false, None, false);
                    if resp.clicked() {
                        cmds.push(Cmd::SelectHash(t.target.clone()));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Copy name").clicked() {
                            cmds.push(Cmd::Copy(t.name.clone()));
                        }
                        if ui.button("Delete tag…").clicked() {
                            cmds.push(Cmd::Dialog(Dialog::Confirm {
                                title: "Delete tag".into(),
                                message: format!("Do you want to delete the tag “{}”?", t.name),
                                label: format!("Deleted tag {}", t.name),
                                args: vec!["tag".into(), "-d".into(), t.name.clone()],
                                network: false,
                            }));
                        }
                    });
                }
            }

            // Worktrees (the original's RepositoryWorktrees / WorktreeSidebarItem). Shown whenever
            // there are linked worktrees, since Claude Code creates many of them.
            let linked: Vec<crate::worktree::Worktree> = self.worktrees.iter().filter(|w| !w.is_main && keep(&format!("{} {}", w.name(), w.branch.clone().unwrap_or_default()))).cloned().collect();
            if !linked.is_empty() && self.section(ui, "Worktrees", linked.len()) {
                for w in &linked {
                    let label = match (&w.branch, w.missing) {
                        (_, true) => format!("{} (folder gone)", w.name()),
                        (Some(b), _) => format!("{} · {b}", w.name()),
                        (None, _) => format!("{} · detached", w.name()),
                    };
                    let resp = side_item(ui, "worktree", &label, None, false, None, false).on_hover_text(w.path.display().to_string());
                    if resp.clicked() {
                        if let Some(b) = &w.branch {
                            if let Some(t) = self.refs.iter().find(|r| r.kind == RefKind::Local && &r.name == b).map(|r| r.target.clone()) {
                                cmds.push(Cmd::SelectHash(t));
                            }
                        }
                    }
                    resp.context_menu(|ui| {
                        if !w.missing && ui.button("Open as repository").clicked() {
                            cmds.push(Cmd::Open(w.path.clone()));
                        }
                        if ui.button("Copy worktree path").clicked() {
                            cmds.push(Cmd::Copy(w.path.display().to_string()));
                        }
                        ui.separator();
                        if ui.button(RichText::new("Delete worktree…").color(theme::c().deleted)).clicked() {
                            cmds.push(Cmd::AskDeleteWorktree(w.path.clone()));
                        }
                    });
                }
                if linked.iter().any(|w| w.missing) && ui.small_button("Clean up missing worktrees").clicked() {
                    cmds.push(Cmd::PruneWorktrees);
                }
            }

            let stashes = self.stashes.clone();
            if self.section(ui, "Stashes", stashes.len()) {
                for s in &stashes {
                    let resp = side_item(ui, "stash", &s.message, None, false, None, false).on_hover_text(&s.name);
                    resp.context_menu(|ui| {
                        if ui.button("Apply").clicked() {
                            cmds.push(Cmd::ApplyStash(s.name.clone()));
                        }
                        if ui.button("Pop").clicked() {
                            cmds.push(Cmd::Git { label: "Stash popped".into(), args: vec!["stash".into(), "pop".into(), s.name.clone()], network: false });
                        }
                        if ui.button("Delete…").clicked() {
                            cmds.push(Cmd::Dialog(Dialog::Confirm {
                                title: "Delete stash".into(),
                                message: format!("Do you want to delete the stash “{}”?", s.message),
                                label: "Stash deleted".into(),
                                args: vec!["stash".into(), "drop".into(), s.name.clone()],
                                network: false,
                            }));
                        }
                    });
                }
            }
            ui.add_space(12.0);
        });
    }

    // ---------------------------------------------------------------- commit list

    /// Width of the branch / tag column: just enough for the widest first label (plus a "+n").
    fn refs_width(&self, ui: &Ui, full_w: f32) -> f32 {
        let mut need: f32 = if self.status.is_empty() { 0.0 } else { 96.0 };
        for refs in self.by_target.values() {
            if let Some(r) = refs.first() {
                let w = ui.painter().layout_no_wrap(ref_label(r), theme::small(), theme::c().text).size().x + 16.0;
                need = need.max(w + 16.0 + if refs.len() > 1 { 28.0 } else { 0.0 });
            }
        }
        need.clamp(72.0, if full_w > 900.0 { 210.0 } else { 150.0 })
    }

    fn commit_list(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let wip = !self.status.is_empty();
        let n = self.commits.len() + wip as usize;
        let full_w = ui.available_width();
        let refs_w = self.refs_width(ui, full_w);
        let graph_w = graph_width(self.max_lanes).min((full_w * 0.3).max(60.0));
        // Keep at least 280 px for the message: drop SHA, then date, then author when narrow.
        let room = full_w - refs_w - graph_w - 280.0;
        let author_w = if room >= 150.0 { 150.0 } else { 0.0 };
        let date_w = if room >= 150.0 + 104.0 { 104.0 } else { 0.0 };
        let sha_w = if room >= 150.0 + 104.0 + 76.0 { 76.0 } else { 0.0 };
        let show_meta = author_w > 0.0;

        let (hrect, _) = ui.allocate_exact_size(vec2(full_w, 30.0), Sense::hover());
        {
            let p = ui.painter();
            p.rect_filled(hrect, CornerRadius::ZERO, theme::c().bg);
            p.line_segment([hrect.left_bottom(), hrect.right_bottom()], Stroke::new(1.0_f32, theme::c().border));
            let y = hrect.center().y;
            let f = theme::small();
            p.text(pos2(hrect.left() + 12.0, y), Align2::LEFT_CENTER, "Branch", f.clone(), theme::c().faint);
            p.text(pos2(hrect.left() + refs_w + 4.0, y), Align2::LEFT_CENTER, "Graph", f.clone(), theme::c().faint);
            p.text(pos2(hrect.left() + refs_w + graph_w + 8.0, y), Align2::LEFT_CENTER, "Message", f.clone(), theme::c().faint);
            if show_meta {
                let r = hrect.right();
                p.text(pos2(r - sha_w - date_w - author_w + 4.0, y), Align2::LEFT_CENTER, "Author", f.clone(), theme::c().faint);
                if date_w > 0.0 {
                    p.text(pos2(r - sha_w - date_w + 4.0, y), Align2::LEFT_CENTER, "Date", f.clone(), theme::c().faint);
                }
                if sha_w > 0.0 {
                    p.text(pos2(r - sha_w + 4.0, y), Align2::LEFT_CENTER, "SHA", f, theme::c().faint);
                }
            }
        }

        let search = self.search.trim().to_lowercase();
        // Rows touch each other, so the graph lines run on without a gap between rows.
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if let Some(i) = self.scroll_to.take() {
            let row = i + wip as usize;
            let h = ui.available_height();
            area = area.vertical_scroll_offset((row as f32 * theme::ROW_H - h / 2.0).max(0.0));
        }
        area.show_rows(ui, theme::ROW_H, n, |ui, range| {
            for vis in range {
                let (rect, resp) = ui.allocate_exact_size(vec2(full_w, theme::ROW_H), Sense::click());
                if wip && vis == 0 {
                    self.wip_row(ui, rect, &resp, refs_w, graph_w, cmds);
                    continue;
                }
                let i = vis - wip as usize;
                let selected = self.sel == Sel::Commit(i);
                let c = &self.commits[i];
                let row = &self.rows[i];
                let lane_col = theme::lane_color(row.color);
                let matched = search.is_empty() || c.subject.to_lowercase().contains(&search) || c.author.to_lowercase().contains(&search) || c.hash.starts_with(&search);
                let p = ui.painter_at(rect);
                let gx = rect.left() + refs_w;
                let node_x = lane_x(gx, row.lane);
                let y = rect.center().y;

                // Background: hover, or the selection with a bar in the branch colour.
                let bg = if selected {
                    theme::c().select
                } else if resp.hovered() {
                    theme::c().hover
                } else {
                    theme::c().bg
                };
                if bg != theme::c().bg {
                    p.rect_filled(rect, CornerRadius::ZERO, bg);
                }
                if selected {
                    p.rect_filled(Rect::from_min_size(rect.min, vec2(2.0, rect.height())), CornerRadius::ZERO, lane_col);
                }

                // Branch / tag labels, joined to the commit by a thin line.
                if let Some(refs) = self.by_target.get(&c.hash) {
                    let mut x = rect.left() + 10.0;
                    let max_x = gx - 4.0;
                    let mut pills: Vec<(Rect, &GitRef)> = Vec::new();
                    for (k, r) in refs.iter().enumerate() {
                        let (fill, stroke, text) = ref_colors(r, lane_col, bg);
                        let galley = p.layout_no_wrap(ref_label(r), theme::small(), text);
                        let w = galley.size().x + 14.0;
                        if x + w > max_x && k > 0 {
                            let mg = p.layout_no_wrap(format!("+{}", refs.len() - k), theme::small(), theme::c().dim);
                            let mr = Rect::from_min_size(pos2(x, y - 9.0), vec2(mg.size().x + 10.0, 18.0));
                            p.rect_filled(mr, CornerRadius::same(4), theme::c().elevated);
                            p.galley(pos2(mr.left() + 5.0, y - mg.size().y / 2.0), mg, theme::c().dim);
                            x = mr.right() + theme::REF_GAP;
                            break;
                        }
                        let pr = Rect::from_min_size(pos2(x, y - 9.0), vec2(w.min(max_x - x), 18.0));
                        pills.push((pr, r));
                        p.rect_filled(pr, CornerRadius::same(4), fill);
                        if let Some(s) = stroke {
                            p.rect_stroke(pr, CornerRadius::same(4), Stroke::new(1.0_f32, s), StrokeKind::Inside);
                        }
                        // A label wider than its pill ends with "…" inside it, never cut at the edge.
                        let galley = if w > max_x - x { fit_galley(&p, &ref_label(r), theme::small(), text, pr.width() - 14.0) } else { galley };
                        p.with_clip_rect(pr.shrink(1.0)).galley(pos2(pr.left() + 7.0, y - galley.size().y / 2.0), galley, text);
                        x = pr.right() + theme::REF_GAP;
                    }
                    p.line_segment([pos2(x - theme::REF_GAP, y), pos2(node_x, y)], Stroke::new(1.0_f32, theme::tint(lane_col, bg, 0.45)));
                    // A double-click on a label checks its branch out.
                    if resp.double_clicked() {
                        if let Some(q) = ui.input(|i| i.pointer.interact_pos()) {
                            if let Some((_, r)) = pills.iter().find(|(pr, _)| pr.contains(q)) {
                                cmds.extend(label_checkout(r, &self.refs));
                            }
                        }
                    }
                    // Hovering the labels lists every ref of the commit in full, the hidden ones included.
                    if resp.hovered() && ui.input(|i| i.pointer.hover_pos()).is_some_and(|q| q.x < gx) {
                        resp.clone().on_hover_ui_at_pointer(|ui| {
                            for r in refs {
                                ui.label(ref_label(r));
                            }
                        });
                    }
                }

                // Graph
                let gp = p.with_clip_rect(Rect::from_min_max(pos2(gx, rect.top()), pos2(gx + graph_w, rect.bottom())));
                draw_row_graph(&gp, row, gx, rect.top(), rect.bottom(), if matched { 1.0 } else { 0.35 });
                draw_node(&gp, pos2(node_x, y), row, c.hash == self.head, matched, bg);

                // Text columns
                let tc = if !matched {
                    theme::c().faint
                } else if selected {
                    theme::c().strong
                } else {
                    theme::c().text
                };
                let mx = gx + graph_w + 8.0;
                let msg_right = rect.right() - sha_w - date_w - author_w - 12.0;
                fit_text(&p, pos2(mx, y), Align2::LEFT_CENTER, &c.subject, theme::body(), tc, msg_right - mx);
                if show_meta {
                    let sc = if matched { theme::c().dim } else { theme::c().faint };
                    let ax = rect.right() - sha_w - date_w - author_w;
                    avatar(&p, pos2(ax + 12.0, y), 9.0, &c.author);
                    fit_text(&p, pos2(ax + 27.0, y), Align2::LEFT_CENTER, &c.author, theme::secondary(), sc, author_w - 35.0);
                    if date_w > 0.0 {
                        fit_text(&p, pos2(rect.right() - sha_w - date_w + 4.0, y), Align2::LEFT_CENTER, &git::relative_time(c.time), theme::secondary(), sc, date_w - 8.0);
                    }
                    if sha_w > 0.0 {
                        p.text(pos2(rect.right() - sha_w + 4.0, y), Align2::LEFT_CENTER, short(&c.hash), theme::mono(), sc);
                    }
                }

                if resp.clicked() {
                    cmds.push(Cmd::Select(Sel::Commit(i)));
                }
                let (hash, subject, branch) = (c.hash.clone(), c.subject.clone(), self.branch.clone());
                resp.context_menu(|ui| commit_menu(ui, &hash, &subject, branch.as_deref(), cmds));
            }
        });
    }

    fn wip_row(&self, ui: &Ui, rect: Rect, resp: &egui::Response, refs_w: f32, graph_w: f32, cmds: &mut Vec<Cmd>) {
        let p = ui.painter_at(rect);
        let bg = if self.sel == Sel::Wip {
            theme::c().select
        } else if resp.hovered() {
            theme::c().hover
        } else {
            theme::c().bg
        };
        p.rect_filled(rect, CornerRadius::ZERO, bg);
        let gx = rect.left() + refs_w;
        let head_row = self.index.get(&self.head).copied();
        let (lane, col) = head_row.and_then(|i| self.rows.get(i)).map(|r| (r.lane, theme::lane_color(r.color))).unwrap_or((0, theme::c().dim));
        if self.sel == Sel::Wip {
            p.rect_filled(Rect::from_min_size(rect.min, vec2(2.0, rect.height())), CornerRadius::ZERO, col);
        }
        // Not committed yet: a dashed ring, joined to the checked-out commit by a dashed line.
        let c = pos2(lane_x(gx, lane), rect.center().y);
        let soft = theme::tint(col, bg, 0.75);
        let r = theme::NODE_R + 1.0;
        if head_row == Some(0) {
            p.extend(Shape::dashed_line(&[pos2(c.x, c.y + r), pos2(c.x, rect.bottom())], Stroke::new(1.5_f32, soft), 3.0, 3.0));
        }
        let pts: Vec<Pos2> = (0..=32)
            .map(|k| {
                let a = k as f32 / 32.0 * std::f32::consts::TAU;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect();
        p.extend(Shape::dashed_line(&pts, Stroke::new(1.5_f32, soft), 2.5, 2.0));
        let mx = gx + graph_w + 8.0;
        let tr = p.text(pos2(mx, c.y), Align2::LEFT_CENTER, "Uncommitted changes", theme::body(), theme::c().dim);
        let count = |pred: &dyn Fn(&FileChange) -> bool| self.status.unstaged.iter().filter(|f| pred(f)).count() + self.status.staged.iter().filter(|f| pred(f)).count();
        let mut bx = tr.right() + 10.0;
        for (n, colr, sym) in [
            (count(&|f| f.code == 'M' || f.code == 'R' || f.code == 'U'), theme::c().modified, "~"),
            (count(&|f| f.code == '?' || f.code == 'A'), theme::c().added, "+"),
            (count(&|f| f.code == 'D'), theme::c().deleted, "−"),
        ] {
            if n == 0 {
                continue;
            }
            let g = p.layout_no_wrap(format!("{sym}{n}"), theme::small(), colr);
            let r = Rect::from_min_size(pos2(bx, c.y - 9.0), vec2(g.size().x + 12.0, 18.0));
            p.rect_filled(r, CornerRadius::same(4), theme::tint(colr, bg, 0.14));
            p.galley(r.center() - g.size() / 2.0, g, colr);
            bx = r.right() + 4.0;
        }
        p.text(pos2(rect.left() + 12.0, c.y), Align2::LEFT_CENTER, "Local changes", theme::small(), theme::c().faint);
        if resp.clicked() {
            cmds.push(Cmd::Select(Sel::Wip));
        }
    }

    // ---------------------------------------------------------------- details panel

    fn details_panel(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        if self.board.open {
            return self.board_detail_panel(ui, cmds);
        }
        // At the bottom the panel is wide and short: its content goes side by side in two columns.
        let wide = self.details_layout.place == DetailsPlace::Bottom && ui.available_width() >= WIDE_DETAILS_MIN_W;
        match self.sel.clone() {
            Sel::Wip if wide => self.wip_panel_wide(ui, cmds),
            Sel::Wip => self.wip_panel(ui, cmds),
            Sel::Commit(_) => {
                self.ensure_details();
                match self.details.clone() {
                    Some(d) if wide => {
                        let (mut left, mut right) = two_columns(ui, "details");
                        egui::ScrollArea::vertical().id_salt("details_info").auto_shrink([false, false]).show(&mut left, |ui| {
                            ui.add_space(12.0);
                            self.commit_info(ui, &d, cmds);
                            ui.add_space(12.0);
                        });
                        egui::ScrollArea::vertical().id_salt("details_files").auto_shrink([false, false]).show(&mut right, |ui| {
                            ui.add_space(12.0);
                            self.commit_files(ui, &d, cmds);
                        });
                    }
                    Some(d) => {
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            ui.add_space(12.0);
                            self.commit_info(ui, &d, cmds);
                            ui.add_space(12.0);
                            ui.separator();
                            self.commit_files(ui, &d, cmds);
                        });
                    }
                    None => {
                        ui.label("Couldn't read this commit.");
                    }
                }
            }
            Sel::None => {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| ui.label(RichText::new("Select a commit to see its details").color(theme::c().dim)));
            }
        }
    }

    /// Subject, message, Claude's explanation, people and SHA, parents and refs.
    fn commit_info(&self, ui: &mut Ui, d: &CommitDetails, cmds: &mut Vec<Cmd>) {
        ui.label(RichText::new(&d.subject).size(theme::FS_TITLE).color(theme::c().strong));
        if !d.body.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&d.body).color(theme::c().dim));
        }
        ui.add_space(8.0);
        // Plain-words explanation by Claude.
        match &self.ai.explanation {
            Some((h, text)) if h == &d.hash => {
                egui::Frame::new().fill(theme::c().accent_soft).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if text.is_empty() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("Claude is reading this commit…").color(theme::c().dim));
                        });
                    } else {
                        ui.label(RichText::new("In plain words").small().color(theme::c().accent_text));
                        ui.label(RichText::new(text).color(theme::c().text));
                    }
                });
            }
            _ => {
                if ui.button(RichText::new("✨ Explain with Claude").color(theme::c().accent_text)).on_hover_text("Claude explains this commit in plain words").clicked() {
                    cmds.push(Cmd::AiExplain(d.hash.clone()));
                }
            }
        }
        ui.add_space(12.0);
        person_card(ui, "Author", &d.author, &d.email, d.author_time);
        if d.committer != d.author || d.committer_email != d.email {
            ui.add_space(6.0);
            person_card(ui, "Committer", &d.committer, &d.committer_email, d.committer_time);
        }
        ui.add_space(10.0);
        egui::Grid::new("meta").num_columns(2).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
            ui.label(RichText::new("SHA").size(theme::FS_SECONDARY).color(theme::c().faint));
            ui.horizontal(|ui| {
                ui.label(RichText::new(short(&d.hash)).monospace().color(theme::c().text));
                if ui.small_button("Copy").clicked() {
                    cmds.push(Cmd::Copy(d.hash.clone()));
                }
            });
            ui.end_row();
            ui.label(RichText::new("Parents").size(theme::FS_SECONDARY).color(theme::c().faint));
            ui.horizontal(|ui| {
                if d.parents.is_empty() {
                    ui.label(RichText::new("none").color(theme::c().faint));
                }
                for p in &d.parents {
                    if ui.link(RichText::new(short(p)).monospace().color(theme::c().accent_text)).clicked() {
                        cmds.push(Cmd::SelectHash(p.clone()));
                    }
                }
            });
            ui.end_row();
            if let Some(refs) = self.by_target.get(&d.hash) {
                ui.label(RichText::new("Refs").size(theme::FS_SECONDARY).color(theme::c().faint));
                ui.horizontal_wrapped(|ui| {
                    let col = self.ref_color(&d.hash);
                    for r in refs {
                        let (fill, stroke, text) = ref_colors(r, col, theme::c().bg);
                        egui::Frame::new()
                            .fill(fill)
                            .stroke(stroke.map(|s| Stroke::new(1.0_f32, s)).unwrap_or(Stroke::NONE))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(7, 2))
                            .show(ui, |ui| ui.add(egui::Label::new(RichText::new(ref_label(r)).size(theme::FS_SMALL).color(text)).extend()));
                    }
                });
                ui.end_row();
            }
        });
    }

    /// The changed files of a commit, with their counts.
    fn commit_files(&self, ui: &mut Ui, d: &CommitDetails, cmds: &mut Vec<Cmd>) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} changed file{}", d.files.len(), if d.files.len() == 1 { "" } else { "s" })).size(theme::FS_SECONDARY).color(theme::c().dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (a, m, del) = d.files.iter().fold((0, 0, 0), |acc, f| match f.code {
                    'A' => (acc.0 + 1, acc.1, acc.2),
                    'D' => (acc.0, acc.1, acc.2 + 1),
                    _ => (acc.0, acc.1 + 1, acc.2),
                });
                ui.label(RichText::new(format!("− {del}")).color(theme::c().deleted).small());
                ui.label(RichText::new(format!("~ {m}")).color(theme::c().modified).small());
                ui.label(RichText::new(format!("+ {a}")).color(theme::c().added).small());
            });
        });
        ui.add_space(4.0);
        let current = self.diff.as_ref().map(|d| d.title.clone());
        for f in &d.files {
            if file_row(ui, f, current.as_deref() == Some(f.path.as_str()), None).0.clicked() {
                cmds.push(Cmd::Diff(DiffReq::Commit { hash: d.hash.clone(), path: f.path.clone() }));
            }
        }
    }

    fn file_list(&self, ui: &mut Ui, staged: bool, list_h: f32, cmds: &mut Vec<Cmd>) {
        let files = if staged { self.status.staged.clone() } else { self.status.unstaged.clone() };
        let current = self.diff.as_ref().map(|d| d.title.clone());
        egui::Frame::new().fill(theme::c().sidebar).stroke(Stroke::new(1.0_f32, theme::c().border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(4)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(if staged { "staged" } else { "unstaged" }).max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                for f in &files {
                    let (resp, btn) = file_row(ui, f, current.as_deref() == Some(f.path.as_str()), Some(if staged { "Unstage" } else { "Stage" }));
                    let stage = Cmd::Git { label: format!("Staged {}", f.path), args: vec!["add".into(), "-A".into(), "--".into(), f.path.clone()], network: false };
                    let unstage = Cmd::Git { label: format!("Unstaged {}", f.path), args: vec!["reset".into(), "-q".into(), "--".into(), f.path.clone()], network: false };
                    if btn {
                        cmds.push(if staged { unstage } else { stage });
                    } else if resp.clicked() {
                        cmds.push(Cmd::Diff(DiffReq::Worktree { path: f.path.clone(), staged, untracked: f.code == '?' }));
                    }
                    resp.context_menu(|ui| {
                        if !staged {
                            if ui.button("Stage").clicked() {
                                cmds.push(Cmd::Git { label: format!("Staged {}", f.path), args: vec!["add".into(), "-A".into(), "--".into(), f.path.clone()], network: false });
                            }
                            if ui.button(RichText::new("Discard changes…").color(theme::c().deleted)).clicked() {
                                if f.code == '?' {
                                    // Like the original, a new file goes to the Trash instead of being erased.
                                    cmds.push(Cmd::Discard { path: f.path.clone(), untracked: true });
                                } else {
                                    cmds.push(Cmd::Dialog(Dialog::Confirm {
                                        title: "Discard changes".into(),
                                        message: format!("Do you want to discard the changes in “{}”?", f.path),
                                        label: format!("Discarded {}", f.path),
                                        args: vec!["checkout".into(), "--".into(), f.path.clone()],
                                        network: false,
                                    }));
                                }
                            }
                        } else if ui.button("Unstage").clicked() {
                            cmds.push(Cmd::Git { label: format!("Unstaged {}", f.path), args: vec!["reset".into(), "-q".into(), "--".into(), f.path.clone()], network: false });
                        }
                        if ui.button("Copy path").clicked() {
                            cmds.push(Cmd::Copy(f.path.clone()));
                        }
                    });
                }
                if files.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(if staged { "Stage files to include them in the commit" } else { "No unstaged changes" }).color(theme::c().faint));
                }
            });
        });
    }

    fn wip_title(&self, ui: &mut Ui) {
        ui.add_space(12.0);
        let total = self.status.staged.len() + self.status.unstaged.len();
        let on = self.branch.clone().unwrap_or_else(|| "HEAD".into());
        ui.label(RichText::new(format!("{total} file change{} on {on}", if total == 1 { "" } else { "s" })).size(theme::FS_TITLE).color(theme::c().strong));
        ui.add_space(8.0);
    }

    fn wip_panel(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        self.wip_title(ui);
        let list_h = ((ui.available_height() - 290.0) / 2.0).max(80.0);
        self.wip_files(ui, list_h, cmds);
        ui.add_space(12.0);
        self.wip_message(ui, cmds);
    }

    /// At the bottom: the file lists on the left, the commit message on the right.
    fn wip_panel_wide(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        self.wip_title(ui);
        let (mut left, mut right) = two_columns(ui, "wip");
        // Two headers, the lists' frames, the gap between them and a bottom margin.
        let list_h = ((left.available_height() - 100.0) / 2.0).max(40.0);
        self.wip_files(&mut left, list_h, cmds);
        egui::ScrollArea::vertical().id_salt("wip_message").auto_shrink([false, false]).show(&mut right, |ui| {
            self.wip_message(ui, cmds);
            ui.add_space(8.0);
        });
    }

    /// Unstaged and staged files, each list `list_h` high.
    fn wip_files(&mut self, ui: &mut Ui, list_h: f32, cmds: &mut Vec<Cmd>) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Unstaged files  {}", self.status.unstaged.len())).size(theme::FS_SECONDARY).color(theme::c().dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(!self.status.unstaged.is_empty(), egui::Button::new(RichText::new("Stage all").color(theme::c().added))).clicked() {
                    cmds.push(Cmd::Git { label: "All changes staged".into(), args: vec!["add".into(), "-A".into()], network: false });
                }
            });
        });
        self.file_list(ui, false, list_h, cmds);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Staged files  {}", self.status.staged.len())).size(theme::FS_SECONDARY).color(theme::c().dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(!self.status.staged.is_empty(), egui::Button::new(RichText::new("Unstage all").color(theme::c().deleted))).clicked() {
                    cmds.push(Cmd::Git { label: "All changes unstaged".into(), args: vec!["reset".into(), "-q".into()], network: false });
                }
            });
        });
        self.file_list(ui, true, list_h, cmds);
    }

    /// Subject, description, amend and the commit button.
    fn wip_message(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Commit message").size(theme::FS_SECONDARY).color(theme::c().dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let thinking = matches!(self.ai.job, Some((assistant::AiJob::CommitMessage, _)));
                if thinking {
                    ui.spinner();
                } else if ui
                    .add_enabled(!self.status.staged.is_empty(), egui::Button::new(RichText::new("✨ Suggest").color(theme::c().accent_text)))
                    .on_hover_text("Claude writes a message from your staged changes")
                    .clicked()
                {
                    cmds.push(Cmd::AiCommitMessage);
                }
            });
        });
        let resp = ui.add(egui::TextEdit::singleline(&mut self.summary).hint_text(faint_hint("Commit subject")).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 6)));
        let n = self.summary.chars().count();
        if n > 0 {
            // Subject length warning at the settings' limits (50 / 70 characters by default).
            let col = if n > self.subject_limits.1 { theme::c().deleted } else if n > self.subject_limits.0 { theme::c().modified } else { theme::c().faint };
            ui.painter().text(pos2(resp.rect.right() - 8.0, resp.rect.center().y), Align2::RIGHT_CENTER, n.to_string(), theme::small(), col);
        }
        ui.add(egui::TextEdit::multiline(&mut self.description).hint_text(faint_hint("Description")).desired_rows(3).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 6)));
        if ui.checkbox(&mut self.amend, "Amend last commit").changed() && self.amend && self.summary.is_empty() {
            if let Some(d) = self.repo.as_ref().and_then(|r| r.details("HEAD")) {
                self.summary = d.subject;
                self.description = d.body;
            }
        }
        let staged = self.status.staged.len();
        let can = !self.summary.trim().is_empty() && (staged > 0 || self.amend);
        let label = if self.amend {
            "Amend commit".to_string()
        } else if staged == 0 {
            "Stage files to commit".into()
        } else {
            format!("Commit {staged} file{}", if staged == 1 { "" } else { "s" })
        };
        let btn = egui::Button::new(RichText::new(label).color(if can { theme::c().on_accent } else { theme::c().faint }))
            .fill(if can { theme::c().accent } else { theme::c().elevated })
            .stroke(Stroke::new(1.0_f32, if can { theme::c().accent } else { theme::c().border }))
            .corner_radius(CornerRadius::same(6))
            .min_size(vec2(ui.available_width(), 32.0));
        if ui.add_enabled(can, btn).clicked() {
            cmds.push(Cmd::Commit);
        }
    }

    // ---------------------------------------------------------------- diff view

    /// `settings_open`: Esc belongs to the Settings window while it is open.
    fn diff_view(&self, ui: &mut Ui, cmds: &mut Vec<Cmd>, settings_open: bool) {
        let Some(d) = &self.diff else { return };
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(&d.title).size(theme::FS_BODY).color(theme::c().strong));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                if ui.button("Close").clicked() || (!settings_open && ui.input(|i| i.key_pressed(Key::Escape))) {
                    cmds.push(Cmd::CloseDiff);
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
        let lines: Vec<&str> = d.text.lines().collect();
        if lines.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| ui.label(RichText::new("No text changes to show (binary file or only a mode change).").color(theme::c().dim)));
            return;
        }
        let is_header = |l: &str| {
            ["diff ", "index ", "+++", "---", "new file", "deleted file", "similarity", "rename ", "old mode", "new mode"].iter().any(|p| l.starts_with(p))
        };
        // Line numbers from the hunk headers.
        let mut nums: Vec<(Option<u32>, Option<u32>)> = Vec::with_capacity(lines.len());
        let (mut o, mut n) = (0u32, 0u32);
        for l in &lines {
            if l.starts_with("@@") {
                let parse = |s: &str| s.get(1..).and_then(|x| x.split(',').next()).and_then(|x| x.parse::<u32>().ok()).unwrap_or(1);
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() > 2 {
                    o = parse(parts[1]);
                    n = parse(parts[2]);
                }
                nums.push((None, None));
            } else if is_header(l) {
                nums.push((None, None));
            } else if l.starts_with('+') {
                nums.push((None, Some(n)));
                n += 1;
            } else if l.starts_with('-') {
                nums.push((Some(o), None));
                o += 1;
            } else {
                nums.push((Some(o), Some(n)));
                o += 1;
                n += 1;
            }
        }
        let font = theme::mono();
        let h = 18.0;
        egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, h, lines.len(), |ui, range| {
            for i in range {
                let l = lines[i];
                let w = (l.len() as f32 * 7.6 + 120.0).max(ui.available_width());
                let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                let p = ui.painter();
                let (bg, fg) = if is_header(l) {
                    (theme::c().bg, theme::c().faint)
                } else if l.starts_with("@@") {
                    (theme::c().accent_soft, theme::c().accent_text)
                } else if l.starts_with('+') {
                    (theme::tint(theme::c().added, theme::c().bg, 0.13), theme::c().diff_added_text)
                } else if l.starts_with('-') {
                    (theme::tint(theme::c().deleted, theme::c().bg, 0.13), theme::c().diff_deleted_text)
                } else {
                    (Color32::TRANSPARENT, theme::c().text)
                };
                p.rect_filled(rect, CornerRadius::ZERO, bg);
                let num = |v: Option<u32>| v.map(|x| x.to_string()).unwrap_or_default();
                p.text(pos2(rect.left() + 40.0, rect.center().y), Align2::RIGHT_CENTER, num(nums[i].0), FontId::monospace(theme::FS_SMALL), theme::c().faint);
                p.text(pos2(rect.left() + 80.0, rect.center().y), Align2::RIGHT_CENTER, num(nums[i].1), FontId::monospace(theme::FS_SMALL), theme::c().faint);
                p.text(pos2(rect.left() + 96.0, rect.center().y), Align2::LEFT_CENTER, l.replace('\t', "    "), font.clone(), fg);
            }
        });
    }

    // ---------------------------------------------------------------- dialogs, keys, screenshot

    fn dialogs(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let mut close = false;
        match &mut self.dialog {
            None => return,
            Some(Dialog::NewRef { at, label, name, checkout, tag }) => {
                egui::Modal::new(egui::Id::new("newref")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading(if *tag { "New tag" } else { "New branch" });
                    ui.label(RichText::new(format!("Starting at {label}")).color(theme::c().dim));
                    ui.add_space(8.0);
                    ui.add(egui::TextEdit::singleline(name).hint_text(faint_hint(if *tag { "v1.0.0" } else { "feature/my-idea" })).desired_width(f32::INFINITY)).request_focus();
                    if !*tag {
                        ui.checkbox(checkout, "Switch to the new branch");
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let ok = !name.trim().is_empty() && !name.trim().contains(' ');
                        let create = ui.add_enabled(ok, egui::Button::new(RichText::new("Create").color(theme::c().on_accent)).fill(theme::c().accent).stroke(Stroke::new(1.0_f32, theme::c().accent))).clicked();
                        if create || (ok && ui.input(|i| i.key_pressed(Key::Enter))) {
                            let n = name.trim().to_string();
                            let args = if *tag {
                                vec!["tag".into(), n.clone(), at.clone()]
                            } else if *checkout {
                                vec!["checkout".into(), "-b".into(), n.clone(), at.clone()]
                            } else {
                                vec!["branch".into(), n.clone(), at.clone()]
                            };
                            cmds.push(Cmd::Git { label: format!("Created {n}"), args, network: false });
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            close = true;
                        }
                    });
                });
            }
            Some(Dialog::DeleteBranch { name, merged, force, worktree, dirty, also_worktree }) => {
                // Layout and wording of the original's delete-branch sheet (sub_100070d94).
                egui::Modal::new(egui::Id::new("delete_branch")).show(ctx, |ui| {
                    ui.set_width(420.0);
                    ui.label(RichText::new("Delete Branch").size(theme::FS_TITLE).color(theme::c().strong));
                    ui.label(RichText::new("Delete local branch from your repository").color(theme::c().dim));
                    ui.add_space(8.0);
                    ui.label(RichText::new(name.as_str()).monospace().color(theme::c().text));
                    if let Some(w) = worktree.as_ref() {
                        ui.add_space(8.0);
                        egui::Frame::new().fill(theme::tint(theme::c().modified, theme::c().elevated, 0.1)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚠").color(theme::c().modified));
                                ui.label(RichText::new(format!("Checked out in worktree “{}”", w.name())).color(theme::c().text)).on_hover_text(w.path.display().to_string());
                            });
                            ui.checkbox(also_worktree, "Also delete worktree");
                            if *dirty && *also_worktree {
                                ui.label(RichText::new("It has uncommitted changes. They're saved before it's removed, so Undo (⌘Z) brings them back.").small().color(theme::c().modified));
                            }
                            if !*also_worktree {
                                ui.label(RichText::new("Git can't delete a branch that is checked out in a worktree.").small().color(theme::c().deleted));
                            }
                        });
                    }
                    if !*merged {
                        ui.add_space(6.0);
                        ui.checkbox(force, "Delete even though it isn't merged into the current branch");
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new("You can take this back with Undo (⌘Z).").small().color(theme::c().faint));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let ok = (worktree.is_none() || *also_worktree) && (*merged || *force);
                        if ui.add_enabled(ok, egui::Button::new(RichText::new("Delete").color(theme::c().on_accent)).fill(theme::tint(theme::c().deleted, theme::c().elevated, 0.8)).stroke(Stroke::new(1.0_f32, theme::tint(theme::c().deleted, theme::c().elevated, 0.8)))).clicked() {
                            cmds.push(Cmd::DeleteBranch { name: name.clone(), force: *force, also_worktree: *also_worktree });
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            close = true;
                        }
                    });
                });
            }
            Some(Dialog::DeleteWorktree { wt, dirty }) => {
                // The original's wording (sub_1002008ec).
                egui::Modal::new(egui::Id::new("delete_worktree")).show(ctx, |ui| {
                    ui.set_width(420.0);
                    ui.label(RichText::new(format!("Delete worktree '{}'", wt.name())).size(theme::FS_TITLE).color(theme::c().strong));
                    ui.label(RichText::new(format!("Do you want to delete worktree '{}'?", wt.name())).color(theme::c().text));
                    ui.label(RichText::new(wt.path.display().to_string()).small().color(theme::c().faint));
                    if *dirty {
                        ui.label(RichText::new("It has uncommitted changes. They're saved before it's removed, so Undo (⌘Z) brings them back.").small().color(theme::c().modified));
                    }
                    if let Some(b) = &wt.branch {
                        ui.label(RichText::new(format!("The branch “{b}” is kept.")).small().color(theme::c().dim));
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Delete").color(theme::c().on_accent)).fill(theme::tint(theme::c().deleted, theme::c().elevated, 0.8)).stroke(Stroke::new(1.0_f32, theme::tint(theme::c().deleted, theme::c().elevated, 0.8)))).clicked() {
                            cmds.push(Cmd::DeleteWorktree(wt.path.clone()));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            close = true;
                        }
                    });
                });
            }
            Some(Dialog::Confirm { title, message, label, args, network }) => {
                // Same layout as the original's alerts (sub_1003b95c0, sub_10026d55c…): a question,
                // an informative line, then the action's verb and Cancel. The original says
                // "You can't undo this action"; here every local action can be undone.
                let verb = title.split_whitespace().next().unwrap_or("Continue").to_string();
                egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
                    ui.set_width(400.0);
                    ui.label(RichText::new(message.as_str()).size(theme::FS_TITLE).color(theme::c().strong));
                    ui.add_space(4.0);
                    ui.label(RichText::new("You can take this back with Undo (⌘Z).").color(theme::c().dim));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new(verb).color(theme::c().on_accent)).fill(theme::tint(theme::c().deleted, theme::c().elevated, 0.8)).stroke(Stroke::new(1.0_f32, theme::tint(theme::c().deleted, theme::c().elevated, 0.8)))).clicked() {
                            cmds.push(Cmd::Git { label: label.clone(), args: args.clone(), network: *network });
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            close = true;
                        }
                    });
                });
            }
            Some(Dialog::NewGroup { path, name }) => {
                egui::Modal::new(egui::Id::new("new_group")).show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.heading("New group");
                    let repo = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    ui.label(RichText::new(format!("{repo} goes into it")).color(theme::c().dim));
                    ui.add_space(8.0);
                    ui.add(egui::TextEdit::singleline(name).hint_text(faint_hint("Clients, Side projects…")).desired_width(f32::INFINITY)).request_focus();
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let ok = !name.trim().is_empty();
                        let create = ui.add_enabled(ok, egui::Button::new(RichText::new("Create").color(theme::c().on_accent)).fill(theme::c().accent).stroke(Stroke::new(1.0_f32, theme::c().accent))).clicked();
                        if create || (ok && ui.input(|i| i.key_pressed(Key::Enter))) {
                            cmds.push(Cmd::RepoGroup(path.clone(), Some(name.trim().to_string())));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                            close = true;
                        }
                    });
                });
            }
        }
        if close {
            self.dialog = None;
        }
    }

}

impl App {
    fn keyboard(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        if self.tab().dialog.is_some() {
            return;
        }
        // ⌘, opens or closes Settings; while it is open, it takes the keyboard.
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Comma)) {
            cmds.push(Cmd::Settings);
            return;
        }
        if self.settings_open {
            return;
        }
        // ⌘K opens the command palette from anywhere. Plain ⌘ only: ⇧⌘P is Push and ⌃⌘O Show in
        // Finder in the menu bar, which hands those keys on to egui while they are greyed out.
        let plain = ctx.input(|i| i.modifiers.command && !i.modifiers.shift && !i.modifiers.ctrl && !i.modifiers.alt);
        if plain && ctx.input(|i| i.key_pressed(Key::K) || i.key_pressed(Key::P)) {
            cmds.push(Cmd::Palette);
            return;
        }
        // ⌘O switches repository; ⌘1…⌘9 open the favourites.
        if plain && ctx.input(|i| i.key_pressed(Key::O)) {
            cmds.push(Cmd::Switcher);
            return;
        }
        // ⌘B goes to the Project view and back to the history.
        let (board_key, escape) = board::board_keys(ctx);
        if board_key {
            cmds.push(Cmd::Board);
            return;
        }
        // ⌘J hides the details panel or brings it back where it was.
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::J)) {
            cmds.push(Cmd::ToggleDetails);
        }
        self.favorite_keys(ctx, cmds);
        if self.switcher.is_some() || self.palette.is_some() || ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let t = self.tab_mut();
        if t.board.open {
            // Esc closes the ticket's detail; the history's keys wait.
            if escape {
                t.board_escape(ctx.input(|i| i.time));
            }
            return;
        }
        let (down, up, refresh, undo) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::ArrowUp),
                i.modifiers.command && i.key_pressed(Key::R),
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::Z),
            )
        });
        if refresh {
            cmds.push(Cmd::Reload);
        }
        if undo {
            cmds.push(Cmd::Undo);
        }
        let n = t.commits.len() as i64;
        if n == 0 || !(up || down) {
            return;
        }
        let cur: i64 = match t.sel {
            Sel::Wip => -1,
            Sel::Commit(i) => i as i64,
            Sel::None => -1,
        };
        let next = if down { (cur + 1).min(n - 1) } else { cur - 1 };
        if next < 0 {
            if !t.status.is_empty() {
                cmds.push(Cmd::Select(Sel::Wip));
            }
        } else {
            cmds.push(Cmd::Select(Sel::Commit(next as usize)));
            t.scroll_to = Some(next as usize);
        }
    }

    // ---------------------------------------------------------------- menu bar

    /// What the macOS menu bar needs to grey out or tick its items.
    fn menu_state(&self, ctx: &egui::Context) -> menu::State {
        let t = self.tab();
        menu::State {
            tabs: self.tabs.len(),
            repo: t.repo.is_some(),
            busy: t.busy.is_some(),
            modal: t.dialog.is_some(),
            settings: self.settings_open,
            text_focus: ctx.wants_keyboard_input(),
            can_undo: !t.undo.is_empty(),
            changes: !t.status.is_empty(),
            // The Commit button's rule.
            can_commit: !t.summary.trim().is_empty() && (!t.status.staged.is_empty() || t.amend),
            upstream: t.head_has_upstream(),
            branch: t.branch.is_some(),
            stashes: !t.stashes.is_empty(),
            head_shown: t.index.contains_key(&t.head),
            details: t.details_layout,
        }
    }

    /// The action behind a menu item: the same `Cmd` its button or key pushes.
    fn menu_cmd(&self, id: menu::Id) -> Option<Cmd> {
        use menu::Id;
        let root = || self.tab().repo.as_ref().map(|r| r.root.clone());
        Some(match id {
            // About opens AppKit's panel itself; the other edit items only act in a text field.
            Id::About | Id::NoRecent | Id::Redo | Id::Cut | Id::Copy | Id::Paste | Id::SelectAll => return None,
            Id::Settings => Cmd::Settings,
            Id::OpenRepository => Cmd::OpenDialog,
            Id::Switcher => Cmd::Switcher,
            Id::Palette => Cmd::Palette,
            Id::Undo => Cmd::Undo,
            Id::ShowChanges => Cmd::Select(Sel::Wip),
            Id::ToggleDetails => Cmd::ToggleDetails,
            Id::DetailsRight => Cmd::SetDetails(DetailsLayout::shown(DetailsPlace::Right)),
            Id::DetailsBottom => Cmd::SetDetails(DetailsLayout::shown(DetailsPlace::Bottom)),
            Id::ShowHead => Cmd::SelectHash(self.tab().head.clone()),
            Id::Refresh => Cmd::Reload,
            Id::Commit => Cmd::Commit,
            Id::Fetch => Cmd::Fetch { all: false, quiet: false },
            Id::FetchAll => Cmd::Fetch { all: true, quiet: false },
            Id::Pull => Cmd::Pull,
            Id::Push => Cmd::Push,
            Id::Stash => Cmd::stash(),
            Id::PopStash => Cmd::pop_stash(),
            Id::CheckoutBranch => Cmd::PaletteCheckout,
            Id::NewBranch => Cmd::new_ref_at_head(false),
            Id::NewTag => Cmd::new_ref_at_head(true),
            Id::ShowInFinder => Cmd::Reveal(root()?),
            Id::OpenInTerminal => Cmd::OpenTerminal(root()?),
            Id::Website => Cmd::OpenUrl(menu::REPO_URL.into()),
            Id::Issues => Cmd::OpenUrl(menu::ISSUES_URL.into()),
            Id::NewTab => Cmd::NewTab,
            Id::CloseTab => Cmd::CloseTab(self.active),
            Id::NextTab => Cmd::ShowTab { next: true },
            Id::PrevTab => Cmd::ShowTab { next: false },
        })
    }

    /// Installs the menu bar on the first frame, then hands it this frame's state.
    fn menu_sync(&self, ctx: &egui::Context) {
        menu::install(ctx);
        let recent: Vec<(String, String)> = self.library.recent().iter().map(|e| (e.name(), e.group_name())).collect();
        menu::sync(&self.menu_state(ctx), &menu::recent_titles(&recent));
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frame += 1;
        ctx.request_repaint();
        if self.frame >= 120 && !self.shot_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.shot_requested = true;
        }
        let shot = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
        if let Some(img) = shot {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = image::save_buffer(&path, &bytes, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                eprintln!("could not save screenshot: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if self.frame > 600 {
            eprintln!("screenshot timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// An Edit menu item as the event egui's text field expects from the keyboard.
fn edit_event(id: menu::Id) -> Option<egui::Event> {
    let key = |key, shift| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers { mac_cmd: true, command: true, shift, ..Default::default() } };
    Some(match id {
        menu::Id::Undo => key(Key::Z, false),
        menu::Id::Redo => key(Key::Z, true),
        menu::Id::SelectAll => key(Key::A, false),
        menu::Id::Cut => egui::Event::Cut,
        menu::Id::Copy => egui::Event::Copy,
        menu::Id::Paste => egui::Event::Paste(menu::pasteboard_text()?),
        _ => return None,
    })
}

impl eframe::App for App {
    /// The menu bar's picks. When a menu key equivalent matches, AppKit takes the key and egui
    /// never sees it, so each pick runs exactly once: Edit items as text-field events (Copy also
    /// copies text selected in a label), the others as the `Cmd` of their button.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let text = ctx.wants_keyboard_input();
        // Tab keys, taken before egui sees them (⌃Tab would otherwise move the focus). A dialog
        // or the Settings window keeps the keyboard, as it greys out the menu items.
        if self.tab().dialog.is_none() && !self.settings_open {
            let active = self.active;
            let mut keys = Vec::new();
            raw.events.retain(|e| match e {
                egui::Event::Key { key, pressed, modifiers, .. } => match tabs::TabKey::of(*key, *modifiers) {
                    Some(k) => {
                        if *pressed {
                            keys.push(k.cmd(active));
                        }
                        false
                    }
                    None => true,
                },
                _ => true,
            });
            self.menu_cmds.extend(keys);
        }
        for pick in menu::take() {
            match pick {
                menu::Pick::Recent(n) => self.menu_cmds.extend(self.library.recent().get(n).map(|e| Cmd::Open(e.path.clone()))),
                menu::Pick::Item(id) if id.is_edit() && (text || id == menu::Id::Copy) => raw.events.extend(edit_event(id)),
                menu::Pick::Item(id) => self.menu_cmds.extend(self.menu_cmd(id)),
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The mode on screen can change with macOS (System), so the colours follow it each frame.
        theme::sync(ctx);
        // Every tab's background work, not only the one on screen.
        tab::poll_tabs(&mut self.tabs, ctx, self.library.settings, self.screenshot.is_none());
        let focused = ctx.input(|i| i.focused);
        if focused && !self.was_focused {
            for t in &mut self.tabs {
                t.refresh_status();
            }
        }
        self.take_notices();
        self.was_focused = focused;
        let mut cmds = std::mem::take(&mut self.menu_cmds);

        crate::titlebar::place_buttons();
        egui::TopBottomPanel::top("toolbar")
            .exact_height(crate::titlebar::TOOLBAR_H)
            .frame(egui::Frame::new().fill(theme::c().sidebar))
            .show(ctx, |ui| {
                let key = self.tab().key;
                ui.push_id(key, |ui| self.tab_mut().toolbar(ui, &mut cmds));
            });

        // The tabs, from two on (issue #59): with one, the window looks as before tabs.
        if self.tabs.len() > 1 {
            egui::TopBottomPanel::top("tabs")
                .exact_height(tabs::STRIP_H)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::c().sidebar))
                .show(ctx, |ui| self.tab_strip(ui, &mut cmds));
        }

        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(egui::Frame::new().fill(theme::c().sidebar).inner_margin(Margin::symmetric(12, 4)))
            .show(ctx, |ui| {
                let t = &self.tabs[self.active];
                ui.horizontal_centered(|ui| {
                    let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.0, t.ref_color(&t.head));
                    ui.label(RichText::new(t.branch.clone().unwrap_or_else(|| "detached HEAD".into())).size(theme::FS_SECONDARY).color(theme::c().text));
                    if let Some((a, b)) = t.ahead_behind {
                        ui.label(RichText::new(format!("{a} ahead, {b} behind")).color(theme::c().dim).size(theme::FS_SECONDARY));
                    }
                    ui.label(RichText::new("·").color(theme::c().faint));
                    ui.label(RichText::new(format!("{} commits", t.commits.len())).color(theme::c().dim).size(theme::FS_SECONDARY));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let a = self.library.appearance;
                        let tip = format!("Appearance: {} — click for {}", a.label(), a.next().label());
                        if appearance_button(ui, a).on_hover_text(tip).clicked() {
                            cmds.push(Cmd::SetAppearance(a.next()));
                        }
                        let d = t.details_layout;
                        let tip = if d.hidden { format!("Details panel: Hidden — ⌘J to show it ({})", d.place.label().to_lowercase()) } else { format!("Details panel: {} — ⌘J to hide", d.label()) };
                        let btn = details_button(ui, d).on_hover_text(tip);
                        egui::Popup::menu(&btn).align(egui::RectAlign::TOP_END).show(|ui| {
                            ui.set_min_width(180.0);
                            ui.label(RichText::new("Details panel").size(theme::FS_SMALL).color(theme::c().faint));
                            for place in DetailsPlace::ALL {
                                if ui.add(egui::Button::selectable(!d.hidden && d.place == place, place.label())).clicked() {
                                    cmds.push(Cmd::SetDetails(DetailsLayout::shown(place)));
                                }
                            }
                            if ui.add(egui::Button::selectable(d.hidden, "Hidden").right_text(RichText::new("⌘J").color(theme::c().faint))).clicked() {
                                cmds.push(Cmd::SetDetails(d.hide()));
                            }
                        });
                        ui.add_space(4.0);
                        let where_ = t.repo.as_ref().map(|r| r.root.display().to_string()).unwrap_or_else(|| "Sample project — open a repository to get started".into());
                        ui.label(RichText::new(where_).color(theme::c().faint).size(theme::FS_SECONDARY));
                    });
                });
            });

        // The Project view keeps the sidebar and the details panel; only the centre changes.
        let board_open = self.tab().board.open;
        egui::SidePanel::left("sidebar")
        .resizable(true)
        .default_width(230.0)
        .width_range(190.0..=420.0)
        .frame(egui::Frame::new().fill(theme::c().sidebar).inner_margin(Margin::symmetric(8, 0)))
        .show(ctx, |ui| {
            // Each tab's own id scope keeps its scroll positions and text fields apart.
            let key = self.tab().key;
            ui.push_id(key, |ui| self.tab_mut().sidebar(ui, &mut cmds));
        });

        // Declared after the sidebar, so at the bottom it spans the history only.
        let details_frame = egui::Frame::new().fill(theme::c().bg).inner_margin(Margin::symmetric(16, 0));
        match self.tab().details_layout {
            DetailsLayout { hidden: true, .. } => {}
            DetailsLayout { place: DetailsPlace::Right, .. } => {
                egui::SidePanel::right("details")
                    .resizable(true)
                    .default_width(330.0)
                    .width_range(280.0..=600.0)
                    .frame(details_frame)
                    .show(ctx, |ui| {
                        let key = self.tab().key;
                        ui.push_id(key, |ui| self.tab_mut().details_panel(ui, &mut cmds));
                    });
            }
            DetailsLayout { place: DetailsPlace::Bottom, .. } => {
                let max_h = (ctx.available_rect().height() * 0.7).max(DETAILS_BOTTOM_MIN_H);
                egui::TopBottomPanel::bottom("details_bottom")
                    .resizable(true)
                    .default_height(300.0)
                    .height_range(DETAILS_BOTTOM_MIN_H..=max_h)
                    .frame(details_frame)
                    .show(ctx, |ui| {
                        let key = self.tab().key;
                        ui.push_id(key, |ui| self.tab_mut().details_panel(ui, &mut cmds));
                    });
            }
        }

        let settings_open = self.settings_open;
        let t = &mut self.tabs[self.active];
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::c().bg)).show(ctx, |ui| ui.push_id(t.key, |ui| {
            if board_open {
                t.board_view(ui, &mut cmds);
                return;
            }
            if t.is_demo() {
                egui::Frame::new().fill(theme::c().accent_soft).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("This is a sample history so you can look around.").color(theme::c().text));
                        if ui.button("Open a repository…").clicked() {
                            cmds.push(Cmd::OpenDialog);
                        }
                    });
                });
            }
            if t.diff.is_some() {
                t.diff_view(ui, &mut cmds, settings_open);
            } else if t.commits.is_empty() && t.status.is_empty() {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| ui.label(RichText::new("This repository has no commits yet.").color(theme::c().dim)));
            } else {
                t.commit_list(ui, &mut cmds);
            }
        }));

        let t = self.tab_mut();
        t.dialogs(ctx, &mut cmds);
        t.cleanup_window(ctx, &mut cmds);
        self.keyboard(ctx, &mut cmds);
        self.palette_ui(ctx, &mut cmds);
        if self.tab().dialog.is_none() {
            self.switcher_ui(ctx, &mut cmds);
            self.settings_ui(ctx, &mut cmds);
        }

        if let Some((msg, err, at)) = &self.toast {
            if at.elapsed().as_secs_f32() < 4.0 {
                egui::Area::new(egui::Id::new("toast")).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -40.0)).show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(theme::c().elevated)
                        .stroke(Stroke::new(1.0_f32, theme::tint(if *err { theme::c().deleted } else { theme::c().added }, theme::c().elevated, 0.6)))
                        .shadow(ctx.style().visuals.popup_shadow)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.set_max_width(520.0);
                            ui.label(RichText::new(msg.as_str()).color(theme::c().strong));
                        });
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            } else {
                self.toast = None;
            }
        }
        if self.tabs.iter().any(|t| t.busy.is_some() || t.ai.busy() || t.project.loading.is_some()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        self.run(cmds, ctx);
        self.menu_sync(ctx);
        self.screenshot_step(ctx);
    }
}
