//! Settings window (⌘,), laid out like macOS System Settings: the sections on the left, each with
//! a drawn icon, and the chosen section's settings on the right, in rounded cards. Every choice
//! applies and is saved at once, so there is no OK button; Esc, the close button or a click
//! outside closes it. A new section is one entry in `SECTIONS` and one function drawing its cards.
use super::*;
use crate::settings::{count_label, minutes_label, ClaudeModel, CodeFont, CommitLanguage, Settings, UiFont, DIFF_CONTEXTS, FETCH_MINUTES, MAX_COMMITS, SUBJECT_RANGE};
use crate::theme::{Appearance, Colors, Variant};
use crate::ui::appearance_icon;
use egui::{pos2, vec2, Align, Align2, CornerRadius, Key, Layout, Margin, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};

// Everything sits on an 8-point grid.
const WIN_W: f32 = 720.0;
const WIN_H: f32 = 480.0;
const SIDEBAR_W: f32 = 176.0;
/// The title row at the top of both panes.
const HEADER_H: f32 = 56.0;
const ITEM_H: f32 = 32.0;
/// Space around the cards, and between them.
const PAD: f32 = 24.0;
const GAP: f32 = 16.0;
const ROW_H: f32 = 56.0;
const SEG_W: f32 = 264.0;
/// The Claude section's controls: five short model names, then three languages, narrower than
/// `SEG_W` so that their help text fits beside them.
const MODEL_W: f32 = 280.0;
const LANG_W: f32 = 240.0;
const SEG_H: f32 = 28.0;
const FONT_W: f32 = 200.0;
const TILE_H: f32 = 64.0;
const RADIUS: u8 = crate::modal::RADIUS;

/// What a section draws with: the choices on screen, and where changes go.
pub(super) struct Pane<'a> {
    appearance: Appearance,
    variant: Variant,
    settings: &'a mut Settings,
    cmds: &'a mut Vec<Cmd>,
}

/// One section of the window.
pub(super) struct Section {
    /// Its name in `FOURCHETTE_SETTINGS=<key>`.
    pub key: &'static str,
    pub label: &'static str,
    /// The fill of its icon's rounded square; the icon is drawn in white on it.
    tint: Color32,
    /// Draws the icon centred on the point, in white.
    icon: fn(&Painter, Pos2),
    /// Draws the section's cards.
    body: fn(&mut Ui, &mut Pane<'_>),
}

pub(super) const SECTIONS: [Section; 5] = [
    Section { key: "appearance", label: "Appearance", tint: Color32::from_rgb(0x6e, 0x5b, 0xd6), icon: appearance_glyph, body: appearance },
    Section { key: "fetch", label: "Fetch", tint: Color32::from_rgb(0x1c, 0x8c, 0xf0), icon: fetch_glyph, body: fetch },
    Section { key: "git", label: "Git", tint: Color32::from_rgb(0xe8, 0x5d, 0x3a), icon: git_glyph, body: git },
    Section { key: "diff", label: "Diff and commit", tint: Color32::from_rgb(0xe0, 0x7a, 0x1f), icon: diff_glyph, body: diff_commit },
    Section { key: "claude", label: "Claude", tint: Color32::from_rgb(0xc2, 0x4f, 0x9c), icon: claude_glyph, body: claude },
];

/// The section a key names (any case), or the first one.
pub(super) fn section_index(key: &str) -> usize {
    let key = key.trim().to_lowercase();
    SECTIONS.iter().position(|s| s.key == key).unwrap_or(0)
}

/// The section `delta` rows away (Up / Down), staying inside the list.
fn step(i: usize, delta: i32) -> usize {
    (i as i32 + delta).clamp(0, SECTIONS.len() as i32 - 1) as usize
}

fn appearance_glyph(p: &Painter, c: Pos2) {
    appearance_icon(p, c, Appearance::System, Color32::WHITE, Color32::TRANSPARENT);
}

fn fetch_glyph(p: &Painter, c: Pos2) {
    crate::ui::glyph(p, c, "fetch", 15.0, 1.4, Color32::WHITE);
}

fn git_glyph(p: &Painter, c: Pos2) {
    crate::ui::glyph(p, c, "branch", 15.0, 1.4, Color32::WHITE);
}

fn diff_glyph(p: &Painter, c: Pos2) {
    crate::ui::glyph(p, c, "diff", 15.0, 1.4, Color32::WHITE);
}

/// The assistant's sparkles, as on the toolbar's Claude button.
fn claude_glyph(p: &Painter, c: Pos2) {
    crate::ui::glyph(p, c, "ai", 15.0, 1.4, Color32::WHITE);
}

/// A section's entry in the sidebar: its icon in a coloured square, then its name.
fn sidebar_item(ui: &mut Ui, s: &Section, selected: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ITEM_H), Sense::click());
    let k = theme::c();
    let p = ui.painter();
    if selected {
        p.rect_filled(r, CornerRadius::same(6), k.select);
    } else if resp.hovered() {
        p.rect_filled(r, CornerRadius::same(6), k.hover);
    }
    let tile = Rect::from_center_size(pos2(r.left() + 18.0, r.center().y), vec2(20.0, 20.0));
    p.rect_filled(tile, CornerRadius::same(5), s.tint);
    (s.icon)(p, tile.center());
    p.text(pos2(tile.right() + 10.0, r.center().y), Align2::LEFT_CENTER, s.label, theme::body(), if selected { k.strong } else { k.text });
    resp
}

/// The fill of a card: the window's colour with a touch of the text colour, like macOS's grouped
/// boxes, so it reads in both modes and every variant.
fn card_fill(k: &Colors) -> Color32 {
    theme::tint(k.strong, k.elevated, if theme::is_light() { 0.03 } else { 0.035 })
}

/// A rounded card of rows, separated by hairlines.
struct Card<'u> {
    ui: &'u mut Ui,
    rows: usize,
}

impl Card<'_> {
    /// A full-width strip `h` high, with a hairline above it when `sep` and it is not the first.
    fn strip(&mut self, h: f32, sep: bool) -> Rect {
        let (r, _) = self.ui.allocate_exact_size(vec2(self.ui.available_width(), h), Sense::hover());
        if sep && self.rows > 0 {
            // In dark mode `border` almost matches the card's fill, so the stronger one shows.
            let k = theme::c();
            let line = if theme::is_light() { k.border } else { k.border_strong };
            self.ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0_f32, line));
        }
        self.rows += 1;
        r
    }

    /// A setting's row: its name and a line of help on the left. Returns the rect of the control,
    /// `ctrl` wide, against the card's right edge so that every control lines up.
    fn row(&mut self, title: &str, help: &str, ctrl: Vec2, enabled: bool) -> Rect {
        let r = self.strip(ROW_H, true);
        let k = theme::c();
        let (tc, hc) = if enabled { (k.text, k.dim) } else { (k.faint, k.faint) };
        let p = self.ui.painter();
        p.text(pos2(r.left(), r.center().y - 9.0), Align2::LEFT_CENTER, title, theme::body(), tc);
        p.text(pos2(r.left(), r.center().y + 9.0), Align2::LEFT_CENTER, help, theme::secondary(), hc);
        Rect::from_center_size(pos2(r.right() - ctrl.x * 0.5, r.center().y), ctrl)
    }
}

fn card(ui: &mut Ui, add: impl FnOnce(&mut Card)) {
    let k = theme::c();
    egui::Frame::new()
        .fill(card_fill(k))
        .stroke(Stroke::new(1.0_f32, k.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(16, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add(&mut Card { ui, rows: 0 });
        });
    ui.add_space(GAP);
}

/// A segmented control of `n` options in `rect`, the chosen one raised. `draw` paints option `i`
/// in its rect with the given text colour and background. Returns the option clicked.
fn segmented(ui: &mut Ui, rect: Rect, n: usize, chosen: usize, enabled: bool, id: &str, mut draw: impl FnMut(&Painter, Rect, usize, Color32, Color32)) -> Option<usize> {
    let k = theme::c();
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    let p = ui.painter().clone();
    p.rect(rect, CornerRadius::same(7), k.input, Stroke::new(1.0_f32, fade(k.border_strong)), StrokeKind::Inside);
    let w = rect.width() / n as f32;
    let mut clicked = None;
    for i in 0..n {
        let seg = Rect::from_min_size(pos2(rect.left() + w * i as f32, rect.top()), vec2(w, rect.height())).shrink(3.0);
        let resp = ui.interact(seg, ui.id().with((id, i)), if enabled { Sense::click() } else { Sense::hover() });
        let on = i == chosen;
        let hovered = enabled && resp.hovered();
        let bg = if on {
            p.rect(seg, CornerRadius::same(5), k.widget_active, Stroke::new(1.0_f32, fade(k.widget_active_border)), StrokeKind::Inside);
            k.widget_active
        } else if hovered {
            p.rect_filled(seg, CornerRadius::same(5), k.hover);
            k.hover
        } else {
            k.input
        };
        let fg = if on { k.strong } else if hovered { k.text } else { k.dim };
        draw(&p, seg, i, fade(fg), bg);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// An on/off switch in `rect`. Returns true when clicked.
fn toggle(ui: &mut Ui, rect: Rect, id: &str, on: bool) -> bool {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let t = ui.ctx().animate_bool(ui.id().with((id, "anim")), on);
    let k = theme::c();
    let track = theme::tint(k.accent, k.border_strong, t);
    let track = if resp.hovered() { theme::lighten(track, 0.08) } else { track };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same((rect.height() * 0.5) as u8), track);
    let r = rect.height() * 0.5 - 2.5;
    let x = egui::lerp((rect.left() + r + 2.5)..=(rect.right() - r - 2.5), t);
    p.circle_filled(pos2(x, rect.center().y), r, k.on_accent);
    resp.clicked()
}

/// A tiny window in a variant's colours (those of the mode on screen): sidebar, history rows with
/// their graph, a selected row and a button in the accent.
fn variant_preview(p: &Painter, r: Rect, k: &Colors) {
    let rad = 8u8;
    p.rect_filled(r, CornerRadius::same(rad), k.bg);
    let side = Rect::from_min_size(r.min, vec2((r.width() * 0.3).round(), r.height()));
    p.rect_filled(side, CornerRadius { nw: rad, sw: rad, ne: 0, se: 0 }, k.sidebar);
    p.vline(side.right(), r.y_range(), Stroke::new(1.0_f32, k.border));
    // The window buttons, then three sidebar rows, the second one selected.
    for (i, c) in [Color32::from_rgb(0xff, 0x5f, 0x57), Color32::from_rgb(0xfe, 0xbc, 0x2e), Color32::from_rgb(0x28, 0xc8, 0x40)].into_iter().enumerate() {
        p.circle_filled(pos2(side.left() + 7.0 + i as f32 * 5.0, r.top() + 7.0), 1.6, c);
    }
    for i in 0..3 {
        let y = r.top() + 22.0 + i as f32 * 11.0;
        if i == 1 {
            p.rect_filled(Rect::from_min_max(pos2(side.left() + 3.0, y - 4.5), pos2(side.right() - 3.0, y + 4.5)), CornerRadius::same(2), k.select);
        }
        let w = (side.width() - 12.0) * [0.7, 0.85, 0.55][i];
        p.rect_filled(Rect::from_min_size(pos2(side.left() + 6.0, y - 1.25), vec2(w, 2.5)), CornerRadius::same(1), if i == 1 { k.text } else { k.faint });
    }
    // History: a branch joining the main lane, a line of text per commit, the second one selected.
    let main = Rect::from_min_max(pos2(side.right() + 1.0, r.top()), r.max);
    let lane = |i: usize| main.left() + 9.0 + i as f32 * 7.0;
    let ys: [f32; 4] = std::array::from_fn(|i| r.top() + 11.0 + i as f32 * 11.0);
    p.rect_filled(Rect::from_min_max(pos2(main.left(), ys[1] - 5.0), pos2(main.right(), ys[1] + 5.0)), CornerRadius::ZERO, k.select);
    let (main_lane, branch) = (theme::lane_color(0), theme::lane_color(1));
    p.line_segment([pos2(lane(0), ys[1]), pos2(lane(0), r.bottom())], Stroke::new(1.5_f32, main_lane));
    p.line_segment([pos2(lane(1), ys[0]), pos2(lane(1), ys[1] + 4.0)], Stroke::new(1.5_f32, branch));
    p.line_segment([pos2(lane(1), ys[1] + 4.0), pos2(lane(0), ys[2])], Stroke::new(1.5_f32, branch));
    p.circle_filled(pos2(lane(1), ys[0]), 2.4, branch);
    for y in &ys[1..] {
        p.circle_filled(pos2(lane(0), *y), 2.4, main_lane);
    }
    let x0 = lane(2) + 2.0;
    for (i, y) in ys.iter().enumerate() {
        let w = (main.right() - x0 - 8.0) * [0.8, 0.62, 0.7, 0.45][i];
        p.rect_filled(Rect::from_min_size(pos2(x0, y - 1.25), vec2(w, 2.5)), CornerRadius::same(1), if i == 1 { k.strong } else { k.dim });
    }
    // A primary button in the accent, bottom right.
    let btn = Rect::from_min_size(pos2(main.right() - 26.0, r.bottom() - 12.0), vec2(20.0, 7.0));
    p.rect_filled(btn, CornerRadius::same(3), k.accent);
    p.rect_stroke(r, CornerRadius::same(rad), Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
}

/// The four variants as tiles: a preview and the name below, the one on screen ringed with the
/// accent. Returns the tile clicked.
fn variant_tiles(c: &mut Card, current: Variant) -> Option<Variant> {
    let r = c.strip(8.0 + TILE_H + 32.0, false);
    let k = theme::c();
    let n = Variant::ALL.len() as f32;
    let gap = 16.0;
    let w = ((r.width() - gap * (n - 1.0)) / n).floor();
    let mut picked = None;
    for (i, v) in Variant::ALL.into_iter().enumerate() {
        let tile = Rect::from_min_size(pos2(r.left() + i as f32 * (w + gap), r.top() + 8.0), vec2(w, TILE_H));
        let hit = Rect::from_min_max(tile.min, pos2(tile.right(), tile.bottom() + 24.0));
        let resp = c.ui.interact(hit, c.ui.id().with(("variant", i)), Sense::click()).on_hover_text(format!("{} · {}", v.label(), v.blurb()));
        let p = c.ui.painter();
        variant_preview(p, tile, v.colors(theme::is_light()));
        let on = v == current;
        if on {
            p.rect_stroke(tile.expand(3.0), CornerRadius::same(11), Stroke::new(2.0_f32, k.accent), StrokeKind::Inside);
        } else if resp.hovered() {
            p.rect_stroke(tile.expand(3.0), CornerRadius::same(11), Stroke::new(2.0_f32, k.widget_hover_border), StrokeKind::Inside);
        }
        let col = if on { k.strong } else if resp.hovered() { k.text } else { k.dim };
        p.text(pos2(tile.center().x, tile.bottom() + 16.0), Align2::CENTER_CENTER, v.label(), theme::body(), col);
        if resp.clicked() {
            picked = Some(v);
        }
    }
    picked
}

/// A compact drop-down in `rect` listing `options` by label; returns the one picked.
fn font_picker<T: Copy + PartialEq>(ui: &mut Ui, rect: Rect, id: &str, current: T, options: &[T], label: fn(T) -> &'static str) -> Option<T> {
    let mut picked = None;
    // A child, not a scope: a scope would move the card's cursor back up to the control.
    let mut child = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    {
        let ui = &mut child;
        egui::ComboBox::from_id_salt(id).selected_text(label(current)).width(rect.width() - 24.0).show_ui(ui, |ui| {
            for &o in options {
                if ui.selectable_label(o == current, label(o)).clicked() && o != current {
                    picked = Some(o);
                }
            }
        });
    }
    picked
}

fn appearance(ui: &mut Ui, pane: &mut Pane<'_>) {
    card(ui, |c| {
        let ctrl = c.row("Mode", "System follows macOS", vec2(SEG_W, SEG_H), true);
        let modes = Appearance::ALL;
        let chosen = modes.iter().position(|a| *a == pane.appearance).unwrap_or(0);
        let picked = segmented(c.ui, ctrl, modes.len(), chosen, true, "appearance", |p, r, i, fg, bg| {
            let a = modes[i];
            let w = 14.0 + 6.0 + p.layout_no_wrap(a.label().into(), theme::body(), fg).size().x;
            let x0 = r.center().x - w * 0.5;
            appearance_icon(p, pos2(x0 + 7.0, r.center().y), a, fg, bg);
            p.text(pos2(x0 + 20.0, r.center().y), Align2::LEFT_CENTER, a.label(), theme::body(), fg);
        });
        if let Some(i) = picked {
            if modes[i] != pane.appearance {
                pane.cmds.push(Cmd::SetAppearance(modes[i]));
            }
        }
    });
    card(ui, |c| {
        let s = &mut *pane.settings;
        let ctrl = c.row("Interface font", "Inter is built in; the others come from macOS", vec2(FONT_W, SEG_H), true);
        if let Some(f) = font_picker(c.ui, ctrl, "ui_font", s.ui_font, &UiFont::ALL, UiFont::label) {
            s.ui_font = f;
        }
        let ctrl = c.row("Code font", "Diffs, hashes and file contents", vec2(FONT_W, SEG_H), true);
        if let Some(f) = font_picker(c.ui, ctrl, "code_font", s.code_font, &CodeFont::ALL, CodeFont::label) {
            s.code_font = f;
        }
    });
    card(ui, |c| {
        c.row("Variant", "The colours of the whole app, and its Dock icon", Vec2::ZERO, true);
        if let Some(v) = variant_tiles(c, pane.variant) {
            if v != pane.variant {
                pane.cmds.push(Cmd::SetVariant(v));
            }
        }
    });
}

fn fetch(ui: &mut Ui, pane: &mut Pane<'_>) {
    let s = &mut *pane.settings;
    card(ui, |c| {
        let ctrl = c.row("Fetch automatically", "Gets new commits from the remote in the background", vec2(36.0, 20.0), true);
        if toggle(c.ui, ctrl, "auto_fetch", s.auto_fetch) {
            s.auto_fetch = !s.auto_fetch;
        }
        let ctrl = c.row("Every", "While a repository is open", vec2(SEG_W, SEG_H), s.auto_fetch);
        let chosen = FETCH_MINUTES.iter().position(|m| *m == s.fetch_minutes).unwrap_or(1);
        let picked = segmented(c.ui, ctrl, FETCH_MINUTES.len(), chosen, s.auto_fetch, "fetch_minutes", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, minutes_label(FETCH_MINUTES[i]), theme::body(), fg);
        });
        if let Some(i) = picked {
            s.fetch_minutes = FETCH_MINUTES[i];
        }
    });
}

/// Pull's mode and the stash, Fork's "pullSheetRebase", "pullSheetStashAndReapply" and
/// "deleteStashAfterApply" (all off by default).
fn git(ui: &mut Ui, pane: &mut Pane<'_>) {
    let s = &mut *pane.settings;
    card(ui, |c| {
        let ctrl = c.row("Pull", "Rebase keeps the history linear", vec2(SEG_W, SEG_H), true);
        let modes = ["Merge", "Rebase"];
        let picked = segmented(c.ui, ctrl, modes.len(), s.pull_rebase as usize, true, "pull_rebase", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, modes[i], theme::body(), fg);
        });
        if let Some(i) = picked {
            s.pull_rebase = i == 1;
        }
        let ctrl = c.row("Stash and reapply", "Puts uncommitted changes aside during a pull, then back", vec2(36.0, 20.0), true);
        if toggle(c.ui, ctrl, "pull_autostash", s.pull_autostash) {
            s.pull_autostash = !s.pull_autostash;
        }
    });
    card(ui, |c| {
        let ctrl = c.row("Delete stash after Apply", "Once applied without conflicts; Undo (⌘Z) brings it back", vec2(36.0, 20.0), true);
        if toggle(c.ui, ctrl, "delete_stash_after_apply", s.delete_stash_after_apply) {
            s.delete_stash_after_apply = !s.delete_stash_after_apply;
        }
    });
}

/// A `- value +` stepper in `rect`. Returns -1 or +1 when a button is clicked and enabled.
fn stepper(ui: &mut Ui, rect: Rect, id: &str, value: usize, can_dec: bool, can_inc: bool) -> i32 {
    let k = theme::c();
    let p = ui.painter().clone();
    p.rect(rect, CornerRadius::same(7), k.input, Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
    let btn_w = rect.height();
    let mut out = 0;
    for (i, (sign, on)) in [(-1, can_dec), (1, can_inc)].into_iter().enumerate() {
        let r = if i == 0 { Rect::from_min_size(rect.min, vec2(btn_w, rect.height())) } else { Rect::from_min_size(pos2(rect.right() - btn_w, rect.top()), vec2(btn_w, rect.height())) };
        let resp = ui.interact(r, ui.id().with((id, i)), if on { Sense::click() } else { Sense::hover() });
        if on && resp.hovered() {
            p.rect_filled(r.shrink(3.0), CornerRadius::same(5), k.hover);
        }
        let col = if !on { k.faint } else if resp.hovered() { k.strong } else { k.dim };
        let c = r.center();
        p.line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(1.4_f32, col));
        if sign > 0 {
            p.line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], Stroke::new(1.4_f32, col));
        }
        if on && resp.clicked() {
            out = sign;
        }
    }
    p.text(rect.center(), Align2::CENTER_CENTER, value.to_string(), theme::body(), k.strong);
    out
}

/// The choices of a segmented control, plus the current value when it is not one of them (the
/// palette's `settings maxCommitCount <n>` can set any number).
fn with_current<T: Copy + Ord>(options: &[T], current: T) -> Vec<T> {
    let mut v = options.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v.sort();
    }
    v
}

fn diff_commit(ui: &mut Ui, pane: &mut Pane<'_>) {
    let s = &mut *pane.settings;
    card(ui, |c| {
        let ctrl = c.row("Diff context", "Lines shown around each change", vec2(SEG_W, SEG_H), true);
        let opts = with_current(&DIFF_CONTEXTS, s.diff_context);
        let chosen = opts.iter().position(|n| *n == s.diff_context).unwrap_or(0);
        if let Some(i) = segmented(c.ui, ctrl, opts.len(), chosen, true, "diff_context", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, opts[i].to_string(), theme::body(), fg);
        }) {
            s.diff_context = opts[i];
        }
    });
    card(ui, |c| {
        let ctrl = c.row("Subject: warn above", "The counter turns orange", vec2(120.0, SEG_H), true);
        let low = s.subject_low;
        match stepper(c.ui, ctrl, "subject_low", low, low > *SUBJECT_RANGE.start(), low + 1 < s.subject_high) {
            -1 => s.subject_low = (low.saturating_sub(5)).max(*SUBJECT_RANGE.start()),
            1 => s.subject_low = (low + 5).min(s.subject_high - 1),
            _ => {}
        }
        let ctrl = c.row("Subject: limit", "The counter turns red, and Claude keeps under it", vec2(120.0, SEG_H), true);
        let high = s.subject_high;
        match stepper(c.ui, ctrl, "subject_high", high, high > s.subject_low + 1, high < *SUBJECT_RANGE.end()) {
            -1 => s.subject_high = high.saturating_sub(5).max(s.subject_low + 1),
            1 => s.subject_high = (high + 5).min(*SUBJECT_RANGE.end()),
            _ => {}
        }
    });
    card(ui, |c| {
        let ctrl = c.row("Commits loaded", "The most the history reads", vec2(SEG_W, SEG_H), true);
        let opts = with_current(&MAX_COMMITS, s.max_commits);
        let chosen = opts.iter().position(|n| *n == s.max_commits).unwrap_or(0);
        if let Some(i) = segmented(c.ui, ctrl, opts.len(), chosen, true, "max_commits", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, count_label(opts[i]), theme::body(), fg);
        }) {
            s.max_commits = opts[i];
        }
    });
}

/// The model of every Claude request, and the language of the commit messages it writes.
fn claude(ui: &mut Ui, pane: &mut Pane<'_>) {
    let s = &mut *pane.settings;
    card(ui, |c| {
        let ctrl = c.row("Model", "Default: Claude Code's own", vec2(MODEL_W, SEG_H), true);
        let models = ClaudeModel::ALL;
        let chosen = models.iter().position(|m| *m == s.claude_model).unwrap_or(0);
        if let Some(i) = segmented(c.ui, ctrl, models.len(), chosen, true, "claude_model", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, models[i].label(), theme::body(), fg);
        }) {
            s.claude_model = models[i];
        }
    });
    card(ui, |c| {
        let ctrl = c.row("Commit message language", "Automatic follows recent commits", vec2(LANG_W, SEG_H), true);
        let langs = CommitLanguage::ALL;
        let chosen = langs.iter().position(|l| *l == s.commit_language).unwrap_or(0);
        if let Some(i) = segmented(c.ui, ctrl, langs.len(), chosen, true, "commit_language", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, langs[i].label(), theme::body(), fg);
        }) {
            s.commit_language = langs[i];
        }
    });
    // Under the cards, lined up with the rows' names.
    let note = "The model writes commit messages, explains commits and cleans up branches.\nThe language only applies to commit messages.";
    egui::Frame::new().inner_margin(Margin::symmetric(16, 0)).show(ui, |ui| {
        ui.label(egui::RichText::new(note).size(theme::FS_SMALL).color(theme::c().faint));
    });
}

impl App {
    pub(super) fn settings_ui(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        if !self.settings_open {
            return;
        }
        // The window takes the keyboard while it is open (see `keyboard`): Esc closes it,
        // Up and Down move between sections.
        let (esc, up, down) = ctx.input(|i| (i.key_pressed(Key::Escape), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
        let mut close = esc;
        let mut section = step(self.settings_section.min(SECTIONS.len() - 1), down as i32 - up as i32);
        let mut s: Settings = self.library.settings;
        let k = theme::c();

        let screen = ctx.screen_rect();
        let min = pos2((screen.center().x - WIN_W * 0.5).round(), (screen.top() + 56.0).max(screen.center().y - WIN_H * 0.5).round());
        let t = crate::modal::appear(ctx, egui::Id::new("settings"));
        egui::Area::new(egui::Id::new("settings_backdrop")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::ZERO, crate::modal::backdrop().gamma_multiply(t));
            if resp.clicked() {
                close = true;
            }
        });
        egui::Area::new(egui::Id::new("settings")).order(egui::Order::Foreground).fixed_pos(min).show(ctx, |ui| {
            ui.multiply_opacity(t);
            let (win, _) = ui.allocate_exact_size(vec2(WIN_W, WIN_H), Sense::hover());
            let p = ui.painter().clone();
            crate::modal::paint_card(&p, win);
            let side = Rect::from_min_size(win.min, vec2(SIDEBAR_W, WIN_H));
            p.rect_filled(side, CornerRadius { nw: RADIUS, sw: RADIUS, ne: 0, se: 0 }, k.sidebar);
            p.vline(side.right(), side.y_range(), Stroke::new(1.0_f32, k.border));

            // Sidebar: the window's name, then the sections.
            p.text(pos2(side.left() + 16.0, side.top() + HEADER_H * 0.5), Align2::LEFT_CENTER, "Settings", egui::FontId::proportional(theme::FS_TITLE), k.strong);
            let list = Rect::from_min_max(pos2(side.left() + 8.0, side.top() + HEADER_H), pos2(side.right() - 8.0, side.bottom() - 8.0));
            let mut sui = ui.new_child(UiBuilder::new().id_salt("settings_sidebar").max_rect(list).layout(Layout::top_down(Align::Min)));
            sui.spacing_mut().item_spacing.y = 2.0;
            for (i, sec) in SECTIONS.iter().enumerate() {
                if sidebar_item(&mut sui, sec, i == section).clicked() {
                    section = i;
                }
            }

            // Content: the section's title and a close button, its cards, and a hint at the bottom.
            let content = Rect::from_min_max(pos2(side.right() + 1.0, win.top()), win.max);
            let sec = &SECTIONS[section];
            p.text(pos2(content.left() + PAD, content.top() + HEADER_H * 0.5), Align2::LEFT_CENTER, sec.label, egui::FontId::proportional(theme::FS_TITLE), k.strong);
            let x = Rect::from_center_size(pos2(content.right() - PAD - 4.0, content.top() + HEADER_H * 0.5), vec2(24.0, 24.0));
            let xr = ui.interact(x, ui.id().with("settings_close"), Sense::click()).on_hover_text("Close (Esc)");
            if xr.hovered() {
                p.rect_filled(x, CornerRadius::same(6), k.hover);
            }
            let xc = if xr.hovered() { k.text } else { k.dim };
            for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
                p.line_segment([x.center() - d, x.center() + d], Stroke::new(1.4_f32, xc));
            }
            if xr.clicked() {
                close = true;
            }
            let footer_h = 40.0;
            let body = Rect::from_min_max(pos2(content.left() + PAD, content.top() + HEADER_H), pos2(content.right() - PAD, content.bottom() - footer_h));
            let mut bui = ui.new_child(UiBuilder::new().id_salt(("settings_body", sec.key)).max_rect(body).layout(Layout::top_down(Align::Min)));
            egui::ScrollArea::vertical().id_salt(("settings_scroll", sec.key)).auto_shrink([false, false]).show(&mut bui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut pane = Pane { appearance: self.library.appearance, variant: theme::variant(), settings: &mut s, cmds };
                (sec.body)(ui, &mut pane);
            });
            // Footer, as in every dialog: a hairline, a darker strip and the hint.
            let foot = Rect::from_min_max(pos2(content.left(), content.bottom() - footer_h), content.max);
            p.rect_filled(foot, CornerRadius { nw: 0, ne: 0, sw: 0, se: RADIUS }, theme::tint(k.sidebar, k.elevated, 0.6));
            p.hline(foot.x_range(), foot.top(), Stroke::new(1.0_f32, k.border));
            let mut fui = ui.new_child(UiBuilder::new().id_salt("settings_footer").max_rect(foot.shrink2(vec2(PAD, 0.0))).layout(Layout::left_to_right(Align::Center)));
            crate::modal::hint_row(&mut fui, &[crate::modal::Hint::Text("Changes apply and are saved right away".into()), crate::modal::Hint::Key(crate::modal::Cap::Text("Esc")), crate::modal::Hint::Text("to close".into())]);

            crate::modal::paint_edge(&p, win);
        });
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("settings"));
        crate::modal::animate_layer(ctx, layer, Rect::from_min_size(min, vec2(WIN_W, WIN_H)).center(), t);
        self.settings_section = section;
        if s != self.library.settings {
            cmds.push(Cmd::SetSettings(s));
        }
        if close {
            self.settings_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_have_unique_lowercase_keys_and_names() {
        assert_eq!(SECTIONS.iter().map(|s| s.label).collect::<Vec<_>>(), ["Appearance", "Fetch", "Git", "Diff and commit", "Claude"]);
        for (i, s) in SECTIONS.iter().enumerate() {
            assert!(!s.label.is_empty());
            assert_eq!(s.key, s.key.to_lowercase(), "{}", s.label);
            assert_eq!(section_index(s.key), i, "each key finds its own section");
            assert_eq!(SECTIONS.iter().filter(|o| o.key == s.key || o.label == s.label).count(), 1, "{} is unique", s.key);
        }
    }

    #[test]
    fn with_current_adds_an_unlisted_value_in_order() {
        assert_eq!(with_current(&MAX_COMMITS, 100_000), MAX_COMMITS.to_vec());
        assert_eq!(with_current(&MAX_COMMITS, 5_000), vec![1_000, 5_000, 10_000, 50_000, 100_000]);
        assert_eq!(with_current(&DIFF_CONTEXTS, 7), vec![1, 3, 5, 7, 10, 25]);
    }

    #[test]
    fn section_index_reads_any_case_and_falls_back_to_the_first() {
        assert_eq!(section_index("fetch"), 1);
        assert_eq!(section_index(" Fetch "), 1);
        assert_eq!(section_index("GIT"), 2);
        assert_eq!(section_index("claude"), 4);
        // FOURCHETTE_SETTINGS=1, or a section that does not exist yet, opens Appearance.
        for other in ["1", "", "fonts", "appearance"] {
            assert_eq!(section_index(other), 0, "{other}");
        }
    }

    #[test]
    fn up_and_down_stay_inside_the_list() {
        let last = SECTIONS.len() - 1;
        assert_eq!(step(0, -1), 0);
        assert_eq!(step(0, 1), 1.min(last));
        assert_eq!(step(last, 1), last);
        assert_eq!(step(last, -1), last.saturating_sub(1));
        assert_eq!(step(1, 0), 1);
    }
}
