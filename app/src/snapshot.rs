//! The photo of the board's third view (#222: the city, or the stadium, mountain or food court
//! of a skin) and its postcard with a changelog (#223): what is pure and tested. `app/snapshot.rs`
//! frames, takes and draws it; `share.rs` hands it to macOS.

use crate::changelog::{self, GitHub, Version};
use egui::{ColorImage, Rect};

/// How long the white flash lasts, then the photo shrinking into its thumbnail, in seconds.
pub const FLASH: f64 = 0.22;
pub const SHRINK: f64 = 0.55;

/// Where the animation of a photo taken `age` seconds ago stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The flash over the view, its opacity going from 1 to 0.
    Flash(f32),
    /// The photo on its way from the view to the thumbnail, 0 at the view and 1 there (eased).
    Shrink(f32),
    /// The thumbnail, waiting.
    Thumb,
}

pub fn phase(age: f64) -> Phase {
    if age < FLASH {
        Phase::Flash(1.0 - (age / FLASH) as f32)
    } else if age < FLASH + SHRINK {
        let t = ((age - FLASH) / SHRINK) as f32;
        // Ease out: quick at first, settling into place.
        Phase::Shrink(1.0 - (1.0 - t).powi(3))
    } else {
        Phase::Thumb
    }
}

/// The rectangle of `area` (in points) in an image of `size` pixels at `ppp` pixels per point,
/// kept inside the image; None when nothing of it is in the image.
pub fn pixel_rect(area: Rect, ppp: f32, size: [usize; 2]) -> Option<Rect> {
    let (w, h) = (size[0] as f32, size[1] as f32);
    let r = Rect::from_min_max((area.min * ppp).round(), (area.max * ppp).round());
    let r = r.intersect(Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(w, h)));
    (r.width() >= 1.0 && r.height() >= 1.0).then_some(r)
}

/// The part of the window's image that shows `area`.
pub fn crop(img: &ColorImage, area: Rect, ppp: f32) -> Option<ColorImage> {
    pixel_rect(area, ppp, img.size).map(|r| img.region(&r, None))
}

/// The caption painted in the photo's corner: "repo · Project · 9 Nov 2026".
pub fn caption(repo: &str, project: &str, date: (i32, u32, u32)) -> String {
    let day = day(date);
    [repo, project, &day].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ")
}

/// A date as the caption and the postmark say it: "9 Nov 2026".
pub fn day((y, m, d): (i32, u32, u32)) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{d} {} {y}", MONTHS[(m.clamp(1, 12) - 1) as usize])
}

/// The photo's file name, "fourchette-<repo>-city-2026-11-09-1432.png" ("postcard" for one).
pub fn file_name(repo: &str, postcard: bool, (y, m, d): (i32, u32, u32), (hh, mm): (u32, u32)) -> String {
    let repo: String = repo.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let what = if postcard { "postcard" } else { "city" };
    let repo = if repo.is_empty() { String::new() } else { format!("{repo}-") };
    format!("fourchette-{repo}{what}-{y:04}-{m:02}-{d:02}-{hh:02}{mm:02}.png")
}

/// Today's local date and time, from `date` (the system's time zone), else from the clock in UTC.
pub fn now() -> ((i32, u32, u32), (u32, u32)) {
    let local = std::process::Command::new("date").arg("+%Y %m %d %H %M").output().ok().and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout).to_string();
        let n: Vec<i64> = s.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        (n.len() == 5).then(|| ((n[0] as i32, n[1] as u32, n[2] as u32), (n[3] as u32, n[4] as u32)))
    });
    local.unwrap_or_else(|| {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let rem = secs.rem_euclid(86_400);
        (civil(secs.div_euclid(86_400)), ((rem / 3600) as u32, (rem % 3600 / 60) as u32))
    })
}

/// The date of a day counted from 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

/// How far the postcard on the table is turned, in degrees (anticlockwise on screen).
pub const TABLE_TILT: f32 = -2.5;

/// What the postcard lies on: each skin's world as a table (#232).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The city's: a warm oak table.
    Wood,
    /// The stadium's: the red tartan of the track, with its white lane lines.
    Tartan,
    /// The mountain's: earth and gravel of the trail, with pebbles.
    Trail,
    /// The food court's: a red and white gingham tablecloth.
    Gingham,
    /// The gaming room's: a dark desk mat lit by the lime LED strip.
    Desk,
}

#[cfg(test)]
impl Surface {
    pub const ALL: [Surface; 5] = [Surface::Wood, Surface::Tartan, Surface::Trail, Surface::Gingham, Surface::Desk];
}

/// The postcard `card` lying askew on the `surface`, with its shadow: an image larger than the
/// card, the table all round it.
pub fn on_table(card: &ColorImage, surface: Surface) -> ColorImage {
    use tiny_skia::{FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Rect as SkRect, Transform};
    let (w, h) = (card.size[0] as f32, card.size[1] as f32);
    let unit = w.max(h);
    let (mx, my) = ((unit * 0.07).round(), (unit * 0.08).round());
    let (ow, oh) = ((w + 2.0 * mx) as u32, (h + 2.0 * my) as u32);
    let mut out = Pixmap::new(ow.max(1), oh.max(1)).expect("a table at least one pixel wide");
    table(&mut out, unit, surface);

    let centre = (ow as f32 / 2.0, oh as f32 / 2.0);
    let turn = Transform::from_rotate_at(TABLE_TILT, centre.0, centre.1);
    // The shadow, drawn small and blurred, then laid under the card a little lower.
    let k = 4.0;
    let (sw, sh) = (((ow as f32) / k).ceil() as u32, ((oh as f32) / k).ceil() as u32);
    if let (Some(mut shade), Some(rect)) = (Pixmap::new(sw.max(1), sh.max(1)), SkRect::from_xywh(mx / k, my / k, w / k, h / k)) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(10, 6, 2, 165);
        let path = PathBuilder::from_rect(rect);
        shade.fill_path(&path, &paint, FillRule::Winding, Transform::from_rotate_at(TABLE_TILT, centre.0 / k, centre.1 / k), None);
        blur(&mut shade, (unit * 0.012 / k).max(1.0) as usize);
        let up = PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() };
        out.draw_pixmap(0, 0, shade.as_ref(), &up, Transform::from_scale(k, k).post_translate(0.0, unit * 0.012), None);
    }
    // The card, turned about the table's centre.
    let rgba: Vec<u8> = card.pixels.iter().flat_map(|c| c.to_array()).collect();
    if let Some(src) = tiny_skia::IntSize::from_wh(card.size[0] as u32, card.size[1] as u32).and_then(|s| Pixmap::from_vec(rgba, s)) {
        let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..PixmapPaint::default() };
        out.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_translate(mx, my).post_concat(turn), None);
    }
    let pixels = out.pixels().iter().map(|p| egui::Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha())).collect();
    ColorImage::new([ow as usize, oh as usize], pixels)
}

/// A number in 0..1 for a cell of a grid, the same every time.
fn hash(x: i64, y: i64, salt: i64) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ salt.wrapping_mul(1_442_695_041)) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65_535.0
}

/// Smooth noise in 0..1 at `(x, y)` in cells.
fn noise(x: f32, y: f32, salt: i64) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (ix, iy) = (ix as i64, iy as i64);
    let a = hash(ix, iy, salt) + (hash(ix + 1, iy, salt) - hash(ix, iy, salt)) * sx;
    let b = hash(ix, iy + 1, salt) + (hash(ix + 1, iy + 1, salt) - hash(ix, iy + 1, salt)) * sx;
    a + (b - a) * sy
}

/// Fills the whole image with `surface`, `unit` pixels being the card's larger side.
fn table(px: &mut tiny_skia::Pixmap, unit: f32, surface: Surface) {
    let (w, h) = (px.width() as usize, px.height() as usize);
    let data = px.data_mut();
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = texel(surface, x as f32, y as f32, h as f32, unit);
            let o = (y * w + x) * 4;
            data[o] = r.clamp(0.0, 255.0) as u8;
            data[o + 1] = g.clamp(0.0, 255.0) as u8;
            data[o + 2] = b.clamp(0.0, 255.0) as u8;
            data[o + 3] = 255;
        }
    }
}

/// The colour of `surface` at pixel `(x, y)` of a table `h` pixels tall.
fn texel(surface: Surface, x: f32, y: f32, h: f32, unit: f32) -> (f32, f32, f32) {
    let (u, v) = (x / unit, y / unit);
    match surface {
        Surface::Wood => {
            // Planks across, each its own tone, with grain and dark joints.
            let plank = 0.16;
            let i = (v / plank).floor();
            let seed = (i * 12.9898).sin() * 43_758.547;
            let tone = seed - seed.floor();
            if v - i * plank < 0.0025 + 1.0 / unit {
                return (58.0, 34.0, 20.0);
            }
            let fx = u + tone * 4.0;
            let grain = (v * 198.0 + (fx * 9.0).sin() * 2.2 + (fx * 31.0).sin() * 0.5).sin();
            let fine = (v * 1400.0 + (fx * 23.0).sin() * 3.0).sin();
            let l = 0.86 + tone * 0.22 + grain * 0.06 + fine * 0.025;
            (164.0 * l, 108.0 * l, 66.0 * l)
        }
        Surface::Tartan => {
            // Rubber granules on the red track, and the lanes' white lines.
            let lane = 0.2;
            let within = (v / lane).fract() * lane;
            if within < 0.006 {
                let wear = 0.92 + hash(x as i64 / 2, y as i64 / 2, 7) * 0.08;
                return (240.0 * wear, 236.0 * wear, 228.0 * wear);
            }
            let grit = hash(x as i64 / 2, y as i64 / 2, 3);
            let l = 0.88 + grit * 0.16 + (noise(u * 6.0, v * 6.0, 5) - 0.5) * 0.12;
            (190.0 * l, 76.0 * l, 52.0 * l)
        }
        Surface::Trail => {
            // Earth with moss in patches, and pebbles shaded from the top left.
            let cell = 0.035;
            let (cu, cv) = ((u / cell).floor(), (v / cell).floor());
            let (ci, cj) = (cu as i64, cv as i64);
            if hash(ci, cj, 11) < 0.42 {
                let (pu, pv) = ((cu + 0.25 + hash(ci, cj, 12) * 0.5) * cell, (cv + 0.25 + hash(ci, cj, 13) * 0.5) * cell);
                let r = cell * (0.22 + hash(ci, cj, 14) * 0.2);
                let (du, dv) = ((u - pu) / (r * 1.25), (v - pv) / r);
                let d = du * du + dv * dv;
                if d < 1.0 {
                    let grey = 120.0 + hash(ci, cj, 15) * 70.0;
                    let light = 1.12 - (du + dv) * 0.14 - d * 0.18;
                    return (grey * light, grey * 0.97 * light, grey * 0.92 * light);
                }
            }
            let moss = ((noise(u * 4.0, v * 4.0, 21) - 0.55) * 3.0).clamp(0.0, 1.0);
            let l = 0.84 + noise(u * 40.0, v * 40.0, 22) * 0.16 + hash(x as i64, y as i64, 23) * 0.08;
            let (er, eg, eb) = (128.0, 104.0, 80.0);
            let (mr, mg, mb) = (96.0, 116.0, 62.0);
            ((er + (mr - er) * moss) * l, (eg + (mg - eg) * moss) * l, (eb + (mb - eb) * moss) * l)
        }
        Surface::Gingham => {
            // Red and white checks where the stripes cross, and the cloth's weave.
            let check = 0.045;
            let a = ((u / check).floor() as i64).rem_euclid(2) == 0;
            let b = ((v / check).floor() as i64).rem_euclid(2) == 0;
            let (r, g, bl) = match (a, b) {
                (true, true) => (192.0, 34.0, 40.0),
                (true, false) | (false, true) => (232.0, 150.0, 146.0),
                (false, false) => (250.0, 246.0, 240.0),
            };
            let weave = 0.96 + ((x * 1.3).sin() * (y * 1.3).sin()) * 0.03 + (noise(u * 8.0, v * 8.0, 31) - 0.5) * 0.06;
            (r * weave, g * weave, bl * weave)
        }
        Surface::Desk => {
            // A dark desk mat with a faint grid, lit lime along its top and bottom edges.
            let base = 26.0 + hash(x as i64, y as i64, 41) * 6.0;
            let grid = 0.05;
            let line = (u / grid).fract() * grid < 0.0015 || (v / grid).fract() * grid < 0.0015;
            let base = if line { base + 9.0 } else { base };
            let edge = y.min(h - y) / unit;
            let glow = (-edge / 0.035).exp();
            (base + 150.0 * glow, base + 2.0 + 235.0 * glow, base + 8.0 + 60.0 * glow)
        }
    }
}

/// Puts `photo` back into `card` over the pixel rectangle `at`, resized smoothly: the composed
/// card shows it smaller than it was taken, this keeps its own sharpness.
pub fn paste(card: &mut ColorImage, photo: &ColorImage, at: Rect) {
    let x0 = at.left().round().max(0.0) as usize;
    let y0 = at.top().round().max(0.0) as usize;
    let w = (at.width().round() as usize).min(card.size[0].saturating_sub(x0));
    let h = (at.height().round() as usize).min(card.size[1].saturating_sub(y0));
    if w == 0 || h == 0 {
        return;
    }
    let rgba: Vec<u8> = photo.pixels.iter().flat_map(|c| c.to_array()).collect();
    let Some(src) = image::RgbaImage::from_raw(photo.size[0] as u32, photo.size[1] as u32, rgba) else { return };
    let small = image::imageops::resize(&src, w as u32, h as u32, image::imageops::FilterType::Lanczos3);
    for (x, y, p) in small.enumerate_pixels() {
        let [r, g, b, a] = p.0;
        card.pixels[(y0 + y as usize) * card.size[0] + x0 + x as usize] = egui::Color32::from_rgba_premultiplied(r, g, b, a);
    }
}

/// Three passes of a box blur `r` pixels wide over a premultiplied image: close to a Gaussian.
fn blur(px: &mut tiny_skia::Pixmap, r: usize) {
    let (w, h) = (px.width() as usize, px.height() as usize);
    let data = px.data_mut();
    let mut line = Vec::new();
    for _ in 0..3 {
        for (len, stride, count, step) in [(w, 4, h, w * 4), (h, w * 4, w, 4)] {
            for n in 0..count {
                let base = n * step;
                for ch in 0..4 {
                    line.clear();
                    line.extend((0..len).map(|i| data[base + i * stride + ch] as u32));
                    let at = |i: isize| line[i.clamp(0, len as isize - 1) as usize];
                    let mut sum: u32 = (-(r as isize)..=(r as isize)).map(at).sum();
                    for i in 0..len {
                        data[base + i * stride + ch] = (sum / (2 * r as u32 + 1)) as u8;
                        sum += at(i as isize + r as isize + 1);
                        sum -= at(i as isize - r as isize);
                    }
                }
            }
        }
    }
}

/// The sections a postcard tells: what is not released yet, and the latest release.
pub fn postcard_versions(mut versions: Vec<Version>) -> Vec<Version> {
    versions.truncate(2);
    versions
}

/// The postcard's text in Markdown: each section Claude wrote, under its name and date.
pub fn postcard_text(versions: &[Version], notes: &[String], gh: Option<&GitHub>) -> String {
    let mut s = String::new();
    for (v, n) in versions.iter().zip(notes) {
        if !changelog::shown(v, gh) || n.trim().is_empty() {
            continue;
        }
        if !s.is_empty() {
            s.push('\n');
        }
        if v.date.is_empty() {
            s.push_str(&format!("## {}\n\n", v.name));
        } else {
            s.push_str(&format!("## {} · {}\n\n", v.name, v.date));
        }
        s.push_str(n.trim());
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, Color32};

    #[test]
    fn the_animation_flashes_then_shrinks_then_waits() {
        assert_eq!(phase(0.0), Phase::Flash(1.0));
        assert!(matches!(phase(FLASH / 2.0), Phase::Flash(a) if (a - 0.5).abs() < 1e-3));
        assert_eq!(phase(FLASH), Phase::Shrink(0.0));
        assert!(matches!(phase(FLASH + SHRINK / 2.0), Phase::Shrink(t) if t > 0.5 && t < 1.0));
        assert_eq!(phase(FLASH + SHRINK), Phase::Thumb);
        assert_eq!(phase(60.0), Phase::Thumb);
    }

    #[test]
    fn crops_in_retina_pixels_and_stays_inside_the_image() {
        let area = Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 70.0));
        assert_eq!(pixel_rect(area, 2.0, [1000, 1000]), Some(Rect::from_min_max(pos2(20.0, 40.0), pos2(220.0, 140.0))));
        // Over the image's right and bottom edges: cut there.
        assert_eq!(pixel_rect(area, 2.0, [100, 100]), Some(Rect::from_min_max(pos2(20.0, 40.0), pos2(100.0, 100.0))));
        // Nothing of it in the image.
        assert_eq!(pixel_rect(Rect::from_min_max(pos2(500.0, 500.0), pos2(600.0, 600.0)), 1.0, [100, 100]), None);

        let mut img = ColorImage::filled([8, 4], Color32::BLACK);
        img.pixels[8 + 3] = Color32::RED;
        let c = crop(&img, Rect::from_min_max(pos2(1.5, 0.5), pos2(3.0, 2.0)), 2.0).unwrap();
        assert_eq!(c.size, [3, 3]);
        assert_eq!(c.pixels[0], Color32::RED);
    }

    #[test]
    fn caption_and_file_name() {
        assert_eq!(caption("fourchette", "Fourchette", (2026, 11, 9)), "fourchette · Fourchette · 9 Nov 2026");
        assert_eq!(caption("sample", "", (2026, 1, 31)), "sample · 31 Jan 2026");
        assert_eq!(file_name("my repo", false, (2026, 11, 9), (14, 5)), "fourchette-my-repo-city-2026-11-09-1405.png");
        assert_eq!(file_name("", true, (2026, 3, 1), (9, 0)), "fourchette-postcard-2026-03-01-0900.png");
    }

    #[test]
    fn the_postcard_lies_askew_on_each_skins_table() {
        let card = ColorImage::filled([400, 200], Color32::from_rgb(250, 244, 230));
        let mut corners = Vec::new();
        for surface in Surface::ALL {
            let t = on_table(&card, surface);
            // The table goes all round the card, opaque.
            assert!(t.size[0] > 400 + 40 && t.size[1] > 200 + 40, "{surface:?} {:?}", t.size);
            assert!(t.pixels.iter().all(|c| c.a() == 255), "{surface:?}");
            // The card's middle is the card.
            let m = t.pixels[t.size[1] / 2 * t.size[0] + t.size[0] / 2];
            assert!(m.r() >= 245 && m.g() >= 238 && m.b() >= 222, "{surface:?} {m:?}");
            // Turned: the card's top-left corner is no longer on the card's colour.
            let corner = t.pixels[(t.size[1] - 200) / 2 * t.size[0] + (t.size[0] - 400) / 2 + 2];
            assert!(corner != m, "{surface:?} {corner:?}");
            // The same table every time.
            assert_eq!(on_table(&card, surface).pixels, t.pixels, "{surface:?}");
            let avg = |c: &[Color32]| c.iter().fold([0u32; 3], |a, p| [a[0] + p.r() as u32, a[1] + p.g() as u32, a[2] + p.b() as u32]);
            corners.push(avg(&t.pixels[..t.size[0] * 20]));
        }
        // Each skin has its own table.
        for i in 0..corners.len() {
            for j in i + 1..corners.len() {
                assert_ne!(corners[i], corners[j]);
            }
        }
        // The wood is warm brown, the desk dark, the gingham has its red.
        let wood = on_table(&card, Surface::Wood).pixels[0];
        assert!(wood.r() > wood.g() && wood.g() > wood.b(), "{wood:?}");
        let desk = on_table(&card, Surface::Desk);
        let mid = desk.pixels[desk.size[1] / 2 * desk.size[0] + 3];
        assert!(mid.r() < 60 && mid.g() < 60, "{mid:?}");
    }

    #[test]
    fn the_photo_is_pasted_back_at_its_place() {
        let mut card = ColorImage::filled([100, 80], Color32::WHITE);
        let photo = ColorImage::filled([300, 200], Color32::RED);
        paste(&mut card, &photo, Rect::from_min_max(pos2(10.0, 20.0), pos2(40.0, 40.0)));
        assert_eq!(card.pixels[30 * 100 + 25], Color32::RED);
        assert_eq!(card.pixels[30 * 100 + 45], Color32::WHITE);
        assert_eq!(card.pixels[10 * 100 + 25], Color32::WHITE);
        // Past the card's edge: cut there, nothing written outside.
        paste(&mut card, &photo, Rect::from_min_max(pos2(90.0, 70.0), pos2(200.0, 200.0)));
        assert_eq!(card.pixels[79 * 100 + 99], Color32::RED);
    }

    #[test]
    fn dates_from_days() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_766), (2026, 11, 9));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }

    #[test]
    fn the_postcard_tells_the_unreleased_work_and_the_latest_release() {
        let (versions, notes, gh) = changelog::sample();
        let versions = postcard_versions(versions);
        assert_eq!(versions.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), [changelog::UNRELEASED, "v2.3.0"]);
        let text = postcard_text(&versions, &notes, Some(&gh));
        assert!(text.starts_with(&format!("## {}\n\n### Added", changelog::UNRELEASED)));
        assert!(text.contains("\n## v2.3.0 · 2026-11-03\n\n"));
        assert!(!text.contains("v2.2.1"));
        // A section Claude left empty, or nothing to tell, is left out.
        let empty = Version { name: changelog::UNRELEASED.into(), ..Default::default() };
        assert_eq!(postcard_text(&[empty], &["Something".into()], None), "");
        assert_eq!(postcard_text(&versions[1..], &[String::new()], Some(&gh)), "");
    }
}
