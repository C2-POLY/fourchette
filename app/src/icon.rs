//! The app icon (Dock, Cmd+Tab and Finder): a white Git branch on a gradient, in the macOS icon
//! shape. The default teal one is drawn in `assets/icon.svg`; `scripts/app-icon.sh` renders it
//! into `assets/icon.png`, the `assets/icon.icns` of the .app bundle and the site's icons.
//!
//! Settings › Appearance › App icon picks another colour (#159): `render` draws the same icon
//! from it in Rust, with the gradient `gradient` derives from that one colour. The Dock takes it
//! through the viewport, the Finder through `set_finder_icon` (only from an .app bundle).

/// The colour of the app icon: a preset of the Settings row, or a free colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IconColour {
    /// The shipped icon, `assets/icon.png`.
    #[default]
    Teal,
    Blue,
    Indigo,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Graphite,
    /// A colour chosen in the picker, as sRGB.
    Custom([u8; 3]),
}

impl IconColour {
    /// The presets, in the order of the Settings row.
    pub const PRESETS: [IconColour; 10] = [
        IconColour::Teal,
        IconColour::Blue,
        IconColour::Indigo,
        IconColour::Purple,
        IconColour::Pink,
        IconColour::Red,
        IconColour::Orange,
        IconColour::Yellow,
        IconColour::Green,
        IconColour::Graphite,
    ];

    /// Its name in repos.json and in `FOURCHETTE_ICON`: a preset's name, or `#rrggbb`.
    pub fn key(self) -> String {
        match self {
            IconColour::Custom([r, g, b]) => format!("#{r:02x}{g:02x}{b:02x}"),
            p => p.label().to_lowercase(),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            IconColour::Teal => "Teal",
            IconColour::Blue => "Blue",
            IconColour::Indigo => "Indigo",
            IconColour::Purple => "Purple",
            IconColour::Pink => "Pink",
            IconColour::Red => "Red",
            IconColour::Orange => "Orange",
            IconColour::Yellow => "Yellow",
            IconColour::Green => "Green",
            IconColour::Graphite => "Graphite",
            IconColour::Custom(_) => "Custom",
        }
    }

    /// A preset's name (any case) or `#rrggbb` / `rrggbb`.
    pub fn parse(s: &str) -> Option<IconColour> {
        let s = s.trim().to_lowercase();
        if let Some(p) = IconColour::PRESETS.into_iter().find(|p| p.key() == s) {
            return Some(p);
        }
        let hex = s.strip_prefix('#').unwrap_or(&s);
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(IconColour::Custom([byte(0)?, byte(2)?, byte(4)?]))
    }

    /// The one colour the gradient is derived from, which its swatch shows. Teal's gives back the
    /// stops of `assets/icon.svg` (#3fd9d0 → #0f8fa8) within a few units.
    pub fn base(self) -> [u8; 3] {
        match self {
            IconColour::Teal => [0x09, 0xb4, 0xb9],
            IconColour::Blue => [0x2f, 0x7c, 0xf6],
            IconColour::Indigo => [0x58, 0x56, 0xd6],
            IconColour::Purple => [0xa0, 0x50, 0xdc],
            IconColour::Pink => [0xee, 0x4f, 0x8d],
            IconColour::Red => [0xe5, 0x48, 0x4d],
            IconColour::Orange => [0xf2, 0x86, 0x2a],
            IconColour::Yellow => [0xe8, 0xb3, 0x11],
            IconColour::Green => [0x30, 0xa8, 0x5a],
            IconColour::Graphite => [0x7a, 0x7f, 0x87],
            IconColour::Custom(c) => c,
        }
    }
}

// ---------------------------------------------------------------- colour

fn to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let s = if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// sRGB to OKLCh (lightness 0..1, chroma, hue in degrees).
fn oklch(c: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = c.map(to_linear);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    (lightness, a.hypot(bb), bb.atan2(a).to_degrees().rem_euclid(360.0))
}

/// OKLCh to linear sRGB, possibly out of gamut.
fn oklch_linear(l: f32, c: f32, h: f32) -> [f32; 3] {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_35 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ]
}

/// OKLCh to sRGB, with the chroma lowered until the colour fits (the hue and lightness stay).
fn from_oklch(l: f32, c: f32, h: f32) -> [u8; 3] {
    let fits = |c: f32| oklch_linear(l, c, h).iter().all(|x| (-1e-4..=1.0 + 1e-4).contains(x));
    let (mut lo, mut hi) = (0.0, c);
    if fits(c) {
        lo = c;
    } else {
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    oklch_linear(l, lo, h).map(to_srgb)
}

/// The top and bottom stops of the icon's gradient, from one colour: in OKLCh, 0.10 lighter at
/// the top and 0.10 darker at the bottom, around a lightness kept between 0.40 and 0.80 so the
/// white branch always reads. The hue turns a little along the way, as in the teal icon: towards
/// blue for cool colours, towards red for warm ones (towards green, a yellow would turn olive).
pub fn gradient(base: [u8; 3]) -> ([u8; 3], [u8; 3]) {
    let (l, c, h) = oklch(base);
    let l = l.clamp(0.40, 0.80);
    let shift = if (20.0..130.0).contains(&h) { -8.0 } else { 10.0 };
    (from_oklch(l + 0.10, c, h - shift * 0.4), from_oklch(l - 0.10, c * 0.95, h + shift))
}

// ---------------------------------------------------------------- icon

fn png() -> &'static [u8] {
    include_bytes!("../assets/icon.png")
}

/// The shipped teal icon.
pub fn data() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(png()).ok()
}

/// The icon in `colour`: the shipped PNG for Teal, else drawn by `render`.
pub fn icon(colour: IconColour) -> Option<egui::IconData> {
    match colour {
        IconColour::Teal => data(),
        c => Some(render(c)),
    }
}

/// The icon's side, in pixels.
pub const SIZE: u32 = 1024;
/// The commits' centres, as in `assets/icon.svg` (for the tests).
#[cfg(test)]
const COMMITS: [(f32, f32); 3] = [(410.0, 300.0), (410.0, 724.0), (614.0, 320.0)];

/// The macOS icon shape: an 824-pixel superellipse (n = 5) centred in 1024, as in the SVG.
fn body(dy: f32) -> tiny_skia::Path {
    let mut pb = tiny_skia::PathBuilder::new();
    let (c, r, e) = (512.0_f32, 412.0_f32, 2.0 / 5.0);
    let steps = 720;
    for i in 0..steps {
        let t = i as f32 / steps as f32 * std::f32::consts::TAU;
        let (s, co) = t.sin_cos();
        let x = c + r * co.signum() * co.abs().powf(e);
        let y = c + dy + r * s.signum() * s.abs().powf(e);
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    pb.finish().expect("the superellipse is a valid path")
}

/// Draws the icon of `colour` at 1024 × 1024, layer by layer as in `assets/icon.svg`: the shape's
/// soft shadow, its gradient, the top highlight, the thin white rim, then the white branch with
/// its own shadow. Call it off the UI thread: it takes a few hundred milliseconds in a debug build.
///
/// tiny-skia (pure Rust, no system library) gives anti-aliased paths, strokes with round caps and
/// gradients; egui's tessellator only draws into a GPU frame, and doing the coverage by hand
/// would be far more code. The two Gaussian blurs it lacks are `blur_alpha` below.
pub fn render(colour: IconColour) -> egui::IconData {
    use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, Pixmap, PixmapPaint, Point, Shader, SpreadMode, Stroke, Transform};
    let id = Transform::identity();
    let new = || Pixmap::new(SIZE, SIZE).expect("1024 is a valid size");
    let mut canvas = new();

    // The shape's shadow: black at 30 %, 12 px lower, blurred by 14.
    let shape = body(0.0);
    let mut mask = new();
    let mut paint = Paint { anti_alias: true, ..Paint::default() };
    paint.set_color(Color::BLACK);
    mask.fill_path(&body(12.0), &paint, FillRule::Winding, id, None);
    canvas.draw_pixmap(0, 0, shadow(&mask, 0.30).as_ref(), &PixmapPaint::default(), id, None);

    // The gradient, top to bottom of the shape.
    let (top, bottom) = gradient(colour.base());
    let rgb = |c: [u8; 3], a: f32| Color::from_rgba8(c[0], c[1], c[2], (a * 255.0).round() as u8);
    let stops = vec![GradientStop::new(0.0, rgb(top, 1.0)), GradientStop::new(1.0, rgb(bottom, 1.0))];
    paint.shader = LinearGradient::new(Point::from_xy(0.0, 100.0), Point::from_xy(0.0, 924.0), stops, SpreadMode::Pad, id).unwrap_or(Shader::SolidColor(rgb(top, 1.0)));
    canvas.fill_path(&shape, &paint, FillRule::Winding, id, None);

    // The highlight: white at 22 % at the top, gone at 45 % of the height.
    let white = [255, 255, 255];
    let stops = vec![GradientStop::new(0.0, rgb(white, 0.22)), GradientStop::new(1.0, rgb(white, 0.0))];
    paint.shader = LinearGradient::new(Point::from_xy(0.0, 100.0), Point::from_xy(0.0, 100.0 + 0.45 * 824.0), stops, SpreadMode::Pad, id).unwrap_or(Shader::SolidColor(Color::TRANSPARENT));
    canvas.fill_path(&shape, &paint, FillRule::Winding, id, None);

    // The rim: a 4 px white line at 18 %.
    paint.shader = Shader::SolidColor(rgb(white, 0.18));
    canvas.stroke_path(&shape, &paint, &Stroke { width: 4.0, ..Stroke::default() }, id, None);

    // The branch: a line, a curve leaving it and three commits, in white.
    let mut glyph = new();
    paint.shader = Shader::SolidColor(Color::WHITE);
    let line = Stroke { width: 64.0, line_cap: tiny_skia::LineCap::Round, ..Stroke::default() };
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(410.0, 300.0);
    pb.line_to(410.0, 724.0);
    pb.move_to(410.0, 640.0);
    pb.cubic_to(410.0, 500.0, 614.0, 520.0, 614.0, 380.0);
    if let Some(path) = pb.finish() {
        glyph.stroke_path(&path, &paint, &line, id, None);
    }
    for (x, y) in [(410.0, 300.0), (410.0, 724.0), (614.0, 320.0)] {
        if let Some(dot) = tiny_skia::PathBuilder::from_circle(x, y, 74.0) {
            glyph.fill_path(&dot, &paint, FillRule::Winding, id, None);
        }
    }
    // Its shadow: black at 22 %, 10 px lower, blurred by 14.
    canvas.draw_pixmap(0, 10, shadow(&glyph, 0.22).as_ref(), &PixmapPaint::default(), id, None);
    canvas.draw_pixmap(0, 0, glyph.as_ref(), &PixmapPaint::default(), id, None);

    let rgba = canvas.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    });
    egui::IconData { rgba: rgba.collect(), width: SIZE, height: SIZE }
}

/// A black shadow of `src`'s alpha, blurred like SVG's `stdDeviation="14"`, at `opacity`. The
/// blur runs at a quarter of the size (σ 3.5 there), then is scaled back up: a shadow that soft
/// looks the same, and it is sixteen times less work.
fn shadow(src: &tiny_skia::Pixmap, opacity: f32) -> tiny_skia::Pixmap {
    const F: usize = 4;
    let (w, h) = (src.width() as usize, src.height() as usize);
    let (sw, sh) = (w / F, h / F);
    let px = src.pixels();
    let mut small = vec![0.0_f32; sw * sh];
    for (i, v) in small.iter_mut().enumerate() {
        let (x0, y0) = ((i % sw) * F, (i / sw) * F);
        let sum: u32 = (0..F).flat_map(|dy| (0..F).map(move |dx| (dx, dy))).map(|(dx, dy)| px[(y0 + dy) * w + x0 + dx].alpha() as u32).sum();
        *v = sum as f32 / (255 * F * F) as f32;
    }
    blur_alpha(&mut small, sw, sh, 3);
    // Bilinear, each small pixel's value at its centre.
    let at = |x: isize, y: isize| small[y.clamp(0, sh as isize - 1) as usize * sw + x.clamp(0, sw as isize - 1) as usize];
    let mut out = tiny_skia::Pixmap::new(src.width(), src.height()).expect("same size as the source");
    for (i, px) in out.pixels_mut().iter_mut().enumerate() {
        let fx = ((i % w) as f32 + 0.5) / F as f32 - 0.5;
        let fy = ((i / w) as f32 + 0.5) / F as f32 - 0.5;
        let (x0, y0) = (fx.floor() as isize, fy.floor() as isize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let top = at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx;
        let bottom = at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx;
        let a = ((top * (1.0 - ty) + bottom * ty) * opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        *px = tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, a).expect("black is premultiplied at any alpha");
    }
    out
}

/// A Gaussian blur of standard deviation about `radius`: three box blurs of width 2r + 1 across,
/// then three down (σ² = 3 · ((2r + 1)² − 1) / 12, so 3.5 for r = 3).
fn blur_alpha(buf: &mut [f32], w: usize, h: usize, radius: usize) {
    let mut tmp = vec![0.0; w.max(h)];
    for _ in 0..3 {
        for y in 0..h {
            box_pass(buf, &mut tmp, y * w, 1, w, radius);
        }
        for x in 0..w {
            box_pass(buf, &mut tmp, x, w, h, radius);
        }
    }
}

/// One box blur along `n` values of `buf` from `start`, `stride` apart; outside is transparent.
fn box_pass(buf: &mut [f32], tmp: &mut [f32], start: usize, stride: usize, n: usize, r: usize) {
    let at = |buf: &[f32], i: isize| if i < 0 || i >= n as isize { 0.0 } else { buf[start + i as usize * stride] };
    let r = r as isize;
    let mut sum: f32 = (-r..=r).map(|i| at(buf, i)).sum();
    let width = (2 * r + 1) as f32;
    for (i, t) in tmp.iter_mut().enumerate().take(n) {
        *t = sum / width;
        let i = i as isize;
        sum += at(buf, i + r + 1) - at(buf, i - r);
    }
    for (i, t) in tmp.iter().enumerate().take(n) {
        buf[start + i * stride] = *t;
    }
}

/// The icon as a PNG file's bytes, for the Finder.
pub fn png_bytes(icon: &egui::IconData) -> Option<Vec<u8>> {
    let img = image::RgbaImage::from_raw(icon.width, icon.height, icon.rgba.clone())?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Gives the .app at `bundle` this icon in the Finder (and in the Dock while the app isn't
/// running), or takes its custom icon away with None. macOS keeps it as an `Icon\r` file and a
/// Finder flag at the bundle's root, outside `Contents`: the signature still verifies and the app
/// still launches; only `codesign --verify --strict` reports the Finder information (#159).
/// An update replaces the bundle, so each launch puts the icon back (`App::new`).
#[cfg(target_os = "macos")]
pub fn set_finder_icon(bundle: &std::path::Path, png: Option<&[u8]>) -> bool {
    use objc2::rc::autoreleasepool;
    use objc2::ClassType;
    use objc2_app_kit::{NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::{NSData, NSString};
    autoreleasepool(|_| {
        let image = match png {
            Some(bytes) => match NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes)) {
                Some(i) => Some(i),
                None => return false,
            },
            None => None,
        };
        let path = NSString::from_str(&bundle.to_string_lossy());
        // SAFETY: plain AppKit calls on live objects; NSWorkspace may be used from any thread.
        unsafe { NSWorkspace::sharedWorkspace().setIcon_forFile_options(image.as_deref(), &path, NSWorkspaceIconCreationOptions(0)) }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn set_finder_icon(_bundle: &std::path::Path, _png: Option<&[u8]>) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_a_square_1024_png() {
        let icon = data().expect("the icon decodes");
        assert_eq!((icon.width, icon.height), (1024, 1024));
        assert_eq!(icon.rgba.len(), 1024 * 1024 * 4);
        // The corners are transparent, outside the rounded square.
        assert_eq!(icon.rgba[3], 0);
        assert_eq!(icon.rgba[1024 * 4 - 1], 0);
    }

    fn lightness(c: [u8; 3]) -> f32 {
        oklch(c).0
    }

    #[test]
    fn every_colour_is_lighter_at_the_top() {
        let free = [[0, 0, 0], [255, 255, 255], [255, 255, 0], [0, 0, 255]].map(IconColour::Custom);
        for p in IconColour::PRESETS.into_iter().chain(free) {
            let (top, bottom) = gradient(p.base());
            assert!(lightness(top) > lightness(bottom) + 0.15, "{p:?}: {top:?} over {bottom:?}");
        }
    }

    #[test]
    fn teal_gives_back_the_shipped_gradient() {
        let (top, bottom) = gradient(IconColour::Teal.base());
        let near = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 16);
        assert!(near(top, [0x3f, 0xd9, 0xd0]), "{top:?}");
        assert!(near(bottom, [0x0f, 0x8f, 0xa8]), "{bottom:?}");
        // And Teal itself is the shipped PNG, untouched.
        assert_eq!(icon(IconColour::Teal).map(|i| i.rgba), data().map(|i| i.rgba));
        assert_eq!(IconColour::default(), IconColour::Teal);
    }

    #[test]
    fn graphite_stays_grey() {
        let (top, bottom) = gradient(IconColour::Graphite.base());
        for c in [top, bottom] {
            assert!(oklch(c).1 < 0.03, "{c:?}");
        }
    }

    #[test]
    fn colours_read_back_from_their_keys() {
        for p in IconColour::PRESETS {
            assert_eq!(IconColour::parse(&p.key()), Some(p));
        }
        assert_eq!(IconColour::parse("Indigo"), Some(IconColour::Indigo));
        assert_eq!(IconColour::parse("#1A2b3c"), Some(IconColour::Custom([0x1a, 0x2b, 0x3c])));
        assert_eq!(IconColour::parse("1a2b3c"), Some(IconColour::Custom([0x1a, 0x2b, 0x3c])));
        assert_eq!(IconColour::Custom([0x1a, 0x2b, 0x3c]).key(), "#1a2b3c");
        for bad in ["", "teal2", "#12345", "#1234567", "#gg0000", "custom"] {
            assert_eq!(IconColour::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_rendered_icon_has_clear_corners_and_white_commits() {
        let icon = render(IconColour::Orange);
        assert_eq!((icon.width, icon.height), (SIZE, SIZE));
        assert_eq!(icon.rgba.len(), (SIZE * SIZE * 4) as usize);
        let px = |x: u32, y: u32| {
            let i = ((y * SIZE + x) * 4) as usize;
            [icon.rgba[i], icon.rgba[i + 1], icon.rgba[i + 2], icon.rgba[i + 3]]
        };
        for (x, y) in [(0, 0), (SIZE - 1, 0), (0, SIZE - 1), (SIZE - 1, SIZE - 1), (20, 20)] {
            assert_eq!(px(x, y)[3], 0, "corner {x},{y}");
        }
        for (x, y) in COMMITS {
            assert_eq!(px(x as u32, y as u32), [255, 255, 255, 255], "commit {x},{y}");
        }
        // Inside the shape, away from the branch: the gradient, opaque, lighter at the top.
        let (top, bottom) = (px(800, 160), px(800, 880));
        assert_eq!((top[3], bottom[3]), (255, 255));
        assert!(lightness([top[0], top[1], top[2]]) > lightness([bottom[0], bottom[1], bottom[2]]));
        // The PNG for the Finder decodes to the same pixels.
        let png = png_bytes(&icon).expect("encodes");
        assert_eq!(eframe::icon_data::from_png_bytes(&png).map(|i| i.rgba).ok(), Some(icon.rgba));
    }
}
