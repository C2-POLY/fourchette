//! What the window shows: toolbar, sidebar, history, details, diff, dialogs.
use super::*;
use crate::git::FileChange;
use crate::library::{DetailsLayout, DetailsPlace};
use crate::menu;
use crate::sidebar;
use crate::ui::*;
use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Key, Layout, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui};

/// Width of the SHA column at the left of the commit list.
const SHA_COL_W: f32 = 70.0;
/// How far a folder's branches are indented in the sidebar.
const FOLDER_STEP: f32 = 26.0;
/// The bottom details panel's smallest height.
const DETAILS_BOTTOM_MIN_H: f32 = 160.0;
/// Below this width the bottom panel keeps the one-column layout of the right panel.
const WIDE_DETAILS_MIN_W: f32 = 640.0;

/// Moves the centre's scroll bars off its right and bottom edges. Drawn after the side panels,
/// a bar touching an edge took the drag meant for the details panel's resize handle there, so
/// the history scrolled instead and the panel jumped.
fn keep_panel_edges_free(ui: &mut Ui) {
    let grab = ui.style().interaction.resize_grab_radius_side;
    let scroll = &mut ui.spacing_mut().scroll;
    scroll.bar_outer_margin = scroll.bar_outer_margin.max(grab + 1.0);
}

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

    fn toolbar(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>, update: bool) {
        // The toolbar is the window's top edge (no title bar).
        crate::titlebar::drag_area(ui);
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
            // The issue whose number is in the branch's name, from the project (#36).
            if self.branch_issue().is_some() {
                ui.add_space(4.0);
                self.issue_chip(ui, cmds);
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
                if ui.button("📝  Write the changelog…").clicked() {
                    cmds.push(Cmd::AiChangelog);
                }
                if ui.add_enabled(self.branch.is_some(), egui::Button::new("📤  Draft the pull request…")).on_disabled_hover_text("Check out a branch first").clicked() {
                    cmds.push(Cmd::AiDraftPr);
                }
                let conflicts = super::resolve::conflicted(&self.status);
                if ui.add_enabled(!conflicts.is_empty(), egui::Button::new("🔀  Resolve conflicts…")).on_disabled_hover_text("No file is in conflict").clicked() {
                    cmds.push(Cmd::AiResolve(Vec::new()));
                }
                if crate::ai::find_claude().is_none() {
                    ui.separator();
                    ui.label(RichText::new("Claude Code isn't installed on this Mac.").small().color(theme::c().modified));
                }
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(faint_hint("Search commits")).desired_width(220.0).margin(Margin::symmetric(10, 6)));
                // A newer release on GitHub (issue #80).
                if update {
                    ui.add_space(10.0);
                    if update_pill(ui, "Update available").on_hover_text("A new version of Fourchette is ready to install").clicked() {
                        cmds.push(Cmd::UpdateDialog(true));
                    }
                }
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

    pub(super) fn section(&mut self, ui: &mut Ui, title: &str, count: usize, icon: Option<fn(&egui::Painter, Pos2, Color32)>) -> bool {
        let open = !self.collapsed.contains(title);
        if section_header(ui, title, count, open, icon).clicked() {
            if open {
                self.collapsed.insert(title.to_string());
            } else {
                self.collapsed.remove(title);
            }
        }
        open
    }

    /// A folder row of a branch list, folded or not as `self.collapsed` remembers; a filter opens it.
    fn folder(&mut self, ui: &mut Ui, key: &str, name: &str, count: usize, indent: f32, filtering: bool) -> bool {
        let open = filtering || !self.collapsed.contains(key);
        if sidebar::folder_row(ui, name, count, open, indent).clicked() && !filtering {
            if open {
                self.collapsed.insert(key.to_string());
            } else {
                self.collapsed.remove(key);
            }
        }
        open
    }

    /// A local branch of the sidebar, shown as `label`, with what it tracks.
    #[allow(clippy::too_many_arguments)]
    fn local_branch(&self, ui: &mut Ui, b: &GitRef, label: &str, indent: f32, cur: &str, remotes: &[String], remote_refs: &std::collections::HashSet<String>, cmds: &mut Vec<Cmd>) {
        let gone = !b.upstream.is_empty() && !remote_refs.contains(&b.upstream);
        let t = sidebar::Tracking { remote: sidebar::upstream_remote(&b.upstream, remotes).or(if b.upstream.is_empty() { None } else { b.upstream.split_once('/').map(|x| x.0) }), gone, ahead: b.ahead, behind: b.behind };
        let tip = if b.upstream.is_empty() {
            format!("{} has no upstream: it exists only on this Mac", b.name)
        } else if gone {
            format!("{}\nTracks {}, which no longer exists on the remote", b.name, b.upstream)
        } else {
            format!("{}\nTracks {} · {} ahead, {} behind", b.name, b.upstream, b.ahead, b.behind)
        };
        let pr = self.pull_of(b);
        let tip = match pr {
            Some(p) => format!("{tip}\n{}", p.describe()),
            None => tip,
        };
        let resp = sidebar::local_branch_row(ui, label, self.ref_color(&b.target), b.is_head, indent, &t, pr).on_hover_text(tip);
        if resp.clicked() {
            cmds.push(Cmd::SelectHash(b.target.clone()));
        }
        if resp.double_clicked() {
            cmds.extend(label_checkout(b));
        }
        resp.context_menu(|ui| branch_menu(ui, b, None, cur, &self.refs, &self.worktrees, cmds));
    }

    /// A branch of a remote: a link icon when a local branch tracks it, "not local" otherwise.
    fn remote_branch(&self, ui: &mut Ui, r: &GitRef, label: &str, indent: f32, locals: &[GitRef], cmds: &mut Vec<Cmd>) {
        let tracked = sidebar::is_tracked(&r.name, locals);
        let pr = self.pull_of(r);
        let mut tip = if tracked { format!("{} is tracked by a local branch", r.name) } else { format!("{} has no local branch", r.name) };
        if let Some(p) = pr {
            tip = format!("{tip}\n{}", p.describe());
        }
        let resp = sidebar::remote_branch_row(ui, label, self.ref_color(&r.target), indent, tracked, pr).on_hover_text(tip);
        if resp.clicked() {
            cmds.push(Cmd::SelectHash(r.target.clone()));
        }
        if resp.double_clicked() {
            cmds.push(checkout_remote(r));
        }
        resp.context_menu(|ui| crate::ui::remote_branch_menu(ui, r, cmds));
    }

    fn sidebar(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        ui.add_space(8.0);
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text(faint_hint("Filter branches")).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 6)));
        ui.add_space(8.0);
        let f = self.filter.to_lowercase();
        let keep = |n: &str| f.is_empty() || n.to_lowercase().contains(&f);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {

            let filtering = !f.is_empty();
            let remote_names: Vec<String> = self.remotes.iter().map(|r| r.0.clone()).collect();
            let remote_set: std::collections::HashSet<String> = self.refs.iter().filter(|r| r.kind == RefKind::Remote).map(|r| r.name.clone()).collect();
            let all_locals: Vec<GitRef> = self.refs.iter().filter(|r| r.kind == RefKind::Local).cloned().collect();

            let locals: Vec<GitRef> = all_locals.iter().filter(|r| keep(&r.name)).cloned().collect();
            if self.section(ui, "Local", locals.len(), Some(sidebar::laptop_icon)) {
                let cur = self.branch.clone().unwrap_or_default();
                for e in sidebar::group_folders(&locals, |b| b.name.as_str()) {
                    match e {
                        sidebar::Entry::Item(b) => self.local_branch(ui, b, &b.name, 0.0, &cur, &remote_names, &remote_set, cmds),
                        sidebar::Entry::Folder(dir, items) => {
                            if self.folder(ui, &format!("folder:local:{dir}"), &dir, items.len(), 0.0, filtering) {
                                for b in items {
                                    let short = b.name.strip_prefix(&format!("{dir}/")).unwrap_or(&b.name);
                                    self.local_branch(ui, b, short, FOLDER_STEP, &cur, &remote_names, &remote_set, cmds);
                                }
                            }
                        }
                    }
                }
            }

            let remotes: Vec<GitRef> = self.refs.iter().filter(|r| r.kind == RefKind::Remote && keep(&r.name)).cloned().collect();
            if self.section(ui, "Remote", remotes.len(), Some(sidebar::cloud_icon)) {
                let mut groups: Vec<(String, Vec<GitRef>)> = Vec::new();
                for r in remotes {
                    let remote = r.name.split_once('/').map(|x| x.0).unwrap_or(&r.name).to_string();
                    match groups.iter_mut().find(|g| g.0 == remote) {
                        Some(g) => g.1.push(r),
                        None => groups.push((remote, vec![r])),
                    }
                }
                for (remote, items) in groups {
                    let url = self.remotes.iter().find(|r| r.0 == remote).map(|r| r.1.clone()).unwrap_or_default();
                    let key = format!("remote:{remote}");
                    let open = filtering || !self.collapsed.contains(&key);
                    let mut resp = sidebar::remote_row(ui, &remote, &sidebar::short_url(&url), open);
                    if !url.is_empty() {
                        resp = resp.on_hover_text(&url);
                    }
                    if resp.clicked() && !filtering {
                        if open {
                            self.collapsed.insert(key);
                        } else {
                            self.collapsed.remove(&key);
                        }
                    }
                    if !open {
                        continue;
                    }
                    for e in sidebar::group_folders(&items, |r| r.name.split_once('/').map(|x| x.1).unwrap_or(&r.name)) {
                        match e {
                            sidebar::Entry::Item(r) => self.remote_branch(ui, r, r.name.split_once('/').map(|x| x.1).unwrap_or(&r.name), FOLDER_STEP, &all_locals, cmds),
                            sidebar::Entry::Folder(dir, children) => {
                                if self.folder(ui, &format!("folder:{remote}:{dir}"), &dir, children.len(), FOLDER_STEP, filtering) {
                                    for r in children {
                                        let rest = r.name.split_once('/').map(|x| x.1).unwrap_or(&r.name);
                                        let label = rest.strip_prefix(&format!("{dir}/")).unwrap_or(rest);
                                        self.remote_branch(ui, r, label, FOLDER_STEP * 2.0, &all_locals, cmds);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let tags: Vec<GitRef> = self.refs.iter().filter(|r| r.kind == RefKind::Tag && keep(&r.name)).cloned().collect();
            if self.section(ui, "Tags", tags.len(), None) {
                for t in &tags {
                    let resp = side_item(ui, "tag", &t.name, None, false, None, false);
                    if resp.clicked() {
                        cmds.push(Cmd::SelectHash(t.target.clone()));
                    }
                    resp.context_menu(|ui| {
                        crate::ui::menu_rows(ui);
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
            if !linked.is_empty() && self.section(ui, "Worktrees", linked.len(), None) {
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
                        crate::ui::menu_rows(ui);
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
            if self.section(ui, "Stashes", stashes.len(), None) {
                for s in &stashes {
                    let resp = side_item(ui, "stash", &s.message, None, false, None, false).on_hover_text(&s.name);
                    resp.context_menu(|ui| {
                        crate::ui::menu_rows(ui);
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

    fn commit_list(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let wip = !self.status.is_empty();
        let n = self.commits.len() + wip as usize;
        let full_w = ui.available_width();
        let sha_w = SHA_COL_W;
        let graph_w = graph_width(self.max_lanes).min((full_w * 0.3).max(60.0));
        // Keep at least 280 px for the message: drop the date, then the author when narrow.
        let room = full_w - sha_w - graph_w - 280.0;
        let author_w = if room >= 150.0 { 150.0 } else { 0.0 };
        let date_w = if room >= 150.0 + 104.0 { 104.0 } else { 0.0 };
        let show_meta = author_w > 0.0;

        let (hrect, _) = ui.allocate_exact_size(vec2(full_w, 30.0), Sense::hover());
        {
            let p = ui.painter();
            p.rect_filled(hrect, CornerRadius::ZERO, theme::c().bg);
            p.line_segment([hrect.left_bottom(), hrect.right_bottom()], Stroke::new(1.0_f32, theme::c().border));
            let y = hrect.center().y;
            let f = theme::small();
            p.text(pos2(hrect.left() + 12.0, y), Align2::LEFT_CENTER, "SHA", f.clone(), theme::c().faint);
            p.text(pos2(hrect.left() + sha_w + 4.0, y), Align2::LEFT_CENTER, "Graph", f.clone(), theme::c().faint);
            p.text(pos2(hrect.left() + sha_w + graph_w + 8.0, y), Align2::LEFT_CENTER, "Message", f.clone(), theme::c().faint);
            if show_meta {
                let r = hrect.right();
                p.text(pos2(r - date_w - author_w + 4.0, y), Align2::LEFT_CENTER, "Author", f.clone(), theme::c().faint);
                if date_w > 0.0 {
                    p.text(pos2(r - date_w + 4.0, y), Align2::LEFT_CENTER, "Date", f, theme::c().faint);
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
                    self.wip_row(ui, rect, &resp, sha_w, graph_w, cmds);
                    continue;
                }
                let i = vis - wip as usize;
                let selected = self.sel == Sel::Commit(i);
                let c = &self.commits[i];
                let row = &self.rows[i];
                let lane_col = theme::lane_color(row.color);
                let matched = search.is_empty() || c.subject.to_lowercase().contains(&search) || c.author.to_lowercase().contains(&search) || c.hash.starts_with(&search);
                let p = ui.painter_at(rect);
                let gx = rect.left() + sha_w;
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
                let sc = if matched { theme::c().dim } else { theme::c().faint };
                p.text(pos2(rect.left() + 12.0, y), Align2::LEFT_CENTER, short(&c.hash), theme::mono(), sc);
                let mx = gx + graph_w + 8.0;
                let msg_right = rect.right() - date_w - author_w - 12.0;
                // Branch / tag pills first (at most 40% of the message area), then the message.
                let mut text_x = mx;
                // The pill under a right-click, whose branch menu shows instead of the commit's.
                let mut hit_pill: Option<(GitRef, Option<GitRef>)> = None;
                if let Some(refs) = self.by_target.get(&c.hash) {
                    let all = ref_pills(refs);
                    let max_x = mx + ((msg_right - mx) * 0.4).max(0.0);
                    let mut x = mx;
                    let mut shown: Vec<(Rect, &GitRef, Option<&GitRef>)> = Vec::new();
                    for (k, pill) in all.iter().enumerate() {
                        // A pull request badge sits inside the pill, after its name.
                        let pull = self.pill_pull(pill);
                        let w = pill.width(&p, badge_w(pull));
                        if x + w > max_x && k > 0 {
                            let mg = p.layout_no_wrap(format!("+{}", all.len() - k), theme::small(), theme::c().dim);
                            let mr = Rect::from_min_size(pos2(x, y - 9.0), vec2(mg.size().x + 10.0, 18.0));
                            p.rect_filled(mr, CornerRadius::same(4), theme::c().elevated);
                            p.galley(pos2(mr.left() + 5.0, y - mg.size().y / 2.0), mg, theme::c().dim);
                            x = mr.right() + theme::REF_GAP;
                            break;
                        }
                        // A pill wider than the room left ends its name with "…", never cut at the edge.
                        let pr = Rect::from_min_size(pos2(x, y - 9.0), vec2(w.min((max_x - x).max(24.0)), 18.0));
                        draw_ref_pill(&p, pr, pill, lane_col, bg, pull);
                        shown.push((pr, pill.main, pill.remote));
                        x = pr.right() + theme::REF_GAP;
                    }
                    text_x = x + 2.0;
                    // A double-click on a pill checks its branch out: the local one of a merged pill.
                    if resp.double_clicked() {
                        if let Some(q) = ui.input(|i| i.pointer.interact_pos()) {
                            if let Some((_, r, _)) = shown.iter().find(|(pr, _, _)| pr.contains(q)) {
                                cmds.extend(label_checkout(r));
                            }
                        }
                    }
                    if resp.secondary_clicked() {
                        let q = ui.input(|i| i.pointer.interact_pos());
                        hit_pill = q.and_then(|q| shown.iter().find(|(pr, _, _)| pr.contains(q))).map(|(_, m, r)| ((*m).clone(), r.cloned()));
                    }
                    // Hovering the pills lists every one of the commit in full, the hidden ones included.
                    if resp.hovered() && ui.input(|i| i.pointer.hover_pos()).is_some_and(|q| q.x >= mx && q.x < x) {
                        resp.clone().on_hover_ui_at_pointer(|ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            let tip_bg = ui.visuals().window_fill;
                            for pill in &all {
                                let pull = self.pill_pull(pill);
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(vec2(pill.width(ui.painter(), badge_w(pull)), 18.0), Sense::hover());
                                    draw_ref_pill(ui.painter(), r, pill, lane_col, tip_bg, pull);
                                    if let Some(remote) = pill.remote {
                                        ui.label(RichText::new(&remote.name).small().color(theme::c().dim));
                                    }
                                });
                                if let Some(q) = pull {
                                    ui.label(RichText::new(q.describe()).small().color(theme::c().dim));
                                }
                            }
                        });
                    }
                }
                fit_text(&p, pos2(text_x, y), Align2::LEFT_CENTER, &c.subject, theme::body(), tc, msg_right - text_x);
                if show_meta {
                    let ax = rect.right() - date_w - author_w;
                    avatar(&p, pos2(ax + 12.0, y), 9.0, &c.author);
                    fit_text(&p, pos2(ax + 27.0, y), Align2::LEFT_CENTER, &c.author, theme::secondary(), sc, author_w - 35.0);
                    if date_w > 0.0 {
                        fit_text(&p, pos2(rect.right() - date_w + 4.0, y), Align2::LEFT_CENTER, &git::relative_time(c.time), theme::secondary(), sc, date_w - 8.0);
                    }
                }

                if resp.clicked() {
                    cmds.push(Cmd::Select(Sel::Commit(i)));
                }
                let (hash, subject, branch) = (c.hash.clone(), c.subject.clone(), self.branch.clone());
                let pill_key = resp.id.with("pill_menu");
                if resp.secondary_clicked() {
                    ui.data_mut(|d| d.insert_temp(pill_key, hit_pill));
                }
                let pill_menu: Option<(GitRef, Option<GitRef>)> = ui.data(|d| d.get_temp(pill_key)).flatten();
                resp.context_menu(|ui| match &pill_menu {
                    Some((m, rm)) if m.kind == RefKind::Local => branch_menu(ui, m, rm.as_ref(), branch.as_deref().unwrap_or(""), &self.refs, &self.worktrees, cmds),
                    Some((m, _)) if m.kind == RefKind::Remote => crate::ui::remote_branch_menu(ui, m, cmds),
                    _ => commit_menu(ui, &hash, &subject, branch.as_deref(), cmds),
                });
            }
        });
    }

    fn wip_row(&self, ui: &Ui, rect: Rect, resp: &egui::Response, sha_w: f32, graph_w: f32, cmds: &mut Vec<Cmd>) {
        let p = ui.painter_at(rect);
        let bg = if self.sel == Sel::Wip {
            theme::c().select
        } else if resp.hovered() {
            theme::c().hover
        } else {
            theme::c().bg
        };
        p.rect_filled(rect, CornerRadius::ZERO, bg);
        let gx = rect.left() + sha_w;
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
        let collapsed: std::collections::HashSet<String> = self.collapsed_dirs.iter().filter(|(s, _)| *s == staged).map(|(_, p)| p.clone()).collect();
        egui::Frame::new().fill(theme::c().sidebar).stroke(Stroke::new(1.0_f32, theme::c().border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(4)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(if staged { "staged" } else { "unstaged" }).max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                if self.files_tree {
                    for row in crate::filetree::rows(&files, &collapsed) {
                        match row {
                            crate::filetree::Row::File { file, depth } => file_item(ui, file, &files, staged, current.as_deref(), Some(depth), cmds),
                            crate::filetree::Row::Dir { name, path, depth, open, count } => dir_item(ui, (&name, &path, count), depth, open, staged, cmds),
                        }
                    }
                } else {
                    for f in &files {
                        file_item(ui, f, &files, staged, current.as_deref(), None, cmds);
                    }
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
                // Drawn first, so it sits at the right edge, after Stage all.
                if let Some(on) = crate::ui::files_view_switch(ui, self.files_tree) {
                    cmds.push(Cmd::SetFilesTree(on));
                }
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
                ui.add_space(6.0);
                if let Some((split, whole)) = crate::ui::diff_view_switch(ui, self.diff_split, self.diff_whole) {
                    cmds.push(Cmd::SetDiffView { split, whole });
                }
                if self.status.unstaged.iter().any(|f| f.code == 'U' && f.path == d.title) && ui.button("Resolve with Claude…").clicked() {
                    cmds.push(Cmd::AiResolve(vec![d.title.clone()]));
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
        if self.diff_split {
            Self::diff_split_view(ui, &d.text);
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
        let marks: Vec<(bool, bool)> = lines.iter().map(|l| (!is_header(l) && l.starts_with('-'), !is_header(l) && l.starts_with('+'))).collect();
        let (body, strip) = ruler_split(ui);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        let out = ruler_jump(ui, egui::ScrollArea::both()).auto_shrink([false, false]).show_rows(&mut child, h, lines.len(), |ui, range| {
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
        change_ruler(ui, body, strip, &marks, &out);
    }

    /// The diff in two columns: the old file on the left, the new one on the right, each half of
    /// the visible width. One vertical scroll moves both, and a sideways scroll (trackpad or
    /// Shift+wheel) shifts the text of both halves together, so long lines can be read.
    fn diff_split_view(ui: &mut Ui, text: &str) {
        use crate::diff::{Cell, Full, Kind, Row};
        let rows = crate::diff::side_by_side(text);
        let k = theme::c();
        let font = theme::mono();
        let h = 18.0;
        // Where a line's text starts in its half, after the line number.
        const TEXT_X: f32 = 52.0;
        let longest = rows
            .iter()
            .map(|r| match r {
                Row::Pair(l, r) => l.iter().chain(r).map(|c| c.text.replace('\t', "    ").chars().count()).max().unwrap_or(0),
                Row::Full(..) => 0,
            })
            .max()
            .unwrap_or(0);
        let char_w = ui.fonts(|f| f.glyph_width(&font, 'M'));
        let marks: Vec<(bool, bool)> = rows
            .iter()
            .map(|r| match r {
                Row::Pair(l, r) => (l.as_ref().is_some_and(|c| c.kind == Kind::Deleted), r.as_ref().is_some_and(|c| c.kind == Kind::Added)),
                Row::Full(..) => (false, false),
            })
            .collect();
        let (body, strip) = ruler_split(ui);
        let half = ((body.width() - 1.0) / 2.0).floor().max(120.0);
        let max_off = (longest as f32 * char_w - (half - TEXT_X - 8.0)).max(0.0);
        let id = ui.id().with("diff_split_x");
        let off = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0).clamp(0.0, max_off);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        let out = ruler_jump(ui, egui::ScrollArea::vertical()).auto_shrink([false, false]).show_rows(&mut child, h, rows.len(), |ui, range| {
            for i in range {
                let (rect, _) = ui.allocate_exact_size(vec2(half * 2.0 + 1.0, h), Sense::hover());
                let p = ui.painter();
                match &rows[i] {
                    Row::Full(kind, l) => {
                        let (bg, fg) = if *kind == Full::Hunk { (k.accent_soft, k.accent_text) } else { (k.bg, k.faint) };
                        p.rect_filled(rect, CornerRadius::ZERO, bg);
                        p.text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, l.replace('\t', "    "), font.clone(), fg);
                    }
                    Row::Pair(l, r) => {
                        let left = Rect::from_min_size(rect.min, vec2(half, h));
                        let right = Rect::from_min_size(pos2(rect.left() + half + 1.0, rect.top()), vec2(half, h));
                        for (cell, hr) in [(l, left), (r, right)] {
                            let cell: &Option<Cell> = cell;
                            let (bg, fg) = match cell.as_ref().map(|c| c.kind) {
                                None => (k.faint.gamma_multiply(0.07), k.text),
                                Some(Kind::Added) => (theme::tint(k.added, k.bg, 0.13), k.diff_added_text),
                                Some(Kind::Deleted) => (theme::tint(k.deleted, k.bg, 0.13), k.diff_deleted_text),
                                Some(Kind::Context) => (Color32::TRANSPARENT, k.text),
                            };
                            p.rect_filled(hr, CornerRadius::ZERO, bg);
                            if let Some(c) = cell {
                                p.text(pos2(hr.left() + 40.0, hr.center().y), Align2::RIGHT_CENTER, c.num.to_string(), FontId::monospace(theme::FS_SMALL), k.faint);
                                // The text is clipped to its half, right of the line number.
                                let area = Rect::from_min_max(pos2(hr.left() + TEXT_X - 4.0, hr.top()), hr.max);
                                let clipped = p.with_clip_rect(area.intersect(p.clip_rect()));
                                clipped.text(pos2(hr.left() + TEXT_X - off, hr.center().y), Align2::LEFT_CENTER, c.text.replace('\t', "    "), font.clone(), fg);
                            }
                        }
                        // The divider between the halves.
                        p.rect_filled(Rect::from_min_size(pos2(rect.left() + half, rect.top()), vec2(1.0, h)), CornerRadius::ZERO, k.border);
                    }
                }
            }
        });
        if max_off > 0.0 && ui.rect_contains_pointer(out.inner_rect) {
            let dx = ui.input(|i| i.smooth_scroll_delta.x);
            if dx != 0.0 {
                ui.data_mut(|d| d.insert_temp(id, (off - dx).clamp(0.0, max_off)));
            }
        }
        change_ruler(ui, body, strip, &marks, &out);
    }

    // ---------------------------------------------------------------- dialogs, keys, screenshot

    fn dialogs(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        use crate::modal::{self, pill, remote, text, Cap, Hint, Modal, Width};
        // Files with changes, for the Check out dialog's "Stash and reapply".
        let changed = {
            let mut paths: Vec<&str> = self.status.staged.iter().chain(&self.status.unstaged).map(|f| f.path.as_str()).collect();
            paths.sort_unstable();
            paths.dedup();
            paths.len()
        };
        // The Push dialog counts commits against another remote branch when one is chosen.
        let (repo, refs) = (&self.repo, &self.refs);
        let close = match &mut self.dialog {
            None => return,
            Some(Dialog::NewRef { at, label, name, checkout, tag }) => {
                let n = name.trim().to_string();
                let ok = !n.is_empty() && !n.contains(' ');
                let (title, icon, go) = match (*tag, *checkout) {
                    (true, _) => ("New tag", "tag", "Create tag"),
                    (false, true) => ("New branch", "branch", "Create and check out"),
                    (false, false) => ("New branch", "branch", "Create branch"),
                };
                // "commit abc1234" and branch names as pills, "the current commit" as words.
                let mut status = vec![text("Starting at")];
                match label.strip_prefix("commit ") {
                    Some(h) => status.extend([text("commit"), pill(h)]),
                    None if at.as_str() == "HEAD" => status.push(text(label.as_str())),
                    None => status.push(pill(label.as_str())),
                }
                let hint = if n.contains(' ') { vec![Hint::Text("Names can't contain spaces".into())] } else { vec![Hint::Key(Cap::Text("Enter")), Hint::Text("creates it".into())] };
                let out = Modal::new("newref", Width::Small, icon, vec![text(title)]).status(status).hint(hint).primary(go, false, ok).show(ctx, |ui| {
                    modal::text_field(ui, name, if *tag { "v1.0.0" } else { "feature/my-idea" }).request_focus();
                    if !*tag {
                        ui.add_space(8.0);
                        modal::check_row(ui, checkout, "Switch to the new branch", None);
                    }
                });
                if out.primary {
                    let args = if *tag {
                        vec!["tag".into(), n.clone(), at.clone()]
                    } else if *checkout {
                        vec!["checkout".into(), "-b".into(), n.clone(), at.clone()]
                    } else {
                        vec!["branch".into(), n.clone(), at.clone()]
                    };
                    cmds.push(Cmd::Git { label: format!("Created {n}"), args, network: false });
                }
                out.primary || out.close
            }
            Some(Dialog::DeleteBranch(d)) => {
                // Wording of the original's delete-branch sheet (sub_100070d94), with the remote
                // branch as a second row: deleting it is asked twice, since ⌘Z can't take it back.
                let hint = if d.confirm {
                    vec![Hint::Text(format!("Click again to delete it on {}", d.remote.as_ref().map(|r| r.remote.as_str()).unwrap_or_default()))]
                } else if d.del_remote {
                    vec![Hint::Text("Deleting on the remote can't be undone with ⌘Z".into())]
                } else {
                    modal::undo_hint()
                };
                let title = d.title();
                let out = Modal::new("delete_branch", Width::Medium, "trash", vec![text("Delete branch"), pill(title.as_str())])
                    .danger(true)
                    .status(vec![text(d.status())])
                    .hint(hint)
                    .primary(d.button(), true, d.can_go())
                    .show(ctx, |ui| {
                        let k = theme::c();
                        let before = (d.del_local, d.del_remote);
                        let both = d.both();
                        if let Some(l) = d.local.as_mut() {
                            if both {
                                modal::check_row(ui, &mut d.del_local, &format!("Local branch {}", l.name), Some("⌘Z brings it back"));
                            }
                            if d.del_local {
                                if let Some(w) = l.worktree.as_ref() {
                                    ui.add_space(8.0);
                                    modal::note(ui, k.modified, |ui| {
                                        ui.label(RichText::new(format!("Checked out in worktree “{}”", w.name())).color(k.text)).on_hover_text(w.path.display().to_string());
                                        if l.dirty && l.also_worktree {
                                            ui.label(RichText::new("It has uncommitted changes. They're saved before it's removed, so Undo (⌘Z) brings them back.").size(theme::FS_SECONDARY).color(k.dim));
                                        }
                                        if !l.also_worktree {
                                            ui.label(RichText::new("Git can't delete a branch that is checked out in a worktree.").size(theme::FS_SECONDARY).color(k.deleted));
                                        }
                                    });
                                    ui.add_space(8.0);
                                    modal::check_row(ui, &mut l.also_worktree, "Also delete worktree", None);
                                }
                                if !l.merged {
                                    ui.add_space(8.0);
                                    modal::check_row(ui, &mut l.force, "Delete even though it isn't merged into the current branch", None);
                                }
                                if l.worktree.is_none() && l.merged {
                                    ui.label(RichText::new("It's merged into the current branch, so no work is lost.").color(k.dim));
                                }
                            }
                        }
                        if let Some(r) = d.remote.as_ref() {
                            if both {
                                ui.add_space(8.0);
                                modal::check_row(ui, &mut d.del_remote, &format!("Remote branch {}", r.name()), Some("can't be undone"));
                            }
                            if d.del_remote {
                                if let Some(p) = r.open_pull.as_ref() {
                                    ui.add_space(8.0);
                                    modal::note(ui, k.modified, |ui| {
                                        ui.label(RichText::new(format!("Pull request {p} is open on this branch: GitHub will close it.")).color(k.text));
                                    });
                                }
                                if d.confirm {
                                    ui.add_space(8.0);
                                    modal::note(ui, k.deleted, |ui| {
                                        ui.label(RichText::new(d.warning()).color(k.text));
                                    });
                                }
                            }
                        }
                        if (d.del_local, d.del_remote) != before {
                            d.confirm = false;
                        }
                    });
                // Deleting on the remote takes a second click.
                let go = out.primary && d.can_go();
                if go && d.needs_confirm() {
                    d.confirm = true;
                    false
                } else {
                    if go {
                        cmds.push(Cmd::DeleteBranch(d.clone()));
                    }
                    go || out.close
                }
            }
            Some(Dialog::DeleteWorktree { wt, dirty }) => {
                // The original's wording (sub_1002008ec).
                let out = Modal::new("delete_worktree", Width::Medium, "folder", vec![text("Delete worktree"), pill(wt.name())])
                    .danger(true)
                    .status(vec![text(format!("Do you want to delete worktree '{}'?", wt.name()))])
                    .hint(modal::undo_hint())
                    .primary("Delete worktree", true, true)
                    .show(ctx, |ui| {
                        let k = theme::c();
                        ui.label(RichText::new(wt.path.display().to_string()).font(theme::mono()).color(k.faint));
                        if let Some(b) = &wt.branch {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                ui.label(RichText::new("The branch").color(k.dim));
                                modal::pill_ui(ui, b, false, theme::FS_BODY);
                                ui.label(RichText::new("is kept.").color(k.dim));
                            });
                        }
                        if *dirty {
                            ui.add_space(6.0);
                            modal::note(ui, k.modified, |ui| {
                                ui.label(RichText::new("It has uncommitted changes. They're saved before it's removed, so Undo (⌘Z) brings them back.").color(k.text));
                            });
                        }
                    });
                if out.primary {
                    cmds.push(Cmd::DeleteWorktree(wt.path.clone()));
                }
                out.primary || out.close
            }
            Some(Dialog::Confirm { title, message, label, args, network }) => {
                // The original's alerts (sub_1003b95c0, sub_10026d55c…): a question and the action.
                // The original says "You can't undo this action"; here every local action can be undone.
                let verb = title.split_whitespace().next().unwrap_or("Continue");
                let (icon, danger) = match verb {
                    "Delete" | "Discard" => ("trash", true),
                    "Reset" => ("undo", true),
                    "Rebase" | "Checkout" => ("branch", false),
                    _ => ("warning", true),
                };
                let out = Modal::new("confirm", Width::Small, icon, vec![text(title.as_str())]).danger(danger).hint(modal::undo_hint()).primary(title.as_str(), danger, true).show(ctx, |ui| {
                    ui.label(RichText::new(message.as_str()).color(theme::c().text));
                });
                if out.primary {
                    cmds.push(Cmd::Git { label: label.clone(), args: args.clone(), network: *network });
                }
                out.primary || out.close
            }
            Some(Dialog::Abort { op }) => operation::abort_dialog(ctx, op, cmds),
            Some(Dialog::NewGroup { path, name }) => {
                let repo = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let ok = !name.trim().is_empty();
                let out = Modal::new("new_group", Width::Small, "folder", vec![text("New group")])
                    .status(vec![pill(repo), text("goes into it")])
                    .hint(vec![Hint::Key(Cap::Text("Enter")), Hint::Text("creates it".into())])
                    .primary("Create group", false, ok)
                    .show(ctx, |ui| {
                        modal::text_field(ui, name, "Clients, Side projects…").request_focus();
                    });
                if out.primary {
                    cmds.push(Cmd::RepoGroup(path.clone(), Some(name.trim().to_string())));
                }
                out.primary || out.close
            }
            Some(Dialog::CheckoutSync { sync, action, merge, stash, dirty }) => {
                // The original's CheckoutAndSyncController: the branch, its upstream, a choice of
                // action, "Stash and reapply local changes", "Checkout".
                use crate::checkout::{MergeStyle, Sync, SyncAction};
                use crate::preview::{self, Preview};
                // Layout: one panel. On top the result's title, Merge's styles and Now / After; in
                // the middle the actions on the left and the graph on the right, on the same rows;
                // under them the sentence and the stash.
                const LIST_W: f32 = 190.0;
                let cards = Sync::cards();
                let enabled: Vec<bool> = cards.iter().map(|a| sync.unavailable(*a).is_none()).collect();
                let cur = cards.iter().position(|a| a == action).unwrap_or(0);
                if let Some(i) = modal::choice_keys(ctx, &enabled, cur) {
                    *action = cards[i];
                }
                let status = if sync.diverged() {
                    vec![text("has diverged from"), remote(sync.upstream.as_str()), text(sync.counts())]
                } else {
                    vec![text(format!("is {} behind", if sync.behind == 1 { "1 commit".to_string() } else { format!("{} commits", sync.behind) })), remote(sync.upstream.as_str())]
                };
                let hint = vec![Hint::Key(Cap::Up), Hint::Key(Cap::Down), Hint::Text("choose".into()), Hint::Key(Cap::Text("⌘Z")), Hint::Text("takes it all back".into())];
                let reset = *action == SyncAction::Reset;
                let out = Modal::new("checkout_sync", Width::Large, "branch", vec![text("Check out"), pill(sync.branch.as_str())])
                    .status(status)
                    .hint(hint)
                    .primary(sync.label(*action, *merge), reset, true)
                    .show(ctx, |ui| {
                        let k = theme::c();
                        egui::Frame::new().fill(k.bg).stroke(Stroke::new(1.0_f32, k.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
                            let w = Width::Large.px() - 2.0 * modal::PAD - 32.0;
                            ui.set_width(w);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            // The title, then on its right Now / After and Merge's styles (or the
                            // action's line).
                            let view_id = egui::Id::new("checkout_sync_view");
                            let mut after = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(view_id, || std::env::var("FOURCHETTE_CHECKOUT_VIEW").map_or(true, |v| v != "now")));
                            let (row, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
                            let mut side = ui.new_child(egui::UiBuilder::new().id_salt("checkout_side").max_rect(row).layout(Layout::right_to_left(Align::Center)));
                            side.spacing_mut().item_spacing.x = 8.0;
                            if let Some(i) = modal::segmented(&mut side, &["Now", "After"], after as usize, &[]) {
                                after = i == 1;
                                ctx.data_mut(|d| d.insert_temp(view_id, after));
                            }
                            if *action == SyncAction::Merge {
                                let names: Vec<&str> = MergeStyle::ALL.iter().map(|m| m.short()).collect();
                                let cur = MergeStyle::ALL.iter().position(|m| m == merge).unwrap_or(0);
                                if let Some(j) = modal::segmented(&mut side, &names, cur, &[]) {
                                    *merge = MergeStyle::ALL[j];
                                }
                            } else {
                                side.label(RichText::new(sync.hint(*action)).font(theme::secondary()).color(k.faint));
                            }
                            let title_w = row.width() - side.min_rect().width() - 12.0;
                            let col = if reset { k.deleted } else { k.strong };
                            crate::ui::fit_text(ui.painter(), row.left_center(), egui::Align2::LEFT_CENTER, &sync.text(*action, *merge), egui::FontId::proportional(theme::FS_TITLE), col, title_w);
                            // The actions and the graph, between two rules, on the same rows.
                            let preview = Preview::from_sync(sync, *action, *merge);
                            let block_h = preview::height(preview.rows.max(cards.len()));
                            ui.add_space(12.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(10.0);
                            let (block, _) = ui.allocate_exact_size(vec2(w, block_h), Sense::hover());
                            let list = Rect::from_min_size(block.min, vec2(LIST_W, block_h));
                            let mut left = ui.new_child(egui::UiBuilder::new().id_salt("checkout_actions").max_rect(list).layout(Layout::top_down(Align::Min)));
                            left.spacing_mut().item_spacing.y = 0.0;
                            for (i, a) in cards.iter().enumerate() {
                                let why = sync.unavailable(*a);
                                if modal::list_row(&mut left, Sync::short(*a), Some(i + 1), a == action, *a == SyncAction::Reset, why.as_deref(), preview::ROW).clicked() {
                                    *action = *a;
                                }
                            }
                            let x = list.right() + 16.0;
                            ui.painter().vline(x, block.y_range(), Stroke::new(1.0_f32, k.border));
                            let graph = Rect::from_min_max(pos2(x + 16.0, block.top()), block.max);
                            let mut right = ui.new_child(egui::UiBuilder::new().id_salt("checkout_graph").max_rect(graph).layout(Layout::top_down(Align::Min)));
                            preview::show(&mut right, egui::Id::new("checkout_sync_preview"), &preview, after);
                            ui.add_space(10.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(12.0);
                            // The sentence, in a room as tall as the longest one, then the stash.
                            let font = egui::TextStyle::Body.resolve(ui.style());
                            let mut texts = vec![sync.summary()];
                            for a in sync.actions() {
                                for m in MergeStyle::ALL {
                                    texts.push(sync.explain(a, m));
                                }
                            }
                            let room = texts.iter().map(|t| ui.painter().layout(t.clone(), font.clone(), k.dim, w).size().y).fold(0.0, f32::max);
                            let sentence = if after { sync.explain(*action, *merge) } else { sync.summary() };
                            ui.allocate_ui_with_layout(vec2(w, room), Layout::top_down(Align::Min), |ui| {
                                ui.set_min_size(vec2(w, room));
                                ui.label(RichText::new(sentence).color(k.dim));
                            });
                            if *dirty {
                                ui.add_space(12.0);
                                let side = if changed == 1 { "1 file".to_string() } else { format!("{changed} files") };
                                modal::check_row(ui, stash, "Stash and reapply changes", Some(&side));
                            }
                        });
                    });
                if out.primary {
                    cmds.push(Cmd::CheckoutSync { sync: sync.clone(), action: *action, merge: *merge, stash: *stash && *dirty });
                }
                out.primary || out.close
            }
            Some(Dialog::Push(push)) => {
                // The original's PushWindowController: where to, Force push, Push all tags. One
                // panel like Check out's: the result's title and Now / After on top, the choices on
                // the left and the graph on the right, the sentence under them.
                use crate::preview;
                const FORM_W: f32 = 250.0;
                if push.stale() {
                    let branch = push.branch.clone();
                    push.recount(refs, |r| {
                        let out = repo.as_ref()?.git(&["rev-list", "--left-right", "--count", &format!("refs/heads/{branch}...refs/remotes/{r}")]).ok()?;
                        let mut n = out.split_whitespace().filter_map(|x| x.parse().ok());
                        Some((n.next()?, n.next()?))
                    });
                }
                let k = theme::c();
                let mut status = vec![text("to"), remote(push.remote_ref()), text(push.counts())];
                if push.tags {
                    status.push(text("· all tags"));
                }
                let hint = if push.confirm {
                    vec![Hint::Text("Click again to replace them".into())]
                } else if !push.valid() {
                    vec![Hint::Text("Not a valid branch name".into())]
                } else if push.diverged() && !push.force {
                    vec![Hint::Text("Pull first, or tick Force push".into())]
                } else {
                    vec![Hint::Text("A push can't be undone with ⌘Z".into())]
                };
                let out = Modal::new("push", Width::Large, "push", vec![text("Push"), pill(push.branch.as_str())])
                    .danger(push.replaces() && push.confirm)
                    .status(status)
                    .hint(hint)
                    .primary(push.button(), push.force, push.can_push())
                    .show(ctx, |ui| {
                        egui::Frame::new().fill(k.bg).stroke(Stroke::new(1.0_f32, k.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
                            let w = Width::Large.px() - 2.0 * modal::PAD - 32.0;
                            ui.set_width(w);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let view_id = egui::Id::new("push_view");
                            let mut after = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(view_id, || std::env::var("FOURCHETTE_PUSH_VIEW").map_or(true, |v| v != "now")));
                            let (row, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
                            let mut side = ui.new_child(egui::UiBuilder::new().id_salt("push_side").max_rect(row).layout(Layout::right_to_left(Align::Center)));
                            if let Some(i) = modal::segmented(&mut side, &["Now", "After"], after as usize, &[]) {
                                after = i == 1;
                                ctx.data_mut(|d| d.insert_temp(view_id, after));
                            }
                            let title_w = row.width() - side.min_rect().width() - 12.0;
                            let col = if push.replaces() { k.deleted } else { k.strong };
                            crate::ui::fit_text(ui.painter(), row.left_center(), egui::Align2::LEFT_CENTER, &push.title(), egui::FontId::proportional(theme::FS_TITLE), col, title_w);
                            ui.add_space(12.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(12.0);
                            // The choices on the left, the graph on the right, as tall as the taller.
                            let before = (push.key(), push.force);
                            let top = ui.cursor().min;
                            let form = Rect::from_min_size(top, vec2(FORM_W, 400.0));
                            let mut left = ui.new_child(egui::UiBuilder::new().id_salt("push_form").max_rect(form).layout(Layout::top_down(Align::Min)));
                            left.spacing_mut().item_spacing.y = 6.0;
                            modal::caption(&mut left, "Remote");
                            if push.remotes.len() > 1 {
                                let names: Vec<&str> = push.remotes.iter().map(|r| r.as_str()).collect();
                                let cur = push.remotes.iter().position(|r| *r == push.remote).unwrap_or(0);
                                if let Some(i) = modal::segmented(&mut left, &names, cur, &[]) {
                                    push.remote = push.remotes[i].clone();
                                }
                            } else {
                                modal::pill_ui(&mut left, &push.remote, true, theme::FS_BODY);
                            }
                            left.add_space(6.0);
                            modal::caption(&mut left, "Remote branch");
                            modal::text_field(&mut left, &mut push.target, "branch name");
                            left.label(RichText::new(push.tracking()).font(theme::secondary()).color(k.faint));
                            left.add_space(6.0);
                            modal::check_row(&mut left, &mut push.force, "Force push", Some("with lease"));
                            modal::check_row(&mut left, &mut push.tags, "Push all tags", None);
                            let form_h = left.min_rect().height();
                            let p = push.preview();
                            let graph_h = preview::height(p.rows);
                            let block_h = form_h.max(graph_h);
                            let x = form.right() + 16.0;
                            ui.painter().vline(x, top.y..=top.y + block_h, Stroke::new(1.0_f32, k.border));
                            let graph = Rect::from_min_size(pos2(x + 16.0, top.y + (block_h - graph_h) / 2.0), vec2(w - FORM_W - 32.0, graph_h));
                            let mut right = ui.new_child(egui::UiBuilder::new().id_salt("push_graph").max_rect(graph).layout(Layout::top_down(Align::Min)));
                            preview::show(&mut right, egui::Id::new("push_preview"), &p, after);
                            ui.allocate_exact_size(vec2(w, block_h), Sense::hover());
                            if (push.key(), push.force) != before {
                                push.confirm = false;
                            }
                            ui.add_space(12.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(12.0);
                            let sentence = if after { push.explain() } else { push.summary() };
                            ui.label(RichText::new(sentence).color(k.dim));
                            if push.confirm {
                                ui.add_space(12.0);
                                modal::note(ui, k.deleted, |ui| {
                                    ui.label(RichText::new(push.warning()).color(k.text));
                                });
                            } else if push.diverged() && !push.force {
                                ui.add_space(12.0);
                                modal::note(ui, k.modified, |ui| {
                                    ui.label(RichText::new(format!("{} has commits that {} doesn't. Tick Force push to replace them.", push.remote_ref(), push.branch)).color(k.text));
                                });
                            }
                        });
                    });
                // Replacing commits on the remote takes a second click.
                let go = out.primary && push.can_push();
                if go && push.replaces() && !push.confirm {
                    push.confirm = true;
                    false
                } else {
                    if go {
                        cmds.push(Cmd::PushTo(push.clone()));
                    }
                    go || out.close
                }
            }
            Some(Dialog::Merge { merge, style, no_ff, check }) => {
                // The original's MergeWindowController: the branch, Merge commit or Squash, "No
                // fast-forward", and MergeTestGitCommand's answer before anything is run. Laid
                // out like the Check out dialog: the result's title and Now / After, the styles
                // beside the animated graph, the sentence, then the conflict check.
                use crate::checkout::MergeStyle;
                use crate::merge::Merge;
                use crate::preview;
                const LIST_W: f32 = 190.0;
                if check.poll() {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
                let enabled = [true; 2];
                let cur = MergeStyle::ALL.iter().position(|m| m == style).unwrap_or(0);
                if let Some(i) = modal::choice_keys(ctx, &enabled, cur) {
                    *style = MergeStyle::ALL[i];
                }
                let way = merge.way(*style, *no_ff);
                let branch_seg = if merge.remote { remote(merge.branch.as_str()) } else { pill(merge.branch.as_str()) };
                let mut hint = vec![Hint::Key(Cap::Up), Hint::Key(Cap::Down), Hint::Text("choose".into())];
                hint.extend(modal::undo_hint());
                let out = Modal::new("merge", Width::Large, "branch", vec![text("Merge"), branch_seg, text("into"), pill(merge.head.as_str())])
                    .status(vec![text(merge.counts())])
                    .hint(hint)
                    .primary(Merge::button(way), false, !merge.up_to_date())
                    .show(ctx, |ui| {
                        let k = theme::c();
                        egui::Frame::new().fill(k.bg).stroke(Stroke::new(1.0_f32, k.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
                            let w = Width::Large.px() - 2.0 * modal::PAD - 32.0;
                            ui.set_width(w);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            // The title, then Now / After on its right.
                            let view_id = egui::Id::new("merge_view");
                            let mut after = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(view_id, || std::env::var("FOURCHETTE_CHECKOUT_VIEW").map_or(true, |v| v != "now")));
                            let (row, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
                            let mut side = ui.new_child(egui::UiBuilder::new().id_salt("merge_side").max_rect(row).layout(Layout::right_to_left(Align::Center)));
                            if let Some(i) = modal::segmented(&mut side, &["Now", "After"], after as usize, &[]) {
                                after = i == 1;
                                ctx.data_mut(|d| d.insert_temp(view_id, after));
                            }
                            let title_w = row.width() - side.min_rect().width() - 12.0;
                            crate::ui::fit_text(ui.painter(), row.left_center(), egui::Align2::LEFT_CENTER, &merge.title(way), egui::FontId::proportional(theme::FS_TITLE), k.strong, title_w);
                            // The styles and the graph, between two rules, on the same rows.
                            let preview = merge.preview(*style, *no_ff);
                            let block_h = preview::height(preview.rows.max(MergeStyle::ALL.len()));
                            ui.add_space(12.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(10.0);
                            let (block, _) = ui.allocate_exact_size(vec2(w, block_h), Sense::hover());
                            let list = Rect::from_min_size(block.min, vec2(LIST_W, block_h));
                            let mut left = ui.new_child(egui::UiBuilder::new().id_salt("merge_styles").max_rect(list).layout(Layout::top_down(Align::Min)));
                            left.spacing_mut().item_spacing.y = 0.0;
                            for (i, m) in MergeStyle::ALL.iter().enumerate() {
                                if modal::list_row(&mut left, m.short(), Some(i + 1), m == style, false, None, preview::ROW).clicked() {
                                    *style = *m;
                                }
                            }
                            let x = list.right() + 16.0;
                            ui.painter().vline(x, block.y_range(), Stroke::new(1.0_f32, k.border));
                            let graph = Rect::from_min_max(pos2(x + 16.0, block.top()), block.max);
                            let mut right = ui.new_child(egui::UiBuilder::new().id_salt("merge_graph").max_rect(graph).layout(Layout::top_down(Align::Min)));
                            preview::show(&mut right, egui::Id::new("merge_preview"), &preview, after);
                            ui.add_space(10.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(12.0);
                            // The sentence, in a room as tall as the longest one.
                            let font = egui::TextStyle::Body.resolve(ui.style());
                            let mut texts = vec![merge.summary()];
                            for m in MergeStyle::ALL {
                                for n in [false, true] {
                                    texts.push(merge.explain(m, n));
                                }
                            }
                            let room = texts.iter().map(|t| ui.painter().layout(t.clone(), font.clone(), k.dim, w).size().y).fold(0.0, f32::max);
                            let sentence = if after { merge.explain(*style, *no_ff) } else { merge.summary() };
                            ui.allocate_ui_with_layout(vec2(w, room), Layout::top_down(Align::Min), |ui| {
                                ui.set_min_size(vec2(w, room));
                                ui.label(RichText::new(sentence).color(k.dim));
                            });
                            // What the check foresees, unless there is nothing to merge.
                            if !merge.up_to_date() {
                                ui.add_space(12.0);
                                merge_check(ui, &check.test, w);
                            }
                            if merge.can_fast_forward() {
                                ui.add_space(12.0);
                                modal::check_row(ui, no_ff, "No fast-forward: always create a merge commit", Some("--no-ff"));
                            }
                        });
                    });
                if out.primary {
                    cmds.push(Cmd::Merge { merge: merge.clone(), way });
                }
                out.primary || out.close
            }
            Some(Dialog::Pull { pull, mode, stash, dirty }) => {
                // The original's PullWindowController: the remote branch, "Rebase instead of
                // merge", "Stash and reapply local changes", "Pull". Here the mode is a switch and
                // the result shows on the same animated graph as the Check out dialog.
                use crate::preview;
                use crate::pull::Mode;
                let src = pull.source().name();
                let hint = vec![Hint::Key(Cap::Text("Enter")), Hint::Text("pulls".into()), Hint::Key(Cap::Text("⌘Z")), Hint::Text("takes the pull back, the fetch stays".into())];
                let mut picked = None;
                let out = Modal::new("pull", Width::Large, "pull", vec![text("Pull into"), pill(pull.branch.as_str())])
                    .status(vec![text("from"), remote(src.as_str()), text(pull.counts_text())])
                    .hint(hint)
                    .primary("Pull", false, true)
                    .show(ctx, |ui| {
                        let k = theme::c();
                        let w = Width::Large.px() - 2.0 * modal::PAD;
                        // The remote branch on the left, Merge or Rebase on the right.
                        let (row, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
                        let mut left = ui.new_child(egui::UiBuilder::new().id_salt("pull_from").max_rect(row).layout(Layout::left_to_right(Align::Center)));
                        left.spacing_mut().item_spacing.x = 8.0;
                        modal::caption(&mut left, "From");
                        egui::ComboBox::from_id_salt("pull_source").width(300.0).height(320.0).selected_text(RichText::new(src.as_str()).font(theme::mono()).color(k.text)).show_ui(&mut left, |ui| {
                            for (i, s) in pull.sources.iter().enumerate() {
                                if ui.selectable_label(i == pull.sel, RichText::new(s.name()).font(theme::mono())).clicked() {
                                    picked = Some(i);
                                }
                            }
                        });
                        let mut right = ui.new_child(egui::UiBuilder::new().id_salt("pull_mode").max_rect(row).layout(Layout::right_to_left(Align::Center)));
                        right.spacing_mut().item_spacing.x = 8.0;
                        let names: Vec<&str> = Mode::ALL.iter().map(|m| m.short()).collect();
                        let cur = Mode::ALL.iter().position(|m| m == mode).unwrap_or(0);
                        if let Some(i) = modal::segmented(&mut right, &names, cur, &[]) {
                            *mode = Mode::ALL[i];
                        }
                        modal::caption(&mut right, if pull.fast_forward() { "When it has diverged" } else { "Join with" });
                        ui.add_space(12.0);
                        egui::Frame::new().fill(k.bg).stroke(Stroke::new(1.0_f32, k.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
                            let w = w - 32.0;
                            ui.set_width(w);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            // The result's title, Now / After on its right.
                            let view_id = egui::Id::new("pull_view");
                            let mut after = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(view_id, || std::env::var("FOURCHETTE_CHECKOUT_VIEW").map_or(true, |v| v != "now")));
                            let (row, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
                            let mut side = ui.new_child(egui::UiBuilder::new().id_salt("pull_side").max_rect(row).layout(Layout::right_to_left(Align::Center)));
                            if let Some(i) = modal::segmented(&mut side, &["Now", "After"], after as usize, &[]) {
                                after = i == 1;
                                ctx.data_mut(|d| d.insert_temp(view_id, after));
                            }
                            let title_w = row.width() - side.min_rect().width() - 12.0;
                            crate::ui::fit_text(ui.painter(), row.left_center(), egui::Align2::LEFT_CENTER, &pull.title(*mode), egui::FontId::proportional(theme::FS_TITLE), k.strong, title_w);
                            // The graph, between two rules.
                            let preview = pull.preview(*mode);
                            ui.add_space(12.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(10.0);
                            preview::show(ui, egui::Id::new("pull_preview"), &preview, after);
                            ui.add_space(10.0);
                            let (rule, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                            ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0_f32, k.border));
                            ui.add_space(12.0);
                            // The sentence, in a room as tall as the longest one.
                            let font = egui::TextStyle::Body.resolve(ui.style());
                            let room = pull.sentences().into_iter().map(|t| ui.painter().layout(t, font.clone(), k.dim, w).size().y).fold(0.0, f32::max);
                            let sentence = if after { pull.explain(*mode) } else { pull.summary() };
                            ui.allocate_ui_with_layout(vec2(w, room), Layout::top_down(Align::Min), |ui| {
                                ui.set_min_size(vec2(w, room));
                                ui.label(RichText::new(sentence).color(k.dim));
                            });
                        });
                        if *dirty {
                            ui.add_space(12.0);
                            let side = if changed == 1 { "1 file".to_string() } else { format!("{changed} files") };
                            modal::check_row(ui, stash, "Stash and reapply local changes", Some(&side));
                        }
                    });
                // Another remote branch: count again (the sample project keeps its counts).
                if let Some(i) = picked.filter(|i| *i != pull.sel) {
                    pull.sel = i;
                    if let Some(r) = &self.repo {
                        pull.counts = crate::pull::count(r, &pull.source().name());
                    }
                }
                if out.primary {
                    cmds.push(Cmd::PullFrom { source: pull.source().clone(), args: pull.args(*mode, *stash && *dirty) });
                }
                out.primary || out.close
            }
        };
        if close {
            self.dialog = None;
        }
    }

}

impl App {
    fn keyboard(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        if self.modal() {
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
        // ⇧⌘O shows the home page (the menu bar takes the key while its item is enabled).
        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::O)) {
            cmds.push(Cmd::Home);
            return;
        }
        // The home page has no history or board for the keys below.
        if self.tab().home {
            self.favorite_keys(ctx, cmds);
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
            modal: self.modal(),
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
            Id::Pull => Cmd::PullDialog,
            Id::Push => Cmd::AskPush,
            Id::Stash => Cmd::stash(),
            Id::PopStash => Cmd::pop_stash(),
            Id::CheckoutBranch => Cmd::PaletteCheckout,
            Id::NewBranch => Cmd::new_ref_at_head(false),
            Id::NewTag => Cmd::new_ref_at_head(true),
            Id::ShowInFinder => Cmd::Reveal(root()?),
            Id::OpenInTerminal => Cmd::OpenTerminal(root()?),
            Id::ReleaseNotes => Cmd::OpenUrl(menu::release_notes_url()),
            Id::Website => Cmd::OpenUrl(menu::REPO_URL.into()),
            Id::Issues => Cmd::OpenUrl(menu::ISSUES_URL.into()),
            Id::NewTab => Cmd::NewTab,
            Id::Home => Cmd::Home,
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
        // FOURCHETTE_MENU: a right-click, pressed then released, a while before the screenshot.
        if let (Some(pos), true) = (self.shot_menu, self.frame == 40 || self.frame == 41) {
            let pressed = self.frame == 40;
            raw.events.push(egui::Event::PointerMoved(pos));
            raw.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() });
        }
        // Tab keys, taken before egui sees them (⌃Tab would otherwise move the focus). A dialog
        // or the Settings window keeps the keyboard, as it greys out the menu items.
        if !self.modal() && !self.settings_open {
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
        self.states_poll();
        self.update_poll(ctx);
        self.was_focused = focused;
        let mut cmds = std::mem::take(&mut self.menu_cmds);

        crate::titlebar::place_buttons();
        egui::TopBottomPanel::top("toolbar")
            .exact_height(crate::titlebar::TOOLBAR_H)
            .frame(egui::Frame::new().fill(theme::c().sidebar))
            .show(ctx, |ui| {
                let key = self.tab().key;
                let update = self.update.available.is_some();
                if self.tab().home {
                    self.home_toolbar(ui, &mut cmds);
                } else {
                    ui.push_id(key, |ui| self.tab_mut().toolbar(ui, &mut cmds, update));
                }
            });

        // The tabs, from two on (issue #59): with one, the window looks as before tabs.
        if self.tabs.len() > 1 {
            egui::TopBottomPanel::top("tabs")
                .exact_height(tabs::STRIP_H)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::c().sidebar))
                .show(ctx, |ui| self.tab_strip(ui, &mut cmds));
        }

        // The home page (issue #109) takes the window under the toolbar: no status bar, sidebar or details.
        if self.tab().home {
            self.home_page(ctx, &mut cmds);
        } else {
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
                            let tip = if d.hidden {
                                format!("Details panel: Hidden — click to show it ({}), right-click for more", d.place.label().to_lowercase())
                            } else {
                                format!("Details panel: {} — click to move it to the {}, right-click for more", d.label(), d.clicked().place.label().to_lowercase())
                            };
                            let btn = details_button(ui, d).on_hover_text(tip);
                            if btn.clicked() {
                                cmds.push(Cmd::SetDetails(d.clicked()));
                            }
                            btn.context_menu(|ui| {
                                crate::ui::menu_rows(ui);
                                ui.label(RichText::new("Details panel").size(theme::FS_SMALL).color(theme::c().faint));
                                for place in DetailsPlace::ALL {
                                    if ui.add(egui::Button::selectable(!d.hidden && d.place == place, place.label())).clicked() {
                                        cmds.push(Cmd::SetDetails(DetailsLayout::shown(place)));
                                        ui.close();
                                    }
                                }
                                if ui.add(egui::Button::selectable(d.hidden, "Hidden").right_text(RichText::new("⌘J").color(theme::c().faint))).clicked() {
                                    cmds.push(Cmd::SetDetails(d.hide()));
                                    ui.close();
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
                keep_panel_edges_free(ui);
                t.view_tabs(ui, &mut cmds);
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
                t.operation_banner(ui, &mut cmds);
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
            t.issue_offer_ui(ctx, &mut cmds);
            t.dialogs(ctx, &mut cmds);
            t.cleanup_window(ctx, &mut cmds);
            t.changelog_window(ctx);
            t.pr_draft_window(ctx);
            t.resolve_window(ctx);
        }
        self.keyboard(ctx, &mut cmds);
        self.palette_ui(ctx, &mut cmds);
        self.update_dialog(ctx, &mut cmds);
        if !self.update.dialog {
            self.whats_new_dialog(ctx, &mut cmds);
        }
        if !self.modal() {
            self.switcher_ui(ctx, &mut cmds);
            self.settings_ui(ctx, &mut cmds);
        }

        if let Some(t) = &self.toast {
            let (age, life) = (t.at.elapsed().as_secs_f32(), if t.undo { tab::UNDO_SECS } else { tab::TOAST_SECS });
            if age < life {
                // A toast that offers Cmd+Z shows the time left as a bar that shrinks.
                let left = t.undo.then(|| 1.0 - age / life);
                crate::modal::toast(ctx, &t.msg, t.error, age as f64, left);
                ctx.request_repaint_after(std::time::Duration::from_millis(if t.undo { 33 } else { 250 }));
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

/// The Merge dialog's line about conflicts, `w` wide: checking, none, the files that would
/// conflict, or why it could not tell.
fn merge_check(ui: &mut Ui, test: &crate::merge::Test, w: f32) {
    use crate::merge::Test;
    const SHOWN: usize = 5;
    let k = theme::c();
    // A sign on the left, then a line of text.
    let line = |ui: &mut Ui, text: &str, col: egui::Color32, sign: &dyn Fn(&egui::Painter, Pos2)| {
        let (rect, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
        sign(ui.painter(), pos2(rect.left() + 9.0, rect.center().y));
        crate::ui::fit_text(ui.painter(), pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, text, theme::body(), col, w - 26.0);
    };
    match test {
        Test::Checking => {
            let t = ui.input(|i| i.time) as f32;
            line(ui, "Checking for conflicts…", k.faint, &|p, c| {
                // A turning arc, drawn: the font has no spinner glyph.
                let pts: Vec<Pos2> = (0..=16).map(|i| c + egui::Vec2::angled(t * 5.0 + i as f32 / 16.0 * 4.5) * 6.0).collect();
                p.add(Shape::line(pts, Stroke::new(1.6_f32, k.faint)));
            });
        }
        Test::Clean => line(ui, "No conflicts: Git can merge it on its own.", k.text, &|p, c| {
            p.circle_filled(c, 8.0, theme::tint(k.added, k.bg, 0.2));
            crate::ui::check_mark(p, c, k.added);
        }),
        Test::Conflicts(files) => {
            crate::modal::note(ui, k.modified, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let n = files.len();
                let head = if n == 1 { "Will cause conflicts in 1 file".to_string() } else { format!("Will cause conflicts in {n} files") };
                ui.label(RichText::new(head).color(k.strong).strong());
                for f in files.iter().take(SHOWN) {
                    ui.add(egui::Label::new(RichText::new(f).font(theme::mono()).color(k.text)).truncate());
                }
                if n > SHOWN {
                    ui.label(RichText::new(format!("and {} more", n - SHOWN)).color(k.faint));
                }
                ui.label(RichText::new("You can still merge, then resolve them and Continue from the banner; ⌘Z takes it all back.").color(k.dim));
            });
        }
        Test::Unknown(why) => line(ui, why, k.faint, &|p, c| {
            p.circle_stroke(c, 7.0, Stroke::new(1.4_f32, k.faint));
            p.text(c, Align2::CENTER_CENTER, "?", FontId::proportional(11.0), k.faint);
        }),
    }
}

#[cfg(test)]
mod panel_resize_tests {
    use super::*;
    use eframe::App as _;

    /// A repository with enough commits for the history to scroll, so its scroll bar shows.
    fn long_repo() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("fourchette-resize-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let script = "git init -q -b main && for i in $(seq 80); do git -c user.name=T -c user.email=t@t commit -q --allow-empty -m c$i; done";
        assert!(std::process::Command::new("sh").args(["-c", script]).current_dir(&d).status().unwrap().success());
        crate::git::find_root(&d).unwrap()
    }

    #[test]
    fn the_details_panel_widens_when_its_edge_is_dragged() {
        let repo = long_repo();
        let mut a = App::with_library(crate::library::Library::read(&repo.join("no-repos.json")), None);
        a.open_path(repo.clone(), false);
        a.tab_mut().sel = Sel::Commit(1);
        let ctx = egui::Context::default();
        let mut frame = eframe::Frame::_new_kittest();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1360.0, 840.0));
        let step = |a: &mut App, frame: &mut eframe::Frame, events: Vec<egui::Event>| {
            let raw = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let _ = ctx.run(raw, |ctx| a.update(ctx, frame));
            egui::containers::panel::PanelState::load(&ctx, egui::Id::new("details")).unwrap().rect
        };
        // Presses at `from`, moves to `to` in ten steps and releases: the panel's rect after.
        let mut drag = |a: &mut App, from: f32, to: f32| {
            let (y, button) = (400.0, egui::PointerButton::Primary);
            let press = |x: f32, pressed| egui::Event::PointerButton { pos: pos2(x, y), button, pressed, modifiers: Default::default() };
            step(a, &mut frame, vec![egui::Event::PointerMoved(pos2(from, y))]);
            step(a, &mut frame, vec![press(from, true)]);
            for i in 1..=10 {
                step(a, &mut frame, vec![egui::Event::PointerMoved(pos2(from + (to - from) * i as f32 / 10.0, y))]);
            }
            step(a, &mut frame, vec![press(to, false)]);
            step(a, &mut frame, vec![])
        };
        let start = drag(&mut a, 0.0, 0.0);
        // Anywhere in the handle's grab zone, even on the history's side of the edge.
        for dx in [-4.0, 0.0, 4.0] {
            let grab = start.min.x + dx;
            let wide = drag(&mut a, grab, grab - 100.0);
            // egui puts the edge under the pointer.
            assert_eq!((wide.min.x, wide.max.x), (grab - 100.0, start.max.x), "grabbed {dx:+} from the edge");
            assert_eq!(drag(&mut a, wide.min.x, start.min.x), start);
        }
        let _ = std::fs::remove_dir_all(repo);
    }
}

/// Stages a file, or unstages it.
fn stage_cmd(path: &str, staged: bool) -> Cmd {
    if staged {
        Cmd::Git { label: format!("Unstaged {path}"), args: vec!["reset".into(), "-q".into(), "--".into(), path.into()], network: false }
    } else {
        Cmd::Git { label: format!("Staged {path}"), args: vec!["add".into(), "-A".into(), "--".into(), path.into()], network: false }
    }
}

/// A file of the local changes, in the List or (with `depth`) the Tree view: click opens its diff,
/// the button stages or unstages it, the menu has the rest.
fn file_item(ui: &mut Ui, f: &git::FileChange, files: &[git::FileChange], staged: bool, current: Option<&str>, depth: Option<usize>, cmds: &mut Vec<Cmd>) {
    let (resp, btn) = crate::ui::file_row_at(ui, f, current == Some(f.path.as_str()), Some(if staged { "Unstage" } else { "Stage" }), depth);
    if btn {
        cmds.push(stage_cmd(&f.path, staged));
    } else if resp.clicked() {
        cmds.push(Cmd::Diff(DiffReq::Worktree { path: f.path.clone(), staged, untracked: f.code == '?' }));
    }
    resp.context_menu(|ui| {
        crate::ui::menu_rows(ui);
        if f.code == 'U' {
            if ui.button("Resolve with Claude…").clicked() {
                cmds.push(Cmd::AiResolve(vec![f.path.clone()]));
            }
            if files.iter().filter(|g| g.code == 'U').count() > 1 && ui.button("Resolve all conflicts with Claude…").clicked() {
                cmds.push(Cmd::AiResolve(Vec::new()));
            }
            ui.separator();
        }
        if !staged {
            if ui.button("Stage").clicked() {
                cmds.push(stage_cmd(&f.path, false));
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
            cmds.push(stage_cmd(&f.path, true));
        }
        if ui.button("Copy path").clicked() {
            cmds.push(Cmd::Copy(f.path.clone()));
        }
    });
}

/// A folder of the Tree view: a click folds it, the button and the menu stage or unstage all of it
/// (no Discard on a folder).
fn dir_item(ui: &mut Ui, (name, path, count): (&str, &str, usize), depth: usize, open: bool, staged: bool, cmds: &mut Vec<Cmd>) {
    let action = if staged { "Unstage" } else { "Stage" };
    let (resp, btn) = crate::ui::dir_row(ui, name, depth, open, count, action);
    let dir_cmd = || match stage_cmd(path, staged) {
        Cmd::Git { args, network, .. } => Cmd::Git { label: format!("{} {path}/", if staged { "Unstaged" } else { "Staged" }), args, network },
        c => c,
    };
    if btn {
        cmds.push(dir_cmd());
    } else if resp.clicked() {
        cmds.push(Cmd::ToggleDir(staged, path.to_string()));
    }
    resp.context_menu(|ui| {
        crate::ui::menu_rows(ui);
        if ui.button(format!("{action} folder")).clicked() {
            cmds.push(dir_cmd());
        }
        if ui.button("Copy path").clicked() {
            cmds.push(Cmd::Copy(path.to_string()));
        }
    });
}

/// Width of the change ruler, right of the diff's scroll bar.
const RULER_W: f32 = 10.0;

/// The diff's area split in two: the text on the left, the change ruler's strip on the right.
fn ruler_split(ui: &Ui) -> (Rect, Rect) {
    let full = ui.available_rect_before_wrap();
    let strip = Rect::from_min_max(pos2(full.right() - RULER_W, full.top()), full.max);
    (Rect::from_min_max(full.min, pos2(strip.left() - 2.0, full.bottom())), strip)
}

/// The diff's scroll area, moved to where the ruler was clicked on the frame before.
fn ruler_jump(ui: &Ui, area: egui::ScrollArea) -> egui::ScrollArea {
    match ui.data_mut(|d| d.remove_temp::<f32>(ui.id().with("diff_jump"))) {
        Some(y) => area.vertical_scroll_offset(y),
        None => area,
    }
}

/// Runs of rows with the same mark, as (first row, rows, mark), the unmarked ones left out.
fn ruler_runs(marks: &[(bool, bool)]) -> Vec<(usize, usize, (bool, bool))> {
    let mut runs: Vec<(usize, usize, (bool, bool))> = Vec::new();
    for (i, &m) in marks.iter().enumerate() {
        if m == (false, false) {
            continue;
        }
        match runs.last_mut() {
            Some((start, len, last)) if *last == m && *start + *len == i => *len += 1,
            _ => runs.push((i, 1, m)),
        }
    }
    runs
}

/// Where the changes are: a thin strip right of the diff with a red mark for each deleted row and
/// a green one for each added row, at its place in the whole diff (a row with both, side by side,
/// is half red, half green), and the visible part outlined. A click or a drag scrolls there.
fn change_ruler<R>(ui: &mut Ui, body: Rect, strip: Rect, marks: &[(bool, bool)], out: &egui::scroll_area::ScrollAreaOutput<R>) {
    ui.allocate_rect(body.union(strip), Sense::hover());
    let k = theme::c();
    let resp = ui.interact(strip, ui.id().with("diff_ruler"), Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let p = ui.painter();
    p.rect_filled(strip, CornerRadius::ZERO, k.faint.gamma_multiply(0.06));
    let row = strip.height() / marks.len().max(1) as f32;
    for (start, len, (del, add)) in ruler_runs(marks) {
        let top = strip.top() + start as f32 * row;
        let ys = egui::Rangef::new(top, top + (len as f32 * row).max(2.0));
        let mid = strip.center().x;
        let paint = |x0: f32, x1: f32, c: Color32| {
            p.rect_filled(Rect::from_x_y_ranges(x0..=x1, ys), CornerRadius::ZERO, c);
        };
        match (del, add) {
            (true, true) => {
                paint(strip.left() + 1.0, mid, k.deleted);
                paint(mid, strip.right() - 1.0, k.added);
            }
            (true, false) => {
                paint(strip.left() + 1.0, strip.right() - 1.0, k.deleted);
            }
            _ => {
                paint(strip.left() + 1.0, strip.right() - 1.0, k.added);
            }
        }
    }
    let (content, view, offset) = (out.content_size.y, out.inner_rect.height(), out.state.offset.y);
    if content > view && content > 0.0 {
        let y = |v: f32| strip.top() + v / content * strip.height();
        let r = Rect::from_x_y_ranges(strip.x_range(), y(offset)..=y(offset + view));
        p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0_f32, k.border_strong), egui::StrokeKind::Inside);
    }
    if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
        let frac = ((pos.y - strip.top()) / strip.height()).clamp(0.0, 1.0);
        let to = (frac * content - view / 2.0).clamp(0.0, (content - view).max(0.0));
        ui.data_mut(|d| d.insert_temp(ui.id().with("diff_jump"), to));
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod ruler_tests {
    use super::ruler_runs;

    #[test]
    fn neighbouring_rows_with_the_same_mark_make_one_run() {
        let (n, d, a, b) = ((false, false), (true, false), (false, true), (true, true));
        assert_eq!(ruler_runs(&[n, d, d, a, n, a, b, b]), vec![(1, 2, d), (3, 1, a), (5, 1, a), (6, 2, b)]);
        assert!(ruler_runs(&[n, n]).is_empty());
    }
}
