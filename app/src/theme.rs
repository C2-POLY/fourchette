//! A calm, high-contrast look in the spirit of Linear: quiet surfaces, subtle borders,
//! one accent colour and a four-size type scale, in a dark and a light version.
//! The views read the colours of the mode on screen through `c()`.
use egui::{Color32, CornerRadius, FontId, Shadow, Stroke, TextStyle, Theme, ThemePreference, Visuals};
use std::sync::atomic::{AtomicBool, Ordering};

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
    // The blue/purple/green colours look best first, so the order is rotated (design choice).
    const ORDER: [usize; 9] = [5, 6, 4, 0, 8, 1, 2, 3, 7];
    let col = PALETTE[ORDER[id % PALETTE.len()]];
    // On a white background the bright colours (yellow above all) need to be a little deeper.
    if is_light() {
        shade(col, 0.2)
    } else {
        col
    }
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

/// The colours of one mode.
pub struct Colors {
    /// Sidebar, toolbar and status bar.
    pub sidebar: Color32,
    /// History, details and diff.
    pub bg: Color32,
    /// Popups, menus, dialogs and buttons.
    pub elevated: Color32,
    /// Text fields.
    pub input: Color32,
    pub hover: Color32,
    pub select: Color32,
    pub border: Color32,
    /// Border of fields and buttons, a step above the panel borders.
    pub border_strong: Color32,
    /// Buttons: hovered and pressed fill and border.
    pub widget_hover: Color32,
    pub widget_hover_border: Color32,
    pub widget_active: Color32,
    pub widget_active_border: Color32,
    /// Text: primary, strong (titles, selection), secondary (author, date, SHA), tertiary (hints).
    pub text: Color32,
    pub strong: Color32,
    pub dim: Color32,
    pub faint: Color32,
    /// The one accent: focus, selection, primary buttons, Claude.
    pub accent: Color32,
    /// The accent as text or a link.
    pub accent_text: Color32,
    pub accent_soft: Color32,
    /// Text on an accent or a status-coloured button.
    pub on_accent: Color32,
    pub added: Color32,
    pub modified: Color32,
    pub deleted: Color32,
    pub renamed: Color32,
    /// Text of added and removed lines in a diff.
    pub diff_added_text: Color32,
    pub diff_deleted_text: Color32,
    /// Darkness of shadows and of the backdrop behind the palette.
    pub shadow: u8,
}

pub const DARK: Colors = Colors {
    sidebar: Color32::from_rgb(0x0c, 0x0d, 0x10),
    bg: Color32::from_rgb(0x11, 0x12, 0x15),
    elevated: Color32::from_rgb(0x19, 0x1a, 0x1e),
    input: Color32::from_rgb(0x16, 0x17, 0x1b),
    hover: Color32::from_rgb(0x1a, 0x1b, 0x20),
    select: Color32::from_rgb(0x21, 0x23, 0x2b),
    border: Color32::from_rgb(0x22, 0x24, 0x29),
    border_strong: Color32::from_rgb(0x2d, 0x2f, 0x36),
    widget_hover: Color32::from_rgb(0x22, 0x24, 0x2a),
    widget_hover_border: Color32::from_rgb(0x3a, 0x3d, 0x46),
    widget_active: Color32::from_rgb(0x2a, 0x2c, 0x34),
    widget_active_border: Color32::from_rgb(0x44, 0x47, 0x51),
    text: Color32::from_rgb(0xe3, 0xe4, 0xe8),
    strong: Color32::from_rgb(0xf7, 0xf8, 0xf8),
    dim: Color32::from_rgb(0xa2, 0xa5, 0xae),
    faint: Color32::from_rgb(0x74, 0x78, 0x82),
    accent: Color32::from_rgb(0x5e, 0x6a, 0xd2),
    accent_text: Color32::from_rgb(0x95, 0x9d, 0xf0),
    accent_soft: Color32::from_rgb(0x26, 0x29, 0x45),
    on_accent: Color32::from_rgb(0xf7, 0xf8, 0xf8),
    added: Color32::from_rgb(0x4c, 0xb7, 0x82),
    modified: Color32::from_rgb(0xe2, 0xb3, 0x40),
    deleted: Color32::from_rgb(0xeb, 0x57, 0x57),
    renamed: Color32::from_rgb(0x8e, 0x8c, 0xf0),
    diff_added_text: Color32::from_rgb(0xc8, 0xf0, 0xd8),
    diff_deleted_text: Color32::from_rgb(0xf8, 0xd0, 0xd0),
    shadow: 140,
};

pub const LIGHT: Colors = Colors {
    sidebar: Color32::from_rgb(0xf5, 0xf5, 0xf7),
    bg: Color32::from_rgb(0xff, 0xff, 0xff),
    elevated: Color32::from_rgb(0xff, 0xff, 0xff),
    input: Color32::from_rgb(0xff, 0xff, 0xff),
    hover: Color32::from_rgb(0xee, 0xef, 0xf2),
    select: Color32::from_rgb(0xe6, 0xe8, 0xf0),
    border: Color32::from_rgb(0xe4, 0xe5, 0xe9),
    border_strong: Color32::from_rgb(0xd2, 0xd4, 0xda),
    widget_hover: Color32::from_rgb(0xf3, 0xf4, 0xf6),
    widget_hover_border: Color32::from_rgb(0xc3, 0xc6, 0xce),
    widget_active: Color32::from_rgb(0xe8, 0xe9, 0xee),
    widget_active_border: Color32::from_rgb(0xb5, 0xb8, 0xc1),
    text: Color32::from_rgb(0x23, 0x25, 0x2b),
    strong: Color32::from_rgb(0x0e, 0x0f, 0x12),
    dim: Color32::from_rgb(0x5b, 0x5f, 0x6a),
    faint: Color32::from_rgb(0x8b, 0x8f, 0x99),
    accent: Color32::from_rgb(0x5e, 0x6a, 0xd2),
    accent_text: Color32::from_rgb(0x4a, 0x55, 0xc2),
    accent_soft: Color32::from_rgb(0xec, 0xee, 0xfd),
    on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
    added: Color32::from_rgb(0x1f, 0x93, 0x5c),
    modified: Color32::from_rgb(0xb0, 0x7c, 0x0c),
    deleted: Color32::from_rgb(0xd0, 0x3b, 0x3b),
    renamed: Color32::from_rgb(0x5f, 0x5a, 0xd4),
    diff_added_text: Color32::from_rgb(0x15, 0x52, 0x33),
    diff_deleted_text: Color32::from_rgb(0x7f, 0x1d, 0x1d),
    shadow: 45,
};

static LIGHT_ON_SCREEN: AtomicBool = AtomicBool::new(false);

/// True while the window is drawn in the light mode.
pub fn is_light() -> bool {
    LIGHT_ON_SCREEN.load(Ordering::Relaxed)
}

/// The colours of the mode on screen.
pub fn c() -> &'static Colors {
    if is_light() {
        &LIGHT
    } else {
        &DARK
    }
}

/// Follows the mode egui draws this frame (it changes with macOS in System mode).
pub fn sync(ctx: &egui::Context) {
    LIGHT_ON_SCREEN.store(ctx.theme() == Theme::Light, Ordering::Relaxed);
}

/// The person's choice: follow macOS, or always dark or light.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Dark,
    Light,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Dark, Appearance::Light];

    /// How it is written in repos.json.
    pub fn key(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Dark => "dark",
            Appearance::Light => "light",
        }
    }

    pub fn parse(s: &str) -> Option<Appearance> {
        Appearance::ALL.into_iter().find(|a| a.key() == s.trim().to_lowercase())
    }

    pub fn label(self) -> &'static str {
        match self {
            Appearance::System => "System",
            Appearance::Dark => "Dark",
            Appearance::Light => "Light",
        }
    }

    /// The status bar button goes System → Dark → Light → System.
    pub fn next(self) -> Appearance {
        match self {
            Appearance::System => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
            Appearance::Light => Appearance::System,
        }
    }

    fn preference(self) -> ThemePreference {
        match self {
            Appearance::System => ThemePreference::System,
            Appearance::Dark => ThemePreference::Dark,
            Appearance::Light => ThemePreference::Light,
        }
    }
}

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

/// A colour mixed toward white.
pub fn lighten(c: Color32, k: f32) -> Color32 {
    let m = |v: u8| (v as f32 + (255.0 - v as f32) * k).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// A colour mixed toward black.
pub fn shade(c: Color32, k: f32) -> Color32 {
    let m = |v: u8| (v as f32 * (1.0 - k)).round() as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// Text drawn in a lane colour: lighter on the dark mode, deeper on the light one.
pub fn readable(c: Color32, k: f32) -> Color32 {
    if is_light() {
        shade(c, k)
    } else {
        lighten(c, k)
    }
}

/// A colour mixed into a background, opaque (so it does not depend on what is under it).
pub fn tint(c: Color32, over: Color32, k: f32) -> Color32 {
    let m = |a: u8, b: u8| (b as f32 + (a as f32 - b as f32) * k).round() as u8;
    Color32::from_rgb(m(c.r(), over.r()), m(c.g(), over.g()), m(c.b(), over.b()))
}

/// Shadow colour of the mode on screen.
pub fn shadow(alpha_scale: f32) -> Color32 {
    Color32::from_black_alpha((c().shadow as f32 * alpha_scale) as u8)
}

fn visuals(k: &Colors, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = k.bg;
    v.window_fill = k.elevated;
    v.extreme_bg_color = k.input;
    v.text_edit_bg_color = Some(k.input);
    v.code_bg_color = k.elevated;
    v.faint_bg_color = k.hover;
    v.override_text_color = Some(k.text);
    v.weak_text_color = Some(k.faint);
    v.selection.bg_fill = k.accent_soft;
    v.selection.stroke = Stroke::new(1.0_f32, k.accent);
    v.hyperlink_color = k.accent_text;
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0_f32, k.border_strong);
    v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(k.shadow) };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha((k.shadow as f32 * 0.85) as u8) };
    v.text_cursor.stroke = Stroke::new(1.5_f32, k.accent_text);

    let r = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = k.bg;
    w.noninteractive.weak_bg_fill = k.bg;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, k.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, k.text);
    w.noninteractive.corner_radius = r;
    for (st, fill, stroke, fg) in [
        (&mut w.inactive, k.elevated, k.border_strong, k.text),
        (&mut w.hovered, k.widget_hover, k.widget_hover_border, k.strong),
        (&mut w.active, k.widget_active, k.widget_active_border, k.strong),
        (&mut w.open, k.widget_hover, k.border_strong, k.strong),
    ] {
        st.bg_fill = fill;
        st.weak_bg_fill = fill;
        st.bg_stroke = Stroke::new(1.0_f32, stroke);
        st.fg_stroke = Stroke::new(1.5_f32, fg);
        st.corner_radius = r;
        st.expansion = 0.0;
    }
    v
}

/// Inter for the interface and JetBrains Mono for code, both built into the app
/// (licences beside them in `assets/fonts`). egui's own fonts stay behind them
/// for the glyphs they lack.
pub fn fonts() -> egui::FontDefinitions {
    use egui::{FontData, FontFamily};
    use std::sync::Arc;
    let mut f = egui::FontDefinitions::default();
    for (name, bytes, family) in [
        ("Inter", &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..], FontFamily::Proportional),
        ("JetBrains Mono", &include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..], FontFamily::Monospace),
    ] {
        f.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
        f.families.entry(family).or_default().insert(0, name.into());
    }
    f
}

/// Sets up both modes once, then shows the one the person chose.
pub fn apply(ctx: &egui::Context, appearance: Appearance) {
    ctx.set_fonts(fonts());
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, true));
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, false));
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
    set_appearance(ctx, appearance);
}

pub fn set_appearance(ctx: &egui::Context, appearance: Appearance) {
    ctx.set_theme(appearance.preference());
    sync(ctx);
}

pub fn status_color(code: char) -> Color32 {
    let k = c();
    match code {
        'A' | '?' => k.added,
        'D' => k.deleted,
        'R' | 'C' => k.renamed,
        'U' => k.deleted,
        _ => k.modified,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_round_trips_and_cycles() {
        for a in Appearance::ALL {
            assert_eq!(Appearance::parse(a.key()), Some(a));
        }
        assert_eq!(Appearance::parse(" Light "), Some(Appearance::Light));
        assert_eq!(Appearance::parse("sepia"), None);
        assert_eq!(Appearance::default(), Appearance::System);
        assert_eq!(Appearance::System.next().next().next(), Appearance::System);
        assert_eq!(Appearance::System.next(), Appearance::Dark);
    }

    #[test]
    fn shade_and_lighten_move_toward_black_and_white() {
        let c = Color32::from_rgb(100, 200, 50);
        assert_eq!(shade(c, 1.0), Color32::from_rgb(0, 0, 0));
        assert_eq!(lighten(c, 1.0), Color32::from_rgb(255, 255, 255));
        assert_eq!(shade(c, 0.0), c);
    }

    #[test]
    fn fonts_put_ours_first_and_keep_egui_behind() {
        use egui::FontFamily;
        let f = fonts();
        let default = egui::FontDefinitions::default();
        for (family, ours) in [(FontFamily::Proportional, "Inter"), (FontFamily::Monospace, "JetBrains Mono")] {
            let names = &f.families[&family];
            assert_eq!(names[0], ours);
            assert_eq!(&names[1..], &default.families[&family][..]);
            assert!(f.font_data.contains_key(ours));
        }
    }
}
