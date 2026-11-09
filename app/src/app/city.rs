//! The City view of the board (#190): the project as an isometric city in flat colours, one
//! plot per ticket (`crate::city`). The city stands on a light grey slab, its blocks on raised
//! pavements between asphalt roads with dashed lines and zebra crossings. Every face is one flat
//! colour and everything that stands casts a long hard shadow along x. Free tiles are fenced
//! gardens with pines and round trees, lamp posts stand at the corners of the blocks, cars drive
//! round them. Each district has its own architecture (`styles`): the town, the Mediterranean,
//! glass and steel, or industry. Each building site has its crane, its workers in hard hats and a supply lorry; each
//! open pull request linked to a ticket adds a lorry in the colour of its checks. Drag to pan,
//! pinch or ⌘-scroll to zoom, double-click to fit; a hover shows the ticket, a click opens its
//! detail. Read only.

mod styles;

use super::*;
use crate::city::{Building, District, Plot, Stage, Style};
use crate::github::Item;
use crate::pulls::{Ci, Pulls};
use egui::{vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

/// A tile's width and height on screen at zoom 1, and a floor's height.
pub(super) const TILE_W: f32 = 64.0;
pub(super) const TILE_H: f32 = 32.0;
pub(super) const FLOOR: f32 = 14.0;
/// How far the blocks' pavements stand above the road, and how thick the slab is, in floors.
pub(super) const KERB: f32 = 0.25;
pub(super) const SLAB: f32 = 1.0;
/// How far a shadow reaches along x for each floor of what casts it, in tiles.
const SHADOW_PER_FLOOR: f32 = 0.28;
/// How far the vehicles drive from the middle of the road, in tiles.
pub(super) const LANE: f32 = 0.17;

/// Where the view stands once the person has moved it; None keeps the city fitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CityView {
    pub pan: Vec2,
    pub zoom: f32,
}

/// The colours of a box's three visible faces: the south one on the left, the east one on the
/// right, and its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Faces {
    pub(super) left: Color32,
    pub(super) right: Color32,
    pub(super) top: Color32,
}

impl Faces {
    pub(super) fn of(c: Color32) -> Faces {
        Faces { left: c, right: theme::shade(c, 0.16), top: theme::lighten(c, 0.14) }
    }
}

/// The flat colours of the city: light grey stone, asphalt, brick, white render, blue glass.
pub(super) mod paint {
    use super::Faces;
    use egui::Color32;

    const fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    const fn faces(left: u32, right: u32, top: u32) -> Faces {
        Faces { left: rgb(left), right: rgb(right), top: rgb(top) }
    }

    pub(super) const SLAB: Faces = faces(0xc9c9c9, 0xb3b3b3, 0xe4e4e4);
    pub(super) const PAVEMENT: Faces = faces(0xcfcfcf, 0xbdbdbd, 0xebebeb);
    pub const ROAD: Color32 = rgb(0x7e8083);
    pub const LINE: Color32 = rgb(0xffffff);
    pub const SOFT_LINE: Color32 = rgb(0xe6e6e6);
    pub const GRASS: Color32 = rgb(0x47b14b);
    pub const DIRT: Color32 = rgb(0xa07a55);
    pub const WIRE: Color32 = rgb(0x4a4e55);
    pub const SLAT: Color32 = rgb(0x8b4b3a);
    pub const RAIL: Color32 = rgb(0x6b3a2c);
    pub const TAPE: Color32 = rgb(0xf07b2c);
    pub const YELLOW: Color32 = rgb(0xf2c230);
    pub(super) const CONCRETE: Faces = faces(0xb8b8b8, 0xa3a3a3, 0xd2d2d2);
    pub const COLUMN: Color32 = rgb(0x9a9a9a);
    pub const REBAR: Color32 = rgb(0x7a5a3a);
    pub const WEIGHT: Color32 = rgb(0x6d6f73);
    pub const CABLE: Color32 = rgb(0x444444);
    pub const RED: Color32 = rgb(0xb03a2e);
    pub const DOOR: Color32 = rgb(0xc0392b);
    // The town's tower: a blue glazed base.
    pub(super) const BASE: Faces = faces(0x5b9bd5, 0x4a85bd, 0x5b9bd5);
    pub const SHOP: Color32 = rgb(0x2c3a4f);
    pub const PANE: Color32 = rgb(0x8fa6c9);
    pub const CURTAIN: Color32 = rgb(0x5b7fa8);
    // The brick building's roof and the white one.
    pub const BRICK_ROOF: Color32 = rgb(0xd0d0d0);
    pub(super) const WHITE: Faces = faces(0xf2f2f2, 0xd5d5d5, 0xa0a0a0);
    pub const WHITE_ROOF: Color32 = rgb(0x8a8a8a);
    pub const WINDOW_LEFT: Color32 = rgb(0x4f8fd6);
    pub const WINDOW_RIGHT: Color32 = rgb(0x3f78bd);
    pub(super) const AC: Faces = faces(0xcfcfcf, 0xb9b9b9, 0xffffff);
    // Trees, lamps, vehicles, people.
    pub const TRUNK: Color32 = rgb(0x7a5236);
    pub const PINE: (Color32, Color32) = (rgb(0x5cc26b), rgb(0x3e9c4f));
    pub const ROUND: (Color32, Color32) = (rgb(0x79c66b), rgb(0x58a84d));
    pub const CYPRESS: (Color32, Color32) = (rgb(0x3f8a4f), rgb(0x2c6b3c));
    pub const OLIVE: (Color32, Color32) = (rgb(0xa3b06e), rgb(0x86955a));
    pub const LEAVES: [(Color32, Color32); 6] = [(rgb(0x8fd06a), rgb(0x6aaf4c)), (rgb(0x4f9a5a), rgb(0x3a7d47)), (rgb(0xf0a23a), rgb(0xd9822a)), (rgb(0xe0533a), rgb(0xb83d2c)), (rgb(0xf2cf4a), rgb(0xd6b03a)), (rgb(0xf7bfd2), rgb(0xe896b4))];
    pub const BIRCH: (Color32, Color32) = (rgb(0xb6d77a), rgb(0x95bb5c));
    pub const BIRCH_BARK: Color32 = rgb(0xf1efe8);
    pub const SPRUCE: (Color32, Color32) = (rgb(0x2f7a55), rgb(0x225e42));
    pub const PALM: (Color32, Color32) = (rgb(0x6fbf4f), rgb(0x4f9a3a));
    pub const PALM_TRUNK: Color32 = rgb(0x9a7b52);
    pub const FLOWERS: [Color32; 5] = [rgb(0xe8435a), rgb(0xf2c230), rgb(0xa86bd9), rgb(0xf58a3a), rgb(0xffffff)];
    pub const LAMP: Color32 = rgb(0x8a8d92);
    pub const LAMP_HEAD: Color32 = rgb(0x6d7075);
    pub const TYRE: Color32 = rgb(0x2a2a2e);
    pub const GLASS: (Color32, Color32) = (rgb(0x2b3542), rgb(0x3a4656));
    pub(super) const CAB: Faces = faces(0xf2c230, 0xd9a81c, 0xf7d35e);
    pub(super) const SUPPLY: Faces = faces(0xf4f4f8, 0xd7d6ea, 0xffffff);
    pub const STRIPE: Color32 = rgb(0x9b8fd6);
    pub const CARS: [Color32; 7] = [rgb(0xf2c230), rgb(0x5b6fd6), rgb(0x6b4a32), rgb(0xd9412f), rgb(0x6a5acd), rgb(0x3fa7a0), rgb(0xf4f4f4)];
    pub const TROUSERS: Color32 = rgb(0x3c4660);
}

#[derive(Clone, Copy)]
pub(super) struct Iso {
    pub(super) origin: Pos2,
    pub(super) zoom: f32,
}

impl Iso {
    /// A point of the city, in tiles (z in floors), on screen.
    pub(super) fn at(&self, x: f32, y: f32, z: f32) -> Pos2 {
        self.origin + vec2((x - y) * TILE_W / 2.0, (x + y) * TILE_H / 2.0 - z * FLOOR) * self.zoom
    }

    /// A rectangle of ground from (x0, y0) to (x1, y1) at height z: north, east, south, west.
    pub(super) fn quad(&self, x0: f32, y0: f32, x1: f32, y1: f32, z: f32) -> [Pos2; 4] {
        [self.at(x0, y0, z), self.at(x1, y0, z), self.at(x1, y1, z), self.at(x0, y1, z)]
    }

    pub(super) fn px(&self, v: f32) -> f32 {
        v * self.zoom
    }
}

/// A flat face. One seen edge-on (a roof slope as steep as the line of sight) has no area on
/// screen and is left out: egui's anti-aliasing would draw it as spikes across the view.
pub(super) fn poly(p: &Painter, pts: Vec<Pos2>, fill: Color32) {
    if area(&pts).abs() > 0.5 {
        p.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
    }
}

/// A polygon's signed area on screen, in square points.
fn area(pts: &[Pos2]) -> f32 {
    pts.iter().zip(pts.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum::<f32>() / 2.0
}

/// A box from (x0, y0) to (x1, y1), from floor z0 to z1: three flat faces, no outline.
pub(super) fn cuboid(p: &Painter, iso: &Iso, (x0, y0, x1, y1): (f32, f32, f32, f32), z0: f32, z1: f32, f: Faces) {
    let lo = iso.quad(x0, y0, x1, y1, z0);
    let hi = iso.quad(x0, y0, x1, y1, z1);
    poly(p, vec![lo[3], lo[2], hi[2], hi[3]], f.left);
    poly(p, vec![lo[2], lo[1], hi[1], hi[2]], f.right);
    poly(p, hi.to_vec(), f.top);
}

pub(super) fn shadow() -> Color32 {
    Color32::from_rgba_unmultiplied(30, 32, 40, 66)
}

/// Where the shadow of something whose east side is at `x1`, `h` floors high, ends: the sun
/// stands low to the west, so it falls along x, and stops at `reach`, the edge of its pavement.
pub(super) fn shadow_end(x1: f32, h: f32, reach: f32) -> f32 {
    (x1 + h * SHADOW_PER_FLOOR).min(reach.max(x1))
}

/// The hard shadow a box from (x0, y0) to (x1, y1), `h` floors high, casts on the ground at z.
pub(super) fn cast_shadow(p: &Painter, iso: &Iso, (x0, y0, x1, y1): (f32, f32, f32, f32), z: f32, h: f32, reach: f32) {
    poly(p, iso.quad(x0, y0, shadow_end(x1, h, reach), y1, z).to_vec(), shadow());
}

/// A rectangle on a face: the south one (y = `face`, along x) or the east one (x = `face`,
/// along y), from a0 to a1, from z0 to z1.
pub(super) fn on_face(iso: &Iso, south: bool, face: f32, (a0, a1): (f32, f32), (z0, z1): (f32, f32)) -> Vec<Pos2> {
    let pt = |a: f32, z: f32| if south { iso.at(a, face, z) } else { iso.at(face, a, z) };
    vec![pt(a0, z0), pt(a1, z0), pt(a1, z1), pt(a0, z1)]
}

/// A window: a pane of `fill` in a thin `frame`.
#[allow(clippy::too_many_arguments)]
fn pane(p: &Painter, iso: &Iso, south: bool, face: f32, along: (f32, f32), z: (f32, f32), fill: Color32, frame: Color32) {
    p.add(Shape::convex_polygon(on_face(iso, south, face, along, z), fill, Stroke::new(iso.px(0.9).clamp(0.5, 1.6), frame)));
}

/// The flat roof's parapet: the roof a little lower inside its rim.
fn parapet(p: &Painter, iso: &Iso, (x0, y0, x1, y1): (f32, f32, f32, f32), z: f32, inside: Color32) {
    let e = 0.06;
    poly(p, iso.quad(x0 + e, y0 + e, x1 - e, y1 - e, z).to_vec(), inside);
}

/// An air conditioning unit on a roof.
fn ac(p: &Painter, iso: &Iso, x: f32, y: f32, z: f32) {
    cuboid(p, iso, (x, y, x + 0.14, y + 0.12), z, z + 0.24, paint::AC);
}

/// A fence from `a` to `b` on the ground at z, `h` floors high: a post every `step` tiles and a
/// rail along the top.
#[allow(clippy::too_many_arguments)]
fn fence(p: &Painter, iso: &Iso, a: (f32, f32), b: (f32, f32), z: f32, h: f32, step: f32, post: Stroke, rail: Stroke) {
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / step).ceil().max(1.0) as i32;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        p.line_segment([iso.at(x, y, z), iso.at(x, y, z + h)], post);
    }
    p.line_segment([iso.at(a.0, a.1, z + h), iso.at(b.0, b.1, z + h)], rail);
}

/// A garden's slatted wooden fence.
fn slats(p: &Painter, iso: &Iso, a: (f32, f32), b: (f32, f32)) {
    fence(p, iso, a, b, KERB, 0.5, 0.06, Stroke::new(iso.px(1.3), paint::SLAT), Stroke::new(iso.px(1.1), paint::RAIL));
}

/// A building lot's wire fence.
fn wire(p: &Painter, iso: &Iso, a: (f32, f32), b: (f32, f32)) {
    fence(p, iso, a, b, KERB, 0.55, 0.09, Stroke::new(iso.px(0.7), paint::WIRE), Stroke::new(iso.px(0.9), paint::WIRE));
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Tree {
    Pine,
    Round,
    Bush,
    Cypress,
    Olive,
    /// A leafy tree in one of `paint::LEAVES`: greens, autumn, blossom.
    Leafy(usize),
    Birch,
    Spruce,
    Palm,
    /// A bed of flowers in one of `paint::FLOWERS`.
    Flowers(usize),
}

impl Tree {
    /// The trees a park mixes, for variety.
    pub(super) const PARK: [Tree; 14] = [Tree::Leafy(0), Tree::Leafy(1), Tree::Leafy(2), Tree::Leafy(3), Tree::Leafy(4), Tree::Leafy(5), Tree::Pine, Tree::Round, Tree::Birch, Tree::Spruce, Tree::Bush, Tree::Flowers(0), Tree::Flowers(1), Tree::Flowers(2)];
}

/// A tree in low poly, its lit half and its shaded one, and its shadow along x.
pub(super) fn tree(p: &Painter, iso: &Iso, x: f32, y: f32, kind: Tree) {
    let reach = match kind {
        Tree::Pine => 0.45,
        Tree::Round => 0.38,
        Tree::Bush => 0.16,
        Tree::Cypress => 0.5,
        Tree::Olive => 0.32,
        Tree::Leafy(_) => 0.38,
        Tree::Birch => 0.42,
        Tree::Spruce => 0.48,
        Tree::Palm => 0.5,
        Tree::Flowers(_) => 0.1,
    };
    poly(p, iso.quad(x - 0.04, y - 0.05, x + reach, y + 0.05, KERB).to_vec(), shadow());
    let s = iso.zoom * 0.62;
    let b = iso.at(x, y, KERB);
    let pt = |dx: f32, dy: f32| b + vec2(dx, dy) * s;
    let trunk = Stroke::new(2.5 * s, paint::TRUNK);
    match kind {
        Tree::Pine => {
            p.line_segment([pt(0.0, 2.0), pt(0.0, -6.0)], trunk);
            poly(p, vec![pt(0.0, -36.0), pt(-9.0, -6.0), pt(0.0, -2.0)], paint::PINE.0);
            poly(p, vec![pt(0.0, -36.0), pt(0.0, -2.0), pt(9.0, -6.0)], paint::PINE.1);
        }
        Tree::Round => {
            p.line_segment([pt(0.0, 2.0), pt(0.0, -8.0)], trunk);
            poly(p, vec![pt(0.0, -28.0), pt(-10.0, -22.0), pt(-10.0, -12.0), pt(0.0, -6.0)], paint::ROUND.0);
            poly(p, vec![pt(0.0, -28.0), pt(0.0, -6.0), pt(10.0, -12.0), pt(10.0, -22.0)], paint::ROUND.1);
        }
        Tree::Bush => {
            poly(p, vec![pt(0.0, -10.0), pt(-7.0, -6.0), pt(-7.0, 0.0), pt(0.0, 3.0)], paint::ROUND.0);
            poly(p, vec![pt(0.0, -10.0), pt(0.0, 3.0), pt(7.0, 0.0), pt(7.0, -6.0)], paint::ROUND.1);
        }
        Tree::Cypress => {
            p.line_segment([pt(0.0, 2.0), pt(0.0, -4.0)], trunk);
            poly(p, vec![pt(0.0, -42.0), pt(-5.0, -24.0), pt(-4.5, -6.0), pt(0.0, -3.0)], paint::CYPRESS.0);
            poly(p, vec![pt(0.0, -42.0), pt(0.0, -3.0), pt(4.5, -6.0), pt(5.0, -24.0)], paint::CYPRESS.1);
        }
        Tree::Leafy(i) => {
            let (lit, dark) = paint::LEAVES[i % paint::LEAVES.len()];
            p.line_segment([pt(0.0, 2.0), pt(0.0, -8.0)], trunk);
            poly(p, vec![pt(0.0, -30.0), pt(-8.0, -27.0), pt(-12.0, -19.0), pt(-9.0, -10.0), pt(0.0, -6.0)], lit);
            poly(p, vec![pt(0.0, -30.0), pt(0.0, -6.0), pt(9.0, -10.0), pt(12.0, -19.0), pt(8.0, -27.0)], dark);
        }
        Tree::Birch => {
            p.line_segment([pt(0.0, 2.0), pt(0.0, -16.0)], Stroke::new(2.2 * s, paint::BIRCH_BARK));
            for m in [-4.0, -9.0, -13.0] {
                p.line_segment([pt(-1.0, m), pt(1.0, m)], Stroke::new(1.0 * s, paint::TYRE));
            }
            poly(p, vec![pt(0.0, -34.0), pt(-7.0, -26.0), pt(-6.0, -15.0), pt(0.0, -12.0)], paint::BIRCH.0);
            poly(p, vec![pt(0.0, -34.0), pt(0.0, -12.0), pt(6.0, -15.0), pt(7.0, -26.0)], paint::BIRCH.1);
        }
        Tree::Spruce => {
            p.line_segment([pt(0.0, 2.0), pt(0.0, -4.0)], trunk);
            for (top, base, w) in [(-18.0, -2.0, 10.0), (-28.0, -12.0, 8.0), (-38.0, -22.0, 6.0)] {
                poly(p, vec![pt(0.0, top), pt(-w, base), pt(0.0, base + 2.0)], paint::SPRUCE.0);
                poly(p, vec![pt(0.0, top), pt(0.0, base + 2.0), pt(w, base)], paint::SPRUCE.1);
            }
        }
        Tree::Palm => {
            let palm = Stroke::new(2.4 * s, paint::PALM_TRUNK);
            p.line_segment([pt(0.0, 2.0), pt(2.0, -14.0)], palm);
            p.line_segment([pt(2.0, -14.0), pt(5.0, -30.0)], palm);
            let top = pt(5.0, -30.0);
            for (dx, dy, c) in [(-14.0, 6.0, paint::PALM.1), (14.0, 7.0, paint::PALM.1), (-10.0, -6.0, paint::PALM.0), (11.0, -5.0, paint::PALM.0), (1.0, -10.0, paint::PALM.0)] {
                let tip = top + vec2(dx, dy) * s;
                let side = vec2(-(dy), dx).normalized() * 2.2 * s;
                poly(p, vec![top + side, tip, top - side], c);
            }
        }
        Tree::Flowers(i) => {
            poly(p, vec![pt(0.0, -6.0), pt(-8.0, -3.0), pt(-8.0, 1.0), pt(0.0, 4.0)], paint::ROUND.0);
            poly(p, vec![pt(0.0, -6.0), pt(0.0, 4.0), pt(8.0, 1.0), pt(8.0, -3.0)], paint::ROUND.1);
            let c = paint::FLOWERS[i % paint::FLOWERS.len()];
            for (dx, dy) in [(-5.0, -2.0), (-1.0, -4.0), (3.0, -2.0), (6.0, -0.5), (-3.0, 1.0), (1.5, 1.5)] {
                p.circle_filled(pt(dx, dy), 1.5 * s, c);
            }
        }
        Tree::Olive => {
            p.line_segment([pt(0.0, 2.0), pt(-2.0, -10.0)], trunk);
            poly(p, vec![pt(-1.0, -24.0), pt(-12.0, -19.0), pt(-11.0, -11.0), pt(-1.0, -8.0)], paint::OLIVE.0);
            poly(p, vec![pt(-1.0, -24.0), pt(-1.0, -8.0), pt(10.0, -11.0), pt(11.0, -18.0)], paint::OLIVE.1);
        }
    }
}

/// A lamp post: a grey pole, its arm and its head.
pub(super) fn lamp(p: &Painter, iso: &Iso, x: f32, y: f32) {
    p.line_segment([iso.at(x, y, KERB), iso.at(x + 0.4, y, KERB)], Stroke::new(iso.px(1.2), shadow()));
    let s = iso.zoom * 0.62;
    let b = iso.at(x, y, KERB);
    let pt = |dx: f32, dy: f32| b + vec2(dx, dy) * s;
    let pole = Stroke::new(1.5 * s, paint::LAMP);
    p.line_segment([pt(0.0, 0.0), pt(0.0, -30.0)], pole);
    p.line_segment([pt(0.0, -30.0), pt(5.0, -32.0)], pole);
    p.rect_filled(Rect::from_min_max(pt(4.0, -34.0), pt(9.0, -31.0)), 0.0, paint::LAMP_HEAD);
}

/// A few numbers that stay the same for a place, to vary what stands there.
pub(super) fn mix(a: i32, b: i32) -> usize {
    ((a as u32).wrapping_mul(73_856_093) ^ (b as u32).wrapping_mul(19_349_663)) as usize
}

/// A free tile: a fenced garden with two trees and a bush (its grass is on the ground already).
fn garden(p: &Painter, iso: &Iso, x: i32, y: i32) {
    let h = mix(x, y);
    let (x, y) = (x as f32, y as f32);
    let (x0, y0, x1, y1) = (x + 0.08, y + 0.08, x + 0.92, y + 0.92);
    slats(p, iso, (x0, y0), (x1, y0));
    slats(p, iso, (x0, y0), (x0, y1));
    let first = Tree::PARK[h % Tree::PARK.len()];
    let second = Tree::PARK[(h / 7 + 3) % Tree::PARK.len()];
    // Back to front.
    tree(p, iso, x + 0.32, y + 0.36, first);
    tree(p, iso, x + 0.72, y + 0.3, if h.is_multiple_of(3) { Tree::Flowers(h / 3) } else { Tree::Bush });
    tree(p, iso, x + 0.62, y + 0.66, second);
    slats(p, iso, (x0, y1), (x1, y1));
    slats(p, iso, (x1, y0), (x1, y1));
}

/// A traffic cone.
fn cone(p: &Painter, iso: &Iso, x: f32, y: f32) {
    let b = iso.at(x, y, KERB);
    let t = iso.at(x, y, KERB + 0.45);
    let w = iso.px(3.0);
    poly(p, vec![b - vec2(w, 0.0), b + vec2(w, 0.0), t], paint::TAPE);
    p.line_segment([b - vec2(w * 0.55, iso.px(3.0)), b + vec2(w * 0.55, -iso.px(3.0))], Stroke::new(iso.px(1.0), paint::LINE));
}

/// A tower crane: a lattice mast at the plot's back corner, a jib turning with time, its hook
/// carrying a red beam.
fn crane(p: &Painter, iso: &Iso, x: f32, y: f32, height: f32, angle: f32) {
    let (mx, my) = (x + 0.2, y + 0.2);
    let thick = Stroke::new(iso.px(1.5), paint::YELLOW);
    let thin = Stroke::new(iso.px(0.7), paint::YELLOW);
    p.line_segment([iso.at(mx, my, KERB), iso.at(mx, my, KERB + height)], thick);
    p.line_segment([iso.at(mx + 0.08, my, KERB), iso.at(mx + 0.08, my, KERB + height)], thick);
    let n = (height * 2.0) as i32;
    for i in 0..n {
        let z0 = KERB + height * i as f32 / n as f32;
        let z1 = KERB + height * (i + 1) as f32 / n as f32;
        p.line_segment([iso.at(mx, my, z0), iso.at(mx + 0.08, my, z1)], thin);
    }
    let (dx, dy) = (angle.cos(), angle.sin());
    let top = KERB + height;
    let jib = iso.at(mx + dx * 1.3, my + dy * 1.3, top);
    let back = iso.at(mx - dx * 0.4, my - dy * 0.4, top);
    p.line_segment([back, jib], Stroke::new(iso.px(2.0), paint::YELLOW));
    p.line_segment([iso.at(mx, my, top + 0.5), jib], thin);
    p.line_segment([iso.at(mx, my, top + 0.5), back], thin);
    p.rect_filled(Rect::from_center_size(back, vec2(iso.px(7.0), iso.px(4.5))), 0.0, paint::WEIGHT);
    p.rect_filled(Rect::from_center_size(iso.at(mx, my, top - 0.15), vec2(iso.px(5.0), iso.px(4.0))), 0.0, paint::LINE);
    let hx = 0.95 + 0.25 * (angle * 0.7).sin();
    let hook = iso.at(mx + dx * hx, my + dy * hx, top * 0.55);
    p.line_segment([iso.at(mx + dx * hx, my + dy * hx, top), hook], Stroke::new(iso.px(0.6), paint::CABLE));
    p.line_segment([hook - vec2(iso.px(7.0), 0.0), hook + vec2(iso.px(7.0), 0.0)], Stroke::new(iso.px(2.2), paint::RED));
}

/// Skin tones a worker can have, from light to deep.
const SKIN: [Color32; 6] = [
    Color32::from_rgb(246, 214, 186),
    Color32::from_rgb(234, 192, 152),
    Color32::from_rgb(214, 164, 120),
    Color32::from_rgb(180, 126, 86),
    Color32::from_rgb(140, 92, 60),
    Color32::from_rgb(98, 64, 44),
];

/// What makes one worker look unlike another, always the same for a person: a skin tone, and
/// a beard for some.
pub(super) fn looks(who: &str) -> (Color32, bool) {
    let h = who.bytes().fold(17u32, |a, b| a.wrapping_mul(131).wrapping_add(b as u32)) as usize;
    (SKIN[h % SKIN.len()], (h / SKIN.len()).is_multiple_of(3))
}

/// A worker in an orange vest and a yellow hard hat, bobbing a little as they work.
fn worker(p: &Painter, iso: &Iso, (x, y): (f32, f32), who: &str, t: f32) {
    let feet = iso.at(x, y, KERB) - vec2(0.0, iso.px((t * 6.0).sin().abs() * 1.2));
    let (w, h) = (iso.px(3.2), iso.px(7.4));
    let (skin, beard) = looks(who);
    p.rect_filled(Rect::from_min_max(feet - vec2(w * 0.75, h * 0.45), feet + vec2(w * 0.75, 0.0)), 0.0, paint::TROUSERS);
    p.rect_filled(Rect::from_min_max(feet - vec2(w, h), feet - vec2(-w, h * 0.42)), iso.px(1.5), paint::TAPE);
    let head = feet - vec2(0.0, h + iso.px(2.2));
    p.circle_filled(head, iso.px(2.3), skin);
    if beard {
        let chin = (0..=6).map(|k| {
            let a = std::f32::consts::PI * k as f32 / 6.0;
            head + vec2(iso.px(2.3) * a.cos(), iso.px(2.3) * a.sin())
        });
        poly(p, chin.collect(), theme::shade(skin, 0.6));
    }
    let dome = (0..=8).map(|k| {
        let a = std::f32::consts::PI * k as f32 / 8.0;
        head + vec2(iso.px(2.9) * a.cos(), -iso.px(2.9) * a.sin() - iso.px(0.5))
    });
    poly(p, dome.collect(), paint::YELLOW);
}

/// Where a vehicle is: on the road at (x, y), heading along x or y, forward (+1) or back (-1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct OnRoad {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) along_x: bool,
    pub(super) dir: f32,
}

/// The same place, moved into the lane on the right of the way it heads.
pub(super) fn in_lane(at: OnRoad) -> OnRoad {
    let mut a = at;
    if at.along_x {
        a.y += LANE * at.dir;
    } else {
        a.x -= LANE * at.dir;
    }
    a
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Vehicle {
    Car(Color32),
    /// A lorry, its box in these colours.
    Lorry(Faces),
}

/// A car or a lorry on its wheels, with its hard shadow along x.
fn vehicle(p: &Painter, iso: &Iso, at: OnRoad, v: Vehicle) {
    let (len, wid) = match v {
        Vehicle::Car(_) => (0.36, 0.18),
        Vehicle::Lorry(_) => (0.62, 0.24),
    };
    // A stretch of it along the road, from a0 to a1 around its middle, `w` wide.
    let rect = |a0: f32, a1: f32, w: f32| if at.along_x { (at.x + a0, at.y - w / 2.0, at.x + a1, at.y + w / 2.0) } else { (at.x - w / 2.0, at.y + a0, at.x + w / 2.0, at.y + a1) };
    let (x0, y0, x1, y1) = rect(-len / 2.0, len / 2.0, wid);
    poly(p, iso.quad(x0, y0, x1 + 0.18, y1, 0.0).to_vec(), shadow());
    // The wheels on the side the viewer sees.
    let wheels: &[f32] = if matches!(v, Vehicle::Car(_)) { &[-0.3, 0.3] } else { &[-0.34, 0.0, 0.34] };
    for a in wheels {
        let c = if at.along_x { iso.at(at.x + a * len, y1, 0.12) } else { iso.at(x1, at.y + a * len, 0.12) };
        p.circle_filled(c, iso.px(1.9), paint::TYRE);
    }
    match v {
        Vehicle::Car(c) => {
            cuboid(p, iso, (x0, y0, x1, y1), 0.12, 0.48, Faces { left: c, right: theme::shade(c, 0.16), top: c });
            // The cabin, set back toward the rear.
            let (a0, a1) = if at.dir > 0.0 { (-len * 0.32, len * 0.12) } else { (-len * 0.12, len * 0.32) };
            cuboid(p, iso, rect(a0, a1, wid * 0.84), 0.48, 0.86, Faces { left: paint::GLASS.0, right: paint::GLASS.1, top: c });
        }
        Vehicle::Lorry(f) => {
            let front = len * 0.3;
            let (cab, cargo) = if at.dir > 0.0 { ((len / 2.0 - front, len / 2.0), (-len / 2.0, len / 2.0 - front - 0.02)) } else { ((-len / 2.0, -len / 2.0 + front), (-len / 2.0 + front + 0.02, len / 2.0)) };
            // The part farther from the viewer first: heading +x or +y, the cargo is behind.
            let mut parts = [(cab, true), (cargo, false)];
            if at.dir > 0.0 {
                parts.reverse();
            }
            for ((a0, a1), is_cab) in parts {
                if is_cab {
                    let r @ (cx0, cy0, cx1, cy1) = rect(a0, a1, wid * 0.92);
                    cuboid(p, iso, r, 0.12, 0.78, paint::CAB);
                    // Its windscreen, on the face it heads toward when the viewer sees it.
                    if at.dir > 0.0 {
                        let screen = if at.along_x { on_face(iso, false, cx1, (cy0 + 0.02, cy1 - 0.02), (0.42, 0.7)) } else { on_face(iso, true, cy1, (cx0 + 0.02, cx1 - 0.02), (0.42, 0.7)) };
                        poly(p, screen, paint::GLASS.0);
                    }
                } else {
                    let r @ (bx0, by0, bx1, by1) = rect(a0, a1, wid);
                    cuboid(p, iso, r, 0.12, 1.05, f);
                    if f == paint::SUPPLY {
                        // The supply lorry's stripe down its side.
                        let side = if at.along_x { [iso.at(bx0 + 0.03, by1, 0.5), iso.at(bx1 - 0.03, by1, 0.5)] } else { [iso.at(bx1, by0 + 0.03, 0.5), iso.at(bx1, by1 - 0.03, 0.5)] };
                        p.line_segment(side, Stroke::new(iso.px(2.0), paint::STRIPE));
                    }
                }
            }
        }
    }
}

pub(super) fn ci_color(ci: Option<Ci>) -> Color32 {
    match ci {
        Some(Ci::Success) => Color32::from_rgb(92, 178, 96),
        Some(Ci::Failure) => Color32::from_rgb(214, 74, 70),
        Some(Ci::Pending) => Color32::from_rgb(232, 168, 56),
        None => Color32::from_rgb(150, 156, 166),
    }
}

/// A point on the road round a block, `t` from 0 to 1 clockwise from its north corner.
pub(super) fn round_block(d: &District, t: f32) -> OnRoad {
    let (x0, y0) = (d.x as f32 - 0.5, d.y as f32 - 0.5);
    let (x1, y1) = (x0 + d.w as f32 + 1.0, y0 + d.h as f32 + 1.0);
    let (w, h) = (x1 - x0, y1 - y0);
    let s = t.rem_euclid(1.0) * 2.0 * (w + h);
    if s < w {
        OnRoad { x: x0 + s, y: y0, along_x: true, dir: 1.0 }
    } else if s < w + h {
        OnRoad { x: x1, y: y0 + s - w, along_x: false, dir: 1.0 }
    } else if s < 2.0 * w + h {
        OnRoad { x: x1 - (s - w - h), y: y1, along_x: true, dir: -1.0 }
    } else {
        OnRoad { x: x0, y: y1 - (s - 2.0 * w - h), along_x: false, dir: -1.0 }
    }
}

/// How high what stands on a plot reaches above its pavement, in floors.
fn height(pl: &Plot) -> f32 {
    match pl.stage {
        Stage::Lot | Stage::Staked => 0.55,
        Stage::Foundations => 0.3,
        Stage::Frame => frame_floors(pl) as f32 + 0.3,
        Stage::Testing | Stage::Built => pl.floors as f32,
    }
}

/// The floors a building site under its frame has risen: half the building's.
fn frame_floors(pl: &Plot) -> u32 {
    if pl.stage == Stage::Frame {
        pl.floors.div_ceil(2).max(1)
    } else {
        0
    }
}

/// The screen box a plot takes, for hovering: its tile and what stands on it.
fn plot_hit(iso: &Iso, pl: &Plot) -> Rect {
    let (x, y) = (pl.x as f32, pl.y as f32);
    let mut r = Rect::from_points(&iso.quad(x, y, x + 1.0, y + 1.0, KERB));
    r.extend_with(iso.at(x + 0.5, y + 0.5, KERB + height(pl) + 0.3));
    r
}

/// Something to draw, at its depth: the farther from the viewer, the sooner.
enum Thing<'a> {
    Plot(&'a Plot),
    Garden(i32, i32, Style),
    Tree(f32, f32, Tree),
    Lamp(f32, f32),
    Vehicle(OnRoad, Vehicle),
}

/// A plot on a building site: a concrete slab, or half the floors as a concrete frame in its
/// yellow scaffolding, cones, the crane and the workers.
fn site(p: &Painter, iso: &Iso, pl: &Plot, it: &Item, time: f32) {
    let (x, y) = (pl.x as f32, pl.y as f32);
    let phase = (pl.x * 7 + pl.y * 13) as f32 * 0.9;
    poly(p, iso.quad(x + 0.08, y + 0.08, x + 0.92, y + 0.92, KERB).to_vec(), paint::DIRT);
    let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
    cuboid(p, iso, r, KERB, KERB + 0.3, paint::CONCRETE);
    let mut top = KERB + 0.3;
    let floors = frame_floors(pl);
    let column = Faces::of(paint::COLUMN);
    for _ in 0..floors {
        let z = top;
        for (cx, cy) in [(x0, y0), (x1 - 0.07, y0), (x1 - 0.07, y1 - 0.07), (x0, y1 - 0.07)] {
            cuboid(p, iso, (cx, cy, cx + 0.07, cy + 0.07), z, z + 0.85, column);
        }
        cuboid(p, iso, r, z + 0.85, z + 1.0, paint::CONCRETE);
        top = z + 1.0;
    }
    if floors == 0 {
        // Foundations: rebar sticking up.
        for i in 0..3 {
            let rx = x0 + 0.14 + i as f32 * 0.2;
            p.line_segment([iso.at(rx, y1 - 0.12, top), iso.at(rx, y1 - 0.12, top + 0.6)], Stroke::new(iso.px(1.0), paint::REBAR));
        }
    } else {
        // The scaffolding along the south face.
        let (sy, s) = (y1 + 0.05, Stroke::new(iso.px(0.9), paint::YELLOW));
        for sx in [x0 - 0.03, (x0 + x1) / 2.0, x1] {
            p.line_segment([iso.at(sx, sy, KERB), iso.at(sx, sy, top + 0.3)], s);
        }
        for f in 0..=floors {
            let z = KERB + 0.3 + f as f32;
            p.line_segment([iso.at(x0 - 0.03, sy, z), iso.at(x1, sy, z)], s);
        }
    }
    cone(p, iso, x + 0.86, y + 0.9);
    cone(p, iso, x + 0.92, y + 0.6);
    crane(p, iso, x, y, top + 2.6, time * 0.45 + phase);
    // The workers: one per assignee (two at most), else one anyway.
    let who: Vec<&str> = if it.assignees.is_empty() { vec!["?"] } else { it.assignees.iter().take(2).map(String::as_str).collect() };
    for (i, w) in who.iter().enumerate() {
        worker(p, iso, (x + 0.55 + i as f32 * 0.22, y + 0.95), w, time + phase + i as f32);
    }
}

/// A bare plot: Backlog's earth behind a wire fence, with Ready's orange tape and sign board.
fn lot(p: &Painter, iso: &Iso, pl: &Plot) {
    let (x, y) = (pl.x as f32, pl.y as f32);
    let (x0, y0, x1, y1) = (x + 0.1, y + 0.1, x + 0.9, y + 0.9);
    poly(p, iso.quad(x0, y0, x1, y1, KERB).to_vec(), paint::DIRT);
    wire(p, iso, (x0, y0), (x1, y0));
    wire(p, iso, (x0, y0), (x0, y1));
    if pl.stage == Stage::Staked {
        let tape = iso.quad(x + 0.22, y + 0.22, x + 0.78, y + 0.78, KERB);
        p.add(Shape::closed_line(tape.to_vec(), Stroke::new(iso.px(1.4), paint::TAPE)));
        let post = Stroke::new(iso.px(1.0), paint::WIRE);
        for sx in [x + 0.42, x + 0.58] {
            p.line_segment([iso.at(sx, y + 0.5, KERB), iso.at(sx, y + 0.5, KERB + 0.4)], post);
        }
        pane(p, iso, true, y + 0.5, (x + 0.38, x + 0.62), (KERB + 0.4, KERB + 0.85), paint::LINE, paint::TAPE);
    }
    wire(p, iso, (x0, y1), (x1, y1));
    wire(p, iso, (x1, y0), (x1, y1));
}

impl Tab {
    /// Takes `area` for an isometric view of a city of `size` tiles, fitted in `room` and
    /// centred there, then moved by the person: drag pans, pinch or ⌘-scroll zooms round the
    /// pointer, double-click fits again. The City view and the food court share it.
    pub(super) fn iso_view(&mut self, ui: &mut Ui, area: Rect, room: Rect, size: (i32, i32)) -> (egui::Response, Iso) {
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        let (cols, rows) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let world_w = (cols + rows) * TILE_W / 2.0;
        let world_h = (cols + rows) * TILE_H / 2.0 + (10.0 + SLAB) * FLOOR;
        let fit = (room.width() / world_w).min((room.height() - 40.0) / world_h).clamp(0.3, 1.6);
        let mut view = self.board.city.unwrap_or(CityView { pan: Vec2::ZERO, zoom: fit });
        if resp.dragged() {
            view.pan += resp.drag_delta();
            self.board.city = Some(view);
        }
        if resp.hovered() {
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
            if zoom != 1.0 {
                // Zoom around the pointer: the point under it stays there.
                let at = ui.input(|i| i.pointer.hover_pos()).unwrap_or(room.center());
                let z = (view.zoom * zoom).clamp(0.25, 3.0);
                let centre = room.center() + view.pan;
                view.pan += (at - centre) * (1.0 - z / view.zoom);
                view.zoom = z;
                self.board.city = Some(view);
            } else if scroll != Vec2::ZERO {
                view.pan += scroll;
                self.board.city = Some(view);
            }
        }
        if resp.double_clicked() {
            self.board.city = None;
            view = CityView { pan: Vec2::ZERO, zoom: fit };
        }
        let iso0 = Iso { origin: Pos2::ZERO, zoom: view.zoom };
        let mid = iso0.at(cols / 2.0, rows / 2.0, 0.0).to_vec2();
        (resp, Iso { origin: room.center() + view.pan - mid + vec2(0.0, 3.0 * FLOOR * view.zoom), zoom: view.zoom })
    }

    /// Draws the city of `items` (the tickets the board's filters keep) and returns the ticket
    /// clicked.
    pub(super) fn city_view(&mut self, ui: &mut Ui, items: &[&Item], detail: Option<&str>) -> Option<String> {
        let city = crate::city::layout(items);
        let area = ui.available_rect_before_wrap().shrink2(vec2(12.0, 6.0));
        let (resp, iso) = self.iso_view(ui, area, area, city.size);
        let p = ui.painter().with_clip_rect(area);
        let k = theme::c();
        let time = ui.input(|i| i.time) as f32;
        let (cols, rows) = (city.size.0.max(1) as f32, city.size.1.max(1) as f32);

        // The slab, then the asphalt on it.
        cuboid(&p, &iso, (0.0, 0.0, cols, rows), -SLAB, 0.0, paint::SLAB);
        poly(&p, iso.quad(0.35, 0.35, cols - 0.35, rows - 0.35, 0.0).to_vec(), paint::ROAD);
        // Dashed lines down the middle of the roads, and zebra crossings by the blocks.
        let dash = Stroke::new(iso.px(1.1), paint::LINE);
        for d in &city.districts {
            let (x0, y0) = (d.x as f32 - 0.5, d.y as f32 - 0.5);
            let (x1, y1) = (x0 + d.w as f32 + 1.0, y0 + d.h as f32 + 1.0);
            let mut s = x0 + 0.15;
            while s + 0.25 < x1 {
                p.line_segment([iso.at(s, y0, 0.0), iso.at(s + 0.2, y0, 0.0)], dash);
                p.line_segment([iso.at(s, y1, 0.0), iso.at(s + 0.2, y1, 0.0)], dash);
                s += 0.4;
            }
            let mut s = y0 + 0.15;
            while s + 0.25 < y1 {
                p.line_segment([iso.at(x0, s, 0.0), iso.at(x0, s + 0.2, 0.0)], dash);
                p.line_segment([iso.at(x1, s, 0.0), iso.at(x1, s + 0.2, 0.0)], dash);
                s += 0.4;
            }
            let zebra = |x: f32, y: f32, along_x: bool| {
                for i in 0..5 {
                    let o = -0.28 + i as f32 * 0.12;
                    let (a, b) = if along_x { ((x + o, y - 0.3), (x + o + 0.06, y + 0.3)) } else { ((x - 0.3, y + o), (x + 0.3, y + o + 0.06)) };
                    poly(&p, iso.quad(a.0, a.1, b.0, b.1, 0.0).to_vec(), paint::LINE);
                }
            };
            zebra(d.x as f32 + 0.5, y0, true);
            zebra(x1, d.y as f32 + 0.5, false);
            zebra(d.x as f32 + d.w as f32 - 0.5, y1, true);
        }
        // The blocks: pavements a kerb above the road.
        for d in &city.districts {
            let (x0, y0, x1, y1) = (d.x as f32 - 0.12, d.y as f32 - 0.12, (d.x + d.w) as f32 + 0.12, (d.y + d.h) as f32 + 0.12);
            cuboid(&p, &iso, (x0, y0, x1, y1), 0.0, KERB, styles::pavement(d.style));
        }
        // The ground of the free tiles, in their district's style, and the parks'; the parks' fences.
        let style_at = |x: i32, y: i32| city.districts.iter().find(|d| x >= d.x && x < d.x + d.w && y >= d.y && y < d.y + d.h).map_or(Style::Town, |d| d.style);
        for &(x, y) in &city.trees {
            poly(&p, iso.quad(x as f32 + 0.08, y as f32 + 0.08, x as f32 + 0.92, y as f32 + 0.92, KERB).to_vec(), styles::ground(style_at(x, y)));
        }
        for &(px, py, pw, ph) in &city.parks {
            let (x0, y0, x1, y1) = (px as f32 - 0.12, py as f32 - 0.12, (px + pw) as f32 + 0.12, (py + ph) as f32 + 0.12);
            cuboid(&p, &iso, (x0, y0, x1, y1), 0.0, KERB, paint::PAVEMENT);
            let (x0, y0, x1, y1) = (x0 + 0.14, y0 + 0.14, x1 - 0.14, y1 - 0.14);
            poly(&p, iso.quad(x0, y0, x1, y1, KERB).to_vec(), paint::GRASS);
            slats(&p, &iso, (x0, y0), (x1, y0));
            slats(&p, &iso, (x0, y0), (x0, y1));
        }

        // The shadows of the plots, on the ground before anything stands up, each stopping at
        // the edge of its block.
        for pl in &city.plots {
            let (x, y) = (pl.x as f32, pl.y as f32);
            let d = &city.districts[pl.district];
            let reach = (d.x + d.w) as f32 + 0.12;
            if pl.stage.crane() {
                let top = KERB + 0.3 + frame_floors(pl) as f32 + 2.6;
                p.line_segment([iso.at(x + 0.24, y + 0.2, KERB), iso.at(shadow_end(x + 0.24, top, reach), y + 0.2, KERB)], Stroke::new(iso.px(2.0), shadow()));
            }
            if !matches!(pl.stage, Stage::Lot | Stage::Staked) {
                cast_shadow(&p, &iso, (x + 0.15, y + 0.15, x + 0.85, y + 0.85), KERB, height(pl), reach);
            }
        }

        // What stands on the tiles and drives on the roads, back to front.
        let by_id: std::collections::HashMap<&str, &Item> = items.iter().map(|it| (it.id.as_str(), *it)).collect();
        let pulls: Option<&Pulls> = self.pulls.pulls.as_deref();
        let mut things: Vec<(f32, Thing)> = Vec::new();
        // Each block's traffic: a supply lorry for each building site and one per open pull
        // request in the colour of its checks, then a few cars; all evenly spaced, clockwise
        // round the block in its inner lane.
        let mut traffic: Vec<Vec<Vehicle>> = vec![Vec::new(); city.districts.len()];
        for pl in &city.plots {
            things.push(((pl.x + pl.y) as f32 + 1.0, Thing::Plot(pl)));
            let Some(it) = by_id.get(pl.id.as_str()) else { continue };
            if pl.stage.crane() {
                traffic[pl.district].push(Vehicle::Lorry(paint::SUPPLY));
            }
            for pr in it.pull_requests.iter().filter(|l| l.state == "OPEN") {
                let ci = pulls.and_then(|ps| ps.list.iter().find(|q| q.number == pr.number)).and_then(|q| q.ci);
                traffic[pl.district].push(Vehicle::Lorry(Faces::of(ci_color(ci))));
            }
        }
        for (d, vehicles) in city.districts.iter().zip(&mut traffic) {
            let cars = (2 + d.w * d.h / 6).min(5) as usize;
            let seed = mix(d.x, d.y);
            vehicles.extend((0..cars).map(|j| Vehicle::Car(paint::CARS[(seed + j) % paint::CARS.len()])));
            let perimeter = 2.0 * (d.w + d.h + 2) as f32;
            // No more than the road round the block has room for.
            vehicles.truncate((perimeter / 0.9) as usize);
            let n = vehicles.len() as f32;
            for (j, v) in vehicles.iter().enumerate() {
                let at = in_lane(round_block(d, time * 0.25 / perimeter + (seed % 97) as f32 / 97.0 + j as f32 / n));
                things.push((at.x + at.y, Thing::Vehicle(at, *v)));
            }
            for (lx, ly) in [(d.x as f32 - 0.05, d.y as f32 - 0.05), ((d.x + d.w) as f32 + 0.05, d.y as f32 - 0.05), ((d.x + d.w) as f32 + 0.05, (d.y + d.h) as f32 + 0.05), (d.x as f32 - 0.05, (d.y + d.h) as f32 + 0.05)] {
                things.push((lx + ly, Thing::Lamp(lx, ly)));
            }
        }
        for &(x, y) in &city.trees {
            things.push(((x + y) as f32 + 1.0, Thing::Garden(x, y, style_at(x, y))));
        }
        for &(px, py, pw, ph) in &city.parks {
            for i in px..px + pw {
                for j in py..py + ph {
                    // Three trees or flower beds a tile, of every kind and colour a park mixes.
                    for k in 0..3 {
                        let h = mix(i * 3 + k, j * 5 - k);
                        let kind = Tree::PARK[h % Tree::PARK.len()];
                        let (tx, ty) = (i as f32 + 0.18 + (h % 7) as f32 * 0.1, j as f32 + 0.18 + (h / 7 % 7) as f32 * 0.1);
                        things.push((tx + ty, Thing::Tree(tx, ty, kind)));
                    }
                }
            }
        }
        things.sort_by(|a, b| a.0.total_cmp(&b.0));

        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|h| area.contains(*h) && resp.hovered() && !self.board.shooting);
        let hovered: Option<&Plot> = city.plots.iter().rev().find(|pl| hover.is_some_and(|h| plot_hit(&iso, pl).contains(h)));

        for (_, thing) in &things {
            match thing {
                Thing::Garden(x, y, style) => styles::yard(&p, &iso, (*x, *y), *style),
                Thing::Tree(x, y, kind) => tree(&p, &iso, *x, *y, *kind),
                Thing::Lamp(x, y) => lamp(&p, &iso, *x, *y),
                Thing::Vehicle(at, v) => vehicle(&p, &iso, *at, *v),
                Thing::Plot(pl) => {
                    let Some(it) = by_id.get(pl.id.as_str()) else { continue };
                    let (x, y) = (pl.x as f32, pl.y as f32);
                    let selected = detail == Some(pl.id.as_str());
                    if selected || hovered.is_some_and(|h| h.id == pl.id) {
                        let c = if selected { k.accent } else { k.accent_text };
                        p.add(Shape::convex_polygon(iso.quad(x + 0.03, y + 0.03, x + 0.97, y + 0.97, KERB).to_vec(), c.gamma_multiply(0.35), Stroke::new(2.0, c)));
                    }
                    match pl.stage {
                        Stage::Lot | Stage::Staked => lot(&p, &iso, pl),
                        Stage::Foundations | Stage::Frame => site(&p, &iso, pl, it, time),
                        Stage::Testing | Stage::Built => {
                            let roof = styles::building(&p, &iso, (x, y), city.districts[pl.district].style, pl.building, pl.floors, pl.seed);
                            if pl.stage == Stage::Testing {
                                // To test: a flag on the roof.
                                let base = roof;
                                let tip = base - vec2(0.0, iso.px(16.0));
                                p.line_segment([base, tip], Stroke::new(iso.px(1.0), paint::WIRE));
                                poly(&p, vec![tip, tip + vec2(iso.px(10.0), iso.px(3.0)), tip + vec2(0.0, iso.px(6.5))], paint::TAPE);
                            }
                        }
                    }
                }
            }
        }

        // Each block's name on a tag by its south-west edge, over the traffic.
        for d in &city.districts {
            let at = iso.at(d.x as f32 - 0.07, (d.y + d.h) as f32 + 0.54, 0.0);
            let font = FontId::proportional((theme::FS_SMALL * iso.zoom.sqrt()).max(9.0));
            let g = p.layout_no_wrap(d.name.clone(), font, paint::WIRE);
            p.rect_filled(Rect::from_min_size(at, g.size()).expand2(vec2(5.0, 2.0)), 4.0, Color32::from_white_alpha(235));
            p.galley(at, g, paint::WIRE);
        }

        // The tally, top left; the hints, bottom left.
        let (built, site, todo) = crate::city::counts(&city);
        p.text(area.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, format!("{built} built · {site} under construction · {todo} to build"), theme::secondary(), k.dim);
        p.text(area.left_bottom() + vec2(4.0, -2.0), Align2::LEFT_BOTTOM, "Drag to move · pinch or ⌘ scroll to zoom · double-click to fit", theme::small(), k.faint);

        // The cranes turn and the traffic drives while the city is on screen.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));

        let mut clicked = None;
        if let Some(pl) = hovered {
            if let Some(it) = by_id.get(pl.id.as_str()) {
                let n = it.number.map(|n| format!("#{n} ")).unwrap_or_default();
                let prs = it.pull_requests.iter().filter(|l| l.state == "OPEN").count();
                let who = if it.assignees.is_empty() { String::new() } else { format!(" · {}", it.assignees.join(", ")) };
                let more = if prs > 0 { format!(" · {prs} open pull request{}", if prs > 1 { "s" } else { "" }) } else { String::new() };
                resp.clone().on_hover_text_at_pointer(format!("{n}{}\n{}{who}{more}", it.title, it.status));
                if resp.clicked() {
                    clicked = Some(pl.id.clone());
                }
            }
        }
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn the_projection_puts_east_right_and_south_left() {
        let iso = Iso { origin: Pos2::ZERO, zoom: 1.0 };
        assert_eq!(iso.at(1.0, 0.0, 0.0), pos2(TILE_W / 2.0, TILE_H / 2.0));
        assert_eq!(iso.at(0.0, 1.0, 0.0), pos2(-TILE_W / 2.0, TILE_H / 2.0));
        assert_eq!(iso.at(0.0, 0.0, 1.0), pos2(0.0, -FLOOR));
    }

    #[test]
    fn a_built_tower_reaches_above_its_tile() {
        let iso = Iso { origin: Pos2::ZERO, zoom: 1.0 };
        let pl = Plot { id: "I1".into(), x: 2, y: 2, stage: Stage::Built, building: Building::Tower, floors: 5, district: 0, seed: 1 };
        let r = plot_hit(&iso, &pl);
        assert!(r.top() < iso.at(2.0, 2.0, 0.0).y - 3.0 * FLOOR);
        assert!(r.contains(iso.at(2.5, 2.5, 0.0)));
    }

    #[test]
    fn a_frame_rises_half_the_building() {
        let pl = |stage, floors| Plot { id: "I1".into(), x: 0, y: 0, stage, building: Building::Tower, floors, district: 0, seed: 1 };
        assert_eq!(frame_floors(&pl(Stage::Frame, 5)), 3);
        assert_eq!(frame_floors(&pl(Stage::Frame, 1)), 1);
        assert_eq!(frame_floors(&pl(Stage::Foundations, 5)), 0);
        assert!(height(&pl(Stage::Built, 5)) > height(&pl(Stage::Frame, 5)));
    }

    #[test]
    fn a_face_seen_edge_on_has_no_area() {
        let iso = Iso { origin: Pos2::ZERO, zoom: 1.0 };
        // A west roof slope 0.35 tiles across rising 32 x 0.35 / 14 floors lies along the line of sight.
        let (hw, rise) = (0.35, 32.0 * 0.35 / 14.0);
        let slope = [iso.at(0.0, 0.0, 0.0), iso.at(hw, 0.0, rise), iso.at(hw, 0.7, rise), iso.at(0.0, 0.7, 0.0)];
        assert!(area(&slope).abs() < 0.5);
        assert!(area(&iso.quad(0.0, 0.0, 1.0, 1.0, 0.0)).abs() > 100.0);
    }

    #[test]
    fn shadows_fall_along_x_and_stop_at_the_pavement() {
        // One floor reaches a little way, nine floors would cross the block: they stop at its edge.
        assert!((shadow_end(3.0, 1.0, 9.0) - (3.0 + SHADOW_PER_FLOOR)).abs() < 1e-6);
        assert_eq!(shadow_end(3.0, 9.0, 4.12), 4.12);
        // Never behind what casts it.
        assert_eq!(shadow_end(3.0, 2.0, 2.5), 3.0);
    }

    #[test]
    fn a_person_always_looks_the_same_and_people_differ() {
        assert_eq!(looks("Yonizzer"), looks("Yonizzer"));
        let tones: std::collections::HashSet<[u8; 4]> = ["enzotripoli", "Yonizzer", "camille", "lucas", "ines", "sami", "octocat", "dependabot"].iter().map(|w| looks(w).0.to_array()).collect();
        assert!(tones.len() >= 3, "only {} skin tones among eight people", tones.len());
    }

    #[test]
    fn lorries_go_round_the_block_on_the_road() {
        let d = District { name: "z".into(), x: 1, y: 1, w: 2, h: 2, style: Style::Town };
        // Along the north road first, heading east, then down the east road.
        assert_eq!(round_block(&d, 0.0), OnRoad { x: 0.5, y: 0.5, along_x: true, dir: 1.0 });
        let r = round_block(&d, 0.3);
        assert!(!r.along_x && r.x == 3.5 && r.y > 0.5);
        // Once round, back where it started.
        assert_eq!(round_block(&d, 1.0), round_block(&d, 0.0));
    }

    #[test]
    fn traffic_keeps_to_the_lane_by_its_block() {
        let d = District { name: "z".into(), x: 1, y: 1, w: 2, h: 2, style: Style::Town };
        // Heading east on the north road: the lane on the block's side, south of the middle.
        let north = in_lane(round_block(&d, 0.05));
        assert!(north.y > 0.5 && north.y < 1.0 - 0.12);
        // Heading south on the east road: west of the middle, toward the block.
        let east = in_lane(round_block(&d, 0.3));
        assert!(east.x < 3.5 && east.x > 3.0 + 0.12);
        // Heading west on the south road and north on the west road: the same, on their sides.
        let south = in_lane(round_block(&d, 0.6));
        assert!(south.y < 3.5 && south.y > 3.12);
        let west = in_lane(round_block(&d, 0.9));
        assert!(west.x > 0.5 && west.x < 0.88);
    }
}
