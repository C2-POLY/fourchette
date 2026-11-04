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

/// A commit in the graph: a small dot in its lane colour, a hollow ring for a merge,
/// and a ring around the checked-out commit. `bg` is the row's background.
pub fn draw_node(p: &egui::Painter, c: Pos2, row: &Row, is_head: bool, matched: bool, bg: Color32) {
    let col = theme::lane_color(row.color);
    let col = if matched { col } else { theme::tint(col, bg, 0.4) };
    if is_head {
        p.circle_filled(c, theme::NODE_R + 4.0, bg);
        p.circle_stroke(c, theme::NODE_R + 3.0, Stroke::new(1.5_f32, col));
    }
    if row.is_merge {
        p.circle_filled(c, theme::MERGE_R + 2.0, bg);
        p.circle_stroke(c, theme::MERGE_R, Stroke::new(1.75_f32, col));
        return;
    }
    p.circle_filled(c, theme::NODE_R + 2.0, bg);
    p.circle_filled(c, theme::NODE_R, col);
}

/// Fill, border and text colours of a branch or tag label drawn in its lane colour.
pub fn ref_colors(r: &GitRef, lane: Color32, bg: Color32) -> (Color32, Option<Color32>, Color32) {
    let text = theme::readable(lane, 0.35);
    match r.kind {
        RefKind::Tag => (theme::c().elevated, Some(theme::c().border_strong), theme::c().text),
        RefKind::Remote => (bg, Some(theme::tint(lane, bg, 0.4)), text),
        _ if r.is_head => (theme::tint(lane, bg, 0.3), Some(theme::tint(lane, bg, 0.6)), theme::readable(lane, 0.55)),
        _ => (theme::tint(lane, bg, 0.18), None, text),
    }
}

/// Text on one line that ends with "…" when it is wider than `max_w`. Returns where it was drawn.
pub fn fit_text(p: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32, max_w: f32) -> Rect {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: max_w.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let galley = p.layout_job(job);
    let rect = align.anchor_size(pos, galley.size());
    p.galley(rect.min, galley, color);
    rect
}

/// A small chevron pointing right (closed) or down (open), drawn rather than typed.
pub fn chevron(p: &egui::Painter, c: Pos2, open: bool, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    let pts = if open { vec![c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)] } else { vec![c + vec2(-1.5, -3.5), c + vec2(2.0, 0.0), c + vec2(-1.5, 3.5)] };
    p.add(Shape::line(pts, s));
}

/// A drawn check mark (the font's own is uneven).
pub fn check_mark(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::line(vec![c + vec2(-4.0, 0.0), c + vec2(-1.2, 3.0), c + vec2(4.0, -3.0)], Stroke::new(1.6_f32, col)));
}

/// The status bar button that shows the appearance and switches to the next one.
pub fn appearance_button(ui: &mut Ui, a: theme::Appearance) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::c().hover);
    }
    let col = if resp.hovered() { theme::c().text } else { theme::c().dim };
    let bg = if resp.hovered() { theme::c().hover } else { theme::c().sidebar };
    appearance_icon(p, rect.center(), a, col, bg);
    resp
}

/// The icon of an appearance, about 14 pt wide; `bg` is the colour behind it (the moon is cut from it).
pub fn appearance_icon(p: &egui::Painter, c: Pos2, a: theme::Appearance, col: Color32, bg: Color32) {
    match a {
        // Half dark, half light: follows macOS.
        theme::Appearance::System => {
            p.circle_stroke(c, 5.5, Stroke::new(1.3_f32, col));
            let half: Vec<Pos2> = (0..=16).map(|k| c + egui::Vec2::angled(std::f32::consts::FRAC_PI_2 + k as f32 / 16.0 * std::f32::consts::PI) * 5.5).collect();
            p.add(Shape::convex_polygon(half, col, Stroke::NONE));
        }
        // A crescent moon.
        theme::Appearance::Dark => {
            p.circle_filled(c, 5.5, col);
            p.circle_filled(c + vec2(2.8, -2.3), 4.6, bg);
        }
        // A sun: a disc and eight rays.
        theme::Appearance::Light => {
            p.circle_filled(c, 2.8, col);
            for k in 0..8 {
                let d = egui::Vec2::angled(k as f32 * std::f32::consts::FRAC_PI_4);
                p.line_segment([c + d * 4.6, c + d * 6.6], Stroke::new(1.3_f32, col));
            }
        }
    }
}

/// The status bar button for the details panel: a window outline with its right or bottom
/// part filled where the panel sits, or empty when the panel is hidden.
pub fn details_button(ui: &mut Ui, l: crate::library::DetailsLayout) -> egui::Response {
    use crate::library::DetailsPlace;
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::c().hover);
    }
    let col = if resp.hovered() { theme::c().text } else { theme::c().dim };
    let win = Rect::from_center_size(rect.center(), vec2(15.0, 12.0));
    if !l.hidden {
        let part = match l.place {
            DetailsPlace::Right => Rect::from_min_max(pos2(win.right() - 5.5, win.top()), win.max),
            DetailsPlace::Bottom => Rect::from_min_max(pos2(win.left(), win.bottom() - 4.5), win.max),
        };
        let r = 2;
        let corners = match l.place {
            DetailsPlace::Right => CornerRadius { nw: 0, ne: r, sw: 0, se: r },
            DetailsPlace::Bottom => CornerRadius { nw: 0, ne: 0, sw: r, se: r },
        };
        p.rect_filled(part, corners, col);
    }
    p.rect_stroke(win, CornerRadius::same(2), Stroke::new(1.3_f32, col), StrokeKind::Inside);
    resp
}

/// A round avatar with the author's initials.
pub fn avatar(p: &egui::Painter, c: Pos2, r: f32, name: &str) {
    let col = theme::author_color(name);
    let k = if theme::is_light() { 0.9 } else { 0.75 };
    p.circle_filled(c, r, theme::tint(col, theme::c().bg, k));
    p.text(c, Align2::CENTER_CENTER, theme::initials(name), FontId::proportional((r * 0.95).max(8.0)), theme::c().on_accent);
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
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if selected || current {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().select);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let y = rect.center().y;
    let mut x = rect.left() + 10.0;
    if let Some(c) = color {
        p.circle_filled(pos2(x + 3.0, y), 3.5, c);
        x += 15.0;
    } else if kind != "remote" {
        p.text(pos2(x + 2.0, y), Align2::LEFT_CENTER, icon_glyph(kind), FontId::proportional(12.0), theme::c().faint);
        x += 20.0;
    }
    let tc = if selected || current { theme::c().strong } else { theme::c().text };
    let right = rect.right() - if count.is_some() || current { 34.0 } else { 8.0 };
    fit_text(p, pos2(x, y), Align2::LEFT_CENTER, text, theme::body(), tc, right - x);
    if current {
        check_mark(p, pos2(rect.right() - 14.0, y), theme::c().added);
    } else if let Some(n) = count {
        if n > 0 {
            p.text(pos2(rect.right() - 10.0, y), Align2::RIGHT_CENTER, n.to_string(), theme::small(), theme::c().faint);
        }
    }
    resp
}

/// A foldable group inside a section (a remote, a project column): chevron, name, count.
pub fn group_row(ui: &mut Ui, text: &str, count: usize, open: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let y = rect.center().y;
    chevron(p, pos2(rect.left() + 14.0, y), open, theme::c().faint);
    fit_text(p, pos2(rect.left() + 26.0, y), Align2::LEFT_CENTER, text, theme::body(), theme::c().text, rect.width() - 60.0);
    p.text(pos2(rect.right() - 10.0, y), Align2::RIGHT_CENTER, count.to_string(), theme::small(), theme::c().faint);
    resp
}

/// Placeholder text of a field, in the tertiary colour.
pub fn faint_hint(text: impl Into<String>) -> RichText {
    RichText::new(text).color(theme::c().faint)
}

pub fn side_item_indent(ui: &mut Ui, text: &str, color: Color32, indent: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let y = rect.center().y;
    let x = rect.left() + 10.0 + indent;
    p.circle_filled(pos2(x + 3.0, y), 3.0, color);
    fit_text(p, pos2(x + 15.0, y), Align2::LEFT_CENTER, text, theme::body(), theme::c().text, rect.right() - 8.0 - (x + 15.0));
    resp
}

pub fn section_header(ui: &mut Ui, title: &str, count: usize, open: bool) -> egui::Response {
    ui.add_space(6.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter();
    let col = if resp.hovered() { theme::c().text } else { theme::c().dim };
    let r = p.text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, title, theme::secondary(), col);
    // Like Linear: the chevron sits after the title, shown when hovered or folded.
    if resp.hovered() || !open {
        chevron(p, pos2(r.right() + 9.0, rect.center().y), open, theme::c().faint);
    }
    p.text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, count.to_string(), theme::small(), theme::c().faint);
    resp
}

pub fn toolbar_pill(ui: &mut Ui, caption: &str, value: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(value.to_string(), theme::body(), theme::c().strong);
    let w = (galley.size().x + 40.0).clamp(110.0, 260.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    p.text(pos2(rect.left() + 10.0, rect.top() + 11.0), Align2::LEFT_CENTER, caption, theme::small(), theme::c().faint);
    fit_text(p, pos2(rect.left() + 10.0, rect.top() + 27.0), Align2::LEFT_CENTER, value, theme::body(), theme::c().strong, rect.width() - 34.0);
    chevron(p, pos2(rect.right() - 13.0, rect.top() + 27.0), true, theme::c().dim);
    resp
}

/// Thin outline icons on a 24-unit grid (Tabler style), scaled to about 20 points.
/// egui strokes have flat caps, so open ends get a small dot to look round.
struct Ink<'a> {
    p: &'a egui::Painter,
    c: Pos2,
    s: Stroke,
}

impl Ink<'_> {
    const WIDTH: f32 = 1.5;
    const SCALE: f32 = 20.0 / 24.0;

    /// A point of the 24-unit grid, relative to the icon centre.
    fn t(&self, x: f32, y: f32) -> Pos2 {
        self.c + vec2(x - 12.0, y - 12.0) * Self::SCALE
    }
    fn dot(&self, at: Pos2) {
        self.p.circle_filled(at, Self::WIDTH / 2.0, self.s.color);
    }
    /// Open polyline of grid points, round at both ends.
    fn line(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.t(x, y)).collect();
        self.dot(v[0]);
        self.dot(*v.last().unwrap());
        self.p.add(Shape::line(v, self.s));
    }
    fn closed(&self, pts: Vec<Pos2>) {
        self.p.add(Shape::closed_line(pts, self.s));
    }
    /// Points of an arc (grid units, angles in degrees, screen orientation).
    fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
        let n = ((to - from).abs() / 8.0).ceil().max(2.0) as usize;
        (0..=n)
            .map(|k| {
                let a = (from + (to - from) * k as f32 / n as f32).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    }
    fn ring(&self, x: f32, y: f32, r: f32) {
        self.p.circle_stroke(self.t(x, y), r * Self::SCALE, self.s);
    }
    /// Four-pointed star with concave sides.
    fn star(&self, x: f32, y: f32, r: f32) {
        let k = r * 0.22;
        let tips = [(0.0, -r), (r, 0.0), (0.0, r), (-r, 0.0)];
        let mut v = Vec::new();
        for i in 0..4 {
            let (ax, ay) = tips[i];
            let (bx, by) = tips[(i + 1) % 4];
            let ctl = ((ax + bx) / 2.0 * k / r, (ay + by) / 2.0 * k / r);
            for j in 0..8 {
                let u = j as f32 / 8.0;
                let w = 1.0 - u;
                v.push(self.t(
                    x + w * w * ax + 2.0 * w * u * ctl.0 + u * u * bx,
                    y + w * w * ay + 2.0 * w * u * ctl.1 + u * u * by,
                ));
            }
        }
        self.closed(v);
    }
}

/// Toolbar button with a drawn icon above its label.
pub fn tool_button(ui: &mut Ui, icon: &str, label: &str, badge: usize, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(58.0, 48.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() || active {
        p.rect_filled(rect, CornerRadius::same(7), theme::c().hover);
    }
    let col = if resp.hovered() { theme::c().strong } else { theme::c().text };
    let c = pos2(rect.center().x, rect.top() + 17.0);
    let mut k = Ink { p, c, s: Stroke::new(Ink::WIDTH, col) };
    match icon {
        // two arcs with a chevron each
        "fetch" => {
            k.line(&Ink::arc(12.0, 12.0, 8.0, -7.0, -160.0));
            k.line(&[(4.0, 5.0), (4.0, 9.3), (8.0, 9.3)]);
            k.line(&Ink::arc(12.0, 12.0, 8.0, 173.0, 20.0));
            k.line(&[(20.0, 19.0), (20.0, 14.7), (16.0, 14.7)]);
        }
        "ai" => {
            if active {
                k.s = Stroke::new(Ink::WIDTH, theme::c().accent_text);
            }
            k.star(9.0, 14.5, 8.5);
            k.star(19.5, 5.0, 3.2);
        }
        // arrow pointing left, coming back from a turn on the right
        "undo" => {
            let mut path = vec![(14.0, 19.0)];
            path.extend(Ink::arc(16.0, 14.5, 4.5, 90.0, -90.0));
            path.push((4.0, 10.0));
            k.line(&path);
            k.line(&[(8.5, 14.5), (4.0, 10.0), (8.5, 5.5)]);
        }
        "pull" => {
            k.line(&[(12.0, 4.0), (12.0, 16.0)]);
            k.line(&[(7.5, 11.5), (12.0, 16.0), (16.5, 11.5)]);
            k.line(&[(5.0, 20.0), (19.0, 20.0)]);
        }
        "push" => {
            k.line(&[(12.0, 4.0), (12.0, 16.0)]);
            k.line(&[(7.5, 8.5), (12.0, 4.0), (16.5, 8.5)]);
            k.line(&[(5.0, 20.0), (19.0, 20.0)]);
        }
        "branch" => {
            let r = 2.4;
            k.ring(6.0, 18.0, r);
            k.ring(6.0, 6.0, r);
            k.ring(18.0, 6.0, r);
            k.line(&[(6.0, 6.0 + r), (6.0, 18.0 - r)]);
            let mut curve = Ink::arc(15.6, 15.6, 9.6, -90.0, -180.0);
            curve.push((6.0, 15.6));
            k.line(&curve);
        }
        // lid above an open body, with a handle
        "stash" => {
            let lid = Rect::from_min_max(k.t(3.0, 4.0), k.t(21.0, 8.0));
            k.p.rect_stroke(lid, CornerRadius::same(2), k.s, StrokeKind::Middle);
            let mut body = vec![(5.0, 8.5)];
            body.extend(Ink::arc(7.0, 18.0, 2.0, 180.0, 90.0));
            body.extend(Ink::arc(17.0, 18.0, 2.0, 90.0, 0.0));
            body.push((19.0, 8.5));
            k.line(&body);
            k.line(&[(10.0, 12.5), (14.0, 12.5)]);
        }
        // open body with an arrow leaving it upwards
        "pop" => {
            let mut body = vec![(4.0, 11.0)];
            body.extend(Ink::arc(6.0, 18.0, 2.0, 180.0, 90.0));
            body.extend(Ink::arc(18.0, 18.0, 2.0, 90.0, 0.0));
            body.push((20.0, 11.0));
            k.line(&body);
            k.line(&[(12.0, 15.0), (12.0, 3.0)]);
            k.line(&[(7.5, 7.5), (12.0, 3.0), (16.5, 7.5)]);
        }
        _ => {}
    }
    let p = k.p;
    p.text(pos2(rect.center().x, rect.bottom() - 8.0), Align2::CENTER_CENTER, label, theme::small(), if resp.hovered() { theme::c().strong } else { theme::c().dim });
    if badge > 0 {
        let s = badge.min(99).to_string();
        let b = Rect::from_center_size(pos2(rect.center().x + 13.0, rect.top() + 7.0), vec2(10.0 + s.len() as f32 * 6.0, 16.0));
        p.rect_filled(b, CornerRadius::same(8), theme::c().accent);
        p.text(b.center(), Align2::CENTER_CENTER, s, theme::small(), theme::c().on_accent);
    }
    resp
}

pub fn person_card(ui: &mut Ui, caption: &str, name: &str, email: &str, time: i64) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
        avatar(ui.painter(), rect.center(), 16.0, name);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(name).color(theme::c().strong));
                ui.label(RichText::new(caption).size(theme::FS_SMALL).color(theme::c().faint));
            });
            ui.label(RichText::new(format!("{email} · {}", git::relative_time(time))).size(theme::FS_SECONDARY).color(theme::c().dim));
        });
    });
}

/// A changed file; returns the row response and whether its action button was clicked.
pub fn file_row(ui: &mut Ui, f: &FileChange, selected: bool, action: Option<&str>) -> (egui::Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().select);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let y = rect.center().y;
    let col = theme::status_color(f.code);
    let badge = Rect::from_center_size(pos2(rect.left() + 14.0, y), vec2(18.0, 17.0));
    p.rect_filled(badge, CornerRadius::same(4), theme::tint(col, theme::c().bg, 0.16));
    let letter = if f.code == '?' { 'A' } else { f.code };
    p.text(badge.center(), Align2::CENTER_CENTER, letter.to_string(), FontId::monospace(theme::FS_SMALL), col);
    let (dir, file) = match f.path.rsplit_once('/') {
        Some((d, n)) => (format!("{d}/"), n.to_string()),
        None => (String::new(), f.path.clone()),
    };
    let show_btn = action.is_some() && (resp.hovered() || selected);
    let right = rect.right() - if show_btn { 72.0 } else { 6.0 };
    let x = rect.left() + 30.0;
    let r = fit_text(p, pos2(x, y), Align2::LEFT_CENTER, &file, theme::body(), if selected { theme::c().strong } else { theme::c().text }, right - x);
    if r.right() + 30.0 < right {
        fit_text(p, pos2(r.right() + 6.0, y), Align2::LEFT_CENTER, &dir, theme::secondary(), theme::c().faint, right - r.right() - 6.0);
    }
    let mut clicked_btn = false;
    if let (Some(a), true) = (action, show_btn) {
        let br = Rect::from_min_max(pos2(rect.right() - 68.0, rect.top() + 3.0), pos2(rect.right() - 4.0, rect.bottom() - 3.0));
        let hovered = ui.rect_contains_pointer(br);
        p.rect_filled(br, CornerRadius::same(5), if hovered { theme::c().accent } else { theme::c().elevated });
        p.rect_stroke(br, CornerRadius::same(5), Stroke::new(1.0_f32, if hovered { theme::c().accent } else { theme::c().border_strong }), StrokeKind::Inside);
        p.text(br.center(), Align2::CENTER_CENTER, a, theme::small(), if hovered { theme::c().on_accent } else { theme::c().strong });
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
        if ui.button(RichText::new("Delete branch…").color(theme::c().deleted)).clicked() {
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
