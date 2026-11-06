//! A calm, high-contrast look in the spirit of Linear: quiet surfaces, subtle borders,
//! one accent colour and a four-size type scale, in a dark and a light version.
//! The views read the colours of the mode on screen through `c()`.
use egui::{Color32, CornerRadius, FontId, Shadow, Stroke, TextStyle, Theme, ThemePreference, Visuals};
use crate::settings::{CodeFont, UiFont};
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
    /// Darkness of the shadows under floating windows.
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
    colors(is_light())
}

/// The colours of the dark or the light mode.
pub fn colors(light: bool) -> &'static Colors {
    if light {
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

/// The family of the names in the history's branch and tag pills: the interface font in its
/// Medium weight (`fonts_for`).
pub const MEDIUM: &str = "medium";

/// A branch or tag name in its pill.
pub fn pill() -> FontId {
    FontId::new(FS_SMALL, egui::FontFamily::Name(MEDIUM.into()))
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
    // Windows and menus share the dialogs' frame (crate::modal): rounder corners, a soft shadow.
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0_f32, k.border_strong);
    v.window_shadow = Shadow { offset: [0, 24], blur: 64, spread: 0, color: Color32::from_black_alpha(k.shadow) };
    v.popup_shadow = Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha((k.shadow as f32 * 0.8) as u8) };
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

/// The font definitions for the chosen fonts. Inter and JetBrains Mono are built into the app
/// (licences beside them in `assets/fonts`) and always stay behind the chosen font, then egui's
/// own fonts, for the glyphs they lack. A system font is read with `read` (its path in, bytes
/// out; face 0 for a .ttc) and put first; when it is missing or not a font, the built-in one stays first.
/// The `MEDIUM` family is the interface font's Medium face in front of the interface fonts:
/// Inter's is built in, Helvetica Neue's and Avenir Next's are faces of their system file, and
/// San Francisco, one variable font whose other weights egui cannot load, stays Regular.
/// With `previews`, every font the Settings menus offer also gets its own family
/// (`preview_family`), so each name can be written in its own font.
pub fn fonts_for(ui: UiFont, code: CodeFont, previews: bool, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> egui::FontDefinitions {
    use egui::{FontData, FontFamily};
    use std::sync::Arc;
    let mut f = egui::FontDefinitions::default();
    let chosen = [
        (FontFamily::Proportional, ui.label(), ui.path()),
        (FontFamily::Monospace, code.label(), code.path()),
    ];
    for (name, bytes, family) in [
        ("Inter", &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..], FontFamily::Proportional),
        ("JetBrains Mono", &include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..], FontFamily::Monospace),
    ] {
        f.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
        f.families.entry(family).or_default().insert(0, name.into());
    }
    // Inter Medium, unless a system interface font is read below.
    let mut medium = Some(("Inter Medium".to_string(), FontData::from_static(include_bytes!("../assets/fonts/Inter-Medium.ttf"))));
    for (family, name, path) in chosen {
        let Some(bytes) = path.and_then(read).filter(|b| looks_like_a_font(b)) else { continue };
        if family == FontFamily::Proportional {
            medium = styled_face(&bytes, "Medium").map(|index| (format!("{name} Medium"), FontData { index, ..FontData::from_owned(bytes.clone()) }));
        }
        let index = regular_face(&bytes);
        f.font_data.insert(name.into(), Arc::new(FontData { index, ..FontData::from_owned(bytes) }));
        f.families.entry(family).or_default().insert(0, name.into());
    }
    let mut names = f.families[&FontFamily::Proportional].clone();
    if let Some((name, data)) = medium {
        f.font_data.insert(name.clone(), Arc::new(data));
        names.insert(0, name);
    }
    f.families.insert(FontFamily::Name(MEDIUM.into()), names);
    if previews {
        let ui_fonts = UiFont::ALL.map(|u| (u.label(), u.path(), FontFamily::Proportional, "Inter"));
        let code_fonts = CodeFont::ALL.map(|c| (c.label(), c.path(), FontFamily::Monospace, "JetBrains Mono"));
        for (name, path, base, built_in) in ui_fonts.into_iter().chain(code_fonts) {
            // The font itself, then its family's built-in and egui fonts for the glyphs it lacks.
            let mut names = egui::FontDefinitions::default().families[&base].clone();
            names.insert(0, built_in.into());
            if !f.font_data.contains_key(name) {
                if let Some(bytes) = path.and_then(read).filter(|b| looks_like_a_font(b)) {
                    let index = regular_face(&bytes);
                    f.font_data.insert(name.into(), Arc::new(FontData { index, ..FontData::from_owned(bytes) }));
                }
            }
            if name != built_in && f.font_data.contains_key(name) {
                names.insert(0, name.into());
            }
            f.families.insert(preview_family(name), names);
        }
    }
    f
}

/// The family that writes `label`, a font of the Settings menus, in its own font. It only
/// exists while Settings is open (`fonts_for` with `previews`).
pub fn preview_family(label: &str) -> egui::FontFamily {
    egui::FontFamily::Name(format!("preview {label}").into())
}

/// Reads a system font file; None when it is missing or unreadable.
pub fn read_system_font(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// The first bytes of a TrueType, OpenType or collection file, so that a damaged file is left
/// out instead of reaching the font parser.
fn looks_like_a_font(b: &[u8]) -> bool {
    matches!(b.get(..4), Some([0, 1, 0, 0] | b"OTTO" | b"ttcf" | b"true"))
}

/// The face of a collection (.ttc) whose style is "Regular", or 0: faces are not in the same
/// order in every file (Avenir Next's first face is its Bold). A single font is face 0.
fn regular_face(b: &[u8]) -> u32 {
    styled_face(b, "Regular").unwrap_or(0)
}

/// The face of a collection (.ttc) whose style is `want`; None for a single font or without one.
fn styled_face(b: &[u8], want: &str) -> Option<u32> {
    let u16_at = |o: usize| b.get(o..o + 2).map(|x| u16::from_be_bytes([x[0], x[1]]) as usize);
    let u32_at = |o: usize| b.get(o..o + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]) as usize);
    if b.get(..4) != Some(b"ttcf") {
        return None;
    }
    // The style of the font whose table directory starts at `o`: its typographic style (name id
    // 17) when it has one, else its legacy style (id 2), which calls Avenir Next's Demi Bold "Regular".
    let style = |o: usize| -> Option<String> {
        let tables = u16_at(o + 4)?;
        let name = (0..tables).map(|t| o + 12 + 16 * t).find(|&e| b.get(e..e + 4) == Some(b"name"))?;
        let table = u32_at(name + 8)?;
        let (count, strings) = (u16_at(table + 2)?, table + u16_at(table + 4)?);
        let named = |want: usize| {
            (0..count).map(|r| table + 6 + 12 * r).find_map(|rec| {
                let (platform, id, len, off) = (u16_at(rec)?, u16_at(rec + 6)?, u16_at(rec + 8)?, u16_at(rec + 10)?);
                if id != want {
                    return None;
                }
                let raw = b.get(strings + off..strings + off + len)?;
                Some(if platform == 0 || platform == 3 {
                    String::from_utf16_lossy(&raw.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect::<Vec<_>>())
                } else {
                    String::from_utf8_lossy(raw).into_owned()
                })
            })
        };
        named(17).or_else(|| named(2))
    };
    let faces = u32_at(8).unwrap_or(0).min(64);
    (0..faces).find(|&i| u32_at(12 + 4 * i).and_then(style).as_deref() == Some(want)).map(|i| i as u32)
}

/// Shows the chosen fonts at once (the system ones are read now).
/// With `previews`, also every font the Settings menus offer: read only while Settings is open.
pub fn set_fonts(ctx: &egui::Context, ui: UiFont, code: CodeFont, previews: bool) {
    ctx.set_fonts(fonts_for(ui, code, previews, &read_system_font));
}

/// Sets up both modes once, then shows the mode the person chose.
pub fn apply(ctx: &egui::Context, appearance: Appearance) {
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, true));
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, false));
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 4.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.menu_margin = egui::Margin::same(6);
        // A panel's edge is grabbed 5 points either side, as before egui 0.37 brought it to 3.
        s.interaction.resize_grab_radius_side = 5.0;
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

    /// WCAG contrast ratio of two colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn both_modes_are_readable() {
        for light in [false, true] {
            let k = colors(light);
            let what = if light { "light" } else { "dark" };
            for (name, bg) in [("bg", k.bg), ("sidebar", k.sidebar), ("select", k.select)] {
                assert!(contrast(k.text, bg) >= 7.0, "{what}: text on {name} {:.1}", contrast(k.text, bg));
                assert!(contrast(k.dim, bg) >= 4.5, "{what}: dim on {name} {:.1}", contrast(k.dim, bg));
            }
            assert!(contrast(k.accent_text, k.bg) >= 4.5, "{what}: accent text {:.1}", contrast(k.accent_text, k.bg));
            assert!(contrast(k.on_accent, k.accent) >= 3.0, "{what}: button text {:.1}", contrast(k.on_accent, k.accent));
            for (name, col) in [("added", k.added), ("modified", k.modified), ("deleted", k.deleted)] {
                assert!(contrast(col, k.bg) >= 3.0, "{what}: {name} {:.1}", contrast(col, k.bg));
            }
        }
    }

    #[test]
    fn shade_and_lighten_move_toward_black_and_white() {
        let c = Color32::from_rgb(100, 200, 50);
        assert_eq!(shade(c, 1.0), Color32::from_rgb(0, 0, 0));
        assert_eq!(lighten(c, 1.0), Color32::from_rgb(255, 255, 255));
        assert_eq!(shade(c, 0.0), c);
    }

    #[test]
    fn a_collection_gives_its_regular_face() {
        // A single font, or bytes that are not a collection, give face 0.
        assert_eq!(regular_face(include_bytes!("../assets/fonts/Inter-Regular.ttf")), 0);
        assert_eq!(regular_face(b"ttcf"), 0);
        assert_eq!(regular_face(b""), 0);
        // On this Mac's own files: Avenir Next's Regular is not its first face.
        for (path, face) in [("/System/Library/Fonts/Avenir Next.ttc", 7), ("/System/Library/Fonts/HelveticaNeue.ttc", 0), ("/System/Library/Fonts/Menlo.ttc", 0)] {
            if let Some(b) = read_system_font(path) {
                assert_eq!(regular_face(&b), face, "{path}");
            }
        }
    }

    #[test]
    fn a_collection_gives_its_medium_face_when_it_has_one() {
        // A single font has no other face: San Francisco's file, and Inter's.
        assert_eq!(styled_face(include_bytes!("../assets/fonts/Inter-Regular.ttf"), "Medium"), None);
        for (path, face) in [("/System/Library/Fonts/Avenir Next.ttc", Some(5)), ("/System/Library/Fonts/HelveticaNeue.ttc", Some(10)), ("/System/Library/Fonts/SFNS.ttf", None)] {
            if let Some(b) = read_system_font(path) {
                assert_eq!(styled_face(&b, "Medium"), face, "{path}");
            }
        }
    }

    #[test]
    fn the_pill_family_is_the_interface_font_in_medium() {
        use egui::FontFamily;
        let medium = |f: &egui::FontDefinitions| f.families[&FontFamily::Name(MEDIUM.into())].clone();
        // Inter: its Medium, then the interface fonts.
        let f = fonts_for(UiFont::Inter, CodeFont::JetBrainsMono, false, &no_files);
        assert_eq!(medium(&f)[0], "Inter Medium");
        assert_eq!(&medium(&f)[1..], &f.families[&FontFamily::Proportional][..]);
        // A system font without a Medium face (here Inter's bytes, a single font) stays Regular.
        let f = fonts_for(UiFont::SanFrancisco, CodeFont::JetBrainsMono, false, &fake_file);
        assert_eq!(medium(&f), f.families[&FontFamily::Proportional]);
        assert_eq!(medium(&f)[0], "San Francisco");
        // A system font that can't be read falls back to Inter, in Medium.
        let f = fonts_for(UiFont::AvenirNext, CodeFont::JetBrainsMono, false, &no_files);
        assert_eq!(medium(&f)[..2], ["Inter Medium", "Inter"]);
        // On this Mac: Avenir Next's Medium face.
        if read_system_font("/System/Library/Fonts/Avenir Next.ttc").is_some() {
            let f = fonts_for(UiFont::AvenirNext, CodeFont::JetBrainsMono, false, &read_system_font);
            assert_eq!(medium(&f)[..2], ["Avenir Next Medium", "Avenir Next"]);
            assert_eq!(f.font_data["Avenir Next Medium"].index, 5);
        }
    }

    fn no_files(_: &str) -> Option<Vec<u8>> {
        None
    }

    /// A stand-in for a system file: a real font (Inter's bytes).
    fn fake_file(_: &str) -> Option<Vec<u8>> {
        Some(include_bytes!("../assets/fonts/Inter-Regular.ttf").to_vec())
    }

    #[test]
    fn built_in_fonts_come_first_by_default_with_egui_behind() {
        use egui::FontFamily;
        let f = fonts_for(UiFont::Inter, CodeFont::JetBrainsMono, false, &no_files);
        let default = egui::FontDefinitions::default();
        for (family, ours) in [(FontFamily::Proportional, "Inter"), (FontFamily::Monospace, "JetBrains Mono")] {
            let names = &f.families[&family];
            assert_eq!(names[0], ours);
            assert_eq!(&names[1..], &default.families[&family][..]);
            assert!(f.font_data.contains_key(ours));
        }
    }

    #[test]
    fn a_chosen_system_font_goes_first_with_the_built_in_and_egui_behind() {
        use egui::FontFamily;
        let default = egui::FontDefinitions::default();
        for (u, c) in UiFont::ALL[1..].iter().zip(&CodeFont::ALL[1..]) {
            let f = fonts_for(*u, *c, false, &fake_file);
            for (family, chosen, built_in) in [(FontFamily::Proportional, u.label(), "Inter"), (FontFamily::Monospace, c.label(), "JetBrains Mono")] {
                let names = &f.families[&family];
                assert_eq!(names[0], chosen);
                assert_eq!(names[1], built_in);
                assert_eq!(&names[2..], &default.families[&family][..]);
                assert!(f.font_data.contains_key(chosen) && f.font_data.contains_key(built_in));
                assert_eq!(f.font_data[chosen].index, 0, "face 0 of a collection");
            }
        }
        // Only the font chosen is read: with the built-in interface font, the system one is not asked for.
        let asked = std::cell::RefCell::new(vec![]);
        fonts_for(UiFont::Inter, CodeFont::Menlo, false, &|p| {
            asked.borrow_mut().push(p.to_string());
            None
        });
        assert_eq!(*asked.borrow(), ["/System/Library/Fonts/Menlo.ttc"]);
    }

    #[test]
    fn a_missing_or_damaged_font_falls_back_to_the_built_in_one() {
        use egui::FontFamily;
        let damaged = |_: &str| Some(b"not a font".to_vec());
        let empty = |_: &str| Some(vec![]);
        let missing = |p: &str| read_system_font(&format!("{p}.does-not-exist"));
        let readers: [&dyn Fn(&str) -> Option<Vec<u8>>; 4] = [&no_files, &damaged, &empty, &missing];
        for read in readers {
            let f = fonts_for(UiFont::AvenirNext, CodeFont::Monaco, false, read);
            assert_eq!(f.families[&FontFamily::Proportional][0], "Inter");
            assert_eq!(f.families[&FontFamily::Monospace][0], "JetBrains Mono");
            assert!(!f.font_data.contains_key("Avenir Next") && !f.font_data.contains_key("Monaco"));
        }
        assert_eq!(read_system_font("/nonexistent/Nope.ttf"), None);
    }

    #[test]
    fn previews_give_each_offered_font_its_own_family() {
        use egui::FontFamily;
        // Without previews, no preview family: the menus stay in the interface font.
        let f = fonts_for(UiFont::Inter, CodeFont::JetBrainsMono, false, &fake_file);
        assert!(!f.families.contains_key(&preview_family("Menlo")));
        let read = std::cell::RefCell::new(vec![]);
        let f = fonts_for(UiFont::SanFrancisco, CodeFont::JetBrainsMono, true, &|p| {
            read.borrow_mut().push(p.to_string());
            fake_file(p)
        });
        let default = egui::FontDefinitions::default();
        let ui_fonts = UiFont::ALL.map(|u| (u.label(), FontFamily::Proportional, "Inter"));
        let code_fonts = CodeFont::ALL.map(|c| (c.label(), FontFamily::Monospace, "JetBrains Mono"));
        for (label, base, built_in) in ui_fonts.into_iter().chain(code_fonts) {
            let names = &f.families[&preview_family(label)];
            assert_eq!(names[0], label);
            let rest = if label == built_in {
                &names[1..]
            } else {
                assert_eq!(names[1], built_in);
                &names[2..]
            };
            assert_eq!(rest, &default.families[&base][..]);
        }
        // Each system font is read once, the chosen one included.
        assert_eq!(read.borrow().len(), 6);
        // A missing system font shows in the built-in one.
        let f = fonts_for(UiFont::Inter, CodeFont::JetBrainsMono, true, &no_files);
        assert_eq!(f.families[&preview_family("Avenir Next")][0], "Inter");
        assert_eq!(f.families[&preview_family("Monaco")][0], "JetBrains Mono");
    }

    #[test]
    fn the_system_fonts_of_this_mac_are_real_fonts() {
        // Skipped off macOS; on a Mac a font that is there must pass the check, or the choice would never show.
        for p in UiFont::ALL.iter().filter_map(|f| f.path()).chain(CodeFont::ALL.iter().filter_map(|f| f.path())) {
            if let Some(b) = read_system_font(p) {
                assert!(looks_like_a_font(&b), "{p}");
            }
        }
    }
}
