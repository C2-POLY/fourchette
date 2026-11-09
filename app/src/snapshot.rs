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
    let (y, m, d) = date;
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let day = format!("{d} {} {y}", MONTHS[(m.clamp(1, 12) - 1) as usize]);
    [repo, project, &day].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ")
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
