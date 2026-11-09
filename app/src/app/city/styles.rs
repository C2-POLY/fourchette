//! The finished buildings of the city's district styles (#200); the styles from round the world
//! are in `world`. Each style has three
//! variants of each kind of building and its own colours, all picked from the ticket's seed, so
//! a building looks the same at every launch. The kind still reads by its shape: a Feature's
//! tower is the tallest of its style, a Bug's workshop low and wide, the rest in between.

mod world;

use super::*;
use crate::city::Style;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The colours of the four styles.
mod tone {
    use super::super::Faces;
    use egui::Color32;

    const fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    // The town: brick in several tones, towers in pastel render with a slate curtain wall.
    pub const BRICKS: [Color32; 5] = [rgb(0xa8402e), rgb(0x9c5238), rgb(0xb5653c), rgb(0x7d3426), rgb(0xc27b4f)];
    pub const RENDERS: [(Color32, Color32); 4] = [(rgb(0xefd3d0), rgb(0xd97f71)), (rgb(0xeadbb8), rgb(0xc9a46a)), (rgb(0xcfe0cb), rgb(0x8fb487)), (rgb(0xdcd5ee), rgb(0xa594d4))];
    pub const SLATES: [Color32; 3] = [rgb(0x2f3e56), rgb(0x3b4a5e), rgb(0x2c4a4a)];
    pub const CREAMS: [Color32; 4] = [rgb(0xf1e6cf), rgb(0xe9e2d6), rgb(0xdfe7ea), rgb(0xf3ede4)];
    pub const ROOFS: [Color32; 3] = [rgb(0x5a5f6a), rgb(0xb5523b), rgb(0x6b4f45)];
    pub const AWNINGS: [Color32; 4] = [rgb(0xd9412f), rgb(0x3f8f5a), rgb(0x3f6fb0), rgb(0xe3a13a)];
    // The Mediterranean: white, ochre, terracotta and pale blue render, tile roofs, painted shutters.
    pub const WALLS: [Color32; 5] = [rgb(0xf6f1e8), rgb(0xefd59f), rgb(0xe6aa84), rgb(0xcfe3ee), rgb(0xf2e0c9)];
    pub const TILE: Color32 = rgb(0xc4673f);
    pub const SHUTTERS: [Color32; 3] = [rgb(0x2f6db0), rgb(0x3f8f5a), rgb(0x2a8a8a)];
    pub const DOMES: [Color32; 3] = [rgb(0x2f6db0), rgb(0x3b86c6), rgb(0xf6f1e8)];
    pub const OPENING: Color32 = rgb(0x3a3440);
    pub const WOOD: Color32 = rgb(0x8a5a3a);
    // Glass and steel.
    pub const GLASSES: [(Color32, Color32); 5] = [(rgb(0x5b8fc9), rgb(0x4a78ad)), (rgb(0x4fb3b0), rgb(0x3f9592)), (rgb(0x7d8fa3), rgb(0x667787)), (rgb(0x6a7fd1), rgb(0x5868b0)), (rgb(0x3f5f80), rgb(0x324d68))];
    pub(in super::super) const STEEL: Faces = Faces { left: rgb(0xe8edf1), right: rgb(0xc9d1d8), top: rgb(0xf4f6f8) };
    pub const SOLAR: Color32 = rgb(0x24365a);
    pub const GREEN_ROOF: Color32 = rgb(0x6dbb5a);
    pub const HELIPAD: Color32 = rgb(0xf2c230);
    // Industry: corrugated sheet in several paints, brick, grey roofs.
    pub const SHEETS: [Color32; 5] = [rgb(0x8a8f96), rgb(0xa45a3a), rgb(0x5f7f5a), rgb(0x6b7f99), rgb(0xb0a48a)];
    pub const FACTORY_BRICK: Color32 = rgb(0x9a4a32);
    pub const ROOF: Color32 = rgb(0x6f7378);
    pub const SHED_GLASS: Color32 = rgb(0x4f6f8f);
    pub const SHUTTER: Color32 = rgb(0xa9adb3);
    pub const CONTAINERS: [Color32; 5] = [rgb(0xc0392b), rgb(0x2f6db0), rgb(0x3f8f5a), rgb(0xe3a13a), rgb(0x7d8fa3)];
    pub const BAND: Color32 = rgb(0xd9412f);
    pub const GRAVEL: Color32 = rgb(0xb7b2a6);
}

/// Picks for one building, always the same for its seed (mulberry32).
struct Dice(u32);

impl Dice {
    fn roll(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut h = self.0;
        h = (h ^ (h >> 15)).wrapping_mul(h | 1);
        h ^= h.wrapping_add((h ^ (h >> 7)).wrapping_mul(h | 61));
        (h ^ (h >> 14)) as usize % n
    }

    fn of<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.roll(xs.len())]
    }
}

/// Points along an ellipse round `c`, from angle `from` to `to` (0 to the right, PI/2 down).
fn arc(c: Pos2, (rx, ry): (f32, f32), from: f32, to: f32) -> impl Iterator<Item = Pos2> {
    (0..=10).map(move |k| {
        let a = from + (to - from) * k as f32 / 10.0;
        c + vec2(rx * a.cos(), ry * a.sin())
    })
}

/// The screen radii of a circle of radius `r` tiles lying on the ground.
fn radii(iso: &Iso, r: f32) -> (f32, f32) {
    (iso.px(r * TILE_W * std::f32::consts::FRAC_1_SQRT_2), iso.px(r * TILE_H * std::f32::consts::FRAC_1_SQRT_2))
}

/// A standing cylinder: its lit half, its shaded half and its top.
fn cylinder(p: &Painter, iso: &Iso, (cx, cy): (f32, f32), r: f32, (z0, z1): (f32, f32), f: Faces) {
    let (b, t) = (iso.at(cx, cy, z0), iso.at(cx, cy, z1));
    let (rx, ry) = radii(iso, r);
    let mut left = vec![t - vec2(rx, 0.0), t];
    left.extend(arc(b, (rx, ry), FRAC_PI_2, PI));
    poly(p, left, f.left);
    let mut right = vec![t, t + vec2(rx, 0.0)];
    right.extend(arc(b, (rx, ry), 0.0, FRAC_PI_2));
    poly(p, right, f.right);
    poly(p, arc(t, (rx, ry), 0.0, TAU).collect(), f.top);
}

/// A dome of radius `r` tiles on a roof at z.
fn dome(p: &Painter, iso: &Iso, (cx, cy): (f32, f32), r: f32, z: f32, c: Color32) {
    let b = iso.at(cx, cy, z);
    let (rx, ry) = radii(iso, r);
    let up = (rx, rx * 0.9);
    let mut left: Vec<Pos2> = arc(b, (rx, ry), FRAC_PI_2, PI).collect();
    left.extend(arc(b, up, PI, 1.5 * PI));
    poly(p, left, c);
    let mut right: Vec<Pos2> = arc(b, up, 1.5 * PI, TAU).collect();
    right.extend(arc(b, (rx, ry), 0.0, FRAC_PI_2));
    poly(p, right, theme::shade(c, 0.18));
}

/// A pitched roof over a box at z, `rise` floors high, its ridge along x or along y; `wall` is
/// the box's colours, for the gable end.
#[allow(clippy::too_many_arguments)]
fn gable(p: &Painter, iso: &Iso, (x0, y0, x1, y1): (f32, f32, f32, f32), z: f32, rise: f32, roof: Color32, wall: Faces, along_x: bool) {
    let back = theme::shade(roof, 0.22);
    if along_x {
        let m = (y0 + y1) / 2.0;
        poly(p, vec![iso.at(x0, y0, z), iso.at(x1, y0, z), iso.at(x1, m, z + rise), iso.at(x0, m, z + rise)], back);
        poly(p, vec![iso.at(x1, y0, z), iso.at(x1, y1, z), iso.at(x1, m, z + rise)], wall.right);
        poly(p, vec![iso.at(x0, m, z + rise), iso.at(x1, m, z + rise), iso.at(x1, y1, z), iso.at(x0, y1, z)], roof);
    } else {
        let m = (x0 + x1) / 2.0;
        poly(p, vec![iso.at(x0, y0, z), iso.at(m, y0, z + rise), iso.at(m, y1, z + rise), iso.at(x0, y1, z)], back);
        poly(p, vec![iso.at(x0, y1, z), iso.at(x1, y1, z), iso.at(m, y1, z + rise)], wall.left);
        poly(p, vec![iso.at(m, y0, z + rise), iso.at(x1, y0, z), iso.at(x1, y1, z), iso.at(m, y1, z + rise)], theme::shade(roof, 0.1));
    }
}

/// A hipped roof: four slopes up to a point over the middle.
fn hip(p: &Painter, iso: &Iso, (x0, y0, x1, y1): (f32, f32, f32, f32), z: f32, rise: f32, roof: Color32) {
    let top = iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z + rise);
    let back = theme::shade(roof, 0.22);
    poly(p, vec![iso.at(x0, y0, z), iso.at(x1, y0, z), top], back);
    poly(p, vec![iso.at(x0, y1, z), iso.at(x0, y0, z), top], back);
    poly(p, vec![iso.at(x1, y0, z), iso.at(x1, y1, z), top], theme::shade(roof, 0.12));
    poly(p, vec![iso.at(x0, y1, z), top, iso.at(x1, y1, z)], roof);
}

/// Rows of windows on a face, one row per floor from `from` to `to`, `cols` of them spread
/// from a0 to a1, each from `lo` to `hi` within its floor.
#[allow(clippy::too_many_arguments)]
fn windows(p: &Painter, iso: &Iso, south: bool, face: f32, (a0, a1): (f32, f32), (from, to): (u32, u32), cols: u32, (lo, hi): (f32, f32), fill: Color32, frame: Color32) {
    let step = (a1 - a0) / cols as f32;
    for f in from..to {
        let z = KERB + f as f32;
        for c in 0..cols {
            let a = a0 + step * (c as f32 + 0.2);
            pane(p, iso, south, face, (a, a + step * 0.6), (z + lo, z + hi), fill, frame);
        }
    }
}

/// The ribs of corrugated sheet down a face.
fn ribs(p: &Painter, iso: &Iso, south: bool, face: f32, (a0, a1): (f32, f32), (z0, z1): (f32, f32), c: Color32) {
    let s = Stroke::new(iso.px(0.7), theme::shade(c, 0.22));
    let mut a = a0 + 0.04;
    while a < a1 - 0.02 {
        let pts = on_face(iso, south, face, (a, a), (z0, z1));
        p.line_segment([pts[0], pts[3]], s);
        a += 0.055;
    }
}

/// A roller door: a grey shutter with its slats.
fn roller(p: &Painter, iso: &Iso, south: bool, face: f32, along: (f32, f32), (z0, z1): (f32, f32)) {
    poly(p, on_face(iso, south, face, along, (z0, z1)), tone::SHUTTER);
    let s = Stroke::new(iso.px(0.6), theme::shade(tone::SHUTTER, 0.25));
    let mut z = z0 + 0.1;
    while z < z1 {
        let pts = on_face(iso, south, face, along, (z, z));
        p.line_segment([pts[0], pts[1]], s);
        z += 0.1;
    }
}

/// A window with its two painted shutters open on each side.
fn shuttered(p: &Painter, iso: &Iso, south: bool, face: f32, (a0, a1): (f32, f32), z: (f32, f32), shutter: Color32) {
    let w = (a1 - a0) * 0.45;
    poly(p, on_face(iso, south, face, (a0 - w, a0), z), shutter);
    poly(p, on_face(iso, south, face, (a1, a1 + w), z), shutter);
    poly(p, on_face(iso, south, face, (a0, a1), z), tone::OPENING);
}

/// A finished building of a style on its tile; returns where a flag stands on its roof.
pub(super) fn building(p: &Painter, iso: &Iso, (x, y): (f32, f32), style: Style, kind: Building, floors: u32, seed: u32) -> Pos2 {
    let mut d = Dice(seed);
    let variant = d.roll(3);
    match style {
        Style::Town => town(p, iso, (x, y), kind, floors, variant, &mut d),
        Style::Mediterranean => mediterranean(p, iso, (x, y), kind, floors, variant, &mut d),
        Style::Glass => glass(p, iso, (x, y), kind, floors, variant, &mut d),
        Style::Industrial => industrial(p, iso, (x, y), kind, floors, variant, &mut d),
        _ => world::building(p, iso, (x, y), style, kind, floors, variant, &mut d),
    }
}

fn town(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
    let (lx, ly) = (x1 - x0, y1 - y0);
    match (kind, variant) {
        (Building::Tower, 0) => {
            // Today's tower: a blue glazed shop floor, pastel floors, a slate curtain wall.
            let (wall, roof) = d.of(&tone::RENDERS);
            let slate = d.of(&tone::SLATES);
            cuboid(p, iso, r, z0, z0 + 1.0, paint::BASE);
            pane(p, iso, true, y1, (x0 + lx * 0.08, x0 + lx * 0.66), (z0 + 0.15, z0 + 0.8), paint::SHOP, paint::LINE);
            poly(p, on_face(iso, false, x1, (y0 + ly * 0.62, y0 + ly * 0.8), (z0, z0 + 0.6)), paint::RED);
            cuboid(p, iso, r, z0 + 1.0, z0 + h, Faces { left: wall, right: slate, top: roof });
            windows(p, iso, true, y1, (x0, x1), (1, floors), 2, (0.18, 0.82), paint::PANE, paint::LINE);
            for f in 1..floors {
                let z = z0 + f as f32;
                poly(p, on_face(iso, false, x1, (y0 + ly * 0.08, y0 + ly * 0.92), (z + 0.1, z + 0.9)), paint::CURTAIN);
            }
            parapet(p, iso, r, z0 + h, roof);
            ac(p, iso, x0 + 0.12, y0 + 0.12, z0 + h);
            ac(p, iso, x0 + 0.36, y0 + 0.34, z0 + h);
            iso.at(x0 + 0.47, y0 + 0.47, z0 + h)
        }
        (Building::Tower, 1) => {
            // Set back halfway up, a terrace on the step.
            let (wall, roof) = d.of(&tone::RENDERS);
            let low = (floors / 2).max(2);
            cuboid(p, iso, r, z0, z0 + low as f32, Faces::of(wall));
            windows(p, iso, true, y1, (x0, x1), (0, low), 3, (0.25, 0.8), paint::WINDOW_LEFT, paint::LINE);
            windows(p, iso, false, x1, (y0, y1), (0, low), 3, (0.25, 0.8), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            parapet(p, iso, r, z0 + low as f32, roof);
            let u @ (ux0, uy0, ux1, uy1) = (x0 + 0.12, y0 + 0.1, x1 - 0.1, y1 - 0.14);
            cuboid(p, iso, u, z0 + low as f32, z0 + h, Faces::of(theme::lighten(wall, 0.08)));
            windows(p, iso, true, uy1, (ux0, ux1), (low, floors), 2, (0.2, 0.82), paint::WINDOW_LEFT, paint::LINE);
            windows(p, iso, false, ux1, (uy0, uy1), (low, floors), 2, (0.2, 0.82), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            parapet(p, iso, u, z0 + h, roof);
            ac(p, iso, ux0 + 0.1, uy0 + 0.1, z0 + h);
            iso.at(ux0 + 0.3, uy0 + 0.3, z0 + h)
        }
        (Building::Tower, _) => {
            // A slim tower of brick with an antenna.
            let s @ (sx0, sy0, sx1, sy1) = (x + 0.24, y + 0.24, x + 0.76, y + 0.76);
            let brick = d.of(&tone::BRICKS);
            cuboid(p, iso, s, z0, z0 + h, Faces::of(brick));
            windows(p, iso, true, sy1, (sx0, sx1), (0, floors), 2, (0.22, 0.8), paint::WINDOW_LEFT, paint::LINE);
            windows(p, iso, false, sx1, (sy0, sy1), (0, floors), 2, (0.22, 0.8), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            parapet(p, iso, s, z0 + h, paint::BRICK_ROOF);
            let (ax, ay) = ((sx0 + sx1) / 2.0, (sy0 + sy1) / 2.0);
            p.line_segment([iso.at(ax, ay, z0 + h), iso.at(ax, ay, z0 + h + 1.6)], Stroke::new(iso.px(1.2), paint::WIRE));
            p.circle_filled(iso.at(ax, ay, z0 + h + 1.6), iso.px(1.6), paint::RED);
            iso.at(sx0 + 0.12, sy0 + 0.3, z0 + h)
        }
        (Building::House, 0) => {
            // A brick block: three windows a floor, a red door, skylights.
            let brick = d.of(&tone::BRICKS);
            cuboid(p, iso, r, z0, z0 + h, Faces { left: brick, right: theme::shade(brick, 0.18), top: paint::BRICK_ROOF });
            windows(p, iso, true, y1, (x0, x1), (0, floors), 3, (0.25, 0.78), paint::WINDOW_LEFT, paint::LINE);
            windows(p, iso, false, x1, (y0, y1), (0, floors), 3, (0.25, 0.78), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            pane(p, iso, true, y1, (x0 + lx * 0.4, x0 + lx * 0.6), (z0, z0 + 0.72), paint::DOOR, paint::LINE);
            parapet(p, iso, r, z0 + h, paint::BRICK_ROOF);
            for (sx, sy) in [(0.14, 0.2), (0.4, 0.42)] {
                poly(p, iso.quad(x0 + sx, y0 + sy, x0 + sx + 0.16, y0 + sy + 0.08, z0 + h).to_vec(), paint::LINE);
            }
            iso.at(x0 + 0.5, y0 + 0.5, z0 + h)
        }
        (Building::House, 1) => {
            // A terrace house: two floors under a pitched roof.
            let wall = if d.roll(2) == 0 { d.of(&tone::CREAMS) } else { d.of(&tone::BRICKS) };
            let f = Faces::of(wall);
            let roof = d.of(&tone::ROOFS);
            let walls = floors - 1;
            cuboid(p, iso, r, z0, z0 + walls as f32, f);
            windows(p, iso, true, y1, (x0, x1), (0, walls), 2, (0.25, 0.8), paint::WINDOW_LEFT, paint::LINE);
            windows(p, iso, false, x1, (y0, y1), (0, walls), 2, (0.25, 0.8), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            pane(p, iso, true, y1, (x0 + lx * 0.42, x0 + lx * 0.58), (z0, z0 + 0.7), d.of(&tone::AWNINGS), paint::LINE);
            gable(p, iso, r, z0 + walls as f32, 0.95, roof, f, d.roll(2) == 0);
            iso.at(x0 + 0.35, y0 + 0.35, z0 + h)
        }
        (Building::House, _) => {
            // A corner shop: a glazed shop floor under a striped awning, flats above.
            let brick = d.of(&tone::BRICKS);
            let awning = d.of(&tone::AWNINGS);
            cuboid(p, iso, r, z0, z0 + h, Faces { left: brick, right: theme::shade(brick, 0.18), top: paint::BRICK_ROOF });
            pane(p, iso, true, y1, (x0 + 0.05, x1 - 0.05), (z0 + 0.05, z0 + 0.75), paint::SHOP, paint::LINE);
            pane(p, iso, false, x1, (y0 + 0.1, y1 - 0.08), (z0 + 0.05, z0 + 0.75), paint::SHOP, paint::SOFT_LINE);
            windows(p, iso, true, y1, (x0, x1), (1, floors), 3, (0.25, 0.78), paint::WINDOW_LEFT, paint::LINE);
            windows(p, iso, false, x1, (y0, y1), (1, floors), 3, (0.25, 0.78), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            let n = 6;
            for i in 0..n {
                let (a, b) = (x0 + lx * i as f32 / n as f32, x0 + lx * (i + 1) as f32 / n as f32);
                let c = if i % 2 == 0 { awning } else { paint::LINE };
                poly(p, vec![iso.at(a, y1, z0 + 0.95), iso.at(b, y1, z0 + 0.95), iso.at(b, y1 + 0.13, z0 + 0.7), iso.at(a, y1 + 0.13, z0 + 0.7)], c);
            }
            parapet(p, iso, r, z0 + h, paint::BRICK_ROOF);
            ac(p, iso, x0 + 0.3, y0 + 0.25, z0 + h);
            iso.at(x0 + 0.55, y0 + 0.5, z0 + h)
        }
        (Building::Workshop, 0) => {
            // Today's white workshop.
            let w @ (wx0, wy0, wx1, wy1) = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            cuboid(p, iso, w, z0, z0 + h, paint::WHITE);
            windows(p, iso, true, wy1, (wx0, wx1), (0, floors), 3, (0.25, 0.78), paint::WINDOW_LEFT, paint::LINE);
            windows(p, iso, false, wx1, (wy0, wy0 + (wy1 - wy0) * 0.66), (0, floors), 2, (0.25, 0.78), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            poly(p, on_face(iso, false, wx1, (wy0 + (wy1 - wy0) * 0.72, wy0 + (wy1 - wy0) * 0.86), (z0, z0 + 0.7)), paint::DOOR);
            parapet(p, iso, w, z0 + h, paint::WHITE_ROOF);
            ac(p, iso, wx0 + 0.36, wy0 + 0.3, z0 + h);
            iso.at(wx0 + 0.6, wy0 + 0.55, z0 + h)
        }
        (Building::Workshop, 1) => {
            // A garage: two roller doors, a strip of windows above.
            let w @ (wx0, wy0, wx1, wy1) = (x + 0.1, y + 0.12, x + 0.9, y + 0.88);
            let wall = d.of(&tone::CREAMS);
            cuboid(p, iso, w, z0, z0 + h, Faces::of(wall));
            let lw = wx1 - wx0;
            roller(p, iso, true, wy1, (wx0 + lw * 0.08, wx0 + lw * 0.44), (z0, z0 + 1.1));
            roller(p, iso, true, wy1, (wx0 + lw * 0.54, wx0 + lw * 0.9), (z0, z0 + 1.1));
            windows(p, iso, false, wx1, (wy0, wy1), (1, floors), 3, (0.3, 0.75), paint::WINDOW_RIGHT, paint::SOFT_LINE);
            parapet(p, iso, w, z0 + h, paint::WHITE_ROOF);
            iso.at(wx0 + 0.5, wy0 + 0.5, z0 + h)
        }
        (Building::Workshop, _) => {
            // A shed under a pitched roof.
            let w @ (wx0, wy0, wx1, wy1) = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            let f = Faces::of(d.of(&tone::CREAMS));
            cuboid(p, iso, w, z0, z0 + 1.2, f);
            windows(p, iso, true, wy1, (wx0, wx1), (0, 1), 4, (0.3, 0.8), paint::WINDOW_LEFT, paint::LINE);
            pane(p, iso, false, wx1, (wy0 + 0.2, wy1 - 0.2), (z0, z0 + 0.9), d.of(&tone::AWNINGS), paint::SOFT_LINE);
            gable(p, iso, w, z0 + 1.2, h - 1.2 + 0.3, d.of(&tone::ROOFS), f, false);
            iso.at(wx0 + 0.38, wy0 + 0.38, z0 + h)
        }
    }
}

fn mediterranean(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    let wall = d.of(&tone::WALLS);
    let f = Faces { left: wall, right: theme::shade(wall, 0.14), top: theme::lighten(wall, 0.1) };
    let shutter = d.of(&tone::SHUTTERS);
    // Shuttered windows on both visible faces, `cols` of them, from floor `from` to `to`.
    let fronts = |(x0, y0, x1, y1): (f32, f32, f32, f32), (from, to): (u32, u32), cols: u32| {
        for fl in from..to {
            let z = z0 + fl as f32;
            for c in 0..cols {
                let t = (c as f32 + 0.5) / cols as f32;
                let (ax, ay) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
                shuttered(p, iso, true, y1, (ax - 0.045, ax + 0.045), (z + 0.25, z + 0.78), shutter);
                shuttered(p, iso, false, x1, (ay - 0.045, ay + 0.045), (z + 0.25, z + 0.78), shutter);
            }
        }
    };
    let arch_door = |south: bool, face: f32, a: f32| poly(p, on_face(iso, south, face, (a - 0.06, a + 0.06), (z0, z0 + 0.75)), tone::WOOD);
    match (kind, variant) {
        (Building::Tower, 0) => {
            // A bell tower: a house at its foot, a tall square tower, open arches and a tile roof.
            let base = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            cuboid(p, iso, base, z0, z0 + 2.0, f);
            fronts(base, (0, 2), 2);
            hip(p, iso, base, z0 + 2.0, 0.6, tone::TILE);
            let t @ (tx0, ty0, tx1, ty1) = (x + 0.3, y + 0.3, x + 0.7, y + 0.7);
            cuboid(p, iso, t, z0 + 2.0, z0 + h, f);
            for fl in 3..floors.saturating_sub(1) {
                let z = z0 + fl as f32;
                poly(p, on_face(iso, true, ty1, (tx0 + 0.15, tx1 - 0.15), (z + 0.3, z + 0.7)), tone::OPENING);
            }
            let top = z0 + h - 1.0;
            for (south, face, (a0, a1)) in [(true, ty1, (tx0, tx1)), (false, tx1, (ty0, ty1))] {
                poly(p, on_face(iso, south, face, (a0 + 0.08, a1 - 0.08), (top + 0.15, top + 0.85)), tone::OPENING);
            }
            p.circle_filled(iso.at(tx1, (ty0 + ty1) / 2.0, top + 0.45), iso.px(2.4), paint::YELLOW);
            hip(p, iso, t, z0 + h, 0.8, tone::TILE);
            iso.at((tx0 + tx1) / 2.0, (ty0 + ty1) / 2.0, z0 + h + 0.8)
        }
        (Building::Tower, 1) => {
            // A white church: a tall nave with shutters, a drum and a blue dome.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            let white = Faces { left: tone::WALLS[0], right: theme::shade(tone::WALLS[0], 0.14), top: tone::WALLS[0] };
            let body = floors.saturating_sub(2).max(2);
            cuboid(p, iso, r, z0, z0 + body as f32, white);
            fronts(r, (0, body), 2);
            arch_door(true, y1, (x0 + x1) / 2.0);
            parapet(p, iso, r, z0 + body as f32, theme::shade(tone::WALLS[0], 0.08));
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            cylinder(p, iso, (cx, cy), 0.24, (z0 + body as f32, z0 + body as f32 + 0.8), white);
            dome(p, iso, (cx, cy), 0.26, z0 + body as f32 + 0.8, d.of(&tone::DOMES[..2]));
            iso.at(cx, cy, z0 + h + 0.6)
        }
        (Building::Tower, _) => {
            // A hill village: houses stepping up, each roof the next one's terrace.
            let tiers = 3u32;
            let per = (floors as f32 / tiers as f32).max(1.0);
            let mut last = iso.at(x + 0.5, y + 0.5, z0 + h);
            for t in 0..tiers {
                let inset = t as f32 * 0.11;
                let r @ (x0, y0, x1, y1) = (x + 0.12 + inset, y + 0.12 + inset * 0.6, x + 0.88 - inset * 0.4, y + 0.88 - inset);
                let (za, zb) = (z0 + per * t as f32, z0 + per * (t + 1) as f32);
                let wall = d.of(&tone::WALLS);
                let tf = Faces { left: wall, right: theme::shade(wall, 0.14), top: theme::lighten(wall, 0.1) };
                cuboid(p, iso, r, za, zb, tf);
                for fl in (za - z0).round() as u32..(zb - z0).round() as u32 {
                    let z = z0 + fl as f32;
                    shuttered(p, iso, true, y1, ((x0 + x1) / 2.0 - 0.045, (x0 + x1) / 2.0 + 0.045), (z + 0.25, z + 0.75), shutter);
                    shuttered(p, iso, false, x1, ((y0 + y1) / 2.0 - 0.045, (y0 + y1) / 2.0 + 0.045), (z + 0.25, z + 0.75), shutter);
                }
                if t + 1 == tiers {
                    hip(p, iso, r, zb, 0.55, tone::TILE);
                    last = iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, zb + 0.55);
                } else {
                    parapet(p, iso, r, zb, theme::shade(wall, 0.06));
                }
            }
            last
        }
        (Building::House, 0) => {
            // Two floors of render under a hipped tile roof.
            let r @ (x0, y0, _, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + 2.0, f);
            fronts(r, (0, 2), 2);
            arch_door(true, y1, x0 + 0.5);
            hip(p, iso, r, z0 + 2.0, h - 2.0 + 0.1, tone::TILE);
            iso.at(x0 + 0.35, y0 + 0.35, z0 + h)
        }
        (Building::House, 1) => {
            // A flat roof terrace behind a low wall, the stairway's little hut, a pergola.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h, f);
            fronts(r, (0, floors), 2);
            arch_door(false, x1, y0 + 0.5);
            parapet(p, iso, r, z0 + h, theme::shade(wall, 0.08));
            cuboid(p, iso, (x0 + 0.08, y0 + 0.08, x0 + 0.3, y0 + 0.28), z0 + h, z0 + h + 0.7, f);
            let beam = Stroke::new(iso.px(1.4), tone::WOOD);
            for (px, py) in [(x0 + 0.38, y0 + 0.38), (x1 - 0.1, y0 + 0.38), (x1 - 0.1, y1 - 0.1), (x0 + 0.38, y1 - 0.1)] {
                p.line_segment([iso.at(px, py, z0 + h), iso.at(px, py, z0 + h + 0.7)], beam);
            }
            for k in 0..4 {
                let t = y0 + 0.38 + (y1 - y0 - 0.48) * k as f32 / 3.0;
                p.line_segment([iso.at(x0 + 0.38, t, z0 + h + 0.7), iso.at(x1 - 0.1, t, z0 + h + 0.7)], beam);
            }
            p.circle_filled(iso.at(x1 - 0.12, y1 - 0.12, z0 + h + 0.15), iso.px(3.0), paint::ROUND.1);
            iso.at(x0 + 0.19, y0 + 0.18, z0 + h + 0.7)
        }
        (Building::House, _) => {
            // A main house and a lower wing, both under tile roofs.
            let main @ (mx0, my0, mx1, my1) = (x + 0.12, y + 0.15, x + 0.56, y + 0.85);
            cuboid(p, iso, main, z0, z0 + h - 0.6, f);
            for fl in 0..floors - 1 {
                let z = z0 + fl as f32;
                shuttered(p, iso, true, my1, ((mx0 + mx1) / 2.0 - 0.045, (mx0 + mx1) / 2.0 + 0.045), (z + 0.25, z + 0.75), shutter);
            }
            gable(p, iso, main, z0 + h - 0.6, 0.6, tone::TILE, f, false);
            let wall2 = d.of(&tone::WALLS);
            let f2 = Faces { left: wall2, right: theme::shade(wall2, 0.14), top: wall2 };
            let wing @ (wx0, wy0, wx1, wy1) = (x + 0.56, y + 0.3, x + 0.88, y + 0.85);
            cuboid(p, iso, wing, z0, z0 + 1.2, f2);
            shuttered(p, iso, true, wy1, ((wx0 + wx1) / 2.0 - 0.045, (wx0 + wx1) / 2.0 + 0.045), (z0 + 0.3, z0 + 0.85), shutter);
            arch_door(false, wx1, (wy0 + wy1) / 2.0);
            gable(p, iso, wing, z0 + 1.2, 0.45, tone::TILE, f2, true);
            iso.at((mx0 + mx1) / 2.0, (my0 + my1) / 2.0, z0 + h)
        }
        (Building::Workshop, 0) => {
            // An arcade: arches along the ground floor under a low tile roof.
            let r @ (x0, y0, x1, y1) = (x + 0.1, y + 0.12, x + 0.9, y + 0.88);
            cuboid(p, iso, r, z0, z0 + h - 0.4, f);
            let n = 4;
            for i in 0..n {
                let a = x0 + (x1 - x0) * (i as f32 + 0.2) / n as f32;
                poly(p, on_face(iso, true, y1, (a, a + (x1 - x0) * 0.6 / n as f32), (z0, z0 + 0.8)), tone::OPENING);
            }
            for i in 0..3 {
                let a = y0 + (y1 - y0) * (i as f32 + 0.5) / 3.0;
                shuttered(p, iso, false, x1, (a - 0.04, a + 0.04), (z0 + 0.25, z0 + 0.75), shutter);
            }
            hip(p, iso, r, z0 + h - 0.4, 0.45, tone::TILE);
            iso.at(x0 + 0.4, y0 + 0.38, z0 + h)
        }
        (Building::Workshop, 1) => {
            // A courtyard wall, its wooden gate, and a lemon tree over it.
            let (x0, y0, x1, y1) = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            let yard = (x0, y0 + 0.36, x1, y1);
            let house = (x0, y0, x1, y0 + 0.36);
            cuboid(p, iso, house, z0, z0 + h, f);
            shuttered(p, iso, false, x1, (y0 + 0.14, y0 + 0.22), (z0 + 1.25, z0 + 1.75), shutter);
            parapet(p, iso, house, z0 + h, theme::shade(wall, 0.08));
            poly(p, iso.quad(yard.0, yard.1, yard.2, yard.3, z0).to_vec(), theme::lighten(paint::DIRT, 0.35));
            p.circle_filled(iso.at(x0 + 0.4, y0 + 0.6, z0 + 1.0), iso.px(9.0), paint::ROUND.0);
            for k in 0..5 {
                p.circle_filled(iso.at(x0 + 0.3 + k as f32 * 0.05, y0 + 0.6, z0 + 1.0 + (k % 2) as f32 * 0.25), iso.px(1.6), paint::YELLOW);
            }
            cuboid(p, iso, (x0, y1 - 0.06, x1, y1), z0, z0 + 0.7, f);
            cuboid(p, iso, (x1 - 0.06, y0 + 0.36, x1, y1), z0, z0 + 0.7, f);
            poly(p, on_face(iso, true, y1, (x0 + 0.3, x0 + 0.48), (z0, z0 + 0.6)), tone::WOOD);
            iso.at(x0 + 0.38, y0 + 0.18, z0 + h)
        }
        (Building::Workshop, _) => {
            // A bakery: a long house under a tile roof, its bread oven's chimney.
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.15, x + 0.88, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h - 0.5, f);
            for i in 0..3 {
                let a = x0 + (x1 - x0) * (i as f32 + 0.5) / 3.0;
                shuttered(p, iso, true, y1, (a - 0.045, a + 0.045), (z0 + 0.3, z0 + 0.85), shutter);
            }
            arch_door(false, x1, (y0 + y1) / 2.0);
            gable(p, iso, r, z0 + h - 0.5, 0.5, tone::TILE, f, true);
            cuboid(p, iso, (x0 + 0.12, y0 + 0.1, x0 + 0.24, y0 + 0.22), z0 + h - 0.5, z0 + h + 0.6, f);
            iso.at(x0 + 0.5, (y0 + y1) / 2.0, z0 + h)
        }
    }
}

fn glass(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    let (gl, gr) = d.of(&tone::GLASSES);
    let gf = Faces { left: gl, right: gr, top: tone::STEEL.top };
    // A curtain wall's floor lines and a mullion or two, on both visible faces.
    let grid = |(x0, y0, x1, y1): (f32, f32, f32, f32), (za, zb): (f32, f32), mullions: u32| {
        let s = Stroke::new(iso.px(0.8), Color32::from_white_alpha(150));
        let mut z = za + 1.0;
        while z < zb - 0.01 {
            p.line_segment([iso.at(x0, y1, z), iso.at(x1, y1, z)], s);
            p.line_segment([iso.at(x1, y0, z), iso.at(x1, y1, z)], s);
            z += 1.0;
        }
        for m in 1..=mullions {
            let t = m as f32 / (mullions + 1) as f32;
            let (mx, my) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            p.line_segment([iso.at(mx, y1, za), iso.at(mx, y1, zb)], s);
            p.line_segment([iso.at(x1, my, za), iso.at(x1, my, zb)], s);
        }
    };
    match (kind, variant) {
        (Building::Tower, 0) => {
            // A glass tower, its roof a helipad or plant rooms.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h, gf);
            grid(r, (z0, z0 + h), 2);
            cuboid(p, iso, r, z0 + h, z0 + h + 0.15, tone::STEEL);
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            if d.roll(2) == 0 {
                let ring = arc(iso.at(cx, cy, z0 + h + 0.15), radii(iso, 0.26), 0.0, TAU).collect::<Vec<_>>();
                p.add(Shape::closed_line(ring, Stroke::new(iso.px(1.4), tone::HELIPAD)));
                let s = Stroke::new(iso.px(1.4), paint::LINE);
                let z = z0 + h + 0.15;
                p.line_segment([iso.at(cx - 0.08, cy - 0.1, z), iso.at(cx - 0.08, cy + 0.1, z)], s);
                p.line_segment([iso.at(cx + 0.08, cy - 0.1, z), iso.at(cx + 0.08, cy + 0.1, z)], s);
                p.line_segment([iso.at(cx - 0.08, cy, z), iso.at(cx + 0.08, cy, z)], s);
            } else {
                cuboid(p, iso, (x0 + 0.1, y0 + 0.1, x0 + 0.4, y0 + 0.35), z0 + h + 0.15, z0 + h + 0.6, tone::STEEL);
                ac(p, iso, x1 - 0.25, y1 - 0.25, z0 + h + 0.15);
            }
            iso.at(cx, cy, z0 + h + 0.15)
        }
        (Building::Tower, 1) => {
            // Three tiers of glass, each set back from the one below.
            let tiers = [(0.0, 0.5), (0.08, 0.8), (0.16, 1.0)];
            let mut last = iso.at(x + 0.5, y + 0.5, z0 + h);
            let mut za = z0;
            for (inset, upto) in tiers {
                let r @ (x0, y0, x1, y1) = (x + 0.12 + inset, y + 0.12 + inset, x + 0.88 - inset, y + 0.88 - inset);
                let zb = z0 + (h * upto).round().max(za - z0 + 1.0);
                cuboid(p, iso, r, za, zb, gf);
                grid(r, (za, zb), 1);
                cuboid(p, iso, r, zb, zb + 0.1, tone::STEEL);
                last = iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, zb + 0.1);
                za = zb + 0.1;
            }
            last
        }
        (Building::Tower, _) => {
            // A white frame of fins over the glass, a spire.
            let r @ (x0, y0, x1, y1) = (x + 0.18, y + 0.18, x + 0.82, y + 0.82);
            cuboid(p, iso, r, z0, z0 + h, gf);
            grid(r, (z0, z0 + h), 0);
            for k in 0..5 {
                let t = k as f32 / 4.0;
                let (fx, fy) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
                cuboid(p, iso, (fx - 0.015, y1, fx + 0.015, y1 + 0.04), z0, z0 + h, tone::STEEL);
                cuboid(p, iso, (x1, fy - 0.015, x1 + 0.04, fy + 0.015), z0, z0 + h, tone::STEEL);
            }
            cuboid(p, iso, r, z0 + h, z0 + h + 0.12, tone::STEEL);
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            p.line_segment([iso.at(cx, cy, z0 + h), iso.at(cx, cy, z0 + h + 2.2)], Stroke::new(iso.px(1.6), tone::STEEL.right));
            iso.at(x0 + 0.15, y0 + 0.2, z0 + h + 0.12)
        }
        (Building::House, 0) => {
            // A glass box, a white slab between each floor.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            for f in 0..floors {
                let z = z0 + f as f32;
                cuboid(p, iso, r, z, z + 0.85, gf);
                cuboid(p, iso, (x0 - 0.03, y0 - 0.03, x1 + 0.03, y1 + 0.03), z + 0.85, z + 1.0, tone::STEEL);
            }
            grid(r, (z0, z0 + 0.85), 2);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::House, 1) => {
            // White concrete with long ribbon windows.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h, tone::STEEL);
            for f in 0..floors {
                let z = z0 + f as f32;
                poly(p, on_face(iso, true, y1, (x0 + 0.05, x1 - 0.05), (z + 0.3, z + 0.72)), gl);
                poly(p, on_face(iso, false, x1, (y0 + 0.05, y1 - 0.05), (z + 0.3, z + 0.72)), gr);
            }
            parapet(p, iso, r, z0 + h, tone::STEEL.right);
            ac(p, iso, x0 + 0.2, y0 + 0.2, z0 + h);
            iso.at(x0 + 0.45, y0 + 0.45, z0 + h)
        }
        (Building::House, _) => {
            // Two floors of glass, a white top floor cantilevered over them.
            let low = (x + 0.26, y + 0.22, x + 0.74, y + 0.74);
            cuboid(p, iso, low, z0, z0 + h - 1.0, gf);
            grid(low, (z0, z0 + h - 1.0), 1);
            let top @ (tx0, ty0, tx1, ty1) = (x + 0.12, y + 0.14, x + 0.88, y + 0.88);
            cuboid(p, iso, top, z0 + h - 1.0, z0 + h, tone::STEEL);
            poly(p, on_face(iso, true, ty1, (tx0 + 0.06, tx1 - 0.06), (z0 + h - 0.75, z0 + h - 0.25)), gl);
            poly(p, on_face(iso, false, tx1, (ty0 + 0.06, ty1 - 0.06), (z0 + h - 0.75, z0 + h - 0.25)), gr);
            iso.at((tx0 + tx1) / 2.0, (ty0 + ty1) / 2.0, z0 + h)
        }
        (Building::Workshop, 0) => {
            // A glass pavilion under a thin overhanging roof.
            let r @ (x0, y0, x1, y1) = (x + 0.16, y + 0.16, x + 0.84, y + 0.84);
            cuboid(p, iso, r, z0, z0 + h - 0.3, gf);
            grid(r, (z0, z0 + h - 0.3), 3);
            cuboid(p, iso, (x0 - 0.06, y0 - 0.06, x1 + 0.06, y1 + 0.06), z0 + h - 0.3, z0 + h - 0.15, tone::STEEL);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h - 0.15)
        }
        (Building::Workshop, 1) => {
            // A low white box with a green roof.
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            cuboid(p, iso, r, z0, z0 + h, tone::STEEL);
            poly(p, on_face(iso, true, y1, (x0 + 0.06, x1 - 0.06), (z0 + 0.2, z0 + 1.4)), gl);
            poly(p, on_face(iso, false, x1, (y0 + 0.06, y0 + 0.45), (z0 + 0.2, z0 + 1.4)), gr);
            parapet(p, iso, r, z0 + h, tone::GREEN_ROOF);
            for (bx, by) in [(0.2, 0.25), (0.5, 0.2), (0.35, 0.5)] {
                p.circle_filled(iso.at(x0 + bx, y0 + by, z0 + h + 0.15), iso.px(3.0), paint::ROUND.1);
            }
            iso.at(x0 + 0.6, y0 + 0.55, z0 + h)
        }
        (Building::Workshop, _) => {
            // A dark box, its roof covered in solar panels.
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            let dark = Faces { left: gr, right: theme::shade(gr, 0.2), top: tone::STEEL.right };
            cuboid(p, iso, r, z0, z0 + h, dark);
            grid(r, (z0, z0 + h), 3);
            parapet(p, iso, r, z0 + h, tone::STEEL.right);
            for i in 0..3 {
                for j in 0..2 {
                    let (px, py) = (x0 + 0.08 + i as f32 * 0.22, y0 + 0.1 + j as f32 * 0.3);
                    poly(p, vec![iso.at(px, py, z0 + h + 0.25), iso.at(px + 0.18, py, z0 + h + 0.25), iso.at(px + 0.18, py + 0.22, z0 + h + 0.02), iso.at(px, py + 0.22, z0 + h + 0.02)], tone::SOLAR);
                }
            }
            iso.at(x1 - 0.12, y1 - 0.12, z0 + h)
        }
    }
}

fn industrial(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    let sheet = d.of(&tone::SHEETS);
    let sf = Faces::of(sheet);
    let brick = Faces { left: tone::FACTORY_BRICK, right: theme::shade(tone::FACTORY_BRICK, 0.18), top: tone::ROOF };
    // A box of corrugated sheet.
    let shed = |(x0, y0, x1, y1): (f32, f32, f32, f32), (za, zb): (f32, f32), c: Color32| {
        cuboid(p, iso, (x0, y0, x1, y1), za, zb, Faces::of(c));
        ribs(p, iso, true, y1, (x0, x1), (za, zb), c);
        ribs(p, iso, false, x1, (y0, y1), (za, zb), theme::shade(c, 0.16));
    };
    // A chimney with red and white bands round its top.
    let chimney = |(cx, cy): (f32, f32), r: f32, top: f32| {
        cylinder(p, iso, (cx, cy), r, (z0, top - 1.0), brick);
        for k in 0..4 {
            let c = if k % 2 == 0 { tone::BAND } else { paint::LINE };
            let z = top - 1.0 + k as f32 * 0.25;
            cylinder(p, iso, (cx, cy), r, (z, z + 0.25), Faces::of(c));
        }
    };
    match (kind, variant) {
        (Building::Tower, 0) => {
            // A factory hall and its tall chimney.
            let hall @ (hx0, _, _, hy1) = (x + 0.12, y + 0.3, x + 0.88, y + 0.88);
            chimney((x + 0.28, y + 0.2), 0.09, z0 + h + 0.6);
            let hh = (h * 0.4).max(1.6);
            shed(hall, (z0, z0 + hh), sheet);
            roller(p, iso, true, hy1, (hx0 + 0.3, hx0 + 0.55), (z0, z0 + 1.1));
            parapet(p, iso, hall, z0 + hh, tone::ROOF);
            iso.at(x + 0.28, y + 0.2, z0 + h + 0.6)
        }
        (Building::Tower, 1) => {
            // A row of silos, a conveyor up to their tops.
            let top = z0 + h;
            let c = d.of(&[paint::LINE, tone::SHEETS[0], tone::SHEETS[4]]);
            let silo = Faces::of(c);
            for (sx, sy) in [(x + 0.3, y + 0.3), (x + 0.62, y + 0.42), (x + 0.42, y + 0.7)] {
                cylinder(p, iso, (sx, sy), 0.17, (z0, top), silo);
                dome(p, iso, (sx, sy), 0.17, top, theme::shade(c, 0.1));
                let s = Stroke::new(iso.px(0.7), theme::shade(c, 0.25));
                let (rx, _) = radii(iso, 0.17);
                for k in 1..floors {
                    let a = iso.at(sx, sy, z0 + k as f32);
                    p.line_segment([a - vec2(rx, 0.0), a + vec2(rx, 0.0)], s);
                }
            }
            p.line_segment([iso.at(x + 0.85, y + 0.85, z0), iso.at(x + 0.62, y + 0.42, top)], Stroke::new(iso.px(2.0), paint::WEIGHT));
            iso.at(x + 0.3, y + 0.3, top + 0.2)
        }
        (Building::Tower, _) => {
            // A brick mill, a water tank on its roof.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h - 1.0, brick);
            windows(p, iso, true, y1, (x0, x1), (0, floors - 1), 3, (0.2, 0.75), tone::SHED_GLASS, paint::LINE);
            windows(p, iso, false, x1, (y0, y1), (0, floors - 1), 3, (0.2, 0.75), tone::SHED_GLASS, paint::SOFT_LINE);
            parapet(p, iso, r, z0 + h - 1.0, tone::ROOF);
            let (cx, cy) = (x0 + 0.38, y0 + 0.38);
            let legs = Stroke::new(iso.px(1.2), paint::WIRE);
            for (dx, dy) in [(-0.12, -0.12), (0.12, -0.12), (0.12, 0.12), (-0.12, 0.12)] {
                p.line_segment([iso.at(cx + dx, cy + dy, z0 + h - 1.0), iso.at(cx + dx, cy + dy, z0 + h - 0.5)], legs);
            }
            cylinder(p, iso, (cx, cy), 0.17, (z0 + h - 0.5, z0 + h + 0.2), Faces::of(tone::WOOD));
            hip(p, iso, (cx - 0.17, cy - 0.17, cx + 0.17, cy + 0.17), z0 + h + 0.2, 0.3, tone::ROOF);
            iso.at(cx, cy, z0 + h + 0.5)
        }
        (Building::House, 0) => {
            // A brick warehouse, a loading door on its side, a water tank on legs.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h, brick);
            windows(p, iso, true, y1, (x0, x1), (0, floors), 3, (0.25, 0.75), tone::SHED_GLASS, paint::LINE);
            for f in 0..floors {
                let z = z0 + f as f32;
                poly(p, on_face(iso, false, x1, (y0 + 0.25, y0 + 0.45), (z + 0.1, z + 0.85)), tone::WOOD);
            }
            parapet(p, iso, r, z0 + h, tone::ROOF);
            iso.at(x0 + 0.5, y0 + 0.5, z0 + h)
        }
        (Building::House, 1) => {
            // A sheet block, its stairs zigzagging up the outside.
            let r @ (x0, _, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            shed(r, (z0, z0 + h), sheet);
            parapet(p, iso, r, z0 + h, tone::ROOF);
            let s = Stroke::new(iso.px(1.4), paint::WIRE);
            for f in 0..floors {
                let z = z0 + f as f32;
                let (a, b) = if f % 2 == 0 { (x0 + 0.08, x1 - 0.12) } else { (x1 - 0.12, x0 + 0.08) };
                p.line_segment([iso.at(a, y1 + 0.04, z), iso.at(b, y1 + 0.04, z + 1.0)], s);
                p.line_segment([iso.at(x0 + 0.06, y1 + 0.04, z + 1.0), iso.at(x1 - 0.06, y1 + 0.04, z + 1.0)], s);
            }
            pane(p, iso, true, y1, (x1 - 0.2, x1 - 0.08), (z0, z0 + 0.7), tone::BAND, paint::LINE);
            iso.at(x0 + 0.35, y + 0.4, z0 + h)
        }
        (Building::House, _) => {
            // An office of sheet and its silo.
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.15, x + 0.6, y + 0.85);
            shed(r, (z0, z0 + h), sheet);
            windows(p, iso, true, y1, (x0, x1), (0, floors), 2, (0.3, 0.75), tone::SHED_GLASS, paint::LINE);
            parapet(p, iso, r, z0 + h, tone::ROOF);
            cylinder(p, iso, (x + 0.75, y + 0.5), 0.13, (z0, z0 + h + 0.4), Faces::of(paint::LINE));
            dome(p, iso, (x + 0.75, y + 0.5), 0.13, z0 + h + 0.4, theme::shade(paint::LINE, 0.1));
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, 0) => {
            // A shed with a saw-tooth roof, its glass facing the light.
            let r @ (x0, y0, x1, y1) = (x + 0.1, y + 0.12, x + 0.9, y + 0.88);
            let wall = h - 0.6;
            shed(r, (z0, z0 + wall), sheet);
            roller(p, iso, true, y1, (x0 + 0.1, x0 + 0.35), (z0, z0 + 0.95));
            let n = 3;
            let w = (x1 - x0) / n as f32;
            let z = z0 + wall;
            for i in 0..n {
                let (a, b) = (x0 + w * i as f32, x0 + w * (i + 1) as f32);
                poly(p, vec![iso.at(a, y0, z), iso.at(b, y0, z + 0.6), iso.at(b, y1, z + 0.6), iso.at(a, y1, z)], tone::ROOF);
                poly(p, vec![iso.at(a, y1, z), iso.at(b, y1, z), iso.at(b, y1, z + 0.6)], sf.left);
                poly(p, vec![iso.at(b, y0, z), iso.at(b, y1, z), iso.at(b, y1, z + 0.6), iso.at(b, y0, z + 0.6)], tone::SHED_GLASS);
            }
            iso.at(x0 + w * 1.5, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, 1) => {
            // A hangar of sheet, a big roller door, a low pitched roof.
            let r @ (x0, _, x1, y1) = (x + 0.1, y + 0.12, x + 0.9, y + 0.88);
            shed(r, (z0, z0 + h - 0.5), sheet);
            roller(p, iso, true, y1, (x0 + 0.15, x1 - 0.15), (z0, z0 + 1.2));
            gable(p, iso, r, z0 + h - 0.5, 0.5, tone::ROOF, sf, false);
            iso.at((x0 + x1) / 2.0, y + 0.5, z0 + h)
        }
        (Building::Workshop, _) => {
            // A depot: brick, two loading bays and a container waiting by them.
            let r @ (x0, y0, x1, y1) = (x + 0.1, y + 0.12, x + 0.9, y + 0.7);
            cuboid(p, iso, r, z0, z0 + h, brick);
            roller(p, iso, true, y1, (x0 + 0.08, x0 + 0.32), (z0, z0 + 1.0));
            roller(p, iso, true, y1, (x0 + 0.42, x0 + 0.66), (z0, z0 + 1.0));
            windows(p, iso, false, x1, (y0, y1), (0, floors), 2, (0.3, 0.75), tone::SHED_GLASS, paint::SOFT_LINE);
            parapet(p, iso, r, z0 + h, tone::ROOF);
            let c = d.of(&tone::CONTAINERS);
            shed((x0 + 0.05, y1 + 0.04, x0 + 0.6, y1 + 0.2), (z0, z0 + 0.65), c);
            iso.at(x0 + 0.5, y0 + 0.3, z0 + h)
        }
    }
}

/// A free tile of a district, in its style: the town's fenced garden, an olive grove behind a
/// low white wall, a trimmed lawn, or a yard of containers behind wire.
pub(super) fn yard(p: &Painter, iso: &Iso, (x, y): (i32, i32), style: Style) {
    let h = mix(x, y);
    let (fx, fy) = (x as f32, y as f32);
    let (x0, y0, x1, y1) = (fx + 0.08, fy + 0.08, fx + 0.92, fy + 0.92);
    match style {
        Style::Town => garden(p, iso, x, y),
        Style::Mediterranean => {
            let wall = Faces::of(tone::WALLS[0]);
            cuboid(p, iso, (x0, y0, x1, y0 + 0.05), KERB, KERB + 0.35, wall);
            cuboid(p, iso, (x0, y0, x0 + 0.05, y1), KERB, KERB + 0.35, wall);
            tree(p, iso, fx + 0.3, fy + 0.32, Tree::Cypress);
            tree(p, iso, fx + 0.68, fy + 0.4, if h.is_multiple_of(2) { Tree::Olive } else { Tree::Cypress });
            tree(p, iso, fx + 0.45, fy + 0.7, Tree::Olive);
            cuboid(p, iso, (x0, y1 - 0.05, x1, y1), KERB, KERB + 0.35, wall);
            cuboid(p, iso, (x1 - 0.05, y0, x1, y1), KERB, KERB + 0.35, wall);
        }
        Style::Glass => {
            for k in 0..4 {
                tree(p, iso, fx + 0.15 + k as f32 * 0.2, fy + 0.18, Tree::Bush);
            }
            tree(p, iso, fx + 0.35, fy + 0.5, Tree::Round);
            tree(p, iso, fx + 0.7, fy + 0.68, Tree::Round);
        }
        Style::Industrial => {
            wire(p, iso, (x0, y0), (x1, y0));
            wire(p, iso, (x0, y0), (x0, y1));
            let c1 = tone::CONTAINERS[h % tone::CONTAINERS.len()];
            let c2 = tone::CONTAINERS[(h / 7) % tone::CONTAINERS.len()];
            for (r, c, z) in [((fx + 0.18, fy + 0.2, fx + 0.75, fy + 0.4), c1, 0.0), ((fx + 0.18, fy + 0.2, fx + 0.75, fy + 0.4), c2, 0.6), ((fx + 0.2, fy + 0.52, fx + 0.77, fy + 0.72), c2, 0.0)] {
                cuboid(p, iso, r, KERB + z, KERB + z + 0.6, Faces::of(c));
                ribs(p, iso, true, r.3, (r.0, r.2), (KERB + z, KERB + z + 0.6), c);
            }
            tree(p, iso, fx + 0.82, fy + 0.84, Tree::Bush);
            wire(p, iso, (x0, y1), (x1, y1));
            wire(p, iso, (x1, y0), (x1, y1));
        }
        _ => world::yard(p, iso, (x, y), style),
    }
}

/// The pavement of a district's block: grey stone in town, warm sandstone by the sea, pale cool
/// slabs among the glass, dark concrete in industry.
pub(super) fn pavement(style: Style) -> Faces {
    let top = match style {
        Style::Town => return paint::PAVEMENT,
        Style::Mediterranean => Color32::from_rgb(0xec, 0xdc, 0xbd),
        Style::Glass => Color32::from_rgb(0xe3, 0xea, 0xf0),
        Style::Industrial => Color32::from_rgb(0xbc, 0xb8, 0xae),
        _ => world::colours(style).0,
    };
    Faces { left: theme::shade(top, 0.12), right: theme::shade(top, 0.2), top }
}

/// The ground of a district's free tile.
pub(super) fn ground(style: Style) -> Color32 {
    match style {
        Style::Industrial => tone::GRAVEL,
        Style::Mediterranean => Color32::from_rgb(0x9c, 0xb8, 0x5a),
        Style::Town | Style::Glass => paint::GRASS,
        _ => world::colours(style).1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_always_gives_the_same_building_and_seeds_vary() {
        let rolls = |seed| {
            let mut d = Dice(seed);
            (0..6).map(|_| d.roll(5)).collect::<Vec<_>>()
        };
        assert_eq!(rolls(190), rolls(190));
        let variants: std::collections::HashSet<usize> = (0..60).map(|s| Dice(crate::city::stable_hash(&s.to_string())).roll(3)).collect();
        assert_eq!(variants.len(), 3, "every variant comes up among sixty tickets");
    }
}
