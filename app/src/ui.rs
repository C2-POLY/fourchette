//! Small drawing helpers: graph lines, nodes, sidebar rows, toolbar buttons.
use crate::app::{short, Cmd, Dialog};
use crate::git::{self, FileChange, GitRef, RefKind};
use crate::graph::Row;
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui};

pub fn lane_x(gx: f32, lane: usize) -> f32 {
    gx + theme::GRAPH_INSET + lane as f32 * theme::LANE_W
}

fn segment(p: &egui::Painter, a: Pos2, b: Pos2, col: Color32) {
    let stroke = Stroke::new(theme::LINE_W, col);
    if (a.x - b.x).abs() < 0.5 {
        p.line_segment([a, b], stroke);
    } else {
        // Smooth S-curve between lanes.
        let dy = (b.y - a.y) * 0.65;
        p.add(egui::epaint::CubicBezierShape::from_points_stroke(
            [a, pos2(a.x, a.y + dy), pos2(b.x, b.y - dy), b],
            false,
            Color32::TRANSPARENT,
            stroke,
        ));
    }
}

pub fn draw_row_graph(p: &egui::Painter, row: &Row, gx: f32, top: f32, bottom: f32, alpha: f32) {
    let mid = (top + bottom) * 0.5; // lines meet at the row middle, like the original
    for e in &row.top {
        segment(p, pos2(lane_x(gx, e.from), top), pos2(lane_x(gx, e.to), mid), theme::lane_color(e.color).gamma_multiply(alpha));
    }
    for e in &row.bottom {
        segment(p, pos2(lane_x(gx, e.from), mid), pos2(lane_x(gx, e.to), bottom), theme::lane_color(e.color).gamma_multiply(alpha));
    }
}

pub fn draw_node(p: &egui::Painter, c: Pos2, row: &Row, author: &str, is_head: bool, matched: bool) {
    let col = theme::lane_color(row.color);
    let col = if matched { col } else { col.gamma_multiply(0.4) };
    if is_head {
        p.circle_stroke(c, theme::NODE_R + 3.5, Stroke::new(1.5, col.gamma_multiply(0.8)));
    }
    if row.is_merge {
        p.circle_filled(c, theme::MERGE_R + 1.5, theme::BG);
        p.circle_filled(c, theme::MERGE_R, col);
        return;
    }
    let a = theme::author_color(author);
    let fill = Color32::from_rgb(a.r() / 3 + 0x14, a.g() / 3 + 0x14, a.b() / 3 + 0x1a);
    p.circle_filled(c, theme::NODE_R, if matched { fill } else { theme::BG });
    p.circle_stroke(c, theme::NODE_R, Stroke::new(2.0, col));
    p.text(c, Align2::CENTER_CENTER, theme::initials(author), FontId::proportional(8.5), if matched { Color32::WHITE } else { theme::FAINT });
}

pub fn ref_label(r: &GitRef) -> String {
    match r.kind {
        RefKind::Head => "HEAD".into(),
        RefKind::Local if r.is_head => format!("✔ {}", r.name),
        RefKind::Local => r.name.clone(),
        RefKind::Remote => format!("☁ {}", r.name),
        RefKind::Tag => format!("🏷 {}", r.name),
        RefKind::Stash => r.name.clone(),
    }
}

fn icon_glyph(kind: &str) -> &'static str {
    match kind {
        "changes" => "✏",
        "history" => "☰",
        "tag" => "🏷",
        "stash" => "📦",
        "worktree" => "🗀",
        _ => "•",
    }
}

pub fn side_item(ui: &mut Ui, kind: &str, text: &str, count: Option<usize>, selected: bool, color: Option<Color32>, current: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 25.0), Sense::click());
    let p = ui.painter();
    if selected || current {
        p.rect_filled(rect, CornerRadius::same(5), theme::SELECT);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::HOVER);
    }
    let y = rect.center().y;
    let mut x = rect.left() + 10.0;
    if let Some(c) = color {
        p.circle_filled(pos2(x + 3.0, y), 3.5, c);
        x += 14.0;
    } else if kind != "remote" {
        p.text(pos2(x + 2.0, y), Align2::LEFT_CENTER, icon_glyph(kind), FontId::proportional(12.0), theme::DIM);
        x += 20.0;
    }
    let tc = if current { Color32::WHITE } else { theme::TEXT };
    let right = rect.right() - if count.is_some() || current { 36.0 } else { 6.0 };
    p.with_clip_rect(Rect::from_min_max(rect.min, pos2(right, rect.max.y))).text(pos2(x, y), Align2::LEFT_CENTER, text, FontId::proportional(13.0), tc);
    if current {
        p.text(pos2(rect.right() - 10.0, y), Align2::RIGHT_CENTER, "✔", FontId::proportional(12.0), theme::ADDED);
    } else if let Some(n) = count {
        if n > 0 {
            let s = n.to_string();
            let w = 10.0 + s.len() as f32 * 7.0;
            let r = Rect::from_center_size(pos2(rect.right() - 8.0 - w / 2.0, y), vec2(w, 17.0));
            p.rect_filled(r, CornerRadius::same(8), theme::BORDER);
            p.text(r.center(), Align2::CENTER_CENTER, s, FontId::proportional(10.5), theme::DIM);
        }
    }
    resp
}

pub fn side_item_indent(ui: &mut Ui, text: &str, color: Color32, indent: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::HOVER);
    }
    let y = rect.center().y;
    let x = rect.left() + 10.0 + indent;
    p.circle_filled(pos2(x + 3.0, y), 3.0, color);
    p.with_clip_rect(rect.shrink(2.0)).text(pos2(x + 14.0, y), Align2::LEFT_CENTER, text, FontId::proportional(12.5), theme::TEXT);
    resp
}

pub fn section_header(ui: &mut Ui, title: &str, count: usize, open: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::HOVER);
    }
    p.text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, if open { "⏷" } else { "⏵" }, FontId::proportional(10.0), theme::FAINT);
    p.text(pos2(rect.left() + 22.0, rect.center().y), Align2::LEFT_CENTER, title.to_uppercase(), FontId::proportional(11.0), theme::DIM);
    p.text(pos2(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, count.to_string(), FontId::proportional(11.0), theme::FAINT);
    resp
}

pub fn toolbar_pill(ui: &mut Ui, caption: &str, value: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(value.to_string(), FontId::proportional(14.0), Color32::WHITE);
    let w = (galley.size().x + 34.0).clamp(110.0, 260.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 42.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), theme::HOVER);
    }
    p.text(pos2(rect.left() + 10.0, rect.top() + 11.0), Align2::LEFT_CENTER, caption.to_uppercase(), FontId::proportional(9.5), theme::FAINT);
    p.with_clip_rect(rect.shrink(2.0)).galley(pos2(rect.left() + 10.0, rect.top() + 19.0), galley, Color32::WHITE);
    p.text(pos2(rect.right() - 10.0, rect.top() + 27.0), Align2::RIGHT_CENTER, "⏷", FontId::proportional(10.0), theme::DIM);
    resp
}

/// Toolbar button with a drawn icon above its label.
pub fn tool_button(ui: &mut Ui, icon: &str, label: &str, badge: usize, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(58.0, 48.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() || active {
        p.rect_filled(rect, CornerRadius::same(7), theme::HOVER);
    }
    let col = if resp.hovered() { Color32::WHITE } else { theme::TEXT };
    let c = pos2(rect.center().x, rect.top() + 17.0);
    let s = Stroke::new(1.8, col);
    match icon {
        "fetch" => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|k| {
                    let a = -0.4 + k as f32 / 24.0 * 5.2;
                    c + vec2(a.cos(), a.sin()) * 7.0
                })
                .collect();
            let end = *pts.last().unwrap();
            p.add(Shape::line(pts, s));
            p.line_segment([end, end + vec2(4.0, -1.0)], s);
            p.line_segment([end, end + vec2(1.0, 4.0)], s);
        }
        "ai" => {
            // four-pointed sparkle with a small one beside it
            let star = |p: &egui::Painter, c: Pos2, r: f32, col: Color32| {
                let k = r * 0.28;
                let pts = vec![
                    c + vec2(0.0, -r),
                    c + vec2(k, -k),
                    c + vec2(r, 0.0),
                    c + vec2(k, k),
                    c + vec2(0.0, r),
                    c + vec2(-k, k),
                    c + vec2(-r, 0.0),
                    c + vec2(-k, -k),
                ];
                p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
            };
            let col = if active { theme::ACCENT } else { col };
            star(p, c + vec2(-1.5, 1.0), 8.0, col);
            star(p, c + vec2(7.0, -6.5), 3.5, col);
        }
        "undo" => {
            // curved arrow turning back to the left
            let pts: Vec<Pos2> = (0..=20)
                .map(|k| {
                    let a = std::f32::consts::PI * (1.0 - k as f32 / 20.0 * 1.25);
                    c + vec2(1.5, 2.0) + vec2(-a.cos(), -a.sin()) * 7.0
                })
                .collect();
            let start = pts[0];
            p.add(Shape::line(pts, s));
            p.line_segment([start, start + vec2(-3.5, -3.5)], s);
            p.line_segment([start, start + vec2(3.5, -3.5)], s);
        }
        "pull" | "push" => {
            let down = icon == "pull";
            let (a, b) = if down { (c - vec2(0.0, 8.0), c + vec2(0.0, 5.0)) } else { (c + vec2(0.0, 5.0), c - vec2(0.0, 8.0)) };
            p.line_segment([a, b], s);
            let d = if down { -1.0 } else { 1.0 };
            p.line_segment([b, b + vec2(-5.0, 5.0 * d)], s);
            p.line_segment([b, b + vec2(5.0, 5.0 * d)], s);
            p.line_segment([c + vec2(-8.0, 9.0), c + vec2(8.0, 9.0)], s);
        }
        "branch" => {
            p.circle_stroke(c + vec2(-4.0, 7.0), 2.5, s);
            p.circle_stroke(c + vec2(-4.0, -7.0), 2.5, s);
            p.circle_stroke(c + vec2(5.0, -4.0), 2.5, s);
            p.line_segment([c + vec2(-4.0, -4.5), c + vec2(-4.0, 4.5)], s);
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [c + vec2(5.0, -1.5), c + vec2(5.0, 3.0), c + vec2(-4.0, 1.0), c + vec2(-4.0, 4.5)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        "board" => {
            // three columns holding cards
            for (i, cards) in [3, 2, 1].iter().enumerate() {
                let x = c.x - 9.0 + i as f32 * 6.5;
                let col = Rect::from_min_size(pos2(x - 0.5, c.y - 8.5), vec2(6.0, 17.0));
                p.rect_stroke(col, CornerRadius::same(2), Stroke::new(1.0, s.color.gamma_multiply(0.45)), StrokeKind::Inside);
                for k in 0..*cards {
                    p.rect_filled(Rect::from_min_size(pos2(x + 0.7, c.y - 7.0 + k as f32 * 5.0), vec2(3.6, 3.6)), CornerRadius::same(1), s.color);
                }
            }
        }
        "stash" | "pop" => {
            let r = Rect::from_center_size(c + vec2(0.0, 3.0), vec2(16.0, 10.0));
            p.rect_stroke(r, CornerRadius::same(2), s, StrokeKind::Middle);
            p.line_segment([c + vec2(0.0, -9.0), c + vec2(0.0, 1.0)], s);
            let tip = if icon == "stash" { c + vec2(0.0, 1.0) } else { c + vec2(0.0, -9.0) };
            let d = if icon == "stash" { -3.5 } else { 3.5 };
            p.line_segment([tip, tip + vec2(-3.5, d)], s);
            p.line_segment([tip, tip + vec2(3.5, d)], s);
        }
        _ => {}
    }
    p.text(pos2(rect.center().x, rect.bottom() - 8.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), if resp.hovered() { Color32::WHITE } else { theme::DIM });
    if badge > 0 {
        let b = Rect::from_center_size(pos2(rect.center().x + 13.0, rect.top() + 7.0), vec2(17.0, 15.0));
        p.rect_filled(b, CornerRadius::same(7), theme::ACCENT);
        p.text(b.center(), Align2::CENTER_CENTER, badge.min(99).to_string(), FontId::proportional(9.5), Color32::WHITE);
    }
    resp
}

pub fn person_card(ui: &mut Ui, caption: &str, name: &str, email: &str, time: i64) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
        let col = theme::author_color(name);
        ui.painter().circle_filled(rect.center(), 17.0, col.gamma_multiply(0.85));
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, theme::initials(name), FontId::proportional(13.0), Color32::WHITE);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(name).strong().color(Color32::WHITE));
                ui.label(RichText::new(caption).size(9.5).color(theme::FAINT));
            });
            ui.label(RichText::new(email).small().color(theme::DIM));
            ui.label(RichText::new(git::relative_time(time)).small().color(theme::FAINT));
        });
    });
}

/// A changed file; returns the row response and whether its action button was clicked.
pub fn file_row(ui: &mut Ui, f: &FileChange, selected: bool, action: Option<&str>) -> (egui::Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(4), theme::SELECT);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(4), theme::HOVER);
    }
    let y = rect.center().y;
    let col = theme::status_color(f.code);
    let badge = Rect::from_center_size(pos2(rect.left() + 13.0, y), vec2(18.0, 16.0));
    p.rect_filled(badge, CornerRadius::same(4), col.gamma_multiply(0.22));
    let letter = if f.code == '?' { 'A' } else { f.code };
    p.text(badge.center(), Align2::CENTER_CENTER, letter.to_string(), FontId::monospace(10.5), col);
    let (dir, file) = match f.path.rsplit_once('/') {
        Some((d, n)) => (format!("{d}/"), n.to_string()),
        None => (String::new(), f.path.clone()),
    };
    let show_btn = action.is_some() && (resp.hovered() || selected);
    let right = rect.right() - if show_btn { 70.0 } else { 4.0 };
    let clip = p.with_clip_rect(Rect::from_min_max(rect.min, pos2(right, rect.max.y)));
    let r = clip.text(pos2(rect.left() + 28.0, y), Align2::LEFT_CENTER, &file, FontId::proportional(12.5), theme::TEXT);
    clip.text(pos2(r.right() + 6.0, y), Align2::LEFT_CENTER, &dir, FontId::proportional(11.5), theme::FAINT);
    let mut clicked_btn = false;
    if let (Some(a), true) = (action, show_btn) {
        let br = Rect::from_min_max(pos2(rect.right() - 66.0, rect.top() + 3.0), pos2(rect.right() - 4.0, rect.bottom() - 3.0));
        let hovered = ui.rect_contains_pointer(br);
        p.rect_filled(br, CornerRadius::same(4), if hovered { theme::ACCENT } else { theme::BORDER });
        p.text(br.center(), Align2::CENTER_CENTER, a, FontId::proportional(11.0), Color32::WHITE);
        clicked_btn = resp.clicked() && hovered;
    }
    (resp, clicked_btn)
}

pub fn checkout_remote(r: &GitRef, refs: &[GitRef]) -> Cmd {
    let local = r.name.split_once('/').map(|x| x.1).unwrap_or(&r.name).to_string();
    if refs.iter().any(|x| x.kind == RefKind::Local && x.name == local) {
        Cmd::Git { label: format!("Switched to {local}"), args: vec!["checkout".into(), local], network: false }
    } else {
        Cmd::Git { label: format!("Switched to {local}"), args: vec!["checkout".into(), "--track".into(), r.name.clone()], network: false }
    }
}

pub fn branch_menu(ui: &mut Ui, b: &GitRef, current: &str, cmds: &mut Vec<Cmd>) {
    if !b.is_head && ui.button("Check out").clicked() {
        cmds.push(Cmd::Git { label: format!("Switched to {}", b.name), args: vec!["checkout".into(), b.name.clone()], network: false });
    }
    if !b.is_head && !current.is_empty() {
        if ui.button(format!("Merge {} into {}", b.name, current)).clicked() {
            cmds.push(Cmd::Git { label: format!("Merged {}", b.name), args: vec!["merge".into(), "--no-edit".into(), b.name.clone()], network: false });
        }
        if ui.button(format!("Rebase {} onto {}", current, b.name)).clicked() {
            cmds.push(Cmd::Dialog(Dialog::Confirm {
                title: "Rebase".into(),
                message: format!("Do you want to rebase “{current}” onto “{}”?", b.name),
                label: "Rebased".into(),
                args: vec!["rebase".into(), b.name.clone()],
                network: false,
            }));
        }
    }
    if ui.button("New branch from here…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::NewRef { at: b.name.clone(), label: b.name.clone(), name: String::new(), checkout: true, tag: false }));
    }
    if ui.button("Copy name").clicked() {
        cmds.push(Cmd::Copy(b.name.clone()));
    }
    if !b.is_head {
        ui.separator();
        if ui.button(RichText::new("Delete branch…").color(theme::DELETED)).clicked() {
            // Opens the delete dialog, which also handles a worktree using the branch.
            cmds.push(Cmd::AskDeleteBranch(b.name.clone()));
        }
    }
}

pub fn commit_menu(ui: &mut Ui, hash: &str, subject: &str, branch: Option<&str>, cmds: &mut Vec<Cmd>) {
    let s = short(hash).to_string();
    if ui.button("New branch here…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::NewRef { at: hash.into(), label: format!("commit {s}"), name: String::new(), checkout: true, tag: false }));
    }
    if ui.button("New tag here…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::NewRef { at: hash.into(), label: format!("commit {s}"), name: String::new(), checkout: false, tag: true }));
    }
    ui.separator();
    if ui.button("Cherry-pick this commit").clicked() {
        cmds.push(Cmd::Git { label: format!("Cherry-picked {s}"), args: vec!["cherry-pick".into(), hash.into()], network: false });
    }
    if ui.button("Revert this commit").clicked() {
        cmds.push(Cmd::Git { label: format!("Reverted {s}"), args: vec!["revert".into(), "--no-edit".into(), hash.into()], network: false });
    }
    if ui.button("Check out this commit…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::Confirm {
            title: "Checkout".into(),
            message: format!("Do you want to check out commit {s} without a branch (detached HEAD)?"),
            label: format!("Checked out {s}"),
            args: vec!["checkout".into(), hash.into()],
            network: false,
        }));
    }
    if let Some(b) = branch {
        ui.menu_button(format!("Reset {b} to here"), |ui| {
            for (mode, explain) in [("soft", "keep all changes staged"), ("mixed", "keep changes, unstaged"), ("hard", "throw away all changes")] {
                if ui.button(format!("{mode} — {explain}")).clicked() {
                    cmds.push(Cmd::Dialog(Dialog::Confirm {
                        title: "Reset branch".into(),
                        message: format!("Do you want to reset “{b}” to commit {s} ({mode}: {explain})?"),
                        label: format!("Reset {b} to {s}"),
                        args: vec!["reset".into(), format!("--{mode}"), hash.into()],
                        network: false,
                    }));
                }
            }
        });
    }
    ui.separator();
    if ui.button("Copy SHA").clicked() {
        cmds.push(Cmd::Copy(hash.into()));
    }
    if ui.button("Copy message").clicked() {
        cmds.push(Cmd::Copy(subject.into()));
    }
}
