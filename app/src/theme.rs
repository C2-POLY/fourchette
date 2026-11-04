//! A calm, high-contrast dark look in the spirit of Linear: near-black surfaces,
//! subtle borders, one accent colour and a four-size type scale.
use egui::{Color32, CornerRadius, FontId, Shadow, Stroke, TextStyle, Theme, Visuals};

// Branch palette: the original program's 9 colours (see MECHANICS.md, sub_1001e0440).
pub const PALETTE: [Color32; 9] = [
    Color32::from_rgb(0xFF, 0x95, 0x02),
    Color32::from_rgb(0xFF, 0xCC, 0x00),
    Color32::from_rgb(0xFF, 0x3B, 0x30),
    Color32::from_rgb(0xA2, 0x84, 0x5E),
    Color32::from_rgb(0x64, 0xDA, 0x38),
    Color32::from_rgb(0x1C, 0xAD, 0xF8),
    Color32::from_rgb(0xCB, 0x73, 0xE1),
    Color32::from_rgb(0x8E, 0x8E, 0x91),
    Color32::from_rgb(0xFF, 0x29, 0x68),
];

pub fn lane_color(id: usize) -> Color32 {
    // Same rule as the original: colour id modulo the palette size.
    // The blue/purple/green colours look best first on a dark background,
    // so the order is rotated (design choice).
    const ORDER: [usize; 9] = [5, 6, 4, 0, 8, 1, 2, 3, 7];
    PALETTE[ORDER[id % PALETTE.len()]]
}

// New-look graph numbers (mechanics.json "new look" group).
pub const LANE_W: f32 = 16.0;
pub const LINE_W: f32 = 2.0;
pub const NODE_R: f32 = 5.0;
pub const MERGE_R: f32 = 3.5;
pub const ROW_H: f32 = 28.0;
pub const GRAPH_INSET: f32 = 3.0 + LANE_W / 2.0;
/// Gap between branch labels: 2 when "compactBranchLabels" is on, which is its default
/// (sub_100105e34 picks 2 or 4; default in sub_10031b7cc).
pub const REF_GAP: f32 = 2.0;

// Surfaces, darkest to lightest: a calm near-black look with few, subtle borders.
pub const SIDEBAR: Color32 = Color32::from_rgb(0x0c, 0x0d, 0x10);
pub const TOOLBAR: Color32 = SIDEBAR;
pub const BG: Color32 = Color32::from_rgb(0x11, 0x12, 0x15);
pub const PANEL: Color32 = BG;
/// Popups, menus, dialogs, text fields and buttons.
pub const ELEVATED: Color32 = Color32::from_rgb(0x19, 0x1a, 0x1e);
pub const INPUT: Color32 = Color32::from_rgb(0x16, 0x17, 0x1b);
pub const HOVER: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x20);
pub const SELECT: Color32 = Color32::from_rgb(0x21, 0x23, 0x2b);
pub const BORDER: Color32 = Color32::from_rgb(0x22, 0x24, 0x29);
/// Border of fields and buttons, a step above the panel borders.
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x2d, 0x2f, 0x36);

// Text: primary, strong (titles, selection), secondary (author, date, SHA), tertiary (hints).
pub const TEXT: Color32 = Color32::from_rgb(0xe3, 0xe4, 0xe8);
pub const STRONG: Color32 = Color32::from_rgb(0xf7, 0xf8, 0xf8);
pub const DIM: Color32 = Color32::from_rgb(0xa2, 0xa5, 0xae);
pub const FAINT: Color32 = Color32::from_rgb(0x74, 0x78, 0x82);

/// The one accent: focus, selection, primary buttons, Claude.
pub const ACCENT: Color32 = Color32::from_rgb(0x5e, 0x6a, 0xd2);
/// The accent as text or a link on a dark surface.
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x95, 0x9d, 0xf0);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0x26, 0x29, 0x45);

pub const ADDED: Color32 = Color32::from_rgb(0x4c, 0xb7, 0x82);
pub const MODIFIED: Color32 = Color32::from_rgb(0xe2, 0xb3, 0x40);
pub const DELETED: Color32 = Color32::from_rgb(0xeb, 0x57, 0x57);
pub const RENAMED: Color32 = Color32::from_rgb(0x8e, 0x8c, 0xf0);

// Type scale: four sizes, used everywhere.
/// Section headers, column headers, badges, captions.
pub const FS_SMALL: f32 = 11.0;
/// Secondary text: author, date, hints, status bar.
pub const FS_SECONDARY: f32 = 12.0;
/// Body: rows, buttons, fields.
pub const FS_BODY: f32 = 13.0;
/// Titles: details, dialogs, palette query.
pub const FS_TITLE: f32 = 15.0;
pub const FS_MONO: f32 = 12.0;

pub fn small() -> FontId {
    FontId::proportional(FS_SMALL)
}
pub fn secondary() -> FontId {
    FontId::proportional(FS_SECONDARY)
}
pub fn body() -> FontId {
    FontId::proportional(FS_BODY)
}
pub fn mono() -> FontId {
    FontId::monospace(FS_MONO)
}

/// A colour mixed toward white, for text drawn in a lane colour.
pub fn lighten(c: Color32, k: f32) -> Color32 {
    let m = |v: u8| (v as f32 + (255.0 - v as f32) * k).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// A colour mixed into a background, opaque (so it does not depend on what is under it).
pub fn tint(c: Color32, over: Color32, k: f32) -> Color32 {
    let m = |a: u8, b: u8| (b as f32 + (a as f32 - b as f32) * k).round() as u8;
    Color32::from_rgb(m(c.r(), over.r()), m(c.g(), over.g()), m(c.b(), over.b()))
}

pub fn apply(ctx: &egui::Context) {
    // Always dark, whatever the macOS appearance: the colours below are only for the dark theme.
    ctx.set_theme(Theme::Dark);
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = ELEVATED;
    v.extreme_bg_color = INPUT;
    v.text_edit_bg_color = Some(INPUT);
    v.code_bg_color = ELEVATED;
    v.faint_bg_color = HOVER;
    v.override_text_color = Some(TEXT);
    v.weak_text_color = Some(FAINT);
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT_TEXT;
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
    v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(140) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(120) };
    v.text_cursor.stroke = Stroke::new(1.5_f32, ACCENT_TEXT);

    let r = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.noninteractive.corner_radius = r;
    for (st, fill, stroke, fg) in [
        (&mut w.inactive, ELEVATED, BORDER_STRONG, TEXT),
        (&mut w.hovered, Color32::from_rgb(0x22, 0x24, 0x2a), Color32::from_rgb(0x3a, 0x3d, 0x46), STRONG),
        (&mut w.active, Color32::from_rgb(0x2a, 0x2c, 0x34), Color32::from_rgb(0x44, 0x47, 0x51), STRONG),
        (&mut w.open, Color32::from_rgb(0x22, 0x24, 0x2a), BORDER_STRONG, STRONG),
    ] {
        st.bg_fill = fill;
        st.weak_bg_fill = fill;
        st.bg_stroke = Stroke::new(1.0_f32, stroke);
        st.fg_stroke = Stroke::new(1.5_f32, fg);
        st.corner_radius = r;
        st.expansion = 0.0;
    }
    ctx.set_visuals_of(Theme::Dark, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 4.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.menu_margin = egui::Margin::same(6);
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(FS_SMALL)),
            (TextStyle::Body, FontId::proportional(FS_BODY)),
            (TextStyle::Button, FontId::proportional(FS_BODY)),
            (TextStyle::Heading, FontId::proportional(FS_TITLE)),
            (TextStyle::Monospace, FontId::monospace(FS_MONO)),
        ]
        .into();
    });
}

pub fn status_color(code: char) -> Color32 {
    match code {
        'A' | '?' => ADDED,
        'D' => DELETED,
        'R' | 'C' => RENAMED,
        'U' => DELETED,
        _ => MODIFIED,
    }
}

/// A stable colour for an author's avatar circle.
pub fn author_color(name: &str) -> Color32 {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    lane_color((h % 9) as usize)
}

pub fn initials(name: &str) -> String {
    let mut out = String::new();
    for w in name.split_whitespace().take(2) {
        if let Some(c) = w.chars().next() {
            out.extend(c.to_uppercase());
        }
    }
    if out.is_empty() {
        out.push('?');
    }
    out
}
