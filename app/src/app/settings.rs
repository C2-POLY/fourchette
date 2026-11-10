//! Settings window (⌘,), laid out like macOS System Settings: the sections on the left, each with
//! a drawn icon, and the chosen section's settings on the right, in rounded cards. Every choice
//! applies and is saved at once, so there is no OK button; Esc, the close button or a click
//! outside closes it. A new section is one entry in `SECTIONS` and one function drawing its cards.
use super::*;
use super::skin_tiles;
use super::updater::{now_secs, CheckStatus, UpdateState};
use crate::settings::{count_label, minutes_label, ClaudeModel, CodeFont, CommitLanguage, Settings, UiFont, UpdateChannel, DIFF_CONTEXTS, FETCH_MINUTES, MAX_COMMITS, SUBJECT_RANGE};
use crate::icon::IconColour;
use crate::theme::{Appearance, Colors};
use crate::ui::appearance_icon;
use egui::{pos2, vec2, Align, Align2, CornerRadius, Key, Layout, Margin, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};

// Everything sits on an 8-point grid.
const WIN_W: f32 = 832.0;
const WIN_H: f32 = 600.0;
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
/// The Updates section's two channels.
const CHANNEL_W: f32 = 220.0;
const SEG_H: f32 = 28.0;
const FONT_W: f32 = 200.0;
/// The App icon row: ten preset swatches and the free colour's, round, `SWATCH` across.
const SWATCH: f32 = 18.0;
const SWATCH_GAP: f32 = 7.0;
const SWATCHES_W: f32 = 11.0 * SWATCH + 10.0 * SWATCH_GAP;
const RADIUS: u8 = crate::modal::RADIUS;

/// What a section draws with: the choices on screen, and where changes go.
pub(super) struct Pane<'a> {
    appearance: Appearance,
    /// The icon colour the App icon row shows as chosen (FOURCHETTE_ICON's until one is picked).
    icon: IconColour,
    settings: &'a mut Settings,
    cmds: &'a mut Vec<Cmd>,
    /// The update checks, for the Updates section.
    update: &'a UpdateState,
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

pub(super) const SECTIONS: [Section; 6] = [
    Section { key: "appearance", label: "Appearance", tint: Color32::from_rgb(0x6e, 0x5b, 0xd6), icon: appearance_glyph, body: appearance },
    Section { key: "fetch", label: "Fetch", tint: Color32::from_rgb(0x1c, 0x8c, 0xf0), icon: fetch_glyph, body: fetch },
    Section { key: "git", label: "Git", tint: Color32::from_rgb(0xe8, 0x5d, 0x3a), icon: git_glyph, body: git },
    Section { key: "diff", label: "Diff and commit", tint: Color32::from_rgb(0xe0, 0x7a, 0x1f), icon: diff_glyph, body: diff_commit },
    Section { key: "claude", label: "Claude", tint: Color32::from_rgb(0xc2, 0x4f, 0x9c), icon: claude_glyph, body: claude },
    Section { key: "updates", label: "Updates", tint: Color32::from_rgb(0x2e, 0xa0, 0x5a), icon: updates_glyph, body: updates },
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

/// An arrow down onto a line, as on the update dialog.
fn updates_glyph(p: &Painter, c: Pos2) {
    crate::ui::glyph(p, c, "pull", 15.0, 1.4, Color32::WHITE);
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
/// boxes, so it reads in both modes.
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
fn segmented(ui: &mut Ui, rect: Rect, n: usize, chosen: usize, enabled: bool, id: &str, draw: impl FnMut(&Painter, Rect, usize, Color32, Color32)) -> Option<usize> {
    segmented_by(ui, rect, &vec![1.0; n], chosen, enabled, id, draw)
}

/// The same, each option as wide as its share of `weights`.
fn segmented_by(ui: &mut Ui, rect: Rect, weights: &[f32], chosen: usize, enabled: bool, id: &str, mut draw: impl FnMut(&Painter, Rect, usize, Color32, Color32)) -> Option<usize> {
    let k = theme::c();
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    let p = ui.painter().clone();
    p.rect(rect, CornerRadius::same(7), k.input, Stroke::new(1.0_f32, fade(k.border_strong)), StrokeKind::Inside);
    let unit = rect.width() / weights.iter().sum::<f32>().max(1.0);
    let mut clicked = None;
    let mut x = rect.left();
    for (i, weight) in weights.iter().enumerate() {
        let w = weight * unit;
        let seg = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height())).shrink(3.0);
        x += w;
        let resp = ui.interact(seg, ui.scope_id().with((id, i)), if enabled { Sense::click() } else { Sense::hover() });
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
    let resp = ui.interact(rect, ui.scope_id().with(id), Sense::click());
    let t = ui.ctx().animate_bool(ui.scope_id().with((id, "anim")), on);
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

/// A compact drop-down in `rect` listing `options` by label; returns the one picked.
fn font_picker<T: Copy + PartialEq>(ui: &mut Ui, rect: Rect, id: &str, current: T, options: &[T], label: fn(T) -> &'static str) -> Option<T> {
    let mut picked = None;
    // A child, not a scope: a scope would move the card's cursor back up to the control.
    let mut child = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    {
        let ui = &mut child;
        // Each name in its own font, once Settings has loaded them (from the frame after opening).
        let families = ui.fonts(|f| f.families());
        let styled = |o: T| {
            let (family, text) = (theme::preview_family(label(o)), egui::RichText::new(label(o)));
            if families.contains(&family) {
                text.family(family)
            } else {
                text
            }
        };
        egui::ComboBox::from_id_salt(id).selected_text(styled(current)).width(rect.width() - 24.0).show_ui(ui, |ui| {
            for &o in options {
                if ui.selectable_label(o == current, styled(o)).clicked() && o != current {
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
    card(ui, |c| skin_card(c, &mut pane.settings.skin));
    card(ui, |c| {
        let ctrl = c.row("App icon", "In the Dock and the Finder", vec2(SWATCHES_W, SWATCH + 8.0), true);
        if let Some(colour) = icon_swatches(c.ui, ctrl, pane.icon, pane.cmds) {
            pane.settings.app_icon = colour;
        }
    });
}

/// The Skin card (#220): the chosen skin's description in full, then one tile per skin across
/// the card, each with its picture, its name and a short line. A click chooses it.
fn skin_card(c: &mut Card, skin: &mut crate::settings::Skin) {
    use crate::settings::Skin;
    let k = theme::c();
    let width = c.ui.available_width();
    let about = c.ui.painter().layout(skin.about().into(), theme::secondary(), k.dim, width);
    let gap = 8.0;
    let tile_w = (width - gap * (Skin::ALL.len() - 1) as f32) / Skin::ALL.len() as f32;
    let preview_h = 74.0;
    // Each short line may wrap on a narrow tile: the tiles are as tall as the tallest.
    let lines: Vec<_> = Skin::ALL.iter().map(|s| c.ui.painter().layout(s.tagline().into(), theme::small(), k.dim, tile_w - 8.0)).collect();
    let text_h = 8.0 + 16.0 + lines.iter().map(|g| g.size().y).fold(0.0, f32::max) + 9.0;
    let tile_h = preview_h + text_h;
    let r = c.strip(14.0 + 18.0 + 4.0 + about.size().y + 12.0 + tile_h + 14.0, true);
    let p = c.ui.painter().clone();
    p.text(pos2(r.left(), r.top() + 23.0), Align2::LEFT_CENTER, "Skin", theme::body(), k.text);
    let about_pos = pos2(r.left(), r.top() + 14.0 + 18.0 + 4.0);
    p.galley(about_pos, about.clone(), k.dim);
    let top = about_pos.y + about.size().y + 12.0;
    let well = theme::tint(k.strong, k.bg, if theme::is_light() { 0.06 } else { 0.0 });
    for (i, s) in Skin::ALL.into_iter().enumerate() {
        let tile = Rect::from_min_size(pos2(r.left() + i as f32 * (tile_w + gap), top), vec2(tile_w, tile_h));
        let resp = c.ui.interact(tile, c.ui.scope_id().with(("skin_tile", i)), Sense::click());
        let on = s == *skin;
        if resp.hovered() {
            c.ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        p.rect_filled(tile, CornerRadius::same(8), k.elevated);
        let top_round = CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 };
        let pre = Rect::from_min_size(tile.min, vec2(tile_w, preview_h));
        p.rect_filled(pre, top_round, skin_tiles::backdrop(s).unwrap_or(well));
        skin_tiles::draw(&p.with_clip_rect(pre), pre.shrink2(vec2(6.0, 5.0)), s);
        p.hline(tile.x_range(), pre.bottom(), Stroke::new(1.0_f32, k.border));
        p.text(pos2(tile.left() + 6.0, pre.bottom() + 15.0), Align2::LEFT_CENTER, s.label(), theme::body(), if on { k.strong } else { k.text });
        p.galley(pos2(tile.left() + 6.0, pre.bottom() + 8.0 + 16.0), lines[i].clone(), k.dim);
        let (w, col) = if on {
            (2.0, k.accent)
        } else if resp.hovered() {
            (1.0, k.widget_hover_border)
        } else {
            (1.0, k.border_strong)
        };
        p.rect_stroke(tile, CornerRadius::same(8), Stroke::new(w, col), StrokeKind::Inside);
        if on {
            let badge = pos2(tile.right() - 11.0, tile.top() + 11.0);
            p.circle_filled(badge, 8.0, k.accent);
            let tick = Stroke::new(1.6_f32, k.on_accent);
            p.line_segment([badge + vec2(-3.5, 0.5), badge + vec2(-1.0, 3.0)], tick);
            p.line_segment([badge + vec2(-1.0, 3.0), badge + vec2(3.5, -2.5)], tick);
        }
        if resp.clicked() {
            *skin = s;
        }
    }
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The ring around the chosen swatch, a hair away from it.
fn swatch_ring(p: &Painter, centre: Pos2, colour: Color32) {
    p.circle_stroke(centre, SWATCH * 0.5 + 3.0, Stroke::new(2.0_f32, colour));
}

/// A round swatch of `fill`, with a hairline so that light colours keep their edge.
fn swatch(p: &Painter, centre: Pos2, fill: Color32) {
    p.circle_filled(centre, SWATCH * 0.5, fill);
    p.circle_stroke(centre, SWATCH * 0.5 - 0.5, Stroke::new(1.0_f32, Color32::from_black_alpha(40)));
}

/// The free colour's swatch: a colour wheel, with the colour chosen in its middle once there is one.
fn wheel_swatch(p: &Painter, centre: Pos2, custom: Option<Color32>) {
    let r = SWATCH * 0.5;
    let n = 36;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(centre, Color32::WHITE);
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let a = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let colour: Color32 = egui::ecolor::Hsva::new(t, 0.85, 1.0, 1.0).into();
        mesh.colored_vertex(centre + vec2(a.cos(), a.sin()) * r, colour);
        if i > 0 {
            mesh.add_triangle(0, i, i + 1);
        }
    }
    p.add(egui::Shape::mesh(mesh));
    if let Some(c) = custom {
        p.circle_filled(centre, r * 0.55, c);
        p.circle_stroke(centre, r * 0.55, Stroke::new(1.5_f32, Color32::WHITE));
    }
}

/// The App icon row's swatches in `rect`: the presets, then the free colour, whose picker opens in
/// a popup. While its colour is dragged only the Dock follows (`Cmd::PreviewIcon`); the colour is
/// chosen, and saved, once the button is released. Returns the colour chosen this frame.
fn icon_swatches(ui: &mut Ui, rect: Rect, chosen: IconColour, cmds: &mut Vec<Cmd>) -> Option<IconColour> {
    let k = theme::c();
    let p = ui.painter().clone();
    let mut picked = None;
    let centre = |i: usize| pos2(rect.left() + SWATCH * 0.5 + i as f32 * (SWATCH + SWATCH_GAP), rect.center().y);
    for (i, preset) in IconColour::PRESETS.into_iter().enumerate() {
        let c = centre(i);
        let hit = Rect::from_center_size(c, vec2(SWATCH + 6.0, SWATCH + 6.0));
        let resp = ui.interact(hit, ui.scope_id().with(("icon_colour", i)), Sense::click()).on_hover_text(preset.label());
        swatch(&p, c, rgb(preset.base()));
        if preset == chosen {
            swatch_ring(&p, c, k.strong);
        } else if resp.hovered() {
            swatch_ring(&p, c, k.border_strong);
        }
        if resp.clicked() && preset != chosen {
            picked = Some(preset);
        }
    }

    // The free colour.
    let c = centre(IconColour::PRESETS.len());
    let hit = Rect::from_center_size(c, vec2(SWATCH + 6.0, SWATCH + 6.0));
    let resp = ui.interact(hit, ui.scope_id().with("icon_colour_custom"), Sense::click()).on_hover_text("Another colour…");
    let custom = matches!(chosen, IconColour::Custom(_));
    wheel_swatch(&p, c, custom.then(|| rgb(chosen.base())));
    if custom {
        swatch_ring(&p, c, k.strong);
    } else if resp.hovered() {
        swatch_ring(&p, c, k.border_strong);
    }
    // The picker's colour, and whether it was dragged since the last chosen one.
    let id = ui.scope_id().with("icon_colour_picker");
    egui::Popup::from_toggle_button_response(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).gap(6.0).show(|ui| {
        let (mut colour, mut dirty) = ui.data_mut(|d| *d.get_temp_mut_or(id, (rgb(chosen.base()), false)));
        ui.spacing_mut().slider_width = 220.0;
        if egui::color_picker::color_picker_color32(ui, &mut colour, egui::color_picker::Alpha::Opaque) {
            dirty = true;
            cmds.push(Cmd::PreviewIcon(IconColour::Custom([colour.r(), colour.g(), colour.b()])));
        }
        let custom = IconColour::Custom([colour.r(), colour.g(), colour.b()]);
        if dirty && !ui.input(|i| i.pointer.any_down()) {
            dirty = false;
            if custom != chosen {
                picked = Some(custom);
            }
        }
        ui.data_mut(|d| d.insert_temp(id, (colour, dirty)));
    });
    if !egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp)) {
        // Closed: it opens next time on the colour chosen then.
        ui.data_mut(|d| d.remove::<(Color32, bool)>(id));
    }
    picked
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

/// Pull's mode and the stash, the original's "pullSheetRebase", "pullSheetStashAndReapply" and
/// "deleteStashAfterApply" (all off by default), and moving the branch's issue on the project.
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
    card(ui, |c| {
        let ctrl = c.row("Local changes", "Files grouped by folder, or one flat list", vec2(SEG_W, SEG_H), true);
        let modes = ["List", "Tree"];
        let picked = segmented(c.ui, ctrl, modes.len(), s.files_tree as usize, true, "files_tree", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, modes[i], theme::body(), fg);
        });
        if let Some(i) = picked {
            s.files_tree = i == 1;
        }
    });
    card(ui, |c| {
        let ctrl = c.row("Move the branch's issue without asking", "In progress at checkout, In review once its pull request exists", vec2(36.0, 20.0), true);
        if toggle(c.ui, ctrl, "issue_status_auto", s.issue_status_auto) {
            s.issue_status_auto = !s.issue_status_auto;
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
        let resp = ui.interact(r, ui.scope_id().with((id, i)), if on { Sense::click() } else { Sense::hover() });
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

/// A dialog-style button against the right of `rect`. Returns true when clicked.
fn button_at(ui: &mut Ui, rect: Rect, id: &str, text: &str, kind: crate::modal::Kind, enabled: bool) -> bool {
    let mut child = ui.new_child(UiBuilder::new().id_salt(id).max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    crate::modal::button(&mut child, text, kind, enabled).clicked()
}

/// `text` cut to `max` characters, with an ellipsis when it was longer.
fn ellipsize(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= max && !text.trim().contains('\n') {
        return line.to_string();
    }
    format!("{}…", line.chars().take(max.saturating_sub(1)).collect::<String>().trim_end())
}

/// "Checked just now", "Checked 5 min ago", "Checked 2 h ago", from seconds since then.
fn checked_ago(secs: i64) -> String {
    match secs.max(0) {
        s if s < 60 => "Checked just now".into(),
        s if s < 3600 => format!("Checked {} min ago", s / 60),
        s if s < 86400 => format!("Checked {} h ago", s / 3600),
        s => format!("Checked {} d ago", s / 86400),
    }
}

/// The update channel, the installed version and the last check, with "Check now".
fn updates(ui: &mut Ui, pane: &mut Pane<'_>) {
    use crate::modal::Kind;
    use crate::update::{commit, Version};
    let s = &mut *pane.settings;
    let u = pane.update;
    card(ui, |c| {
        let ctrl = c.row("Channel", "The releases Fourchette offers", vec2(CHANNEL_W, SEG_H), true);
        let channels = UpdateChannel::ALL;
        let chosen = channels.iter().position(|ch| *ch == s.update_channel).unwrap_or(1);
        if let Some(i) = segmented(c.ui, ctrl, channels.len(), chosen, true, "update_channel", |p, r, i, fg, _| {
            p.text(r.center(), Align2::CENTER_CENTER, channels[i].label(), theme::body(), fg);
        }) {
            s.update_channel = channels[i];
        }
    });
    let status = u.status();
    card(ui, |c| {
        let current = Version::current();
        let built = match commit() { Some(c) => format!("Built from {}", short(c)), None => "The version installed".to_string() };
        let can = u.can_check();
        let ctrl = c.row(&format!("Fourchette {current}"), &built, vec2(120.0, 30.0), true);
        if button_at(c.ui, ctrl, "update_check_now", "Check now", Kind::Plain, can && status != CheckStatus::Checking) {
            pane.cmds.push(Cmd::UpdateCheckNow);
        }
        let ago = u.checked_at.map(|t| checked_ago(now_secs() - t));
        let (title, help) = match &status {
            CheckStatus::Off => ("Not checked".to_string(), "Checks only run in the installed app".to_string()),
            CheckStatus::Never => ("Not checked yet".to_string(), "The first check runs at launch".to_string()),
            CheckStatus::Checking => ("Checking…".to_string(), format!("On the {} channel", s.update_channel.label())),
            CheckStatus::UpToDate => ("Up to date".to_string(), ago.unwrap_or_default()),
            CheckStatus::Available(v) if *v == current => (format!("A new build of {v} is available"), ago.unwrap_or_else(|| format!("On the {} channel", s.update_channel.label()))),
            CheckStatus::Available(v) => (format!("{v} is available"), ago.unwrap_or_else(|| format!("On the {} channel", s.update_channel.label()))),
            CheckStatus::Failed(e) => ("Couldn't check".to_string(), ellipsize(e, 52)),
        };
        let ctrl = c.row(&title, &help, vec2(140.0, 30.0), true);
        match &status {
            CheckStatus::Available(_) => {
                if button_at(c.ui, ctrl, "update_show", "Show Update", Kind::Primary, true) {
                    pane.cmds.push(Cmd::UpdateDialog(true));
                }
            }
            CheckStatus::Checking => {
                let mut child = c.ui.new_child(UiBuilder::new().id_salt("update_spinner").max_rect(ctrl).layout(Layout::right_to_left(Align::Center)));
                child.add_space(8.0);
                child.spinner();
            }
            CheckStatus::Failed(e) => {
                // The whole error on hover, over the row's text.
                let row = Rect::from_x_y_ranges(c.ui.min_rect().x_range(), ctrl.expand2(vec2(0.0, 13.0)).y_range());
                c.ui.interact(row, c.ui.scope_id().with("update_error"), Sense::hover()).on_hover_text(e.as_str());
            }
            _ => {}
        }
    });
    // Under the cards, lined up with the rows' names.
    let note = "Stable: only full releases. Pre-release: every published build.\nFourchette checks at launch, then every hour.";
    egui::Frame::new().inner_margin(Margin::symmetric(16, 0)).show(ui, |ui| {
        ui.label(egui::RichText::new(note).size(theme::FS_SMALL).color(theme::c().faint));
    });
}

impl App {
    pub(super) fn settings_ui(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        // The font menus write each font in its own style: those fonts are read on opening
        // and let go on closing, so they don't stay in memory.
        if self.font_previews != self.settings_open {
            self.font_previews = self.settings_open;
            let (ui_font, code_font) = fonts_from_env(&self.library.settings);
            theme::set_fonts(ctx, ui_font, code_font, self.font_previews);
        }
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

        let screen = ctx.content_rect();
        let min = pos2((screen.center().x - WIN_W * 0.5).round(), (screen.top() + 56.0).max(screen.center().y - WIN_H * 0.5).round());
        let t = crate::modal::appear(ctx, egui::Id::unique("settings"));
        egui::Area::new(egui::Id::unique("settings_backdrop")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            if resp.clicked() {
                close = true;
            }
        });
        egui::Area::new(egui::Id::unique("settings")).order(egui::Order::Foreground).fixed_pos(min).show(ctx, |ui| {
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
            let xr = ui.interact(x, ui.scope_id().with("settings_close"), Sense::click()).on_hover_text("Close (Esc)");
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
                let icon = if s.app_icon == self.library.settings.app_icon { self.icon.chosen(s.app_icon) } else { s.app_icon };
                let mut pane = Pane { appearance: self.library.appearance, icon, settings: &mut s, cmds, update: &self.update };
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
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("settings"));
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
        assert_eq!(SECTIONS.iter().map(|s| s.label).collect::<Vec<_>>(), ["Appearance", "Fetch", "Git", "Diff and commit", "Claude", "Updates"]);
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
        assert_eq!(section_index("updates"), 5);
        // FOURCHETTE_SETTINGS=1, or a section that does not exist yet, opens Appearance.
        for other in ["1", "", "fonts", "appearance"] {
            assert_eq!(section_index(other), 0, "{other}");
        }
    }

    #[test]
    fn errors_and_check_times_read_short() {
        assert_eq!(ellipsize("No network", 52), "No network");
        assert_eq!(ellipsize("abcdefghij", 5), "abcd…");
        assert_eq!(ellipsize("first line\nsecond", 52), "first line…");
        assert_eq!([0, 59, 60, 3599, 7200, 200_000, -5].map(checked_ago), ["Checked just now", "Checked just now", "Checked 1 min ago", "Checked 59 min ago", "Checked 2 h ago", "Checked 2 d ago", "Checked just now"]);
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
