//! The pictures on the Skin tiles of Settings › Appearance (#220): one small flat drawing per
//! skin, painted with shapes scaled to the preview rect (the app's font has no pictograms).
use crate::settings::Skin;
use egui::epaint::{PathShape, QuadraticBezierShape};
use egui::{pos2, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke};

/// A point of the preview rect from its fractions (0 to 1).
fn at(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + r.width() * x, r.top() + r.height() * y)
}

/// A rect from fractions of the preview rect.
fn area(r: Rect, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(at(r, x0, y0), at(r, x1, y1))
}

fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Draws `skin`'s picture inside `r`.
pub(super) fn draw(p: &Painter, r: Rect, skin: Skin) {
    match skin {
        Skin::Normal => normal(p, r),
        Skin::Runner => runner(p, r),
        Skin::Trail => trail(p, r),
        Skin::Eater => eater(p, r),
        Skin::Player | Skin::Gamer => player(p, r),
    }
}

/// The background some pictures paint over the whole preview (the night sky).
pub(super) fn backdrop(skin: Skin) -> Option<Color32> {
    (skin == Skin::Trail).then(|| rgb(0x1B2340))
}

fn normal(p: &Painter, r: Rect) {
    let blue = rgb(0x4C8DF6);
    let green = rgb(0x3FB68B);
    let lane = |y: f32| at(r, 0.2, y);
    p.line_segment([lane(0.18), lane(0.82)], Stroke::new(2.0_f32, blue));
    p.add(QuadraticBezierShape::from_points_stroke([at(r, 0.2, 0.5), at(r, 0.2, 0.28), at(r, 0.42, 0.28)], false, Color32::TRANSPARENT, Stroke::new(2.0_f32, green)));
    let dot = |c: Pos2, col: Color32| {
        p.circle_filled(c, 4.0, col);
    };
    for y in [0.18, 0.5, 0.82] {
        dot(lane(y), blue);
    }
    dot(at(r, 0.42, 0.28), green);
    let grey = Color32::from_gray(128).gamma_multiply(0.6);
    for (y, x1) in [(0.2, 0.92), (0.5, 0.82), (0.8, 0.88)] {
        p.rect_filled(area(r, 0.56, y - 0.045, x1, y + 0.045), CornerRadius::same(2), grey);
    }
}

fn runner(p: &Painter, r: Rect) {
    let orange = rgb(0xD85A30);
    let track = area(r, 0.05, 0.14, 0.7, 0.86);
    let rad = |t: Rect, f: f32| CornerRadius::same((t.height() * 0.5 * f) as u8);
    p.rect_filled(track, rad(track, 1.0), orange);
    let inner = track.shrink(7.0);
    p.rect_filled(inner, rad(inner, 1.0), rgb(0x4E9B54));
    // The dashed lane line runs along the middle of the track.
    let lane = track.shrink(3.5);
    let (rr, cy) = (lane.height() * 0.5, lane.center().y);
    let (xl, xr) = (lane.left() + rr, lane.right() - rr);
    let mut pts = vec![];
    for i in 0..=12 {
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
        pts.push(pos2(xl + rr * a.cos(), cy - rr * a.sin()));
    }
    for i in 0..=12 {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
        pts.push(pos2(xr + rr * a.cos(), cy - rr * a.sin()));
    }
    pts.push(pts[0]);
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.0_f32, Color32::WHITE), 3.0, 3.0));
    // The chequered finish across the bottom straight.
    let cs = 2.5;
    let o = pos2(at(r, 0.4, 0.0).x, track.bottom() - 7.0);
    for i in 0..2 {
        for j in 0..3 {
            let col = if (i + j) % 2 == 0 { Color32::WHITE } else { Color32::BLACK };
            p.rect_filled(Rect::from_min_size(o + vec2(i as f32 * cs, j as f32 * cs - 0.5), vec2(cs, cs)), CornerRadius::ZERO, col);
        }
    }
    // The podium: second, first, third.
    let base = r.bottom() - r.height() * 0.14;
    for (i, (h, col)) in [(0.2, 0xB8BEC8), (0.34, 0xF2C14E), (0.14, 0xC98A5B)].into_iter().enumerate() {
        let x = at(r, 0.76, 0.0).x + i as f32 * r.width() * 0.07;
        p.rect_filled(Rect::from_min_max(pos2(x, base - r.height() * h), pos2(x + r.width() * 0.07 - 1.0, base)), CornerRadius::ZERO, rgb(col));
    }
}

fn trail(p: &Painter, r: Rect) {
    for (x, y) in [(0.12, 0.14), (0.3, 0.1), (0.84, 0.2)] {
        p.circle_filled(at(r, x, y), 1.2, Color32::from_white_alpha(210));
    }
    let tri = |a: Pos2, b: Pos2, c: Pos2, col: Color32| {
        p.add(PathShape::convex_polygon(vec![a, b, c], col, Stroke::NONE));
    };
    tri(at(r, 0.12, 0.92), at(r, 0.5, 0.2), at(r, 0.92, 0.92), rgb(0x7D8699));
    // Snow cap, on the slopes just below the peak.
    tri(at(r, 0.5, 0.2), at(r, 0.4, 0.4), at(r, 0.6, 0.4), Color32::WHITE);
    // Foreground ridge: a low hill.
    let ridge: Vec<Pos2> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            at(r, t, 0.98 - 0.3 * (std::f32::consts::PI * t).sin())
        })
        .collect();
    p.add(PathShape::convex_polygon(ridge, rgb(0x3E8E4F), Stroke::NONE));
    // The trail climbing to the arch.
    let trail = [at(r, 0.22, 0.88), at(r, 0.62, 0.78), at(r, 0.38, 0.6), at(r, 0.56, 0.44), at(r, 0.5, 0.3)];
    p.extend(Shape::dashed_line(&trail, Stroke::new(1.5_f32, rgb(0xF2C14E)), 3.0, 2.5));
    let arch = area(r, 0.44, 0.15, 0.56, 0.3);
    p.rect_stroke(arch, CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, Stroke::new(1.5_f32, rgb(0xE8793A)), egui::StrokeKind::Inside);
    p.circle_filled(at(r, 0.56, 0.44), 2.8, rgb(0xE8793A));
}

fn eater(p: &Painter, r: Rect) {
    // The stall: striped awning over a pink counter.
    let (x0, x1) = (0.06, 0.46);
    let sw = (x1 - x0) / 6.0;
    for i in 0..6 {
        let col = if i % 2 == 0 { rgb(0xE5483F) } else { Color32::WHITE };
        p.rect_filled(area(r, x0 + sw * i as f32, 0.14, x0 + sw * (i + 1) as f32, 0.34), CornerRadius::ZERO, col);
    }
    p.rect_filled(area(r, x0 + 0.02, 0.34, x1 - 0.02, 0.72), CornerRadius::same(2), rgb(0xF4A6BC));
    p.rect_filled(area(r, x0 + 0.06, 0.44, x1 - 0.06, 0.56), CornerRadius::same(2), rgb(0xFBD3DF));
    // The burger: bun, lettuce, patty, cheese, bun.
    let (b0, b1) = (0.58, 0.86);
    p.rect_filled(area(r, b0, 0.14, b1, 0.34), CornerRadius { nw: 10, ne: 10, sw: 1, se: 1 }, rgb(0xE0A04C));
    p.rect_filled(area(r, b0 - 0.02, 0.34, b1 + 0.02, 0.4), CornerRadius::same(2), rgb(0x5FB04F));
    p.rect_filled(area(r, b0, 0.4, b1, 0.5), CornerRadius::same(2), rgb(0x7A4A2B));
    p.rect_filled(area(r, b0 - 0.02, 0.5, b1 + 0.02, 0.55), CornerRadius::ZERO, rgb(0xF6C945));
    p.rect_filled(area(r, b0, 0.55, b1, 0.68), CornerRadius { nw: 1, ne: 1, sw: 5, se: 5 }, rgb(0xE0A04C));
    // The kcal bar, half filled.
    let bar = area(r, 0.06, 0.85, 0.94, 0.85);
    let bar = Rect::from_center_size(bar.center(), vec2(bar.width(), 4.0));
    p.rect_filled(bar, CornerRadius::same(2), Color32::from_gray(128).gamma_multiply(0.35));
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * 0.5, 4.0)), CornerRadius::same(2), rgb(0xF2A93B));
}

fn player(p: &Painter, r: Rect) {
    let lime = rgb(0xA3E635);
    // The neon shelf with its game boxes.
    p.line_segment([at(r, 0.06, 0.2), at(r, 0.62, 0.2)], Stroke::new(1.5_f32, lime));
    for (x, col) in [(0.1, 0xD4537E), (0.24, 0x4C8DF6), (0.38, 0xF2C14E), (0.5, 0x7F77DD)] {
        p.rect_filled(area(r, x, 0.04, x + 0.08, 0.19), CornerRadius::same(1), rgb(col));
    }
    // The code screen and the game screen with its boss.
    let code = area(r, 0.08, 0.3, 0.44, 0.64);
    p.rect_filled(code, CornerRadius::same(2), rgb(0x1E2230));
    for (y, x1) in [(0.38, 0.32), (0.46, 0.38), (0.54, 0.28)] {
        p.line_segment([at(r, 0.13, y), at(r, x1, y)], Stroke::new(1.5_f32, lime.gamma_multiply(0.85)));
    }
    let game = area(r, 0.5, 0.26, 0.92, 0.64);
    p.rect_filled(game, CornerRadius::same(2), rgb(0x3C3489));
    let boss = Rect::from_center_size(game.center(), vec2(game.height() * 0.5, game.height() * 0.5));
    p.rect_filled(boss, CornerRadius::same(2), rgb(0xE24B4A));
    for dx in [-0.25, 0.25] {
        p.circle_filled(boss.center() + vec2(boss.width() * dx, -boss.height() * 0.1), 1.6, Color32::WHITE);
    }
    // The desk.
    p.rect_filled(area(r, 0.04, 0.66, 0.96, 0.72), CornerRadius::same(1), Color32::from_gray(110));
    // The XP bar: lime segments, two thirds full.
    let n = 12;
    let (x0, x1, y) = (0.06, 0.94, 0.86);
    let w = (x1 - x0) / n as f32;
    for i in 0..n {
        let col = if i < 8 { lime } else { Color32::from_gray(128).gamma_multiply(0.35) };
        p.rect_filled(area(r, x0 + w * i as f32, y - 0.04, x0 + w * (i as f32 + 0.8), y + 0.04), CornerRadius::same(1), col);
    }
}
