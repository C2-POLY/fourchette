//! The Big eater skin's drawings (#212), from the "Icons" artboard of the design canvas: SVG
//! paths in their own boxes, rendered by the litter's `Pen` (`mess::path`), so they share its
//! kawaii look (ink outline, flat fill, shade, highlight, face). The fork, its simplified
//! version below 32 points, the Golden Fork trophy, the burger, the chips, the pizza, the
//! cutlery of the announcer's toasts and the big eater himself.

use super::mess::{Pen, BLUSH, EYE, INK};
use egui::epaint::Mesh;
use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// One path of a drawing: its `d`, its fill, its stroke and the stroke's width in box units.
pub(super) struct Part(pub &'static str, pub Option<Color32>, pub Option<Color32>, pub f32);

/// A drawing placed in a bigger one as `translate(x y) rotate(deg) scale(k)`: parts, x, y, k, deg.
pub(super) struct Layer(pub &'static [Part], pub f32, pub f32, pub f32, pub f32);

/// Below this height, in points, the fork is the simplified one: the full fork's outline would
/// fill its tines.
pub(super) const SMALL_FORK: f32 = 32.0;

/// Gold and silver, for the forks.
pub(super) const GOLD: Color32 = rgb(0xf2c14e);
pub(super) const SILVER: Color32 = rgb(0xe9ecf2);

/// Draws `parts` with their box's (0, 0) at `o`, scaled `k` points per unit, turned `deg`
/// degrees round `o`, their strokes `w` times as wide.
pub(super) fn draw(p: &Painter, parts: &[Part], o: Pos2, k: f32, deg: f32, w: f32) {
    let turn = deg.to_radians();
    let (sin, cos) = turn.sin_cos();
    // The pen draws round the box's point (60, 48).
    let c = o + vec2(60.0 * cos - 48.0 * sin, 60.0 * sin + 48.0 * cos) * k;
    let pen = Pen { p, c, turn, s: k };
    for Part(d, fill, stroke, width) in parts {
        let subs = pen.subpaths(d);
        if let Some(fill) = fill {
            for pts in subs.iter().filter(|s| s.len() >= 3) {
                fill_shape(p, pts, *fill);
            }
        }
        if let Some(col) = stroke {
            let st = Stroke::new((width * w * k).max(0.6), *col);
            let closed = d.contains(['Z', 'z']);
            for pts in subs.into_iter().filter(|s| s.len() >= 2) {
                p.add(if closed { Shape::closed_line(pts, st) } else { Shape::line(pts, st) });
            }
        }
    }
}

/// A filled shape: anti-aliased when convex, else cut into triangles.
fn fill_shape(p: &Painter, pts: &[Pos2], fill: Color32) {
    if convex(pts) {
        p.add(Shape::convex_polygon(pts.to_vec(), fill, Stroke::NONE));
        return;
    }
    let mut mesh = Mesh::default();
    for q in pts {
        mesh.colored_vertex(*q, fill);
    }
    for [a, b, c] in super::mess::triangulate(pts) {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    p.add(Shape::mesh(mesh));
}

/// Whether the polygon turns one way only.
fn convex(pts: &[Pos2]) -> bool {
    let n = pts.len();
    let (mut pos, mut neg) = (false, false);
    for k in 0..n {
        let (a, b, c) = (pts[k], pts[(k + 1) % n], pts[(k + 2) % n]);
        let z = (b - a).x * (c - b).y - (b - a).y * (c - b).x;
        pos |= z > 1e-6;
        neg |= z < -1e-6;
        if pos && neg {
            return false;
        }
    }
    true
}

/// Draws the layers of a composed drawing at `o`, scaled `k`.
fn draw_layers(p: &Painter, layers: &[Layer], o: Pos2, k: f32) {
    for Layer(parts, x, y, lk, deg) in layers {
        draw(p, parts, o + vec2(*x, *y) * k, lk * k, *deg, 1.0);
    }
}

/// The scale and corner that fit a drawing's `content` (in box units) centred in `r`.
fn fit(content: Rect, r: Rect) -> (Pos2, f32) {
    let k = (r.width() / content.width()).min(r.height() / content.height());
    (r.center() - content.center().to_vec2() * k, k)
}

/// A fork `h` points tall centred on `c`, gold or silver: the full one, or the simplified one
/// below `SMALL_FORK`.
pub(super) fn fork(p: &Painter, c: Pos2, h: f32, fill: Color32) {
    if h < SMALL_FORK {
        fork_small(p, c, h, fill);
        return;
    }
    let parts = if fill == SILVER { FORK_SILVER } else { FORK_GOLD };
    let k = h / 152.0;
    draw(p, parts, c - vec2(32.0, 80.0) * k, k, 0.0, 1.0);
}

/// The simplified fork, `h` points tall, centred on `c`: one shape filled and outlined.
pub(super) fn fork_small(p: &Painter, c: Pos2, h: f32, fill: Color32) {
    let k = h / 38.0;
    // Thicker outlines when tiny, as the canvas draws them.
    let w = if h < 16.0 { 1.4 } else if h < 24.0 { 1.2 } else { 1.1 };
    draw(p, &[Part(FORK_SMALL_D, Some(fill), Some(INK), w)], c - vec2(7.5, 20.0) * k, k, 0.0, 1.0);
}

/// The Golden Fork trophy fitted in `r`.
pub(super) fn golden_fork(p: &Painter, r: Rect) {
    let (o, k) = fit(Rect::from_min_max(pos2(4.0, 4.0), pos2(156.0, 200.0)), r);
    draw_layers(p, GOLDEN_FORK, o, k);
}

/// A dish of the menu with its face, its larger side `size` points, centred on `c`. Small ones
/// get thicker outlines, as on the canvas.
pub(super) fn food(p: &Painter, c: Pos2, size: f32, parts: &[Part]) {
    let content = if std::ptr::eq(parts, BURGER) {
        Rect::from_min_max(pos2(6.0, 9.0), pos2(58.0, 50.0))
    } else if std::ptr::eq(parts, CHIPS) {
        Rect::from_min_max(pos2(5.0, 4.0), pos2(43.0, 57.0))
    } else {
        Rect::from_min_max(pos2(4.0, 3.0), pos2(52.0, 55.0))
    };
    let k = size / content.width().max(content.height());
    let w = if k < 0.6 { 1.6 } else { 1.0 };
    draw(p, parts, c - content.center().to_vec2() * k, k, 0.0, w);
}

/// The announcer's icon: the small gold fork and the silver knife, `h` points tall.
pub(super) fn cutlery(p: &Painter, c: Pos2, h: f32) {
    let k = h / 26.0;
    let o = c - vec2(17.75, 16.0) * k;
    draw(p, &[Part(FORK_SMALL_D, Some(GOLD), Some(INK), 1.3)], o + vec2(6.0, 2.5) * k, 0.68 * k, 0.0, 1.0);
    draw(p, KNIFE, o, k, 0.0, 1.0);
}

/// The big eater, his box's (0, 0) at `o`, scaled `k`; his raised hand is at `EATER_HAND`.
pub(super) fn eater(p: &Painter, o: Pos2, k: f32) {
    draw(p, EATER, o, k, 0.0, 1.0);
}

/// Where the big eater's raised hand is, in his box.
pub(super) const EATER_HAND: Vec2 = vec2(85.0, 38.0);

/// The gold fork, in a 64 × 160 box.
pub(super) const FORK_GOLD: &[Part] = &[
    Part("M15 8 Q15 4 17.5 4 Q20 4 20 8 L20 46 L24.5 46 L24.5 8 Q24.5 4 27 4 Q29.5 4 29.5 8 L29.5 46 L34.5 46 L34.5 8 Q34.5 4 37 4 Q39.5 4 39.5 8 L39.5 46 L44 46 L44 8 Q44 4 46.5 4 Q49 4 49 8 L49 52 C49 64 40 70 35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 C28 156 25 150 25 140 C25 120 29 104 29 92 L29 76 C24 70 15 64 15 52 Z", Some(rgb(0xf2c14e)), None, 0.0),
    Part("M32 76 L35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 Z M49 52 C49 64 40 70 35 76 L32 76 L32 72 C38 68 45.5 62 45.5 52 Z M18.4 8 H20 V46 H18.4 Z M27.9 8 H29.5 V46 H27.9 Z M37.9 8 H39.5 V46 H37.9 Z M47.4 8 H49 V52 H47.4 Z", Some(rgb(0xd69a1e)), None, 0.0),
    Part("M27.6 96 C27.4 110 26.2 124 26.6 138 M17.4 10 V40", None, Some(rgb(0xfff1c2)), 2.2),
    Part("M15 8 Q15 4 17.5 4 Q20 4 20 8 L20 46 L24.5 46 L24.5 8 Q24.5 4 27 4 Q29.5 4 29.5 8 L29.5 46 L34.5 46 L34.5 8 Q34.5 4 37 4 Q39.5 4 39.5 8 L39.5 46 L44 46 L44 8 Q44 4 46.5 4 Q49 4 49 8 L49 52 C49 64 40 70 35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 C28 156 25 150 25 140 C25 120 29 104 29 92 L29 76 C24 70 15 64 15 52 Z", None, Some(INK), 2.4),
];
/// The silver fork, in the same box.
pub(super) const FORK_SILVER: &[Part] = &[
    Part("M15 8 Q15 4 17.5 4 Q20 4 20 8 L20 46 L24.5 46 L24.5 8 Q24.5 4 27 4 Q29.5 4 29.5 8 L29.5 46 L34.5 46 L34.5 8 Q34.5 4 37 4 Q39.5 4 39.5 8 L39.5 46 L44 46 L44 8 Q44 4 46.5 4 Q49 4 49 8 L49 52 C49 64 40 70 35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 C28 156 25 150 25 140 C25 120 29 104 29 92 L29 76 C24 70 15 64 15 52 Z", Some(rgb(0xe9ecf2)), None, 0.0),
    Part("M32 76 L35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 Z M49 52 C49 64 40 70 35 76 L32 76 L32 72 C38 68 45.5 62 45.5 52 Z M18.4 8 H20 V46 H18.4 Z M27.9 8 H29.5 V46 H27.9 Z M37.9 8 H39.5 V46 H37.9 Z M47.4 8 H49 V52 H47.4 Z", Some(rgb(0xb8bec9)), None, 0.0),
    Part("M27.6 96 C27.4 110 26.2 124 26.6 138 M17.4 10 V40", None, Some(rgb(0xffffff)), 2.2),
    Part("M15 8 Q15 4 17.5 4 Q20 4 20 8 L20 46 L24.5 46 L24.5 8 Q24.5 4 27 4 Q29.5 4 29.5 8 L29.5 46 L34.5 46 L34.5 8 Q34.5 4 37 4 Q39.5 4 39.5 8 L39.5 46 L44 46 L44 8 Q44 4 46.5 4 Q49 4 49 8 L49 52 C49 64 40 70 35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 C28 156 25 150 25 140 C25 120 29 104 29 92 L29 76 C24 70 15 64 15 52 Z", None, Some(INK), 2.4),
];
/// The gold fork with a face, for the Golden Fork.
pub(super) const FORK_FACE: &[Part] = &[
    Part("M15 8 Q15 4 17.5 4 Q20 4 20 8 L20 46 L24.5 46 L24.5 8 Q24.5 4 27 4 Q29.5 4 29.5 8 L29.5 46 L34.5 46 L34.5 8 Q34.5 4 37 4 Q39.5 4 39.5 8 L39.5 46 L44 46 L44 8 Q44 4 46.5 4 Q49 4 49 8 L49 52 C49 64 40 70 35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 C28 156 25 150 25 140 C25 120 29 104 29 92 L29 76 C24 70 15 64 15 52 Z", Some(rgb(0xf2c14e)), None, 0.0),
    Part("M32 76 L35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 Z M49 52 C49 64 40 70 35 76 L32 76 L32 72 C38 68 45.5 62 45.5 52 Z M18.4 8 H20 V46 H18.4 Z M27.9 8 H29.5 V46 H27.9 Z M37.9 8 H39.5 V46 H37.9 Z M47.4 8 H49 V52 H47.4 Z", Some(rgb(0xd69a1e)), None, 0.0),
    Part("M27.6 96 C27.4 110 26.2 124 26.6 138 M17.4 10 V40", None, Some(rgb(0xfff1c2)), 2.2),
    Part("M15 8 Q15 4 17.5 4 Q20 4 20 8 L20 46 L24.5 46 L24.5 8 Q24.5 4 27 4 Q29.5 4 29.5 8 L29.5 46 L34.5 46 L34.5 8 Q34.5 4 37 4 Q39.5 4 39.5 8 L39.5 46 L44 46 L44 8 Q44 4 46.5 4 Q49 4 49 8 L49 52 C49 64 40 70 35 76 L35 92 C35 104 39 120 39 140 C39 150 36 156 32 156 C28 156 25 150 25 140 C25 120 29 104 29 92 L29 76 C24 70 15 64 15 52 Z", None, Some(INK), 2.4),
    Part("M23 60 Q26 56.5 29 60 M35 60 Q38 56.5 41 60", None, Some(EYE), 2.2),
    Part("M17 66 a3.2 1.8 0 1 0 6.4 0 a3.2 1.8 0 1 0 -6.4 0 Z M40.6 66 a3.2 1.8 0 1 0 6.4 0 a3.2 1.8 0 1 0 -6.4 0 Z", Some(BLUSH), None, 0.0),
    Part("M29.5 65 Q32 68 34.5 65", None, Some(EYE), 1.8),
];
/// The simplified fork, for sizes below 32 points, in a 15 × 40 box.
pub(super) const FORK_SMALL_D: &str = "M3 2.5 Q3 1 4 1 Q5 1 5 2.5 V10 H6.5 V2.5 Q6.5 1 7.5 1 Q8.5 1 8.5 2.5 V10 H10 V2.5 Q10 1 11 1 Q12 1 12 2.5 V12 C12 15 10 16.5 9 17.5 V22 C9 26 10 30 10 35 C10 37.5 9 39 7.5 39 C6 39 5 37.5 5 35 C5 30 6 26 6 22 V17.5 C5 16.5 3 15 3 12 Z";
/// A burger with a face, in a 64 × 56 box.
pub(super) const BURGER: &[Part] = &[
    Part("M9 41 H55 Q56 49 48 50 H16 Q8 49 9 41 Z", Some(rgb(0xd99350)), Some(INK), 2.0),
    Part("M7 33 Q7 30 10 30 H54 Q57 30 57 33 V36 Q57 40 53 40 H11 Q7 40 7 36 Z", Some(rgb(0x7a4528)), Some(INK), 2.0),
    Part("M8 29 H56 L52 33 L47 38 L43 33 L22 33 L17 37 L13 33 Z", Some(rgb(0xffc93c)), Some(INK), 2.0),
    Part("M6 28 Q9 24 12 28 Q15 24 18 28 Q21 24 24 28 Q27 24 30 28 Q33 24 36 28 Q39 24 42 28 Q45 24 48 28 Q51 24 54 28 Q57 24 58 28 L58 30 H6 Z", Some(rgb(0x6cc04a)), Some(INK), 2.0),
    Part("M8 27 C8 9 56 9 56 27 Q56 29 54 29 H10 Q8 29 8 27 Z", Some(rgb(0xe9a85c)), None, 0.0),
    Part("M44 13 C52 17 55 22 55 27 H47 C48 21 47 17 44 13 Z", Some(rgb(0xc98a45)), None, 0.0),
    Part("M14 22 Q16.5 17 22 14.5", None, Some(rgb(0xfff1c2)), 2.2),
    Part("M8 27 C8 9 56 9 56 27 Q56 29 54 29 H10 Q8 29 8 27 Z", None, Some(INK), 2.0),
    Part("M20 15.5 l3 -1.2 l0.6 1.5 l-3 1.2 Z M30.5 13.5 l3 0 l0 1.6 l-3 0 Z M41 14.3 l3 1.2 l-0.6 1.5 l-3 -1.2 Z", Some(rgb(0xfff4dc)), Some(INK), 0.8),
    Part("M22.6 21.5 a2.4 2.4 0 1 0 4.8 0 a2.4 2.4 0 1 0 -4.8 0 Z M36.6 21.5 a2.4 2.4 0 1 0 4.8 0 a2.4 2.4 0 1 0 -4.8 0 Z", Some(EYE), None, 0.0),
    Part("M25 20.5 a0.8 0.8 0 1 0 1.6 0 a0.8 0.8 0 1 0 -1.6 0 Z M39 20.5 a0.8 0.8 0 1 0 1.6 0 a0.8 0.8 0 1 0 -1.6 0 Z", Some(rgb(0xffffff)), None, 0.0),
    Part("M19 25 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z M39 25 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z", Some(BLUSH), None, 0.0),
    Part("M29 24 Q32 27 35 24", None, Some(EYE), 1.6),
];
/// A bag of chips with a face, in a 48 × 60 box.
pub(super) const CHIPS: &[Part] = &[
    Part("M7 12 L9 6 L12 12 L15 6 L18 12 L21 6 L24 12 L27 6 L30 12 L33 6 L36 12 L39 6 L41 12 Z", Some(rgb(0xf2b21e)), Some(INK), 2.0),
    Part("M41 50 L39 56 L36 50 L33 56 L30 50 L27 56 L24 50 L21 56 L18 50 L15 56 L12 50 L9 56 L7 50 Z", Some(rgb(0xf2b21e)), Some(INK), 2.0),
    Part("M23.5 9 a6.5 4 -20 1 0 13 0 a6.5 4 -20 1 0 -13 0 Z", Some(rgb(0xf0c75a)), Some(INK), 1.6),
    Part("M8 12 L40 12 Q43 30 41 50 L7 50 Q5 30 8 12 Z", Some(rgb(0xffc93c)), None, 0.0),
    Part("M7.5 33 Q24 36 40.6 33 L40.8 41 Q24 44 7.2 41 Z", Some(rgb(0xff5a4e)), None, 0.0),
    Part("M35 14 Q40 30 38.5 48 L41 50 Q43 30 40 12 Z", Some(rgb(0xe0a21a)), None, 0.0),
    Part("M12 16 Q11 22 11.5 28", None, Some(rgb(0xfff6c2)), 2.2),
    Part("M8 12 L40 12 Q43 30 41 50 L7 50 Q5 30 8 12 Z", None, Some(INK), 2.0),
    Part("M15.6 22 a2.4 2.4 0 1 0 4.8 0 a2.4 2.4 0 1 0 -4.8 0 Z M27.6 22 a2.4 2.4 0 1 0 4.8 0 a2.4 2.4 0 1 0 -4.8 0 Z", Some(EYE), None, 0.0),
    Part("M18 21 a0.8 0.8 0 1 0 1.6 0 a0.8 0.8 0 1 0 -1.6 0 Z M30 21 a0.8 0.8 0 1 0 1.6 0 a0.8 0.8 0 1 0 -1.6 0 Z", Some(rgb(0xffffff)), None, 0.0),
    Part("M12 25.5 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z M30 25.5 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z", Some(BLUSH), None, 0.0),
    Part("M21 24.5 Q24 27.5 27 24.5", None, Some(EYE), 1.6),
];
/// A slice of pizza with a face, in a 56 × 58 box.
pub(super) const PIZZA: &[Part] = &[
    Part("M8 15 Q28 8 48 15 L29 54 Q28 55.5 27 54 Z", Some(rgb(0xffd45c)), Some(INK), 2.0),
    Part("M14 22 Q15 28 13 30 M41 24 Q40 29 42 31", None, Some(rgb(0xf5b52e)), 2.4),
    Part("M5 11 Q28 1 51 11 Q52 15 48 16 Q28 8 8 16 Q4 15 5 11 Z", Some(rgb(0xc98a45)), Some(INK), 2.0),
    Part("M12 30 a4 4 0 1 0 8 0 a4 4 0 1 0 -8 0 Z M34.4 31 a3.6 3.6 0 1 0 7.2 0 a3.6 3.6 0 1 0 -7.2 0 Z M23.6 43 a3.4 3.4 0 1 0 6.8 0 a3.4 3.4 0 1 0 -6.8 0 Z", Some(rgb(0xd9433a)), Some(INK), 1.5),
    Part("M20.1 22 a2.4 2.4 0 1 0 4.8 0 a2.4 2.4 0 1 0 -4.8 0 Z M31.1 22 a2.4 2.4 0 1 0 4.8 0 a2.4 2.4 0 1 0 -4.8 0 Z", Some(EYE), None, 0.0),
    Part("M22.5 21 a0.8 0.8 0 1 0 1.6 0 a0.8 0.8 0 1 0 -1.6 0 Z M33.5 21 a0.8 0.8 0 1 0 1.6 0 a0.8 0.8 0 1 0 -1.6 0 Z", Some(rgb(0xffffff)), None, 0.0),
    Part("M16.5 25.5 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z M33.5 25.5 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z", Some(BLUSH), None, 0.0),
    Part("M25 24.5 Q28 27.5 31 24.5", None, Some(EYE), 1.6),
];
/// The big eater, his raised hand on the right, in a 100 × 106 box.
pub(super) const EATER: &[Part] = &[
    Part("M22 72 C12 80 14 94 27 96 C32 92 31 82 31 76 Z", Some(rgb(0xff8a80)), Some(INK), 2.4),
    Part("M17 104 C9 72 22 48 50 48 C78 48 91 72 83 104 Z", Some(rgb(0xff8a80)), Some(INK), 2.4),
    Part("M60 52 C72 56 80 70 82 104 L74 104 C74 80 68 64 60 58 Z", Some(rgb(0xe8706a)), None, 0.0),
    Part("M36 52 L64 52 L60 86 Q50 92 40 86 Z", Some(rgb(0xffffff)), Some(INK), 2.0),
    Part("M42 64 l2 0 l0 2 l-2 0 Z M50 60 l2 0 l0 2 l-2 0 Z M56 70 l2 0 l0 2 l-2 0 Z M46 76 l2 0 l0 2 l-2 0 Z", Some(rgb(0xff5a4e)), None, 0.0),
    Part("M75 68 C88 60 92 46 88 36 C84 33 79 35 79 40 C81 48 75 56 69 61 Z", Some(rgb(0xff8a80)), Some(INK), 2.4),
    Part("M21 96 a6 6 0 1 0 12 0 a6 6 0 1 0 -12 0 Z M79 38 a6 6 0 1 0 12 0 a6 6 0 1 0 -12 0 Z", Some(rgb(0xf6c9a0)), Some(INK), 2.0),
    Part("M28 36 a22 21 0 1 0 44 0 a22 21 0 1 0 -44 0 Z", Some(rgb(0xf6c9a0)), Some(INK), 2.4),
    Part("M38 17 Q44 8 50 15 Q55 7 62 16", None, Some(rgb(0x5a3a28)), 3.2),
    Part("M37 34 Q41 30 45 34 M55 34 Q59 30 63 34", None, Some(EYE), 2.4),
    Part("M29 41 a7 5 0 1 0 14 0 a7 5 0 1 0 -14 0 Z M57 41 a7 5 0 1 0 14 0 a7 5 0 1 0 -14 0 Z", Some(BLUSH), None, 0.0),
    Part("M44 43 Q50 52 56 43 Q50 47 44 43 Z", Some(rgb(0x7a2e2e)), Some(INK), 1.6),
    Part("M39.7 50 a1.3 1.3 0 1 0 2.6 0 a1.3 1.3 0 1 0 -2.6 0 Z M58.9 49 a1.1 1.1 0 1 0 2.2 0 a1.1 1.1 0 1 0 -2.2 0 Z M56 53 a1 1 0 1 0 2 0 a1 1 0 1 0 -2 0 Z", Some(rgb(0xe9a85c)), None, 0.0),
];
/// The silver knife of the cutlery, in its 28 × 30 box.
pub(super) const KNIFE: &[Part] = &[
    Part("M22 3 C26.5 5 27.5 10.5 26.5 16 L24 16 L24 27.5 Q24 29 22.5 29 Q21 29 21 27.5 V4.5 Q21 3 22 3 Z", Some(rgb(0xe9ecf2)), Some(INK), 1.4),
    Part("M22.6 6 V14", None, Some(rgb(0xffffff)), 1.2),
];
/// A laurel leaf, from its stem end.
pub(super) const LEAF: &[Part] = &[
    Part("M0 0 C4 -5.5 12 -5.5 17 0 C12 5.5 4 5.5 0 0 Z", Some(rgb(0x7cc152)), Some(INK), 1.5),
    Part("M2 0 H13", None, Some(rgb(0x4f8a3c)), 1.2),
];
/// A four-pointed sparkle round its centre.
pub(super) const STAR: &[Part] = &[
    Part("M0 -8 Q1 -1 8 0 Q1 1 0 8 Q-1 1 -8 0 Q-1 -1 0 -8 Z", Some(rgb(0xfff1c2)), None, 0.0),
];
/// Part of the Golden Fork trophy.
pub(super) const TROPHY_0: &[Part] = &[
    Part("M46 176 C24 150 22 112 38 84", None, Some(rgb(0x4f8a3c)), 3.0),
];
/// Part of the Golden Fork trophy.
pub(super) const TROPHY_1: &[Part] = &[
    Part("M114 176 C136 150 138 112 122 84", None, Some(rgb(0x4f8a3c)), 3.0),
];
/// Part of the Golden Fork trophy.
pub(super) const TROPHY_2: &[Part] = &[
    Part("M38 168 H122 Q126 168 126 172 V178 H34 V172 Q34 168 38 168 Z", Some(rgb(0x5b4370)), Some(INK), 2.4),
    Part("M40 178 H120 L114 198 H46 Z", Some(rgb(0x4a3560)), Some(INK), 2.4),
    Part("M60 182 H100 Q102 182 102 184 V192 Q102 194 100 194 H60 Q58 194 58 192 V184 Q58 182 60 182 Z", Some(rgb(0xf2c14e)), Some(INK), 1.6),
];
/// The Golden Fork trophy in a 160 × 200 box: the laurels, the fork with a face, the plinth and its plaque, the sparkles.
pub(super) const GOLDEN_FORK: &[Layer] = &[
    Layer(TROPHY_0, 0.0, 0.0, 1.0, 0.0),
    Layer(TROPHY_1, 0.0, 0.0, 1.0, 0.0),
    Layer(LEAF, 44.0, 168.0, 1.0, -125.0),
    Layer(LEAF, 116.0, 168.0, 1.0, 305.0),
    Layer(LEAF, 36.0, 152.0, 1.0, -140.0),
    Layer(LEAF, 124.0, 152.0, 1.0, 320.0),
    Layer(LEAF, 31.0, 134.0, 1.0, -155.0),
    Layer(LEAF, 129.0, 134.0, 1.0, 335.0),
    Layer(LEAF, 30.0, 116.0, 1.0, -170.0),
    Layer(LEAF, 130.0, 116.0, 1.0, 350.0),
    Layer(LEAF, 33.0, 99.0, 1.0, -185.0),
    Layer(LEAF, 127.0, 99.0, 1.0, 365.0),
    Layer(LEAF, 38.0, 86.0, 1.0, -200.0),
    Layer(LEAF, 122.0, 86.0, 1.0, 380.0),
    Layer(FORK_FACE, 48.0, 10.0, 1.0, 0.0),
    Layer(TROPHY_2, 0.0, 0.0, 1.0, 0.0),
    Layer(STAR, 132.0, 30.0, 1.0, 0.0),
    Layer(STAR, 24.0, 58.0, 0.7, 0.0),
    Layer(STAR, 120.0, 92.0, 0.5, 0.0),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::mess::path;

    #[test]
    fn every_drawing_parses_into_shapes() {
        let all: [&[Part]; 11] = [FORK_GOLD, FORK_SILVER, FORK_FACE, BURGER, CHIPS, PIZZA, EATER, KNIFE, LEAF, STAR, TROPHY_0];
        for parts in all {
            for Part(d, ..) in parts {
                let subs = path(d);
                assert!(!subs.is_empty() && subs.iter().all(|s| s.len() >= 2), "{d}");
            }
        }
        assert_eq!(path(FORK_SMALL_D).len(), 1);
    }

    #[test]
    fn the_drawings_stay_in_their_boxes() {
        let inside = |parts: &[Part], w: f32, h: f32| {
            parts.iter().flat_map(|Part(d, ..)| path(d)).flatten().all(|v| (-1.0..=w + 1.0).contains(&v.x) && (-1.0..=h + 1.0).contains(&v.y))
        };
        assert!(inside(FORK_GOLD, 64.0, 160.0));
        assert!(inside(BURGER, 64.0, 56.0));
        assert!(inside(CHIPS, 48.0, 60.0));
        assert!(inside(PIZZA, 56.0, 58.0));
        assert!(inside(EATER, 100.0, 106.0));
    }

    #[test]
    fn convex_shapes_are_told_from_concave_ones() {
        assert!(convex(&[pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0)]));
        assert!(!convex(&[pos2(0.0, 0.0), pos2(2.0, 0.0), pos2(2.0, 1.0), pos2(1.0, 1.0), pos2(1.0, 3.0), pos2(0.0, 3.0)]));
    }
}
