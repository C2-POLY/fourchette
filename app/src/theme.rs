//! Dark, colourful look in the spirit of the person's request.
use egui::{Color32, CornerRadius, Stroke, Visuals};

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
pub const LANE_W: f32 = 20.0;
pub const LINE_W: f32 = 2.5;
pub const NODE_R: f32 = 9.0;
pub const MERGE_R: f32 = 4.5;
pub const ROW_H: f32 = 28.0;
pub const GRAPH_INSET: f32 = 3.0 + LANE_W / 2.0;
/// Gap between branch labels: 2 when "compactBranchLabels" is on, which is its default
/// (sub_100105e34 picks 2 or 4; default in sub_10031b7cc).
pub const REF_GAP: f32 = 2.0;

pub const BG: Color32 = Color32::from_rgb(0x15, 0x17, 0x1f);
pub const PANEL: Color32 = Color32::from_rgb(0x1b, 0x1e, 0x28);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x18, 0x1b, 0x24);
pub const TOOLBAR: Color32 = Color32::from_rgb(0x20, 0x24, 0x31);
pub const BORDER: Color32 = Color32::from_rgb(0x2b, 0x30, 0x3f);
pub const HOVER: Color32 = Color32::from_rgb(0x23, 0x28, 0x36);
pub const SELECT: Color32 = Color32::from_rgb(0x2a, 0x35, 0x50);
pub const TEXT: Color32 = Color32::from_rgb(0xdc, 0xdf, 0xe6);
pub const DIM: Color32 = Color32::from_rgb(0x8b, 0x92, 0xa5);
pub const FAINT: Color32 = Color32::from_rgb(0x5c, 0x63, 0x77);
pub const ACCENT: Color32 = Color32::from_rgb(0x1C, 0xAD, 0xF8);
pub const ADDED: Color32 = Color32::from_rgb(0x5f, 0xd0, 0x6a);
pub const MODIFIED: Color32 = Color32::from_rgb(0xf0, 0xb2, 0x3c);
pub const DELETED: Color32 = Color32::from_rgb(0xff, 0x5b, 0x5b);
pub const RENAMED: Color32 = Color32::from_rgb(0x9a, 0x8c, 0xff);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = Color32::from_rgb(0x12, 0x14, 0x1b);
    v.faint_bg_color = HOVER;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = SELECT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x2a, 0x2f, 0x3e);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x2a, 0x2f, 0x3e);
    v.widgets.inactive.corner_radius = CornerRadius::same(6);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x34, 0x3a, 0x4c);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x34, 0x3a, 0x4c);
    v.widgets.hovered.corner_radius = CornerRadius::same(6);
    v.widgets.active.bg_fill = Color32::from_rgb(0x3b, 0x44, 0x5c);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x3b, 0x44, 0x5c);
    v.widgets.active.corner_radius = CornerRadius::same(6);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
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
