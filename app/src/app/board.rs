//! The GitHub project as a view next to the history (sidebar "Project", ⌘B): a board with one
//! column per status, or a list grouped by status; a ticket's detail shows in the details panel.
//! Cards move by dragging them or with "Move to" (right-click), which writes only their Status
//! and their place in the project.
use super::project::{named_color, status_color};
use super::*;
use crate::github::{self, Board, Item, Kind, Linked, Move};
use crate::ui::board_icon;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::emath::TSTransform;
use egui::{pos2, vec2, Align, Align2, CornerRadius, FontId, Id, Key, Layout, Margin, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Cards a finished column shows before "Show all": the most recently updated ones.
const DONE_LIMIT: usize = 20;
const COL_W: f32 = 284.0;
const COL_GAP: f32 = 12.0;
/// Space between two cards of a column.
const CARD_GAP: f32 = 8.0;
/// Seconds the cards take to part for a dragged one, and a dropped card to glide to its place.
const PART_TIME: f32 = 0.15;
const GLIDE_TIME: f32 = 0.18;
/// How close to a column's or the board's edge a dragged card scrolls it.
const SCROLL_EDGE: f32 = 48.0;
/// The board scrolls with the wheel and its bars, not by dragging it: dragging moves cards.
const NO_DRAG_SCROLL: egui::scroll_area::ScrollSource = egui::scroll_area::ScrollSource { drag: egui::scroll_area::DragScroll::Never, ..egui::scroll_area::ScrollSource::ALL };

// Surfaces of the mode on screen: columns sit below the page, cards above it.
fn column_fill() -> Color32 {
    theme::c().sidebar
}
fn card_fill() -> Color32 {
    theme::c().elevated
}
fn card_hover() -> Color32 {
    theme::c().widget_hover
}
fn chip_fill() -> Color32 {
    theme::c().select
}

/// A status or type colour as text: as it is on the dark mode, deeper on the light one.
fn ink(c: Color32) -> Color32 {
    if theme::is_light() {
        theme::shade(c, 0.3)
    } else {
        c
    }
}

/// How the board shows sub-issues: only the top-level tickets, every ticket at its place, or
/// each sub-issue inside its parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tree {
    Parents,
    All,
    #[default]
    Nested,
}

impl Tree {
    pub const ALL: [Tree; 3] = [Tree::Parents, Tree::All, Tree::Nested];

    pub fn label(self) -> &'static str {
        match self {
            Tree::Parents => "Parents",
            Tree::All => "All",
            Tree::Nested => "Nested",
        }
    }
}

/// The tickets placed in the columns, and the sub-issues shown inside their parent (by the
/// parent's item id), for `tree`. `shown` are the tickets the filters keep, in project order;
/// `project` is every ticket, to tell a sub-issue whose parent is in the project.
pub fn arrange<'a>(shown: Vec<&'a Item>, project: &[Item], tree: Tree) -> (Vec<&'a Item>, HashMap<String, Vec<&'a Item>>) {
    let mut kids: HashMap<String, Vec<&'a Item>> = HashMap::new();
    match tree {
        Tree::All => (shown, kids),
        // Top level: no parent, or a parent the project doesn't hold.
        Tree::Parents => (shown.into_iter().filter(|it| it.parent.as_ref().is_none_or(|p| !project.iter().any(|o| !p.url.is_empty() && o.url == p.url))).collect(), kids),
        // A sub-issue goes inside its parent when the parent is shown, else stays at its place.
        Tree::Nested => {
            let by_url: HashMap<&str, &str> = shown.iter().filter(|it| !it.url.is_empty()).map(|it| (it.url.as_str(), it.id.as_str())).collect();
            let mut top = Vec::new();
            for it in shown.iter().copied() {
                match it.parent.as_ref().and_then(|p| by_url.get(p.url.as_str())).filter(|pid| **pid != it.id.as_str()) {
                    Some(pid) => kids.entry(pid.to_string()).or_default().push(it),
                    None => top.push(it),
                }
            }
            (top, kids)
        }
    }
}

#[derive(Default)]
pub struct BoardState {
    /// Whether the Project view is the one on screen (rather than the history).
    pub open: bool,
    /// The list rather than the board (kept in memory only).
    pub list: bool,
    /// The city rather than the board or the list (#190, kept in memory only).
    pub city_on: bool,
    /// Where the city view stands once moved; None keeps it fitted.
    pub city: Option<super::city::CityView>,
    /// How sub-issues show (kept in memory, per repository's tab).
    pub tree: Tree,
    /// Parents whose sub-issues are folded away in the nested view.
    pub folded: HashSet<String>,
    /// The whole project rather than the open repository's tickets.
    pub all: bool,
    pub mine: bool,
    pub search: String,
    /// Finished columns showing every card.
    pub done_all: HashSet<String>,
    /// The project item shown in the details panel.
    pub detail: Option<String>,
    /// Whether a text field had the keyboard when the frame began, so Esc only leaves it.
    pub typing: bool,
    /// The card being dragged, or gliding to its place once let go.
    pub drag: Option<Drag>,
    /// Drags so far, so each one's animations start afresh.
    drags: u64,
    /// Where the columns and cards were on the last frame, to find where a drag points.
    columns_at: Vec<(String, Rect)>,
    cards_at: Vec<(String, String, Rect)>,
    /// FOURCHETTE_BOARD_DRAG: the number of a card to show lifted over the next column.
    pub drag_demo: Option<u64>,
}

#[derive(Clone)]
pub struct Drag {
    item: String,
    /// Its column, and its place among the column's other cards, when it was picked up.
    from: (String, usize),
    /// Where the pointer holds the card, and the card's size.
    grab: Vec2,
    size: Vec2,
    /// The card's top-left corner, following the pointer; kept where it was let go.
    pos: Pos2,
    /// The column and place it would land in.
    target: Option<(String, usize)>,
    /// Once let go: the column and place it glides to, and when it was let go.
    glide: Option<(String, usize, f64)>,
    gen: u64,
    /// For a screenshot: the pointer stays on this point and never lets go.
    demo: Option<Pos2>,
}

impl Drag {
    /// Where a column opens a gap: where the card would land, else in its own column where it
    /// was picked up. Done keeps its date order, so it doesn't part while a card is over it.
    fn gap(&self, status: &str) -> Option<usize> {
        if let Some((c, slot, _)) = &self.glide {
            return (c == status).then_some(*slot);
        }
        match &self.target {
            Some((c, slot)) if c == status && !github::is_done(c) => Some(*slot),
            _ if self.from.0 == status => Some(self.from.1),
            _ => None,
        }
    }
}

/// The column under the pointer and the place among its cards (the dragged one left out) that
/// the pointer is at: the count of cards whose middle is above it.
fn drop_target(p: Pos2, columns: &[(String, Rect)], cards: &[(String, String, Rect)], dragged: &str) -> Option<(String, usize)> {
    let (status, _) = columns.iter().find(|(_, r)| r.contains(p))?;
    let slot = cards.iter().filter(|(s, id, r)| s == status && id != dragged && r.center().y < p.y).count();
    Some((status.clone(), slot))
}

/// The move a drop makes, None when the card lands where it was. Done keeps its date order, so
/// a card dropped there takes its status only. `column` is the target column's cards as shown.
fn drop_move(d: &Drag, status: &str, slot: usize, column: &[&str]) -> Option<Move> {
    let done = github::is_done(status);
    let same = status == d.from.0;
    if same && (done || slot == d.from.1) {
        return None;
    }
    let after = if done { None } else { github::drop_after(column, &d.item, slot) };
    Some(Move { item: d.item.clone(), status: (!same).then(|| status.to_string()), place: !done, after })
}

/// "Move to" a status: the card goes to the top of that column, or among Done by its date.
pub fn move_to(it: &Item, status: &str) -> Move {
    Move { item: it.id.clone(), status: Some(status.to_string()), place: !github::is_done(status), after: None }
}

/// The right-click menu of a card or a row: "Move to" and the project's statuses, its own one
/// greyed out.
fn card_menu(ui: &mut Ui, board: &Board, it: &Item, cmds: &mut Vec<Cmd>) {
    ui.menu_button("Move to", |ui| {
        for (s, _) in &board.statuses {
            if ui.add_enabled(s != &it.status, egui::Button::new(s)).clicked() {
                cmds.push(Cmd::ProjectMove(move_to(it, s)));
            }
        }
    });
}

/// The room a column leaves for a dragged card, opening and closing smoothly: a pale slot where
/// it would land. Returns the slot's top-left corner while it is open.
fn card_gap(ui: &mut Ui, id: Id, open: bool, card: Vec2) -> Option<Pos2> {
    let full = card.y + CARD_GAP;
    let h = ui.ctx().animate_value_with_time(id, if open { full } else { 0.0 }, PART_TIME);
    if h < 0.5 {
        return None;
    }
    let top = ui.cursor().min;
    ui.add_space(h);
    let f = h / full;
    let slot = Rect::from_min_size(top, vec2(ui.available_width(), card.y * f));
    ui.painter().rect_filled(slot, CornerRadius::same(8), theme::c().hover.gamma_multiply(f));
    ui.painter().rect_stroke(slot, CornerRadius::same(8), Stroke::new(1.0_f32, theme::c().border.gamma_multiply(f)), StrokeKind::Inside);
    open.then_some(top)
}

/// How far to scroll when a dragged card is near an edge of `clip` along one axis: faster the
/// closer it is, positive towards the start.
fn edge_scroll(p: f32, start: f32, end: f32) -> f32 {
    if p < start + SCROLL_EDGE {
        (start + SCROLL_EDGE - p).min(SCROLL_EDGE) / SCROLL_EDGE * 14.0
    } else if p > end - SCROLL_EDGE {
        -(p - (end - SCROLL_EDGE)).min(SCROLL_EDGE) / SCROLL_EDGE * 14.0
    } else {
        0.0
    }
}

// ---------------------------------------------------------------- small pure helpers

/// A GitHub label colour ("1cadf8" or "#1cadf8").
pub fn hex_color(s: &str) -> Option<Color32> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Dark or light text, whichever reads on the colour.
pub fn readable_on(c: Color32) -> Color32 {
    let l = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if l > 150.0 {
        theme::LIGHT.strong
    } else {
        theme::DARK.strong
    }
}

/// "5m", "3h", "2d", "4mo", "1y": the age that fits on a card.
pub fn short_age(t: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let d = (now - t).max(0);
    match d {
        _ if d < 60 => "now".into(),
        _ if d < 3600 => format!("{}m", d / 60),
        _ if d < 86400 => format!("{}h", d / 3600),
        _ if d < 86400 * 30 => format!("{}d", d / 86400),
        _ if d < 86400 * 365 => format!("{}mo", d / (86400 * 30)),
        _ => format!("{}y", d / (86400 * 365)),
    }
}

fn login_initials(login: &str) -> String {
    theme::initials(&login.replace(['-', '_', '.'], " "))
}

/// What a project field chip says: "P1" for a priority, "Size M", "Sprint 12".
fn field_text(f: &github::Field) -> String {
    let short = f.value.chars().count() <= 3 || f.value.parse::<f64>().is_ok();
    if f.name.to_lowercase().contains("priority") || !short {
        f.value.clone()
    } else {
        format!("{} {}", f.name, f.value)
    }
}

/// Whether an item matches the search: title, number, labels, repository or assignees.
pub fn matches(it: &Item, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let q = q.trim_start_matches('#');
    it.title.to_lowercase().contains(q)
        || it.number.map(|n| n.to_string().starts_with(q)).unwrap_or(false)
        || it.labels.iter().any(|l| l.name.to_lowercase().contains(q))
        || it.repo.to_lowercase().contains(q)
        || it.assignees.iter().any(|a| a.to_lowercase().contains(q))
}

fn state_color(state: &str) -> Color32 {
    match state {
        "MERGED" => theme::c().renamed,
        "CLOSED" => theme::c().deleted,
        _ => theme::c().added,
    }
}

// ---------------------------------------------------------------- tiny Markdown

#[derive(Debug, PartialEq)]
pub enum Bullet {
    Dot,
    Number(String),
    Check(bool),
}

#[derive(Debug, PartialEq)]
pub enum Block {
    Heading(u8, String),
    Item { indent: usize, bullet: Bullet, text: String },
    Para(String),
    Code(String),
    Rule,
}

#[derive(Debug, PartialEq)]
pub enum Span {
    Plain(String),
    Bold(String),
    Code(String),
    Link(String),
}

/// Blocks of a GitHub Markdown body: headings, lists, task lists, paragraphs, code and rules.
pub fn md_blocks(body: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let mut code: Option<Vec<&str>> = None;
    let flush = |para: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(para.join(" ")));
            para.clear();
        }
    };
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            match code.take() {
                Some(c) => out.push(Block::Code(c.join("\n"))),
                None => {
                    flush(&mut para, &mut out);
                    code = Some(Vec::new());
                }
            }
            continue;
        }
        if let Some(c) = &mut code {
            c.push(line);
            continue;
        }
        if t.is_empty() {
            flush(&mut para, &mut out);
            continue;
        }
        let hashes = t.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            flush(&mut para, &mut out);
            out.push(Block::Heading(hashes as u8, t[hashes..].trim().to_string()));
            continue;
        }
        if t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_')) {
            flush(&mut para, &mut out);
            out.push(Block::Rule);
            continue;
        }
        let indent = (line.len() - line.trim_start().len()) / 2;
        let list = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ "));
        let item = if let Some(rest) = list {
            Some(if let Some(r) = rest.strip_prefix("[ ] ") {
                (Bullet::Check(false), r)
            } else if let Some(r) = rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
                (Bullet::Check(true), r)
            } else {
                (Bullet::Dot, rest)
            })
        } else {
            let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
            (digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") "))).then(|| (Bullet::Number(t[..digits].to_string()), &t[digits + 2..]))
        };
        match item {
            Some((bullet, text)) => {
                flush(&mut para, &mut out);
                out.push(Block::Item { indent, bullet, text: text.trim().to_string() });
            }
            None => para.push(t),
        }
    }
    if let Some(c) = code {
        out.push(Block::Code(c.join("\n")));
    }
    flush(&mut para, &mut out);
    out
}

/// Inline pieces of a line: **bold**, `code` and [links](…); other marks stay as they are.
pub fn md_spans(text: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    let push_plain = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span::Plain(std::mem::take(plain)));
        }
    };
    while let Some(c) = rest.chars().next() {
        if let Some(r) = rest.strip_prefix("**") {
            if let Some(end) = r.find("**") {
                push_plain(&mut plain, &mut out);
                out.push(Span::Bold(r[..end].to_string()));
                rest = &r[end + 2..];
                continue;
            }
        } else if let Some(r) = rest.strip_prefix('`') {
            if let Some(end) = r.find('`') {
                push_plain(&mut plain, &mut out);
                out.push(Span::Code(r[..end].to_string()));
                rest = &r[end + 1..];
                continue;
            }
        } else if let Some(r) = rest.strip_prefix('[') {
            if let Some(close) = r.find("](") {
                if let Some(end) = r[close..].find(')') {
                    push_plain(&mut plain, &mut out);
                    out.push(Span::Link(r[..close].to_string()));
                    rest = &r[close + end + 1..];
                    continue;
                }
            }
        }
        plain.push(c);
        rest = &rest[c.len_utf8()..];
    }
    push_plain(&mut plain, &mut out);
    out
}

fn md_job(text: &str, size: f32, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let fmt = |c: Color32| TextFormat { font_id: FontId::proportional(size), color: c, ..Default::default() };
    for s in md_spans(text) {
        match s {
            Span::Plain(t) => job.append(&t, 0.0, fmt(color)),
            Span::Bold(t) => job.append(&t, 0.0, fmt(theme::c().strong)),
            Span::Link(t) => job.append(&t, 0.0, fmt(theme::c().accent_text)),
            Span::Code(t) => job.append(&t, 0.0, TextFormat { font_id: FontId::monospace(size - 1.0), color: theme::c().modified, background: chip_fill(), ..Default::default() }),
        }
    }
    job.wrap = TextWrapping { max_width: width, ..Default::default() };
    job
}

fn checkbox_icon(p: &egui::Painter, c: Pos2, done: bool) {
    let r = Rect::from_center_size(c, vec2(12.0, 12.0));
    if done {
        p.rect_filled(r, CornerRadius::same(3), theme::c().accent);
        let s = Stroke::new(1.6_f32, theme::c().on_accent);
        p.line_segment([c + vec2(-3.0, 0.2), c + vec2(-0.8, 2.5)], s);
        p.line_segment([c + vec2(-0.8, 2.5), c + vec2(3.2, -2.5)], s);
    } else {
        p.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.2_f32, theme::c().dim), StrokeKind::Inside);
    }
}

fn markdown(ui: &mut Ui, body: &str) {
    let w = ui.available_width();
    for b in md_blocks(body) {
        match b {
            Block::Heading(level, t) => {
                ui.add_space(if level <= 2 { 8.0 } else { 4.0 });
                let size = match level {
                    1 => 17.0,
                    2 => 15.0,
                    _ => 13.5,
                };
                let g = ui.painter().layout_job(md_job(&t, size, theme::c().strong, w));
                ui.add(egui::Label::new(g).selectable(false));
                if level <= 2 {
                    let y = ui.cursor().top();
                    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0_f32, theme::c().border));
                    ui.add_space(4.0);
                }
            }
            Block::Para(t) => {
                let g = ui.painter().layout_job(md_job(&t, 13.0, theme::c().text, w));
                ui.add(egui::Label::new(g).selectable(false));
                ui.add_space(4.0);
            }
            Block::Item { indent, bullet, text } => {
                let left = 4.0 + indent as f32 * 16.0;
                let done = bullet == Bullet::Check(true);
                let g = ui.painter().layout_job(md_job(&text, 13.0, if done { theme::c().dim } else { theme::c().text }, w - left - 20.0));
                let (rect, _) = ui.allocate_exact_size(vec2(w, g.size().y.max(18.0)), Sense::hover());
                let p = ui.painter();
                let c = pos2(rect.left() + left + 6.0, rect.top() + 9.0);
                match &bullet {
                    Bullet::Dot => {
                        p.circle_filled(c, 2.3, theme::c().dim);
                    }
                    Bullet::Number(n) => {
                        p.text(c + vec2(4.0, 0.0), Align2::RIGHT_CENTER, format!("{n}."), FontId::proportional(theme::FS_SECONDARY), theme::c().dim);
                    }
                    Bullet::Check(d) => checkbox_icon(p, c, *d),
                }
                p.galley(pos2(rect.left() + left + 18.0, rect.top() + 1.0), g, theme::c().text);
            }
            Block::Code(t) => {
                egui::Frame::new().fill(theme::c().input).corner_radius(6).inner_margin(Margin::same(8)).show(ui, |ui| {
                    ui.set_width(w - 16.0);
                    ui.label(RichText::new(t).monospace().size(theme::FS_SMALL).color(theme::c().text));
                });
                ui.add_space(4.0);
            }
            Block::Rule => {
                ui.add_space(4.0);
                let y = ui.cursor().top();
                ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0_f32, theme::c().border));
                ui.add_space(6.0);
            }
        }
    }
}

// ---------------------------------------------------------------- drawn widgets

/// A rounded pill; `dot` draws a coloured dot before the text.
fn pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32, dot: Option<Color32>) -> Response {
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(theme::FS_SMALL), fg);
    let dot_w = if dot.is_some() { 10.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 14.0 + dot_w, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), bg);
    let mut x = rect.left() + 7.0;
    if let Some(d) = dot {
        p.circle_filled(pos2(x + 3.0, rect.center().y), 3.0, d);
        x += dot_w;
    }
    p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
    resp
}

fn type_pill(ui: &mut Ui, it: &Item) {
    let (name, color) = match (&it.issue_type, &it.kind) {
        (Some((n, c)), _) => (n.clone(), named_color(c).unwrap_or(theme::c().dim)),
        (None, Kind::PullRequest) => ("Pull request".into(), state_color(&it.state)),
        (None, Kind::Draft) => ("Draft".into(), theme::c().dim),
        (None, Kind::Issue) => ("Issue".into(), theme::c().dim),
    };
    pill(ui, &name, ink(color), color.gamma_multiply(0.16), Some(color));
}

fn label_pill(ui: &mut Ui, l: &github::Label) {
    let c = hex_color(&l.color).unwrap_or(theme::c().dim);
    let r = pill(ui, &l.name, readable_on(c), c, None);
    // Pale labels would melt into a white card.
    if theme::is_light() {
        ui.painter().rect_stroke(r.rect, CornerRadius::same(9), Stroke::new(1.0_f32, theme::c().border_strong), StrokeKind::Inside);
    }
}

fn field_chip(ui: &mut Ui, f: &github::Field) {
    let dot = f.color.as_deref().and_then(named_color);
    pill(ui, &field_text(f), theme::c().dim, chip_fill(), dot).on_hover_text(format!("{}: {}", f.name, f.value));
}

/// The pull request symbol: two dots joined by a line.
fn pr_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    p.circle_filled(c + vec2(-3.0, -3.0), 2.0, color);
    p.circle_filled(c + vec2(3.0, 3.5), 2.0, color);
    p.circle_stroke(c + vec2(3.0, -3.5), 1.6, Stroke::new(1.2_f32, color));
    p.line_segment([c + vec2(-3.0, -3.0), c + vec2(-3.0, 4.5)], Stroke::new(1.3_f32, color));
    p.line_segment([c + vec2(3.0, -2.0), c + vec2(3.0, 3.5)], Stroke::new(1.3_f32, color));
}

/// An issue's state symbol: a ring when open, a filled disc with a tick when closed.
fn issue_icon(p: &egui::Painter, c: Pos2, closed: bool) {
    if closed {
        p.circle_filled(c, 6.0, theme::c().renamed);
        let s = Stroke::new(1.5_f32, theme::c().on_accent);
        p.line_segment([c + vec2(-2.8, 0.2), c + vec2(-0.8, 2.3)], s);
        p.line_segment([c + vec2(-0.8, 2.3), c + vec2(2.8, -2.0)], s);
    } else {
        p.circle_stroke(c, 5.5, Stroke::new(1.5_f32, theme::c().added));
        p.circle_filled(c, 1.7, theme::c().added);
    }
}

fn pr_chip(ui: &mut Ui, pr: &Linked) -> Response {
    let color = state_color(&pr.state);
    let g = ui.painter().layout_no_wrap(format!("#{}", pr.number), FontId::monospace(theme::FS_SMALL), ink(color));
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 26.0, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), color.gamma_multiply(0.14));
    pr_icon(p, pos2(rect.left() + 11.0, rect.center().y), color);
    p.galley(pos2(rect.left() + 19.0, rect.center().y - g.size().y / 2.0), g, color);
    let state = match pr.state.as_str() {
        "MERGED" => "merged",
        "CLOSED" => "closed",
        _ => "open",
    };
    resp.on_hover_text(format!("Pull request #{} is {state}", pr.number))
}

/// Initials circles, overlapping a little, the first on the left.
fn avatars(ui: &mut Ui, logins: &[String], r: f32, ring: Color32) -> Response {
    let shown = logins.len().min(3);
    let step = r * 1.4;
    let (rect, resp) = ui.allocate_exact_size(vec2(2.0 * r + step * shown.saturating_sub(1) as f32, 2.0 * r), Sense::hover());
    let p = ui.painter();
    for (i, login) in logins.iter().take(shown).enumerate().rev() {
        let c = pos2(rect.left() + r + i as f32 * step, rect.center().y);
        p.circle_filled(c, r + 1.5, ring);
        if i == 2 && logins.len() > 3 {
            p.circle_filled(c, r, chip_fill());
            p.text(c, Align2::CENTER_CENTER, format!("+{}", logins.len() - 2), FontId::proportional(r * 0.85), theme::c().text);
        } else {
            p.circle_filled(c, r, theme::author_color(login).gamma_multiply(0.85));
            p.text(c, Align2::CENTER_CENTER, login_initials(login), FontId::proportional(r * 0.95), theme::c().on_accent);
        }
    }
    resp.on_hover_text(logins.join(", "))
}

/// A thin progress bar with "2/5" after it.
fn progress_bar(ui: &mut Ui, caption: &str, (done, total): (u32, u32)) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 14.0), Sense::hover());
    let p = ui.painter();
    let cap = p.text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, caption, FontId::proportional(theme::FS_SMALL), theme::c().faint);
    let count = p.text(pos2(rect.right(), rect.center().y), Align2::RIGHT_CENTER, format!("{done}/{total}"), FontId::proportional(theme::FS_SMALL), theme::c().dim);
    let bar = Rect::from_min_max(pos2(cap.right() + 8.0, rect.center().y - 2.0), pos2(count.left() - 8.0, rect.center().y + 2.0));
    p.rect_filled(bar, CornerRadius::same(2), chip_fill());
    let f = if total == 0 { 0.0 } else { done as f32 / total as f32 };
    let color = if done >= total { theme::c().added } else { theme::c().accent };
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height())), CornerRadius::same(2), color);
}

fn close_button(ui: &mut Ui) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let s = Stroke::new(1.6_f32, if resp.hovered() { theme::c().strong } else { theme::c().dim });
    let c = rect.center();
    p.line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], s);
    p.line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], s);
    resp.on_hover_text("Close (Esc)")
}

fn refresh_button(ui: &mut Ui, loading: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let col = if resp.hovered() { theme::c().strong } else { theme::c().dim };
    let c = rect.center();
    // Turns while the project is being read.
    let spin = if loading { ui.input(|i| i.time) as f32 * 5.0 } else { 0.0 };
    let pts: Vec<Pos2> = (0..=20).map(|k| c + egui::Vec2::angled(spin - 0.4 + k as f32 / 20.0 * 5.0) * 6.5).collect();
    let end = *pts.last().unwrap_or(&c);
    let dir = egui::Vec2::angled(spin - 0.4 + 5.0 + std::f32::consts::FRAC_PI_2);
    p.add(egui::Shape::line(pts, Stroke::new(1.6_f32, col)));
    p.add(egui::Shape::convex_polygon(vec![end + dir * 3.5, end + dir.rot90() * 3.0, end - dir.rot90() * 3.0], col, Stroke::NONE));
    resp.on_hover_text("Read the project again")
}

/// Space left of the first column: the columns centred in `width` when they fit, else the margin.
fn centre_lead(width: f32, columns: usize) -> f32 {
    let n = columns as f32;
    let total = n * COL_W + (n - 1.0).max(0.0) * COL_GAP;
    ((width - total) / 2.0).floor().max(16.0)
}

/// "Full board": shows the board alone in the window (#153), labelled so it can be found
/// without knowing ⌃⌘B.
fn full_button(ui: &mut Ui) -> Response {
    let font = FontId::proportional(theme::FS_SECONDARY);
    let g = ui.painter().layout_no_wrap("Full board".into(), font, theme::c().text);
    let (rect, resp) = ui.allocate_exact_size(vec2(10.0 + 16.0 + 6.0 + g.size().x + 10.0, 28.0), Sense::click());
    let p = ui.painter();
    let hot = resp.hovered();
    p.rect_filled(rect, CornerRadius::same(7), if hot { theme::c().widget_hover } else { theme::c().input });
    p.rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.0_f32, theme::c().border), StrokeKind::Inside);
    let col = if hot { theme::c().strong } else { theme::c().text };
    super::board_full::enter_icon(p, pos2(rect.left() + 10.0 + 8.0, rect.center().y), col);
    p.galley_with_override_text_color(pos2(rect.left() + 10.0 + 16.0 + 6.0, rect.center().y - g.size().y / 2.0), g, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("The board alone in the window, without the toolbar, sidebar and details (⌃⌘B)")
}

/// The search field, the refresh and full board buttons and their margins, right of the pickers.
const BAR_RIGHT_W: f32 = 370.0;

/// How wide `segmented` draws `options`.
pub(super) fn segmented_width(ui: &Ui, options: &[&str]) -> f32 {
    let font = FontId::proportional(theme::FS_SECONDARY);
    options.iter().map(|o| ui.painter().layout_no_wrap(o.to_string(), font.clone(), theme::c().text).size().x + 22.0).sum::<f32>() + 4.0
}

/// Options side by side, the chosen one filled.
pub(super) fn segmented(ui: &mut Ui, options: &[&str], chosen: usize) -> Option<usize> {
    let font = FontId::proportional(theme::FS_SECONDARY);
    let widths: Vec<f32> = options.iter().map(|o| ui.painter().layout_no_wrap(o.to_string(), font.clone(), theme::c().text).size().x + 22.0).collect();
    let (rect, _) = ui.allocate_exact_size(vec2(widths.iter().sum::<f32>() + 4.0, 28.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(8), theme::c().input);
    ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0_f32, theme::c().border), StrokeKind::Inside);
    let mut x = rect.left() + 2.0;
    let mut picked = None;
    for (i, (o, w)) in options.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(*w, rect.height() - 4.0));
        // Two pickers can share an option ("All"): the id has the whole set.
        let resp = ui.interact(r, ui.scope_id().with(("seg", options.join("|"), o)), Sense::click());
        let p = ui.painter();
        if i == chosen {
            p.rect_filled(r, CornerRadius::same(6), theme::c().select);
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(6), theme::c().hover);
        }
        p.text(r.center(), Align2::CENTER_CENTER, *o, font.clone(), if i == chosen { theme::c().strong } else { theme::c().dim });
        if resp.clicked() && i != chosen {
            picked = Some(i);
        }
        x += w;
    }
    picked
}

/// "↳ #n title" on one clipped line, the arrow drawn (the font has none). Returns its rect.
fn parent_line(ui: &mut Ui, parent: &Linked, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), size + 6.0), Sense::hover());
    let hot = ui.rect_contains_pointer(rect);
    let col = if hot { theme::c().accent } else { theme::c().dim };
    let p = ui.painter();
    let (x, y) = (rect.left() + 2.0, rect.center().y);
    let s = Stroke::new(1.3_f32, col);
    p.line_segment([pos2(x + 1.0, y - 5.0), pos2(x + 1.0, y)], s);
    p.line_segment([pos2(x + 1.0, y), pos2(x + 9.0, y)], s);
    p.line_segment([pos2(x + 9.0, y), pos2(x + 6.0, y - 3.0)], s);
    p.line_segment([pos2(x + 9.0, y), pos2(x + 6.0, y + 3.0)], s);
    let n = p.text(pos2(x + 15.0, y), Align2::LEFT_CENTER, format!("#{}", parent.number), FontId::monospace(size - 1.0), col);
    p.with_clip_rect(rect.intersect(ui.clip_rect())).text(pos2(n.right() + 6.0, y), Align2::LEFT_CENTER, &parent.title, FontId::proportional(size), col);
    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(format!("Sub-issue of #{} {}", parent.number, parent.title))
}

/// An on/off filter, filled when on.
fn toggle_chip(ui: &mut Ui, text: &str, on: bool) -> Response {
    let font = FontId::proportional(theme::FS_SECONDARY);
    let w = ui.painter().layout_no_wrap(text.to_string(), font.clone(), theme::c().text).size().x + 30.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    let p = ui.painter();
    let fill = if on { theme::c().select } else if resp.hovered() { theme::c().hover } else { theme::c().input };
    p.rect_filled(rect, CornerRadius::same(8), fill);
    p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0_f32, if on { theme::c().accent } else { theme::c().border }), StrokeKind::Inside);
    // A small person: head and shoulders.
    let c = pos2(rect.left() + 13.0, rect.center().y);
    let col = if on { theme::c().strong } else { theme::c().dim };
    p.circle_filled(c + vec2(0.0, -3.0), 2.6, col);
    p.add(egui::epaint::CubicBezierShape::from_points_stroke([c + vec2(-4.5, 5.0), c + vec2(-4.0, 0.5), c + vec2(4.0, 0.5), c + vec2(4.5, 5.0)], false, Color32::TRANSPARENT, Stroke::new(1.5_f32, col)));
    p.text(pos2(rect.left() + 22.0, rect.center().y), Align2::LEFT_CENTER, text, font, if on { theme::c().strong } else { theme::c().dim });
    resp
}

/// Whether a branch name carries the ticket number as a token of its own: `feat/51-groups`,
/// `51-foo`, `fix_51`, but not `feat/510-x`.
pub fn branch_has_number(branch: &str, n: u64) -> bool {
    let n = n.to_string();
    branch.split(|c: char| !c.is_alphanumeric()).any(|t| t == n)
}

/// The project as a list: a header per status (dot, name, count), then one row per ticket.
/// Returns the ticket whose row was clicked.
/// The list, grouped by status; in the nested view a parent's sub-issues follow it, indented,
/// unless it is folded. Returns the ticket clicked and the parent whose fold was clicked.
#[allow(clippy::too_many_arguments)]
fn list_view(ui: &mut Ui, board: &Board, cols: &[(String, Vec<&Item>)], kids: &HashMap<String, Vec<&Item>>, folded: &HashSet<String>, show_repo: bool, selected: Option<&str>, cmds: &mut Vec<Cmd>) -> (Option<String>, Option<String>) {
    let mut clicked = None;
    let mut fold = None;
    egui::ScrollArea::vertical().id_salt("board_list").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.set_width(ui.available_width());
        for (status, items) in cols {
            if items.is_empty() {
                continue;
            }
            let color = status_color(board, status);
            ui.add_space(10.0);
            let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
            let p = ui.painter();
            p.circle_filled(pos2(hr.left() + 21.0, hr.center().y), 4.5, color);
            let name = p.text(pos2(hr.left() + 33.0, hr.center().y), Align2::LEFT_CENTER, status, FontId::proportional(theme::FS_BODY), theme::c().strong);
            let n = p.layout_no_wrap(items.len().to_string(), FontId::proportional(theme::FS_SMALL), ink(color));
            let badge = Rect::from_min_size(pos2(name.right() + 8.0, hr.center().y - 9.0), vec2(n.size().x + 14.0, 18.0));
            p.rect_filled(badge, CornerRadius::same(9), color.gamma_multiply(0.16));
            p.galley(badge.center() - n.size() / 2.0, n, color);
            for it in items {
                let inside = kids.get(&it.id).filter(|v| !v.is_empty());
                let open = inside.map(|_| !folded.contains(&it.id));
                let resp = list_row(ui, it, show_repo, selected == Some(it.id.as_str()), 0.0, None);
                // The chevron sits in the row's left margin.
                if let Some(o) = open {
                    let c = pos2(resp.rect.left() + 8.0, resp.rect.center().y);
                    chevron(ui.painter(), c, o, theme::c().dim);
                }
                if resp.clicked() {
                    let on_chevron = open.is_some() && resp.interact_pointer_pos().is_some_and(|p| p.x < resp.rect.left() + 16.0);
                    if on_chevron {
                        fold = Some(it.id.clone());
                    } else {
                        clicked = Some(it.id.clone());
                    }
                }
                resp.context_menu(|ui| card_menu(ui, board, it, cmds));
                for k in inside.filter(|_| open == Some(true)).into_iter().flatten() {
                    let resp = list_row(ui, k, show_repo, selected == Some(k.id.as_str()), 26.0, Some(status_color(board, &k.status)));
                    if resp.clicked() {
                        clicked = Some(k.id.clone());
                    }
                    resp.context_menu(|ui| card_menu(ui, board, k, cmds));
                }
            }
        }
        ui.add_space(16.0);
    });
    (clicked, fold)
}

/// One ticket of the list: type, number, title, then labels, pull request, age and assignees.
/// `indent` moves a sub-issue right under its parent, with `dot` its own status colour.
fn list_row(ui: &mut Ui, it: &Item, show_repo: bool, selected: bool, indent: f32, dot: Option<Color32>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let mut inner = rect.shrink2(vec2(16.0, 0.0));
    inner.min.x += indent;
    if selected {
        ui.painter().rect_filled(rect.shrink2(vec2(8.0, 1.0)), CornerRadius::same(6), theme::c().select);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect.shrink2(vec2(8.0, 1.0)), CornerRadius::same(6), theme::c().hover);
    }
    // Right side first, to know how much room the title has.
    let mut right_edge = inner.right();
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(Layout::right_to_left(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if !it.assignees.is_empty() {
            avatars(ui, &it.assignees, 9.0, theme::c().bg);
        }
        let t = it.updated.max(it.created);
        ui.allocate_ui(vec2(30.0, 18.0), |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if t > 0 {
                    ui.label(RichText::new(short_age(t)).size(theme::FS_SMALL).color(theme::c().faint));
                }
            });
        });
        if let Some(pr) = it.pull_requests.first() {
            pr_chip(ui, pr);
        }
        if it.sub_progress.1 > 0 {
            ui.label(RichText::new(format!("{}/{}", it.sub_progress.0, it.sub_progress.1)).size(theme::FS_SMALL).color(theme::c().dim)).on_hover_text("Sub-issues done");
        }
        // Labels only while they leave the title some room.
        for l in it.labels.iter().take(2).rev() {
            if ui.available_width() > 360.0 {
                label_pill(ui, l);
            }
        }
        right_edge = ui.min_rect().left();
    });
    let left = Rect::from_min_max(inner.min, pos2((right_edge - 10.0).max(inner.left() + 60.0), inner.max.y));
    ui.scope_builder(egui::UiBuilder::new().max_rect(left).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.set_clip_rect(left.intersect(ui.clip_rect()));
        if let Some(c) = dot {
            let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.5, c);
        }
        type_pill(ui, it);
        if let Some(n) = it.number {
            ui.label(RichText::new(format!("#{n}")).monospace().size(theme::FS_SMALL).color(theme::c().faint));
        }
        if show_repo && !it.repo.is_empty() {
            ui.label(RichText::new(it.repo.rsplit('/').next().unwrap_or(&it.repo)).size(theme::FS_SMALL).color(theme::c().faint));
        }
        ui.add(egui::Label::new(RichText::new(&it.title).size(theme::FS_BODY).color(if it.is_closed() { theme::c().dim } else { theme::c().strong })).truncate().selectable(false));
    });
    resp
}

/// A centred message for an empty board; returns whether its button was clicked.
fn empty_state(ui: &mut Ui, title: &str, detail: &str, action: Option<&str>) -> bool {
    let mut clicked = false;
    ui.add_space((ui.available_height() * 0.28).max(30.0));
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(64.0, 52.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(10), theme::c().elevated);
        p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0_f32, theme::c().border), StrokeKind::Inside);
        board_icon(p, rect.center() + vec2(1.0, 2.0), theme::c().faint);
        ui.add_space(14.0);
        ui.label(RichText::new(title).size(16.0).color(theme::c().strong));
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(detail).color(theme::c().dim)).wrap());
        if let Some(a) = action {
            ui.add_space(12.0);
            clicked = crate::modal::button(ui, a, crate::modal::Kind::Plain, true).clicked();
        }
    });
    clicked
}

/// One ticket on the board, with the rect of its parent line when it has one: the card's click
/// covers the whole card, so the caller tells a click on the parent apart by where it landed.
/// Its response covers the frame, border included, and can start a drag.
fn card(ui: &mut Ui, it: &Item, show_repo: bool, selected: bool, nest: Option<Nest>) -> CardOut {
    let mut parent_rect = None;
    let mut fold_rect = None;
    let mut kid_rects = Vec::new();
    let mut prepared = egui::Frame::new().fill(card_fill()).stroke(Stroke::new(1.0_f32, theme::c().border)).corner_radius(8).inner_margin(Margin::same(10)).begin(ui);
    {
        let ui = &mut prepared.content_ui;
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = vec2(5.0, 7.0);
        ui.horizontal(|ui| {
            type_pill(ui, it);
            if let Some(n) = it.number {
                ui.label(RichText::new(format!("#{n}")).monospace().size(theme::FS_SMALL).color(theme::c().faint));
            }
            if show_repo && !it.repo.is_empty() {
                let name = it.repo.rsplit('/').next().unwrap_or(&it.repo);
                ui.label(RichText::new(name).size(theme::FS_SMALL).color(theme::c().faint)).on_hover_text(&it.repo);
            }
            if it.is_closed() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    if it.kind == Kind::PullRequest {
                        pr_icon(ui.painter(), r.center(), state_color(&it.state));
                    } else {
                        issue_icon(ui.painter(), r.center(), true);
                    }
                });
            }
        });
        let mut job = LayoutJob::single_section(it.title.clone(), TextFormat { font_id: FontId::proportional(theme::FS_BODY), color: theme::c().strong, ..Default::default() });
        job.wrap = TextWrapping { max_width: ui.available_width(), max_rows: 2, break_anywhere: false, overflow_character: Some('…') };
        let g = ui.painter().layout_job(job);
        let title = ui.add(egui::Label::new(g).selectable(false));
        if it.title.chars().count() > 60 {
            title.on_hover_text(&it.title);
        }
        if let Some(parent) = &it.parent {
            parent_rect = Some(parent_line(ui, parent, 11.5).rect);
        }
        if !it.labels.is_empty() || !it.fields.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for l in &it.labels {
                    label_pill(ui, l);
                }
                for f in &it.fields {
                    field_chip(ui, f);
                }
            });
        }
        if let Some(pr) = it.progress() {
            progress_bar(ui, if it.sub_progress.1 > 0 { "Sub-issues" } else { "Tasks" }, pr);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for pr in it.pull_requests.iter().take(2) {
                pr_chip(ui, pr);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !it.assignees.is_empty() {
                    avatars(ui, &it.assignees, 10.0, card_fill());
                }
                let t = it.updated.max(it.created);
                if t > 0 {
                    ui.label(RichText::new(short_age(t)).size(theme::FS_SMALL).color(theme::c().faint)).on_hover_text(format!("Updated {}", git::relative_time(t)));
                }
            });
        });
        if let Some(n) = nest {
            let (f, rows) = kids_inside(ui, &n);
            fold_rect = Some(f);
            kid_rects = rows;
        }
    }
    let outer = prepared.content_ui.min_rect().expand(11.0);
    let resp = ui.interact(outer, ui.scope_id().with(("card", &it.id)), Sense::click_and_drag());
    if selected {
        prepared.frame.stroke = Stroke::new(1.5_f32, theme::c().accent);
        prepared.frame.fill = card_hover();
    } else if resp.contains_pointer() {
        prepared.frame.fill = card_hover();
        prepared.frame.stroke = Stroke::new(1.0_f32, theme::c().widget_hover_border);
    }
    prepared.end(ui);
    CardOut { resp, parent: parent_rect, fold: fold_rect, kids: kid_rects }
}

/// Height of a sub-issue lifted out of its parent's card.
const KID_DRAG_H: f32 = 92.0;

/// A parent's sub-issues, drawn inside its card in the nested view.
struct Nest<'a> {
    kids: &'a [&'a Item],
    folded: bool,
    board: &'a Board,
}

/// What a card's click or drag landed on: the card, its parent line, the fold line of its
/// sub-issues, or one of them (item id and row).
struct CardOut {
    resp: Response,
    parent: Option<Rect>,
    fold: Option<Rect>,
    kids: Vec<(String, Rect)>,
}

/// The sub-issues section of a card: a line to fold them ("3 sub-issues"), then one row each with
/// its status colour, number and title. Returns the fold line and each row; the card's own
/// response takes the clicks, so the caller tells them apart by where they land.
fn kids_inside(ui: &mut Ui, n: &Nest) -> (Rect, Vec<(String, Rect)>) {
    let k = theme::c();
    let w = ui.available_width();
    let (line, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0_f32, k.border));
    let (fold, _) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
    let hot = ui.rect_contains_pointer(fold);
    let col = if hot { k.accent } else { k.dim };
    chevron(ui.painter(), pos2(fold.left() + 5.0, fold.center().y), !n.folded, col);
    let count = n.kids.len();
    ui.painter().text(pos2(fold.left() + 16.0, fold.center().y), Align2::LEFT_CENTER, format!("{count} sub-issue{}", if count == 1 { "" } else { "s" }), FontId::proportional(theme::FS_SMALL), col);
    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut rows = Vec::new();
    if !n.folded {
        for kid in n.kids {
            let (r, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
            let hot = ui.rect_contains_pointer(r);
            let p = ui.painter();
            if hot {
                p.rect_filled(r.expand2(vec2(4.0, 0.0)), CornerRadius::same(5), theme::c().hover);
            }
            p.circle_filled(pos2(r.left() + 5.0, r.center().y), 3.5, status_color(n.board, &kid.status));
            let num = p.text(pos2(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, kid.number.map(|n| format!("#{n}")).unwrap_or_default(), FontId::monospace(theme::FS_SMALL - 1.0), k.faint);
            let tc = if kid.is_closed() { k.dim } else { k.text };
            p.with_clip_rect(r.intersect(ui.clip_rect())).text(pos2(num.right() + 6.0, r.center().y), Align2::LEFT_CENTER, &kid.title, FontId::proportional(theme::FS_SECONDARY), tc);
            rows.push((kid.id.clone(), r));
        }
    }
    (fold, rows)
}

/// A small chevron centred on `c`, pointing down when `open`, right when folded.
fn chevron(p: &egui::Painter, c: Pos2, open: bool, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    let pts = if open { [c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)] } else { [c + vec2(-1.5, -3.5), c + vec2(2.0, 0.0), c + vec2(-1.5, 3.5)] };
    p.line_segment([pts[0], pts[1]], s);
    p.line_segment([pts[1], pts[2]], s);
}

impl App {
    /// Shows the Project view or goes back to the history; opening reads the project again.
    pub(super) fn board_toggle(&mut self) {
        let t = self.tab_mut();
        t.board.open = !t.board.open;
        t.board.detail = None;
        if t.board.open && t.project.loading.is_none() {
            self.project_load();
        }
    }
}

impl Tab {

    /// Esc puts a dragged card back where it was, else closes the ticket's detail. Returns
    /// whether it did something.
    pub(super) fn board_escape(&mut self, now: f64) -> bool {
        if !self.board.open || self.board.typing {
            return false;
        }
        if let Some(d) = self.board.drag.as_mut().filter(|d| d.glide.is_none()) {
            d.glide = Some((d.from.0.clone(), d.from.1, now));
            d.target = None;
            return true;
        }
        self.board.detail.take().is_some()
    }

    /// `full`: the board alone in the window (#153), which has its own way out.
    pub(super) fn board_view(&mut self, ui: &mut Ui, full: bool, cmds: &mut Vec<Cmd>) {
        self.board.typing = ui.ctx().memory(|m| m.focused().is_some());
        let loading = self.project.loading.is_some();
        let data = self.project.board.clone();

        egui::Frame::new().inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 10 }).show(ui, |ui| self.board_bar(ui, data.as_deref(), loading, full, cmds));

        let Some(board) = data else {
            let what = if loading {
                ui.add_space(ui.available_height() * 0.3);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Reading the GitHub project…").color(theme::c().dim));
                });
                None
            } else if let Some(e) = self.project.error.clone() {
                Some(empty_state(ui, "Can't read the GitHub project", &e, Some("Try again")))
            } else {
                empty_state(ui, "No GitHub project here", "This repository has no GitHub remote, so there is no project to show. Add one with a github.com address, then refresh.", None);
                None
            };
            if what == Some(true) {
                cmds.push(Cmd::ProjectReload);
            }
            return;
        };
        if board.projects.is_empty() {
            if empty_state(ui, "No project for this repository", "None of the GitHub projects you can see holds this repository's issues. Link a project from the repository's Projects tab on GitHub, then refresh.", Some("Refresh")) {
                cmds.push(Cmd::ProjectReload);
            }
            return;
        }

        let st = &self.board;
        let visible = |it: &&Item| (st.all || board.in_repo(it)) && (!st.mine || it.assignees.iter().any(|a| a == &board.me)) && matches(it, &st.search);
        let (top, kids) = arrange(board.items.iter().filter(visible).collect(), &board.items, st.tree);
        let mut cols = github::columns(&board.order(), top, true);
        if cols.iter().all(|c| c.1.is_empty()) {
            let (title, detail) = if !st.search.trim().is_empty() || st.mine {
                ("No card matches", "Nothing in the project matches these filters. Clear the search or turn off Mine.")
            } else if !st.all {
                ("Nothing for this repository yet", "The project has no ticket from this repository. Choose All to see the whole project.")
            } else {
                ("The project is empty", "Add tickets to the project on GitHub, then refresh.")
            };
            empty_state(ui, title, detail, None);
            return;
        }
        for (status, items) in &mut cols {
            if github::is_done(status) {
                items.sort_by_key(|i| std::cmp::Reverse(i.updated));
            }
        }

        let show_repo = st.all;
        let detail = st.detail.clone();
        let mut clicked: Option<String> = None;
        let mut goto: Option<Linked> = None;
        let mut flip: Option<String> = None;
        let mut fold_flip: Option<String> = None;
        let h = ui.available_height() - 12.0;
        if st.city_on {
            // Every ticket the filters keep, sub-issues included: each is a plot of its own.
            let shown: Vec<&Item> = board.items.iter().filter(visible).collect();
            // The Runner skin shows the stadium in its place (#206), the Ultra-trail the mountain
            // (#207), the Big eater the food court (#212).
            clicked = match self.skin {
                crate::settings::Skin::Runner => self.stadium_view(ui, &shown, detail.as_deref()),
                crate::settings::Skin::Trail => self.mountain_view(ui, &shown, detail.as_deref()),
                crate::settings::Skin::Eater => self.court_view(ui, &shown, detail.as_deref()),
                _ => self.city_view(ui, &shown, detail.as_deref()),
            };
        } else if st.list {
            let (id, fold) = list_view(ui, &board, &cols, &kids, &self.board.folded, show_repo, detail.as_deref(), cmds);
            if let Some(id) = id {
                clicked = Some(id);
            }
            fold_flip = fold;
        } else {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.drag_step(&ctx, &cols, now, cmds);
        let drag = self.board.drag.clone();
        // Where the pointer drags a card, for scrolling near the edges.
        let pointer = drag.as_ref().filter(|d| d.glide.is_none() && d.demo.is_none()).map(|d| d.pos + d.grab);
        let mut started: Option<Drag> = None;
        let mut landing: Option<Pos2> = None;
        let (mut columns_at, mut cards_at) = (Vec::new(), Vec::new());
        // The columns sit in the middle of the board when they fit, else from its left edge.
        let lead = centre_lead(ui.available_width(), cols.len());
        egui::ScrollArea::horizontal().id_salt("board_columns").scroll_source(NO_DRAG_SCROLL).auto_shrink([false, false]).show(ui, |ui| {
            if let Some(p) = pointer {
                let clip = ui.clip_rect();
                let dx = edge_scroll(p.x, clip.left(), clip.right());
                if dx != 0.0 {
                    ui.scroll_with_delta(vec2(dx, 0.0));
                }
            }
            ui.horizontal_top(|ui| {
                ui.add_space(lead);
                ui.spacing_mut().item_spacing.x = COL_GAP;
                for (status, items) in &cols {
                    let color = status_color(&board, status);
                    let done = github::is_done(status);
                    let limit = if done && !self.board.done_all.contains(status) { DONE_LIMIT } else { usize::MAX };
                    let hovered = drag.as_ref().is_some_and(|d| d.glide.is_none() && d.target.as_ref().is_some_and(|t| &t.0 == status));
                    let gap = drag.as_ref().and_then(|d| d.gap(status));
                    // The dragged card leaves its column; a gap keeps its room.
                    let shown: Vec<&Item> = items.iter().copied().filter(|i| drag.as_ref().is_none_or(|d| d.item != i.id)).take(limit).collect();
                    ui.allocate_ui_with_layout(vec2(COL_W, h), Layout::top_down(Align::Min), |ui| {
                        let border = if hovered { Stroke::new(1.5_f32, theme::c().accent) } else { Stroke::new(1.0_f32, theme::c().border) };
                        let frame = egui::Frame::new().fill(column_fill()).stroke(border).corner_radius(10).inner_margin(Margin { left: 8, right: 8, top: 0, bottom: 8 });
                        let col = frame.show(ui, |ui| {
                            ui.set_min_size(vec2(COL_W - 18.0, h - 10.0));
                            ui.set_max_width(COL_W - 18.0);
                            // Header: a coloured line, the status and its count.
                            let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::hover());
                            let p = ui.painter();
                            let top = Rect::from_min_max(pos2(hr.left() - 8.0, hr.top()), pos2(hr.right() + 8.0, hr.top() + 3.0));
                            p.rect_filled(top, CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, color);
                            p.circle_filled(pos2(hr.left() + 7.0, hr.center().y + 1.0), 4.5, color);
                            let name = p.text(pos2(hr.left() + 18.0, hr.center().y + 1.0), Align2::LEFT_CENTER, status, FontId::proportional(theme::FS_BODY), theme::c().strong);
                            let n = p.layout_no_wrap(items.len().to_string(), FontId::proportional(theme::FS_SMALL), ink(color));
                            let badge = Rect::from_min_size(pos2(name.right() + 8.0, hr.center().y + 1.0 - 9.0), vec2(n.size().x + 14.0, 18.0));
                            p.rect_filled(badge, CornerRadius::same(9), color.gamma_multiply(0.16));
                            p.galley(badge.center() - n.size() / 2.0, n, color);

                            egui::ScrollArea::vertical().id_salt(("board_col", status)).scroll_source(NO_DRAG_SCROLL).auto_shrink([false, false]).show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = CARD_GAP;
                                if let Some(p) = pointer.filter(|_| hovered) {
                                    let clip = ui.clip_rect();
                                    let dy = edge_scroll(p.y, clip.top(), clip.bottom());
                                    if dy != 0.0 {
                                        ui.scroll_with_delta(vec2(0.0, dy));
                                    }
                                }
                                if items.is_empty() {
                                    ui.add_space(10.0);
                                    ui.vertical_centered(|ui| ui.label(RichText::new("No cards").size(theme::FS_SECONDARY).color(theme::c().faint)));
                                }
                                let mut gap_at = |ui: &mut Ui, j: usize, below: &str| {
                                    if let Some(d) = &drag {
                                        if let Some(at) = card_gap(ui, Id::unique(("board_gap", d.gen, status, below)), gap == Some(j), d.size) {
                                            landing = Some(at);
                                        }
                                    }
                                };
                                for (j, it) in shown.iter().enumerate() {
                                    gap_at(ui, j, &it.id);
                                    // Its sub-issues inside it, in the nested view (the dragged one left out).
                                    let inside: Vec<&Item> = kids.get(&it.id).map(|v| v.iter().copied().filter(|k| drag.as_ref().is_none_or(|d| d.item != k.id)).collect()).unwrap_or_default();
                                    let nest = (!inside.is_empty()).then(|| Nest { kids: &inside, folded: self.board.folded.contains(&it.id), board: &board });
                                    let c = card(ui, it, show_repo, detail.as_deref() == Some(it.id.as_str()), nest);
                                    let resp = c.resp;
                                    cards_at.push((status.clone(), it.id.clone(), resp.rect));
                                    let at = resp.interact_pointer_pos();
                                    let kid_at = at.and_then(|p| c.kids.iter().find(|(_, r)| r.contains(p))).map(|(id, r)| (id.clone(), *r));
                                    if resp.drag_started() && drag.is_none() {
                                        let p = at.unwrap_or(resp.rect.center());
                                        // A sub-issue leaves its parent's card; the parent moves as a whole.
                                        let d = match kid_at.as_ref().and_then(|(id, r)| inside.iter().find(|k| &k.id == id).map(|k| (k, r))) {
                                            Some((k, r)) => Drag { item: k.id.clone(), from: (k.status.clone(), usize::MAX), grab: p - r.min, size: vec2(resp.rect.width(), KID_DRAG_H), pos: r.min, target: Some((k.status.clone(), usize::MAX)), glide: None, gen: self.board.drags + 1, demo: None },
                                            None => Drag { item: it.id.clone(), from: (status.clone(), j), grab: p - resp.rect.min, size: resp.rect.size(), pos: resp.rect.min, target: Some((status.clone(), j)), glide: None, gen: self.board.drags + 1, demo: None },
                                        };
                                        started = Some(d);
                                    } else if resp.clicked() {
                                        let on = |r: Option<Rect>| r.zip(at).is_some_and(|(r, p)| r.contains(p));
                                        if on(c.fold) {
                                            fold_flip = Some(it.id.clone());
                                        } else if let Some((id, _)) = kid_at {
                                            clicked = Some(id);
                                        } else if on(c.parent) {
                                            goto = it.parent.clone();
                                        } else {
                                            clicked = Some(it.id.clone());
                                        }
                                    }
                                    if drag.is_none() {
                                        resp.context_menu(|ui| card_menu(ui, &board, it, cmds));
                                    }
                                }
                                gap_at(ui, shown.len(), "");
                                if done && items.len() > DONE_LIMIT {
                                    let label = if limit == usize::MAX { "Show the latest 20".to_string() } else { format!("Show all {}", items.len()) };
                                    ui.vertical_centered(|ui| {
                                        if crate::modal::button(ui, &label, crate::modal::Kind::Plain, true).clicked() {
                                            flip = Some(status.clone());
                                        }
                                    });
                                }
                            });
                        });
                        columns_at.push((status.clone(), col.response.rect.intersect(ui.clip_rect())));
                    });
                }
                ui.add_space(16.0);
            });
        });
        self.board.columns_at = columns_at;
        self.board.cards_at = cards_at;
        if let Some(d) = started {
            ctx.animate_value_with_time(Id::unique(("board_lift", d.gen)), 0.0, 0.0);
            self.board.drags = d.gen;
            self.board.drag = Some(d);
        }
        self.drag_overlay(&ctx, &board, show_repo, landing, now);
        }
        if let Some(id) = fold_flip {
            if !self.board.folded.remove(&id) {
                self.board.folded.insert(id);
            }
        }
        if let Some(s) = flip {
            if !self.board.done_all.remove(&s) {
                self.board.done_all.insert(s);
            }
        }
        if let Some(id) = clicked {
            self.board.detail = if self.board.detail.as_deref() == Some(id.as_str()) { None } else { Some(id) };
        }
        // A parent opens in the details panel when it is on the board as shown, else on GitHub.
        if let Some(p) = goto {
            match cols.iter().flat_map(|c| c.1.iter()).find(|i| !p.url.is_empty() && i.url == p.url) {
                Some(it) => self.board.detail = Some(it.id.clone()),
                None if !p.url.is_empty() => cmds.push(Cmd::OpenUrl(p.url)),
                None => {}
            }
        }
    }

    /// Follows the pointer with the dragged card, at the start of a frame and with the last
    /// frame's places: the column and place it points at, then on release the move and the
    /// glide to where the card lands (back where it was when let go outside the columns).
    fn drag_step(&mut self, ctx: &egui::Context, cols: &[(String, Vec<&Item>)], now: f64, cmds: &mut Vec<Cmd>) {
        // FOURCHETTE_BOARD_DRAG: once the board is drawn, the card is lifted over the next column.
        if let Some(n) = self.board.drag_demo {
            let at = cols.iter().position(|c| c.1.iter().any(|i| i.number == Some(n)));
            let card = cols.iter().flat_map(|c| c.1.iter()).find(|i| i.number == Some(n));
            let rect = card.and_then(|it| self.board.cards_at.iter().find(|c| c.1 == it.id)).map(|c| c.2);
            let next = at.and_then(|i| self.board.columns_at.get(i + 1)).map(|c| c.1);
            if let (Some(i), Some(it), Some(r), Some(next)) = (at, card, rect, next) {
                let j = cols[i].1.iter().position(|x| x.id == it.id).unwrap_or(0);
                let grab = vec2(r.width() * 0.4, 26.0);
                let p = pos2(next.center().x - r.width() * 0.1, next.top() + 150.0);
                self.board.drags += 1;
                self.board.drag = Some(Drag { item: it.id.clone(), from: (cols[i].0.clone(), j), grab, size: r.size(), pos: p - grab, target: None, glide: None, gen: self.board.drags, demo: Some(p) });
                self.board.drag_demo = None;
            }
        }
        let Some(d) = self.board.drag.as_mut() else { return };
        ctx.request_repaint();
        if let Some((_, _, t0)) = &d.glide {
            if now - t0 > GLIDE_TIME as f64 {
                self.board.drag = None;
            }
            return;
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        let (pointer, down) = match d.demo {
            Some(p) => (Some(p), true),
            None => ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down())),
        };
        if let Some(p) = pointer {
            d.pos = p - d.grab;
            d.target = drop_target(p, &self.board.columns_at, &self.board.cards_at, &d.item);
        }
        if down {
            return;
        }
        let Some((status, slot)) = d.target.clone() else {
            d.glide = Some((d.from.0.clone(), d.from.1, now));
            return;
        };
        let column: Vec<&Item> = cols.iter().find(|c| c.0 == status).map(|c| c.1.clone()).unwrap_or_default();
        let ids: Vec<&str> = column.iter().map(|i| i.id.as_str()).collect();
        let m = drop_move(d, &status, slot, &ids);
        // Done shows the card at its date's place, among the column's other cards.
        let slot = match (&m, github::is_done(&status)) {
            (None, _) => d.from.1,
            (Some(_), true) => {
                let updated = cols.iter().flat_map(|c| c.1.iter()).find(|i| i.id == d.item).map(|i| i.updated).unwrap_or(0);
                column.iter().filter(|i| i.id != d.item && i.updated > updated).count()
            }
            (Some(_), false) => slot,
        };
        d.glide = Some((status, slot, now));
        if let Some(m) = m {
            cmds.push(Cmd::ProjectMove(m));
        }
    }

    /// The dragged card on top of everything: lifted (a shadow, a little larger) while held,
    /// then gliding to `landing`, the gap left for it, as it settles.
    fn drag_overlay(&mut self, ctx: &egui::Context, board: &Board, show_repo: bool, landing: Option<Pos2>, now: f64) {
        let Some(d) = self.board.drag.clone() else { return };
        let Some(it) = board.items.iter().find(|i| i.id == d.item) else {
            self.board.drag = None;
            return;
        };
        let held = d.glide.is_none();
        let lift = ctx.animate_value_with_time(Id::unique(("board_lift", d.gen)), if held { 1.0 } else { 0.0 }, if held { 0.12 } else { GLIDE_TIME });
        let pos = match (&d.glide, landing) {
            (Some((_, _, t0)), Some(to)) => {
                let t = ((now - t0) as f32 / GLIDE_TIME).clamp(0.0, 1.0);
                d.pos.lerp(to, 1.0 - (1.0 - t).powi(3))
            }
            // Its place is out of sight (folded Done, filtered): it fades where it was let go.
            (Some(_), None) if lift < 0.05 => {
                self.board.drag = None;
                return;
            }
            _ => d.pos,
        };
        let rect = Rect::from_min_size(pos, d.size);
        let area = egui::Area::new(Id::unique("board_drag")).order(egui::Order::Tooltip).fixed_pos(pos).interactable(false).show(ctx, |ui| {
            ui.set_width(d.size.x);
            ui.set_max_width(d.size.x);
            if landing.is_none() && !held {
                ui.set_opacity(lift);
            }
            let shadow = egui::epaint::Shadow { offset: [0, (10.0 * lift) as i8], blur: (26.0 * lift) as u8, spread: 0, color: Color32::from_black_alpha((if theme::is_light() { 60.0 } else { 120.0 } * lift) as u8) };
            ui.painter().add(shadow.as_shape(rect, CornerRadius::same(8)));
            card(ui, it, show_repo, true, None);
        });
        // A little larger while held, around the card's middle.
        let scale = 1.0 + 0.035 * lift;
        ctx.set_transform_layer(area.response.layer_id, TSTransform::new(rect.center().to_vec2() * (1.0 - scale), scale));
    }

    fn board_bar(&mut self, ui: &mut Ui, board: Option<&Board>, loading: bool, full: bool, cmds: &mut Vec<Cmd>) {
        let mut two_rows = false;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(22.0, 28.0), Sense::hover());
            board_icon(ui.painter(), r.center() + vec2(2.0, 2.0), theme::c().accent_text);
            let title = board.and_then(|b| b.project()).map(|p| p.title.clone()).unwrap_or_else(|| "Project".into());
            match board {
                Some(b) if b.projects.len() > 1 => {
                    egui::ComboBox::from_id_salt("board_project").selected_text(RichText::new(title).size(theme::FS_TITLE).color(theme::c().strong)).show_ui(ui, |ui| {
                        for (i, p) in b.projects.iter().enumerate() {
                            if ui.selectable_label(i == b.chosen, &p.title).clicked() && i != b.chosen {
                                cmds.push(Cmd::ProjectPick(i));
                            }
                        }
                    });
                }
                _ => {
                    ui.label(RichText::new(title).size(theme::FS_TITLE).color(theme::c().strong));
                }
            }
            if let Some(p) = board.and_then(|b| b.project()) {
                if ui.link(RichText::new("Open on GitHub").size(theme::FS_SMALL).color(theme::c().dim)).clicked() {
                    cmds.push(Cmd::OpenUrl(p.url.clone()));
                }
            }
            // The pickers go on a second line when the bar is too narrow for them and the search.
            two_rows = ui.available_width() < self.bar_pickers_width(ui, board) + BAR_RIGHT_W;
            if !two_rows {
                ui.add_space(12.0);
                self.bar_pickers(ui, board);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !full && full_button(ui).clicked() {
                    cmds.push(Cmd::BoardFull);
                }
                if refresh_button(ui, loading).clicked() && !loading {
                    cmds.push(Cmd::ProjectReload);
                }
                if loading {
                    ui.ctx().request_repaint();
                }
                ui.add(egui::TextEdit::singleline(&mut self.board.search).hint_text("Search cards").desired_width(216.0).margin(Margin::symmetric(8, 5)));
                if let Some(e) = &self.project.error {
                    if board.is_some() {
                        ui.label(RichText::new(e).size(theme::FS_SMALL).color(theme::c().modified));
                    }
                }
            });
        });
        if two_rows {
            ui.add_space(6.0);
            ui.horizontal(|ui| self.bar_pickers(ui, board));
        }
    }

    /// Where the board looks: this repository or all, board or list, how sub-issues show, Mine.
    fn bar_pickers(&mut self, ui: &mut Ui, board: Option<&Board>) {
        if let Some(i) = segmented(ui, &["This repo", "All"], self.board.all as usize) {
            self.board.all = i == 1;
        }
        let view = if self.board.city_on { 2 } else { self.board.list as usize };
        if let Some(i) = segmented(ui, &["Board", "List", self.city_label()], view) {
            self.board.list = i == 1;
            self.board.city_on = i == 2;
        }
        let trees = Tree::ALL.map(Tree::label);
        if let Some(i) = segmented(ui, &trees, Tree::ALL.iter().position(|t| *t == self.board.tree).unwrap_or(2)) {
            self.board.tree = Tree::ALL[i];
        }
        let me = board.map(|b| b.me.clone()).unwrap_or_default();
        if !me.is_empty() && toggle_chip(ui, "Mine", self.board.mine).on_hover_text(format!("Only tickets assigned to {me}")).clicked() {
            self.board.mine = !self.board.mine;
        }
    }

    /// The third view's name: the city, the Runner skin's stadium, the Ultra-trail's mountain or
    /// the Big eater's food court.
    fn city_label(&self) -> &'static str {
        match self.skin {
            crate::settings::Skin::Runner => "Stadium",
            crate::settings::Skin::Trail => "Mountain",
            crate::settings::Skin::Eater => "Food court",
            _ => "City",
        }
    }

    /// The width `bar_pickers` takes.
    fn bar_pickers_width(&self, ui: &Ui, board: Option<&Board>) -> f32 {
        let sets: [&[&str]; 3] = [&["This repo", "All"], &["Board", "List", self.city_label()], &Tree::ALL.map(Tree::label)];
        let mine = board.is_some_and(|b| !b.me.is_empty());
        12.0 + sets.iter().map(|o| segmented_width(ui, o)).sum::<f32>() + if mine { segmented_width(ui, &["Mine"]) + 8.0 } else { 0.0 } + 4.0 * ui.spacing().item_spacing.x
    }

    /// The details panel while the Project view is on screen: the chosen ticket's detail.
    pub(super) fn board_detail_panel(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let board = self.project.board.clone();
        let item = board.as_ref().zip(self.board.detail.as_ref()).and_then(|(b, id)| b.items.iter().find(|i| &i.id == id));
        let (Some(board), Some(it)) = (board.as_deref(), item) else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| ui.label(RichText::new("Select a ticket to see its details").color(theme::c().dim)));
            return;
        };
        let mut goto: Option<Linked> = None;
        if self.ticket_detail(ui, board, it, &mut goto, cmds) {
            self.board.detail = None;
        }
        if let Some(p) = goto {
            match board.items.iter().find(|i| !p.url.is_empty() && i.url == p.url) {
                Some(it) => self.board.detail = Some(it.id.clone()),
                None if !p.url.is_empty() => cmds.push(Cmd::OpenUrl(p.url)),
                None => {}
            }
        }
    }

    /// A ticket's detail, drawn in the details panel; a click on its parent lands in `goto`.
    /// Returns whether it asked to close.
    fn ticket_detail(&mut self, ui: &mut Ui, board: &Board, it: &Item, goto: &mut Option<Linked>, cmds: &mut Vec<Cmd>) -> bool {
        let mut close = false;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            type_pill(ui, it);
            if let Some(n) = it.number {
                ui.label(RichText::new(format!("#{n}")).monospace().color(theme::c().dim));
            }
            if !it.repo.is_empty() {
                ui.label(RichText::new(&it.repo).size(theme::FS_SMALL).color(theme::c().faint));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if close_button(ui).clicked() {
                    close = true;
                }
            });
        });
        egui::ScrollArea::vertical().id_salt("ticket_detail_scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(&it.title).size(17.0).color(theme::c().strong)).wrap());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let (state, color) = match (it.kind.clone(), it.state.as_str()) {
                    (Kind::Draft, _) => ("Draft", theme::c().dim),
                    (_, "MERGED") => ("Merged", theme::c().renamed),
                    (_, "CLOSED") => ("Closed", theme::c().renamed),
                    _ => ("Open", theme::c().added),
                };
                pill(ui, state, theme::c().on_accent, color, None);
                let sc = status_color(board, &it.status);
                pill(ui, &it.status, ink(sc), sc.gamma_multiply(0.16), Some(sc));
                for f in &it.fields {
                    field_chip(ui, f);
                }
            });
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if !it.url.is_empty() && crate::modal::button(ui, "Open on GitHub", crate::modal::Kind::Primary, true).clicked() {
                    cmds.push(Cmd::OpenUrl(it.url.clone()));
                }
                if let Some(n) = it.number {
                    if let Some(r) = self.refs.iter().find(|r| r.kind == RefKind::Local && branch_has_number(&r.name, n)) {
                        if crate::modal::button(ui, "Show in history", crate::modal::Kind::Plain, true).on_hover_text(format!("Select the tip of {} in All Commits", r.name)).clicked() {
                            cmds.push(Cmd::SelectHash(r.target.clone()));
                        }
                    }
                }
                if it.kind != Kind::PullRequest && crate::modal::button(ui, "Create a branch…", crate::modal::Kind::Plain, true).on_hover_text(format!("Branch “{}” at the current commit", github::branch_name(it))).clicked() {
                    cmds.push(Cmd::Dialog(Dialog::NewRef { at: "HEAD".into(), label: "the current commit".into(), name: github::branch_name(it), checkout: true, tag: false }));
                }
            });
            ui.add_space(12.0);

            let row = |ui: &mut Ui, caption: &str, add: &mut dyn FnMut(&mut Ui)| {
                ui.horizontal_top(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(82.0, 20.0), Sense::hover());
                    ui.painter().text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, caption, FontId::proportional(theme::FS_SMALL), theme::c().faint);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                        add(ui)
                    });
                });
            };
            row(ui, "Assignees", &mut |ui| {
                if it.assignees.is_empty() {
                    ui.label(RichText::new("No one").size(theme::FS_SECONDARY).color(theme::c().faint));
                }
                for a in &it.assignees {
                    avatars(ui, std::slice::from_ref(a), 9.0, theme::c().elevated);
                    ui.label(RichText::new(a).size(theme::FS_SECONDARY));
                    ui.add_space(6.0);
                }
            });
            if !it.labels.is_empty() {
                row(ui, "Labels", &mut |ui| {
                    for l in &it.labels {
                        label_pill(ui, l);
                    }
                });
            }
            if let Some(parent) = &it.parent {
                row(ui, "Parent", &mut |ui| {
                    let r = parent_line(ui, parent, 12.5);
                    if ui.interact(r.rect, ui.scope_id().with("drawer_parent"), Sense::click()).clicked() {
                        *goto = Some(parent.clone());
                    }
                });
            }
            if it.created > 0 {
                row(ui, "Dates", &mut |ui| {
                    ui.label(RichText::new(format!("Opened {} · updated {}", git::relative_time(it.created), git::relative_time(it.updated))).size(theme::FS_SECONDARY).color(theme::c().dim));
                });
            }

            if !it.sub_issues.is_empty() || it.sub_progress.1 > 0 {
                ui.add_space(14.0);
                ui.label(RichText::new("Sub-issues").size(theme::FS_BODY).color(theme::c().strong));
                progress_bar(ui, "Done", it.sub_progress);
                for s in &it.sub_issues {
                    if self.linked_row(ui, s, false).clicked() && !s.url.is_empty() {
                        cmds.push(Cmd::OpenUrl(s.url.clone()));
                    }
                }
            }
            if !it.pull_requests.is_empty() {
                ui.add_space(14.0);
                ui.label(RichText::new("Linked pull requests").size(theme::FS_BODY).color(theme::c().strong));
                for pr in &it.pull_requests {
                    if self.linked_row(ui, pr, true).clicked() && !pr.url.is_empty() {
                        cmds.push(Cmd::OpenUrl(pr.url.clone()));
                    }
                }
            }

            ui.add_space(14.0);
            let y = ui.cursor().top();
            ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0_f32, theme::c().border));
            ui.add_space(10.0);
            if it.body.trim().is_empty() {
                ui.label(RichText::new("No description.").italics().color(theme::c().faint));
            } else {
                markdown(ui, &it.body);
            }
            ui.add_space(20.0);
        });
        close
    }

    fn linked_row(&self, ui: &mut Ui, l: &Linked, pr: bool) -> Response {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, CornerRadius::same(5), theme::c().hover);
        }
        let c = pos2(rect.left() + 12.0, rect.center().y);
        if pr {
            pr_icon(p, c, state_color(&l.state));
        } else {
            issue_icon(p, c, l.state != "OPEN");
        }
        let n = p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, format!("#{}", l.number), FontId::monospace(theme::FS_SMALL), theme::c().dim);
        let text = if pr && l.title.is_empty() {
            match l.state.as_str() {
                "MERGED" => "Merged".to_string(),
                "CLOSED" => "Closed".to_string(),
                _ => "Open".to_string(),
            }
        } else {
            l.title.clone()
        };
        let color = if l.state == "OPEN" { theme::c().text } else { theme::c().dim };
        p.with_clip_rect(Rect::from_min_max(rect.min, pos2(rect.right() - 6.0, rect.max.y))).text(pos2(n.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, text, FontId::proportional(theme::FS_SECONDARY), color);
        resp.on_hover_text(if pr { "Open the pull request on GitHub" } else { "Open the sub-issue on GitHub" })
    }
}

/// The board's keys, from the window's keyboard handler.
pub(super) struct BoardKeys {
    /// ⌘B: the Project view, or back to the history.
    pub toggle: bool,
    /// ⌃⌘B: the board alone in the window (#153).
    pub full: bool,
    pub escape: bool,
}

pub(super) fn board_keys(ctx: &egui::Context) -> BoardKeys {
    ctx.input(|i| {
        let b = i.modifiers.command && !i.modifiers.shift && !i.modifiers.alt && i.key_pressed(Key::B);
        BoardKeys { toggle: b && !i.modifiers.ctrl, full: b && i.modifiers.ctrl, escape: i.key_pressed(Key::Escape) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_markdown_blocks() {
        let b = md_blocks("## Criteria\n- [ ] one\n  - [x] two\n* plain\n1. first\n\ntext\nmore\n```\ncode\n```\n---");
        assert_eq!(
            b,
            [
                Block::Heading(2, "Criteria".into()),
                Block::Item { indent: 0, bullet: Bullet::Check(false), text: "one".into() },
                Block::Item { indent: 1, bullet: Bullet::Check(true), text: "two".into() },
                Block::Item { indent: 0, bullet: Bullet::Dot, text: "plain".into() },
                Block::Item { indent: 0, bullet: Bullet::Number("1".into()), text: "first".into() },
                Block::Para("text more".into()),
                Block::Code("code".into()),
                Block::Rule,
            ]
        );
        assert_eq!(md_blocks("#hashtag"), [Block::Para("#hashtag".into())]);
    }

    #[test]
    fn reads_inline_marks() {
        assert_eq!(
            md_spans("**As** a dev, use `cargo` and [the docs](https://x) **"),
            [
                Span::Bold("As".into()),
                Span::Plain(" a dev, use ".into()),
                Span::Code("cargo".into()),
                Span::Plain(" and ".into()),
                Span::Link("the docs".into()),
                Span::Plain(" **".into()),
            ]
        );
        assert_eq!(md_spans("é `ü"), [Span::Plain("é `ü".into())]);
    }

    #[test]
    fn colours_and_text_on_them() {
        assert_eq!(hex_color("1cadf8"), Some(Color32::from_rgb(0x1c, 0xad, 0xf8)));
        assert_eq!(hex_color("#FFFFFF"), Some(Color32::WHITE));
        assert_eq!(hex_color("zz"), None);
        assert_eq!(readable_on(Color32::from_rgb(0xed, 0xed, 0xed)), theme::LIGHT.strong);
        assert_eq!(readable_on(Color32::from_rgb(0x70, 0x57, 0xff)), theme::DARK.strong);
    }

    #[test]
    fn finds_the_ticket_number_in_a_branch_name() {
        assert!(branch_has_number("feat/51-repo-groups", 51));
        assert!(branch_has_number("fix/51-crash", 51));
        assert!(branch_has_number("51-foo", 51));
        assert!(branch_has_number("topic_51", 51));
        assert!(!branch_has_number("feat/510-x", 51));
        assert!(!branch_has_number("feat/151-x", 51));
        assert!(!branch_has_number("main", 51));
    }

    fn drag(from: (&str, usize), target: Option<(&str, usize)>) -> Drag {
        Drag { item: "x".into(), from: (from.0.into(), from.1), grab: Vec2::ZERO, size: vec2(260.0, 90.0), pos: Pos2::ZERO, target: target.map(|(s, i)| (s.into(), i)), glide: None, gen: 1, demo: None }
    }

    #[test]
    fn finds_where_a_dragged_card_points() {
        let cols = vec![("Todo".to_string(), Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 500.0))), ("Done".to_string(), Rect::from_min_max(pos2(110.0, 0.0), pos2(210.0, 500.0)))];
        let card = |s: &str, id: &str, top: f32| (s.to_string(), id.to_string(), Rect::from_min_size(pos2(0.0, top), vec2(100.0, 80.0)));
        let cards = vec![card("Todo", "a", 50.0), card("Todo", "x", 140.0), card("Todo", "b", 230.0)];
        assert_eq!(drop_target(pos2(50.0, 60.0), &cols, &cards, "x"), Some(("Todo".into(), 0)), "above a's middle");
        assert_eq!(drop_target(pos2(50.0, 200.0), &cols, &cards, "x"), Some(("Todo".into(), 1)), "the dragged card doesn't count");
        assert_eq!(drop_target(pos2(50.0, 400.0), &cols, &cards, "x"), Some(("Todo".into(), 2)));
        assert_eq!(drop_target(pos2(150.0, 10.0), &cols, &cards, "x"), Some(("Done".into(), 0)));
        assert_eq!(drop_target(pos2(105.0, 10.0), &cols, &cards, "x"), None, "between columns");
    }

    #[test]
    fn a_drop_makes_the_right_move() {
        let col = ["a", "x", "b"];
        // Same column, same place: nothing to write.
        assert_eq!(drop_move(&drag(("Todo", 1), None), "Todo", 1, &col), None);
        // Same column, to the top: position only.
        assert_eq!(drop_move(&drag(("Todo", 1), None), "Todo", 0, &col), Some(Move { item: "x".into(), status: None, place: true, after: None }));
        assert_eq!(drop_move(&drag(("Todo", 1), None), "Todo", 2, &col).unwrap().after.as_deref(), Some("b"));
        // Another column: the status and the card above.
        let m = drop_move(&drag(("Todo", 1), None), "Doing", 1, &["c", "d"]).unwrap();
        assert_eq!((m.status.as_deref(), m.place, m.after.as_deref()), (Some("Doing"), true, Some("c")));
        // Done: the status only, whatever the place.
        assert_eq!(drop_move(&drag(("Todo", 1), None), "Done", 3, &["c"]), Some(Move { item: "x".into(), status: Some("Done".into()), place: false, after: None }));
        assert_eq!(drop_move(&drag(("Done", 0), None), "Done", 2, &["c"]), None);
        assert_eq!(move_to(&Item { id: "x".into(), ..Default::default() }, "Ready"), Move { item: "x".into(), status: Some("Ready".into()), place: true, after: None });
    }

    #[test]
    fn columns_part_where_the_card_would_land() {
        let d = drag(("Todo", 1), Some(("Doing", 2)));
        assert_eq!((d.gap("Todo"), d.gap("Doing"), d.gap("Done")), (Some(1), Some(2), None), "a placeholder where it was, a gap where it goes");
        let d = drag(("Todo", 1), Some(("Todo", 0)));
        assert_eq!(d.gap("Todo"), Some(0), "in its own column the gap follows it");
        let d = drag(("Todo", 1), Some(("Done", 4)));
        assert_eq!((d.gap("Todo"), d.gap("Done")), (Some(1), None), "Done keeps its date order");
        let mut d = drag(("Todo", 1), None);
        assert_eq!(d.gap("Todo"), Some(1), "outside the columns, its place waits for it");
        d.glide = Some(("Done".into(), 3, 0.0));
        assert_eq!((d.gap("Todo"), d.gap("Done")), (None, Some(3)), "once let go, only where it lands");
        assert_eq!(edge_scroll(200.0, 0.0, 400.0), 0.0);
        assert!(edge_scroll(10.0, 0.0, 400.0) > 0.0 && edge_scroll(395.0, 0.0, 400.0) < 0.0);
    }

    #[test]
    fn columns_are_centred_when_they_fit() {
        // Five columns take 5 × 284 + 4 × 12 = 1468 points.
        assert_eq!(centre_lead(2000.0, 5), 266.0);
        assert_eq!(centre_lead(1468.0, 5), 16.0, "the margin when they just fit");
        assert_eq!(centre_lead(1000.0, 5), 16.0, "from the left edge when they scroll");
        assert_eq!(centre_lead(1000.0, 0), 500.0);
    }

    #[test]
    fn the_three_trees_place_sub_issues() {
        let it = |n: u64, parent: Option<u64>| Item {
            id: format!("i{n}"),
            number: Some(n),
            url: format!("u{n}"),
            parent: parent.map(|p| Linked { number: p, title: String::new(), state: "OPEN".into(), url: format!("u{p}") }),
            ..Default::default()
        };
        // 1 has the sub-issues 2 and 3; 4 is a sub-issue of 9, which is not in the project.
        let project = vec![it(1, None), it(2, Some(1)), it(3, Some(1)), it(4, Some(9)), it(5, None)];
        let nums = |v: &[&Item]| v.iter().map(|i| i.number.unwrap()).collect::<Vec<_>>();
        let all: Vec<&Item> = project.iter().collect();
        let (top, kids) = arrange(all.clone(), &project, Tree::All);
        assert_eq!((nums(&top), kids.len()), (vec![1, 2, 3, 4, 5], 0));
        let (top, _) = arrange(all.clone(), &project, Tree::Parents);
        assert_eq!(nums(&top), vec![1, 4, 5], "a sub-issue whose parent isn't in the project counts as top level");
        let (top, kids) = arrange(all.clone(), &project, Tree::Nested);
        assert_eq!(nums(&top), vec![1, 4, 5]);
        assert_eq!(nums(&kids["i1"]), vec![2, 3]);
        // The parent filtered out (another repository, Mine, the search): its sub-issues stay at their place.
        let shown: Vec<&Item> = project.iter().filter(|i| i.number != Some(1)).collect();
        let (top, kids) = arrange(shown, &project, Tree::Nested);
        assert_eq!((nums(&top), kids.len()), (vec![2, 3, 4, 5], 0));
    }

    #[test]
    fn filters_and_chips() {
        let it = Item { title: "Draw curves".into(), number: Some(48), labels: vec![github::Label { name: "zone:graphe".into(), color: "fff".into() }], repo: "o/r".into(), assignees: vec!["camille".into()], ..Default::default() };
        assert!(matches(&it, "") && matches(&it, "#48") && matches(&it, "CURV") && matches(&it, "graphe") && matches(&it, "camil"));
        assert!(!matches(&it, "#5"));
        let f = |n: &str, v: &str| github::Field { name: n.into(), value: v.into(), color: None };
        assert_eq!(field_text(&f("Priority", "P1")), "P1");
        assert_eq!(field_text(&f("Size", "M")), "Size M");
        assert_eq!(field_text(&f("Estimate", "13")), "Estimate 13");
        assert_eq!(field_text(&f("Iteration", "Sprint 12")), "Sprint 12");
        assert_eq!(login_initials("jean-paul"), "JP");
    }
}
