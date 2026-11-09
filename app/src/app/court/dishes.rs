//! The food court's dishes and people (#212), drawn round a point on screen at a scale `s`
//! (screen points per unit, the view's zoom): a burger with a layer per sub-issue, a bowl of
//! chili, a pizza, a donut or a soda, most of them with a kawaii face once served; the raw
//! ingredients of a ticket ready to take, the cloche of one in review, the cooks in their
//! toques, the flies round an unfinished bug, the grill's flames and the smoke. They have the
//! look of the skin's drawings (`art`): an ink outline, a shaded side, a highlight and a face.

use super::super::city::Faces;
use super::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub(super) const INK: Color32 = rgb(0x2a2433);
/// The outline of the drawings, and their highlight.
const LINE: Color32 = rgb(0x3b2f4a);
const SHINE: Color32 = Color32::from_rgba_premultiplied(120, 118, 105, 120);
const BLUSH: Color32 = rgb(0xff8fb1);
const BUN: (Color32, Color32) = (rgb(0xc07a35), rgb(0xe0a25a));
const PATTY: (Color32, Color32) = (rgb(0x4a2a19), rgb(0x6b3d24));
const LETTUCE: (Color32, Color32) = (rgb(0x4f9e34), rgb(0x7ccc52));
const TOMATO: (Color32, Color32) = (rgb(0xc8352b), rgb(0xe84a3d));
const CHEESE: Color32 = rgb(0xffc93c);
const SESAME: Color32 = rgb(0xfff4dc);
const CRUST: (Color32, Color32) = (rgb(0xb8742f), rgb(0xf0c75a));
const PEPPERONI: Color32 = rgb(0xc8352b);
const BASIL: Color32 = rgb(0x4f9e34);
/// A donut's icing, and the colours of its sprinkles.
const ICINGS: [Color32; 4] = [rgb(0xff8fc1), rgb(0x7a4a2a), rgb(0xf7f3e8), rgb(0x9fd8ff)];
const SPRINKLES: [Color32; 5] = [rgb(0x7fd1ff), rgb(0xffc93c), rgb(0xc6f432), rgb(0xf7f8f8), rgb(0x8b5cf6)];
/// A soda cup's paint and its lid.
const CUPS: [Color32; 4] = [rgb(0xe8478b), rgb(0x3b82c4), rgb(0x3fb950), rgb(0xff7a1a)];
const BOWLS: [Color32; 3] = [rgb(0xf4f1e6), rgb(0x7fb8f0), rgb(0xffd66b)];
const CHILI: Color32 = rgb(0xa3321f);
const STEEL: (Color32, Color32) = (rgb(0xc9ccd4), rgb(0xeef0f4));
const SKINS: [Color32; 5] = [rgb(0xf6d6ba), rgb(0xeac098), rgb(0xd6a478), rgb(0xa8714a), rgb(0x6e4a32)];

/// What a ticket is served as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dish {
    /// A Feature: a burger with this many layers, one per sub-issue.
    Burger(u32),
    /// A Bug.
    Chili(usize),
    Pizza,
    Donut(usize),
    Soda(usize),
}

impl Dish {
    /// A ticket's dish: its kind, and for the rest a pizza, a donut or a soda from its seed.
    pub(super) fn of(pl: &Plot) -> Dish {
        let v = (pl.seed / 7) as usize;
        match pl.building {
            Building::Tower => Dish::Burger(pl.floors.saturating_sub(3).clamp(1, 5)),
            Building::Workshop => Dish::Chili(v % BOWLS.len()),
            Building::House => match pl.seed % 3 {
                0 => Dish::Pizza,
                1 => Dish::Donut(v % ICINGS.len()),
                _ => Dish::Soda(v % CUPS.len()),
            },
        }
    }

    /// How high it stands above its plate, in units.
    pub(super) fn height(self) -> f32 {
        match self {
            Dish::Burger(n) => 22.0 + n as f32 * 10.0,
            Dish::Chili(_) => 9.0,
            Dish::Pizza => 4.0,
            Dish::Donut(_) => 7.0,
            Dish::Soda(_) => 34.0,
        }
    }
}

/// Points along an ellipse round `c`, from angle `from` to `to` (0 to the right, PI/2 down).
pub(super) fn ell(c: Pos2, rx: f32, ry: f32, from: f32, to: f32, n: usize) -> Vec<Pos2> {
    (0..=n).map(|k| {
        let a = from + (to - from) * k as f32 / n as f32;
        c + vec2(rx * a.cos(), ry * a.sin())
    }).collect()
}

fn fill(p: &Painter, pts: Vec<Pos2>, c: Color32) {
    if pts.len() > 2 {
        p.add(Shape::convex_polygon(pts, c, Stroke::NONE));
    }
}

/// An ellipse lying flat (`ry` half `rx`) or standing, filled.
pub(super) fn oval(p: &Painter, c: Pos2, rx: f32, ry: f32, col: Color32) {
    fill(p, ell(c, rx, ry, 0.0, TAU, 18), col);
}

/// A standing cylinder on `base`, `rx` wide and `h` tall: its lit left half, its shaded right
/// half and its top. Returns the middle of its top.
pub(super) fn cyl(p: &Painter, base: Pos2, rx: f32, h: f32, (side, top): (Color32, Color32)) -> Pos2 {
    let ry = rx / 2.0;
    let t = base - vec2(0.0, h);
    let mut left = ell(base, rx, ry, FRAC_PI_2, PI, 8);
    left.extend(ell(t, rx, ry, PI, FRAC_PI_2, 8));
    fill(p, left, side);
    let mut right = ell(base, rx, ry, 0.0, FRAC_PI_2, 8);
    right.extend(ell(t, rx, ry, FRAC_PI_2, 0.0, 8));
    fill(p, right, theme::shade(side, 0.16));
    oval(p, t, rx, ry, top);
    // The ink outline round it and round its top, and a highlight down its lit side.
    let w = (rx * 0.075).clamp(0.5, 1.8);
    let mut edge = ell(base, rx, ry, 0.0, PI, 12);
    edge.extend(ell(t, rx, ry, PI, TAU, 12));
    p.add(Shape::closed_line(edge, Stroke::new(w, LINE)));
    p.add(Shape::line(ell(t, rx, ry, 0.0, PI, 12), Stroke::new(w * 0.8, LINE)));
    if h > rx * 0.5 {
        let x = -rx * 0.62;
        p.line_segment([base + vec2(x, -h * 0.25), t + vec2(x, h * 0.2 + ry * 0.6)], Stroke::new(w * 1.3, SHINE));
    }
    t
}

/// A dome over `base`, `rx` wide and `h` tall, lit on its left.
fn dome(p: &Painter, base: Pos2, rx: f32, h: f32, c: Color32) {
    let ry = rx / 2.0;
    let mut whole = ell(base, rx, h, PI, TAU, 16);
    whole.extend(ell(base, rx, ry, 0.0, PI, 12));
    fill(p, whole, c);
    let mut shade = ell(base, rx, h, 1.5 * PI, TAU, 8);
    shade.extend(ell(base, rx, ry, 0.0, FRAC_PI_2, 6));
    shade.push(base);
    fill(p, shade, theme::shade(c, 0.12));
    let w = (rx * 0.075).clamp(0.5, 1.8);
    let mut edge = ell(base, rx, h, PI, TAU, 16);
    edge.extend(ell(base, rx, ry, 0.0, PI, 12));
    p.add(Shape::closed_line(edge, Stroke::new(w, LINE)));
    // The highlight on its upper left, as on the burger drawing.
    p.add(Shape::line(ell(base, rx * 0.72, h * 0.72, PI * 1.12, PI * 1.38, 6), Stroke::new(w * 1.4, rgb(0xfff1c2))));
}

/// A kawaii face at `c`, `s` points per unit: two eyes with their glint, pink cheeks and a smile
/// (`happy` curves the eyes up instead).
pub(super) fn face(p: &Painter, c: Pos2, s: f32, happy: bool) {
    if s < 0.45 {
        return;
    }
    let ink = Stroke::new((1.1 * s).max(0.8), INK);
    for dx in [-3.4, 3.4] {
        let e = c + vec2(dx * s, 0.0);
        if happy {
            p.add(Shape::line(ell(e + vec2(0.0, 0.8 * s), 1.4 * s, 1.3 * s, PI, TAU, 6), ink));
        } else {
            p.circle_filled(e, 1.35 * s, INK);
            p.circle_filled(e + vec2(-0.45 * s, -0.5 * s), 0.5 * s, Color32::WHITE);
        }
        oval(p, c + vec2(dx * 1.7 * s, 2.0 * s), 1.6 * s, 0.9 * s, BLUSH.gamma_multiply(0.85));
    }
    p.add(Shape::line(ell(c + vec2(0.0, 1.4 * s), 1.4 * s, 1.1 * s, 0.0, PI, 6), ink));
}

/// A plate on a table, its middle at `c`.
pub(super) fn plate(p: &Painter, c: Pos2, s: f32) {
    oval(p, c + vec2(0.0, 1.2 * s), 21.0 * s, 10.5 * s, rgb(0xc9ccd4));
    oval(p, c, 21.0 * s, 10.5 * s, rgb(0xf7f8f8));
    oval(p, c, 15.0 * s, 7.5 * s, rgb(0xeceef2));
}

/// A dish on its plate at `c`: `served` adds its face, and steam over what is hot.
pub(super) fn dish(p: &Painter, c: Pos2, s: f32, d: Dish, time: f32, served: bool) {
    match d {
        Dish::Burger(n) => burger(p, c, s, n, served),
        Dish::Chili(v) => {
            let bowl = BOWLS[v % BOWLS.len()];
            let t = cyl(p, c, 14.0 * s, 8.0 * s, (bowl, CHILI));
            // A band round the bowl.
            p.add(Shape::line(ell(c - vec2(0.0, 5.6 * s), 14.0 * s, 7.0 * s, 0.15, PI - 0.15, 10), Stroke::new(1.3 * s, theme::shade(bowl, 0.3))));
            for (dx, dy) in [(-6.0, 0.0), (4.0, -2.0), (7.0, 2.0), (-2.0, 3.0), (0.5, -3.5)] {
                oval(p, t + vec2(dx * s, dy * s), 1.7 * s, 1.0 * s, rgb(0x5a1a10));
            }
            oval(p, t + vec2(-2.0 * s, -0.5 * s), 4.2 * s, 2.1 * s, rgb(0xf7f3e8));
            oval(p, t + vec2(3.0 * s, 0.8 * s), 1.4 * s, 0.7 * s, BASIL);
            if served {
                face(p, c - vec2(0.0, 3.0 * s), s * 0.9, false);
                steam(p, t, s, time);
            }
        }
        Dish::Pizza => {
            let t = cyl(p, c, 19.0 * s, 3.0 * s, CRUST);
            oval(p, t, 16.0 * s, 8.0 * s, rgb(0xe2553a));
            oval(p, t + vec2(0.0, -0.3 * s), 14.5 * s, 7.2 * s, rgb(0xf4cf62));
            for (dx, dy) in [(-9.0, -1.0), (8.0, -2.5), (-1.0, 4.0), (10.0, 2.5), (-4.0, -4.5), (-10.0, 3.5), (4.0, -5.0)] {
                oval(p, t + vec2(dx * s, dy * s), 2.6 * s, 1.4 * s, PEPPERONI);
            }
            for (dx, dy) in [(3.0, -1.0), (-6.5, 2.0), (5.0, 4.0)] {
                oval(p, t + vec2(dx * s, dy * s), 1.8 * s, 0.9 * s, BASIL);
            }
            if served && s >= 0.45 {
                // Its eyes on the cheese, seen from above.
                for dx in [-2.8, 2.8] {
                    oval(p, t + vec2(dx * s, 0.5 * s), 1.2 * s, 0.8 * s, INK);
                }
                p.add(Shape::line(ell(t + vec2(0.0, 1.7 * s), 1.4 * s, 0.7 * s, 0.0, PI, 6), Stroke::new((1.0 * s).max(0.7), INK)));
                steam(p, t, s, time);
            }
        }
        Dish::Donut(v) => {
            let icing = ICINGS[v % ICINGS.len()];
            let t = cyl(p, c, 13.0 * s, 6.0 * s, (rgb(0xc98a45), icing));
            // The icing drips over the edge.
            for k in 0..5 {
                let a = 0.4 + k as f32 * 0.55;
                let at = t + vec2(13.0 * s * a.cos(), 6.5 * s * a.sin());
                fill(p, vec![at - vec2(1.6 * s, 0.0), at + vec2(1.6 * s, 0.0), at + vec2(0.0, (2.0 + (k % 2) as f32 * 1.5) * s)], icing);
            }
            oval(p, t, 4.4 * s, 2.2 * s, rgb(0x8a5527));
            for (i, (dx, dy)) in [(-8.0, -1.0), (7.0, -2.0), (-3.0, 3.5), (5.0, 3.0), (0.0, -4.5), (9.0, 1.5), (-6.0, 3.0)].iter().enumerate() {
                oval(p, t + vec2(dx * s, dy * s), 1.5 * s, 0.6 * s, SPRINKLES[(i + v) % SPRINKLES.len()]);
            }
            if served {
                face(p, c - vec2(0.0, 2.4 * s), s * 0.85, true);
            }
        }
        Dish::Soda(v) => {
            let cup = CUPS[v % CUPS.len()];
            let (rb, rt, h) = (6.0 * s, 8.5 * s, 24.0 * s);
            let top = c - vec2(0.0, h);
            let mut body = ell(c, rb, rb / 2.0, 0.0, PI, 10);
            body.extend(ell(top, rt, rt / 2.0, PI, 0.0, 10));
            fill(p, body, cup);
            let mut shade = ell(c, rb, rb / 2.0, 0.0, FRAC_PI_2, 6);
            shade.extend(ell(top, rt, rt / 2.0, FRAC_PI_2, 0.0, 6));
            fill(p, shade, theme::shade(cup, 0.16));
            let mut edge = ell(c, rb, rb / 2.0, 0.0, PI, 10);
            edge.extend(ell(top, rt, rt / 2.0, PI, TAU, 10));
            p.add(Shape::closed_line(edge, Stroke::new((0.9 * s).clamp(0.5, 1.8), LINE)));
            // A white band, the lid and the straw.
            let band = |z: f32| c - vec2(0.0, z * s);
            let w = |z: f32| (6.0 + 2.5 * z / 24.0) * s;
            let (z0, z1) = (14.0, 18.0);
            fill(p, vec![band(z0) - vec2(w(z0), 0.0), band(z0) + vec2(w(z0), 0.0), band(z1) + vec2(w(z1), 0.0), band(z1) - vec2(w(z1), 0.0)], rgb(0xf7f8f8));
            oval(p, top, rt + 0.6 * s, rt / 2.0 + 0.3 * s, rgb(0xe9e6dc));
            oval(p, top - vec2(0.0, 1.0 * s), rt * 0.8, rt * 0.4, rgb(0xf7f3e8));
            p.line_segment([top + vec2(1.5 * s, -s), top + vec2(5.5 * s, -12.0 * s)], Stroke::new(1.8 * s, rgb(0xffc93c)));
            p.line_segment([top + vec2(5.5 * s, -12.0 * s), top + vec2(9.0 * s, -13.0 * s)], Stroke::new(1.8 * s, rgb(0xffc93c)));
            if served {
                face(p, c - vec2(0.0, 8.0 * s), s * 0.9, false);
            }
        }
    }
}

/// A burger of `n` layers: the bun, then for each layer lettuce, a patty, a slice of cheese and,
/// every other one, tomato; the top bun with its sesame seeds.
fn burger(p: &Painter, c: Pos2, s: f32, n: u32, served: bool) {
    let mut t = cyl(p, c, 15.0 * s, 4.0 * s, BUN);
    for i in 0..n {
        t = cyl(p, t, 16.0 * s, 1.6 * s, LETTUCE);
        // Its frilly edge.
        for k in 0..7 {
            let a = 0.2 + k as f32 * 0.45;
            oval(p, t + vec2(16.0 * s * a.cos(), 8.0 * s * a.sin() + 0.6 * s), 2.4 * s, 1.3 * s, LETTUCE.1);
        }
        t = cyl(p, t, 15.0 * s, 4.2 * s, PATTY);
        // The cheese, its corners hanging over.
        fill(p, vec![t - vec2(17.0 * s, 0.0), t - vec2(0.0, 8.5 * s), t + vec2(17.0 * s, 0.0), t + vec2(0.0, 8.5 * s)], CHEESE);
        for dx in [-8.0, 6.0] {
            let at = t + vec2(dx * s, 4.5 * s);
            fill(p, vec![at - vec2(2.2 * s, 0.0), at + vec2(2.2 * s, 0.0), at + vec2(0.0, 4.5 * s)], CHEESE);
        }
        t -= vec2(0.0, 0.8 * s);
        if i % 2 == 0 {
            t = cyl(p, t, 13.5 * s, 1.6 * s, TOMATO);
        }
    }
    t = cyl(p, t, 15.0 * s, 2.0 * s, BUN);
    dome(p, t, 15.0 * s, 11.0 * s, BUN.1);
    for (dx, dy) in [(-6.0, -6.0), (-1.0, -9.0), (5.0, -6.5), (8.5, -3.0), (-9.0, -2.5)] {
        oval(p, t + vec2(dx * s, dy * s), 1.3 * s, 0.75 * s, SESAME);
    }
    if served {
        face(p, t + vec2(0.0, -2.0 * s), s * 0.95, false);
    }
}

/// Steam rising over a hot dish, three wisps that fade as they go up.
fn steam(p: &Painter, top: Pos2, s: f32, time: f32) {
    for k in 0..3 {
        let phase = (time * 0.45 + k as f32 / 3.0).fract();
        let x = (k as f32 - 1.0) * 6.0 * s;
        let a = ((1.0 - phase) * phase * 4.0 * 110.0) as u8;
        let pts: Vec<Pos2> = (0..6).map(|j| {
            let y = j as f32 * 2.2;
            top + vec2(x + (y * 0.5 + time * 2.0 + k as f32).sin() * 1.6 * s, -(6.0 + phase * 12.0 + y) * s)
        }).collect();
        p.add(Shape::line(pts, Stroke::new(1.2 * s, Color32::from_white_alpha(a))));
    }
}

/// Ready: the raw ingredients on the plate, waiting for a cook.
pub(super) fn raw(p: &Painter, c: Pos2, s: f32, d: Dish) {
    match d {
        Dish::Pizza => {
            oval(p, c, 15.0 * s, 7.5 * s, rgb(0xf3e4c8));
            oval(p, c + vec2(-4.0 * s, -s), 3.0 * s, 1.5 * s, TOMATO.1);
            oval(p, c + vec2(6.0 * s, 1.0 * s), 3.5 * s, 1.6 * s, rgb(0xf7f3e8));
        }
        Dish::Donut(_) | Dish::Soda(_) => {
            oval(p, c + vec2(-3.0 * s, 0.0), 8.0 * s, 4.0 * s, rgb(0xf3e4c8));
            oval(p, c + vec2(7.0 * s, -s), 3.0 * s, 1.5 * s, rgb(0xff8fc1));
            oval(p, c + vec2(5.0 * s, 3.0 * s), 2.0 * s, 1.0 * s, rgb(0xfff4dc));
        }
        _ => {
            // A raw patty, tomato, cheese and a leaf of lettuce.
            oval(p, c + vec2(-6.0 * s, -0.5 * s), 6.0 * s, 3.0 * s, rgb(0xe89a9a));
            oval(p, c + vec2(-6.5 * s, -0.8 * s), 3.5 * s, 1.6 * s, rgb(0xf2b8b8));
            oval(p, c + vec2(6.0 * s, -3.0 * s), 3.4 * s, 1.7 * s, TOMATO.1);
            oval(p, c + vec2(6.0 * s, -3.0 * s), 1.8 * s, 0.9 * s, rgb(0xff9a8a));
            oval(p, c + vec2(3.0 * s, 3.0 * s), 3.4 * s, 1.7 * s, TOMATO.1);
            fill(p, vec![c + vec2(-2.0 * s, -6.0 * s), c + vec2(4.0 * s, -7.0 * s), c + vec2(2.0 * s, -3.5 * s)], CHEESE);
            oval(p, c + vec2(10.5 * s, 1.0 * s), 4.0 * s, 2.0 * s, LETTUCE.1);
        }
    }
}

/// In review: a silver cloche over the plate, its highlight and its knob.
pub(super) fn cloche(p: &Painter, c: Pos2, s: f32) {
    dome(p, c, 15.0 * s, 15.0 * s, STEEL.0);
    fill(p, vec![c + vec2(-10.0 * s, -6.0 * s), c + vec2(-7.0 * s, -11.5 * s), c + vec2(-4.5 * s, -10.5 * s), c + vec2(-7.5 * s, -5.0 * s)], STEEL.1);
    oval(p, c - vec2(0.0, 16.0 * s), 2.6 * s, 1.6 * s, rgb(0x9a9ea8));
    oval(p, c - vec2(0.0, 16.6 * s), 1.6 * s, 0.9 * s, rgb(0xd6d8de));
}

/// To test: a fork stuck in the dish, its handle up.
pub(super) fn fork_in(p: &Painter, top: Pos2, s: f32) {
    let st = Stroke::new(1.6 * s, rgb(0xd6d8de));
    p.line_segment([top + vec2(2.0 * s, -s), top + vec2(8.0 * s, -20.0 * s)], st);
    p.circle_filled(top + vec2(8.0 * s, -20.0 * s), 1.3 * s, rgb(0xd6d8de));
}

/// What a cook is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pose {
    /// Ready: arms crossed, tapping a foot.
    Wait,
    /// In progress: flipping with a spatula.
    Flip,
    /// To test: tasting, a fork at the mouth.
    Taste,
}

/// A skin tone for a person, always the same for them.
pub(super) fn skin(who: &str) -> Color32 {
    let h = who.bytes().fold(17u32, |a, b| a.wrapping_mul(131).wrapping_add(b as u32)) as usize;
    SKINS[h % SKINS.len()]
}

/// A cook in a white jacket and toque standing on `feet`, `s` points per unit, facing the viewer.
pub(super) fn cook(p: &Painter, feet: Pos2, s: f32, tone: Color32, pose: Pose, time: f32) {
    let bob = match pose {
        Pose::Wait => (time * 3.0).sin().max(0.0) * 0.8,
        Pose::Flip => (time * 6.0).sin().abs() * 0.6,
        Pose::Taste => (time * 2.0).sin() * 0.5,
    };
    let f = feet - vec2(0.0, bob * s);
    let q = |x: f32, y: f32| f + vec2(x * s, -y * s);
    oval(p, feet, 6.5 * s, 2.4 * s, Color32::from_black_alpha(55));
    // Checked trousers.
    let trousers = rgb(0x2b2d33);
    fill(p, vec![q(-2.8, 9.0), q(-0.5, 9.0), q(-0.7, 0.0), q(-2.8, 0.0)], trousers);
    let tap = if pose == Pose::Wait { ((time * 6.0).sin().max(0.0) * 1.2).min(1.0) } else { 0.0 };
    fill(p, vec![q(0.5, 9.0), q(2.8, 9.0), q(2.8, tap), q(0.7, tap)], trousers);
    // The jacket and its buttons.
    let jacket = rgb(0xf7f8f8);
    fill(p, vec![q(-4.0, 19.0), q(4.0, 19.0), q(4.4, 8.5), q(-4.4, 8.5)], jacket);
    fill(p, vec![q(0.0, 19.0), q(4.0, 19.0), q(4.4, 8.5), q(0.0, 8.5)], rgb(0xdfe2e8));
    for y in [16.0, 13.0, 10.5] {
        p.circle_filled(q(-1.2, y), 0.55 * s, rgb(0x9aa0a6));
    }
    // A red scarf.
    fill(p, vec![q(-2.6, 19.5), q(2.6, 19.5), q(1.6, 17.5), q(-1.6, 17.5)], rgb(0xe84a3d));
    let arm = |a: Pos2, b: Pos2, col: Color32| {
        p.line_segment([a, b], Stroke::new(2.6 * s, col));
        p.circle_filled(b, 1.6 * s, tone);
    };
    match pose {
        Pose::Wait => {
            // Arms crossed on the chest.
            p.line_segment([q(-4.0, 17.0), q(2.5, 13.5)], Stroke::new(2.6 * s, jacket));
            p.line_segment([q(4.0, 17.0), q(-2.5, 13.5)], Stroke::new(2.6 * s, rgb(0xdfe2e8)));
            p.circle_filled(q(2.8, 13.6), 1.4 * s, tone);
            p.circle_filled(q(-2.8, 13.6), 1.4 * s, tone);
        }
        Pose::Flip => {
            arm(q(-4.0, 17.0), q(-5.5, 10.5), jacket);
            let lift = (time * 6.0).sin() * 3.0;
            let hand = q(7.5, 13.0 + lift);
            arm(q(4.0, 17.0), hand, rgb(0xdfe2e8));
            // The spatula.
            let tip = hand + vec2(5.0 * s, -(2.0 + lift * 0.4) * s);
            p.line_segment([hand, tip], Stroke::new(1.0 * s, rgb(0x6d7075)));
            fill(p, vec![tip + vec2(-0.5 * s, -2.0 * s), tip + vec2(3.5 * s, -2.6 * s), tip + vec2(4.0 * s, 1.0 * s), tip + vec2(0.0, 1.6 * s)], rgb(0xc9ccd4));
        }
        Pose::Taste => {
            arm(q(-4.0, 17.0), q(-5.0, 10.0), jacket);
            let hand = q(2.5, 20.0);
            arm(q(4.0, 17.0), hand, rgb(0xdfe2e8));
            p.line_segment([hand, hand + vec2(-s, -5.0 * s)], Stroke::new(0.9 * s, rgb(0xd6d8de)));
        }
    }
    // The head, its face and the toque.
    let head = q(0.0, 22.5);
    p.circle_filled(head, 3.2 * s, tone);
    if s >= 0.5 {
        let happy = pose == Pose::Taste && (time * 1.3).sin() > 0.0;
        for dx in [-1.2, 1.2] {
            if happy {
                p.add(Shape::line(ell(head + vec2(dx * s, 0.2 * s), 0.6 * s, 0.5 * s, PI, TAU, 4), Stroke::new(0.6 * s, INK)));
            } else {
                p.circle_filled(head + vec2(dx * s, 0.0), 0.45 * s, INK);
            }
        }
        oval(p, head + vec2(-2.0 * s, 1.1 * s), 0.8 * s, 0.45 * s, BLUSH);
        oval(p, head + vec2(2.0 * s, 1.1 * s), 0.8 * s, 0.45 * s, BLUSH);
    }
    let hat = Color32::WHITE;
    fill(p, vec![q(-2.8, 24.5), q(2.8, 24.5), q(2.8, 27.0), q(-2.8, 27.0)], rgb(0xeef0f4));
    for (dx, dy, r) in [(-2.0, 28.2, 2.2), (2.0, 28.2, 2.2), (0.0, 29.6, 2.5)] {
        p.circle_filled(q(dx, dy), r * s, hat);
    }
}

/// Flies circling round `c` (an unfinished bug), their wings beating.
pub(super) fn flies(p: &Painter, c: Pos2, s: f32, time: f32, n: usize, seed: f32) {
    for k in 0..n {
        let a = time * (2.2 + k as f32 * 0.4) + k as f32 * 2.1 + seed;
        let at = c + vec2(a.cos() * 11.0 * s, a.sin() * 5.0 * s - (time * 3.0 + k as f32).sin() * 3.0 * s);
        let beat = (time * 40.0 + k as f32).sin().abs();
        let wing = Color32::from_rgba_unmultiplied(220, 235, 255, 170);
        oval(p, at + vec2(-1.6 * s, -1.6 * s), 1.8 * s, (0.5 + beat) * s, wing);
        oval(p, at + vec2(1.6 * s, -1.6 * s), 1.8 * s, (0.5 + beat) * s, wing);
        p.circle_filled(at, 1.4 * s, rgb(0x1b1c20));
    }
}

/// The grill's flames along its front edge from `a` to `b`, flickering, and smoke over `top`.
pub(super) fn flames(p: &Painter, a: Pos2, b: Pos2, top: Pos2, s: f32, time: f32) {
    for k in 0..6 {
        let at = a.lerp(b, (k as f32 + 0.5) / 6.0);
        let h = (0.65 + 0.35 * (time * 9.0 + k as f32 * 1.7).sin()) * 10.0 * s;
        let w = 3.2 * s;
        let lean = (time * 5.0 + k as f32).sin() * 1.5 * s;
        fill(p, vec![at - vec2(w, 0.0), at + vec2(w, 0.0), at + vec2(lean, -h)], rgb(0xff7a1a));
        fill(p, vec![at - vec2(w * 0.5, 0.0), at + vec2(w * 0.5, 0.0), at + vec2(lean * 0.6, -h * 0.6)], rgb(0xffd66b));
    }
    for k in 0..3 {
        let phase = (time * 0.35 + k as f32 / 3.0).fract();
        let at = top + vec2((time + k as f32 * 2.0).sin() * 5.0 * s, -(4.0 + phase * 30.0) * s);
        let a = ((1.0 - phase) * 90.0) as u8;
        p.circle_filled(at, (3.0 + phase * 5.0) * s, Color32::from_rgba_unmultiplied(200, 200, 205, a));
    }
}

/// A delivery scooter on `at` heading right (`right`) or left on screen: wheels, its body, the
/// rider in a helmet of its box's colour, and the box in the colour of the pull request's checks
/// with its number.
pub(super) fn scooter(p: &Painter, at: Pos2, s: f32, right: bool, box_col: Color32, number: Option<u64>) {
    let d = if right { 1.0 } else { -1.0 };
    let q = |x: f32, y: f32| at + vec2(x * d * s, -y * s);
    oval(p, at + vec2(0.0, 1.0 * s), 14.0 * s, 3.6 * s, Color32::from_black_alpha(60));
    for x in [-9.0, 9.0] {
        p.circle_filled(q(x, 3.5), 3.4 * s, rgb(0x1b1c20));
        p.circle_filled(q(x, 3.5), 1.4 * s, rgb(0x9aa0a6));
    }
    // The body and the front column.
    fill(p, vec![q(-12.0, 8.0), q(5.0, 8.0), q(10.0, 12.0), q(10.0, 5.0), q(-12.0, 5.0)], rgb(0xd6d8de));
    fill(p, vec![q(-12.0, 8.0), q(-2.0, 8.0), q(-2.0, 11.0), q(-11.0, 11.0)], rgb(0xe84a3d));
    p.line_segment([q(8.0, 8.0), q(6.5, 23.0)], Stroke::new(2.0 * s, rgb(0x3a3d45)));
    p.line_segment([q(4.5, 23.5), q(9.5, 23.5)], Stroke::new(1.8 * s, rgb(0x3a3d45)));
    p.circle_filled(q(9.5, 18.0), 1.2 * s, rgb(0xffd66b));
    // The rider.
    fill(p, vec![q(-5.0, 22.0), q(1.5, 22.0), q(2.5, 10.0), q(-4.0, 10.0)], rgb(0x2b2d33));
    p.line_segment([q(0.5, 19.0), q(6.0, 22.5)], Stroke::new(2.2 * s, rgb(0x2b2d33)));
    p.circle_filled(q(6.0, 22.5), 1.2 * s, rgb(0xd9a47a));
    p.circle_filled(q(-1.5, 26.0), 4.0 * s, box_col);
    fill(p, vec![q(-0.5, 27.0), q(3.0, 27.0), q(3.0, 24.5), q(-0.5, 24.5)], rgb(0x16171b));
    // The box behind them.
    let (b0, b1) = (q(-18.0, 30.0), q(-6.0, 13.0));
    let r = Rect::from_two_pos(b0, b1);
    p.rect_filled(r, CornerRadius::same((1.5 * s) as u8), box_col);
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 3.0 * s)), 0.0, theme::shade(box_col, 0.25));
    if let Some(n) = number.filter(|_| s >= 0.75) {
        let ink = if box_col.r() as u32 + box_col.g() as u32 > 360 { rgb(0x2a1d00) } else { Color32::WHITE };
        p.text(r.center() + vec2(0.0, 1.0 * s), Align2::CENTER_CENTER, n.to_string(), FontId::proportional(6.5 * s), ink);
    }
}

/// A bistro table's parasol over `top` in `col`, striped with white, on its pole.
pub(super) fn parasol(p: &Painter, foot: Pos2, s: f32, col: Color32) {
    let top = foot - vec2(0.0, 30.0 * s);
    p.line_segment([foot, top], Stroke::new(1.4 * s, rgb(0x6d7075)));
    let rim = top + vec2(0.0, 7.0 * s);
    let (rx, ry) = (17.0 * s, 8.5 * s);
    for k in 0..8 {
        let a0 = k as f32 / 8.0 * TAU;
        let a1 = (k + 1) as f32 / 8.0 * TAU;
        let mut pts = vec![top - vec2(0.0, 3.0 * s)];
        pts.extend(ell(rim, rx, ry, a0, a1, 3));
        let c = if k % 2 == 0 { col } else { rgb(0xf7f8f8) };
        let lit = (a0 + a1) / 2.0;
        let c = if lit.cos() > 0.3 { theme::shade(c, 0.12) } else { c };
        fill(p, pts, c);
    }
}

/// Faces of a box in one colour: lit, shaded and its top.
pub(super) fn faces(c: Color32) -> Faces {
    Faces { left: c, right: theme::shade(c, 0.18), top: theme::lighten(c, 0.12) }
}
