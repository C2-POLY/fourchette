//! The Big eater skin's food court (#212), drawn in place of the board's City view with the
//! city's isometric painter: the same layout (`crate::city::layout_with`), one restaurant per
//! zone with its striped awning and its sign in its own colour, one dish per ticket cooked as far
//! as its status goes (Backlog an order slip on the floor, Ready the raw ingredients and a cook
//! waiting, In progress on the grill in its flames, In review under a cloche, To test tasted by
//! the cook with a fork stuck in, Done served with its face). A Feature is a burger with a layer
//! per sub-issue, a Bug a bowl of chili with flies round it until it is done, the rest a pizza, a
//! donut or a soda from the ticket's seed. Each open pull request linked to a ticket is a
//! delivery scooter driving round its restaurant, its box in the colour of its checks. In the
//! middle, on the block the layout keeps free, the big eater at his table, facing the viewer
//! (`art::EATER`): his bib, his puffed cheeks, a bite of burger on the silver fork in his raised
//! hand, a red and white cloth, the dishes served and a stack of plates in front of him, his big
//! chair behind and pizza boxes on the floor. Drag to pan, pinch or ⌘-scroll to zoom, double-click to fit; a hover
//! shows the ticket, a click opens its detail. Read only.

mod dishes;

use super::city::{cuboid, in_lane, lamp, mix, on_face, poly, round_block, tree, Iso, OnRoad, Tree, FLOOR, KERB, SLAB};
use super::*;
use crate::city::{Building, District, Plot, Stage, Style};
use crate::github::Item;
use crate::pulls::{Ci, Pulls};
use dishes::{faces, Dish, Pose};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2};

/// The big eater's block, in tiles.
const DEN: (i32, i32) = (4, 4);
/// Room left at the bottom of the board for the daily menu bar, which covers it.
const BAR_ROOM: f32 = 100.0;
/// The big eater's drawing, in points per unit at zoom 1.
const EATER_SCALE: f32 = 0.8;
/// How high a table's top stands above the stand's floor, in floors.
const TABLE: f32 = 0.62;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// A restaurant's colour and the ink on its sign: each district style is a kitchen of its own.
fn restaurant(style: Style) -> (Color32, Color32) {
    match style {
        Style::Town => (rgb(0xff5a4e), rgb(0xffffff)),
        Style::Mediterranean => (rgb(0x3fb950), rgb(0x06210d)),
        Style::Glass => (rgb(0x3b82c4), rgb(0xffffff)),
        Style::Industrial => (rgb(0xffc93c), rgb(0x2e2508)),
        Style::Japanese => (rgb(0xe8478b), rgb(0xffffff)),
        Style::Nordic => (rgb(0x3fc5c5), rgb(0x062626)),
        Style::Desert => (rgb(0xff7a1a), rgb(0x2a1200)),
        Style::Dutch => (rgb(0x8b5cf6), rgb(0xffffff)),
    }
}

/// The floor's colours: the aisles, the slab's sides, and the ketchup and mustard lines.
struct Floor {
    aisle: Color32,
    slab: (Color32, Color32),
    lines: [Color32; 2],
}

fn floor() -> Floor {
    if theme::is_light() {
        Floor { aisle: rgb(0xd9d3c7), slab: (rgb(0xb9b2a4), rgb(0xa39c8e)), lines: [rgb(0xe8473b), rgb(0xe8a21a)] }
    } else {
        Floor { aisle: rgb(0x3a3c43), slab: (rgb(0x2b2d33), rgb(0x222429)), lines: [rgb(0xff5a4e), rgb(0xffc93c)] }
    }
}

/// The tones of a stand's floor tiles, from its restaurant's colour.
fn tiles(col: Color32) -> (Color32, Color32) {
    let cream = rgb(0xf4ecd8);
    (theme::tint(col, cream, 0.12), theme::tint(col, cream, 0.24))
}

fn ci_colour(ci: Option<Ci>) -> Color32 {
    match ci {
        Some(Ci::Success) => rgb(0x3fb950),
        Some(Ci::Failure) => rgb(0xf85149),
        Some(Ci::Pending) => rgb(0xe3a008),
        None => rgb(0x9aa0a6),
    }
}

/// Something to draw, at its depth: the farther from the viewer, the sooner.
enum Thing<'a> {
    Sign(&'a District),
    Plot(&'a Plot),
    /// A free tile of a stand: a bistro table under a parasol.
    Bistro(i32, i32, Color32),
    Plant(f32, f32, Tree),
    Lamp(f32, f32),
    Scooter(OnRoad, Color32, u64),
    /// The big eater's chair, then him, then his table and what is on it.
    Chair,
    Eater,
    Table,
    /// The pizza boxes piled on the floor beside him.
    Boxes,
}

/// The screen box a plot takes, for hovering: its tile and its dish.
fn plot_hit(iso: &Iso, pl: &Plot) -> Rect {
    let (x, y) = (pl.x as f32, pl.y as f32);
    let mut r = Rect::from_points(&iso.quad(x, y, x + 1.0, y + 1.0, KERB));
    let up = if pl.stage == Stage::Lot { 0.4 } else { TABLE + 0.4 + Dish::of(pl).height() / FLOOR };
    r.extend_with(iso.at(x + 0.5, y + 0.5, KERB + up));
    r
}

/// A table on four legs, its top at `TABLE` over the stand, in the middle of the tile at (x, y).
fn table(p: &Painter, iso: &Iso, (cx, cy): (f32, f32), w: f32) {
    let wood = faces(rgb(0xa8714a));
    let leg = faces(rgb(0x6b4a32));
    let (x0, y0, x1, y1) = (cx - w, cy - w, cx + w, cy + w);
    for (lx, ly) in [(x0 + 0.03, y0 + 0.03), (x1 - 0.09, y0 + 0.03), (x0 + 0.03, y1 - 0.09), (x1 - 0.09, y1 - 0.09)] {
        cuboid(p, iso, (lx, ly, lx + 0.06, ly + 0.06), KERB, KERB + TABLE - 0.08, leg);
    }
    cuboid(p, iso, (x0, y0, x1, y1), KERB + TABLE - 0.08, KERB + TABLE, wood);
}

/// The hard shadow of a table or a grill on the stand's floor, along x.
fn table_shadow(p: &Painter, iso: &Iso, (cx, cy): (f32, f32), w: f32, reach: f32) {
    let end = (cx + w + 0.35).min(reach);
    poly(p, iso.quad(cx - w, cy - w + 0.05, end, cy + w, KERB).to_vec(), Color32::from_rgba_unmultiplied(30, 32, 40, 60));
}

/// A plot: what its ticket's status puts on it, its cook and its flies.
fn plot(p: &Painter, iso: &Iso, pl: &Plot, it: &Item, col: Color32, time: f32) {
    let (x, y) = (pl.x as f32, pl.y as f32);
    let (cx, cy) = (x + 0.5, y + 0.45);
    let s = iso.zoom;
    let d = Dish::of(pl);
    let phase = (pl.seed % 97) as f32 * 0.37;
    let t = time + phase;
    let who = it.assignees.first().map_or("?", String::as_str);
    let tone = dishes::skin(who);
    let cook_at = iso.at(x + 0.86, y + 0.95, KERB);
    let top = iso.at(cx, cy, KERB + TABLE);
    let mut flies_at = None;
    match pl.stage {
        Stage::Lot => {
            // The order slip on the floor, its lines and its tab in the restaurant's colour.
            let slip = iso.quad(cx - 0.3, cy - 0.18, cx + 0.22, cy + 0.2, KERB);
            poly(p, slip.to_vec(), rgb(0xf7f3e8));
            for o in [-0.08, 0.02, 0.12] {
                poly(p, iso.quad(cx - 0.22, cy + o - 0.015, cx + 0.08, cy + o + 0.015, KERB).to_vec(), rgb(0xb9b2a2));
            }
            poly(p, iso.quad(cx + 0.22, cy - 0.18, cx + 0.32, cy + 0.2, KERB).to_vec(), col);
            flies_at = Some(iso.at(cx, cy, KERB + 1.0));
        }
        Stage::Staked => {
            table(p, iso, (cx, cy), 0.3);
            dishes::plate(p, top, s);
            dishes::raw(p, top, s, d);
            dishes::cook(p, cook_at, s, tone, Pose::Wait, t);
            flies_at = Some(top - vec2(0.0, 18.0 * s));
        }
        Stage::Foundations => {
            // The grill: a black box, its bars glowing, what cooks on it, the flames and smoke.
            let (x0, y0, x1, y1) = (cx - 0.34, cy - 0.28, cx + 0.34, cy + 0.28);
            cuboid(p, iso, (x0, y0, x1, y1), KERB, KERB + TABLE, faces(rgb(0x2b2d33)));
            let glow = 0.75 + 0.25 * (t * 4.0).sin();
            for k in 0..6 {
                let gx = x0 + 0.08 + k as f32 * 0.105;
                poly(p, iso.quad(gx, y0 + 0.05, gx + 0.03, y1 - 0.05, KERB + TABLE).to_vec(), rgb(0xff7a1a).gamma_multiply(glow));
            }
            let gt = iso.at(cx, cy, KERB + TABLE);
            match d {
                Dish::Chili(_) => {
                    let pot = dishes::cyl(p, gt, 12.0 * s, 9.0 * s, (rgb(0x6d7075), dishes::INK));
                    dishes::oval(p, pot, 10.0 * s, 5.0 * s, rgb(0xa3321f));
                    let b = (t * 3.0).sin().abs();
                    dishes::oval(p, pot + vec2(-3.0 * s, -0.5 * s), (1.0 + b) * s, (0.6 + b * 0.5) * s, rgb(0xd04a30));
                }
                Dish::Pizza | Dish::Donut(_) => {
                    dishes::oval(p, gt, 13.0 * s, 6.5 * s, rgb(0xf3e4c8));
                    dishes::oval(p, gt, 10.0 * s, 5.0 * s, rgb(0xe2553a));
                }
                _ => {
                    for (dx, dy) in [(-9.0, 1.0), (8.0, -1.0)] {
                        dishes::cyl(p, gt + vec2(dx * s, dy * s), 7.5 * s, 2.4 * s, (rgb(0x4a2a19), rgb(0x6b3d24)));
                    }
                }
            }
            dishes::flames(p, iso.at(x0, y1, KERB + TABLE), iso.at(x1, y1, KERB + TABLE), gt, s, t);
            dishes::cook(p, cook_at, s, tone, Pose::Flip, t);
            flies_at = Some(gt - vec2(0.0, 22.0 * s));
        }
        Stage::Frame => {
            table(p, iso, (cx, cy), 0.3);
            dishes::plate(p, top, s);
            dishes::cloche(p, top, s);
            // The service bell beside it.
            let bell = iso.at(cx + 0.24, cy + 0.2, KERB + TABLE);
            dishes::oval(p, bell, 3.2 * s, 1.6 * s, rgb(0x6d7075));
            dishes::cyl(p, bell, 2.4 * s, 2.4 * s, (rgb(0xe3c53a), rgb(0xffd66b)));
            flies_at = Some(top - vec2(0.0, 26.0 * s));
        }
        Stage::Testing => {
            table(p, iso, (cx, cy), 0.3);
            dishes::plate(p, top, s);
            let small = match d {
                Dish::Burger(n) => Dish::Burger(n.min(1)),
                other => other,
            };
            dishes::dish(p, top, s * 0.85, small, t, false);
            dishes::fork_in(p, top - vec2(0.0, small.height() * s * 0.85), s);
            dishes::cook(p, cook_at, s, tone, Pose::Taste, t);
            flies_at = Some(top - vec2(0.0, (small.height() + 12.0) * s));
        }
        Stage::Built => {
            table(p, iso, (cx, cy), 0.3);
            dishes::plate(p, top, s);
            dishes::dish(p, top, if matches!(d, Dish::Burger(_)) { s * 1.1 } else { s }, d, t, true);
        }
    }
    // Flies round a bug until it is done.
    if let (Dish::Chili(_), Some(at)) = (d, flies_at) {
        dishes::flies(p, at, s, time, 3, phase);
    }
}

/// The restaurant's sign over the back of its stand: two poles, a striped awning sloping over
/// the first row, and the board in its colour with the zone's name.
fn sign(p: &Painter, iso: &Iso, d: &District) {
    let (col, ink) = restaurant(d.style);
    let (x0, x1) = (d.x as f32 - 0.05, (d.x + d.w) as f32 + 0.05);
    let y = d.y as f32 - 0.08;
    let pole = faces(rgb(0x2b2e35));
    for px in [x0 + 0.05, x1 - 0.12] {
        cuboid(p, iso, (px, y - 0.04, px + 0.07, y + 0.03), KERB, KERB + 2.5, pole);
    }
    // The awning: stripes from the board down toward the stand.
    let (z0, z1, reach) = (KERB + 2.35, KERB + 1.95, 0.38);
    let n = ((x1 - x0) / 0.22).round().max(2.0) as i32;
    let w = (x1 - x0) / n as f32;
    for k in 0..n {
        let a = x0 + k as f32 * w;
        let c = if k % 2 == 0 { col } else { rgb(0xf7f8f8) };
        poly(p, vec![iso.at(a, y, z0), iso.at(a + w, y, z0), iso.at(a + w, y + reach, z1), iso.at(a, y + reach, z1)], c);
        // Its scalloped edge.
        let mid = iso.at(a + w / 2.0, y + reach, z1);
        poly(p, vec![iso.at(a, y + reach, z1), iso.at(a + w, y + reach, z1), mid + vec2(0.0, iso.px(4.0))], theme::shade(c, 0.1));
    }
    // The board, its rim lit, and the name.
    let board = on_face(iso, true, y - 0.02, (x0, x1), (KERB + 2.35, KERB + 3.45));
    poly(p, board, col);
    let rim = on_face(iso, true, y - 0.02, (x0, x1), (KERB + 3.35, KERB + 3.45));
    poly(p, rim, theme::lighten(col, 0.3));
    let name = d.name.to_uppercase();
    let along = iso.at(x1, y, 0.0) - iso.at(x0, y, 0.0);
    let room = along.length() - iso.px(14.0);
    let mut size = iso.px(11.0).min(15.0 * iso.zoom.sqrt());
    let mut g = p.layout_no_wrap(name.clone(), FontId::new(size, egui::FontFamily::Name(theme::MEDIUM.into())), ink);
    if g.size().x > room {
        size *= room / g.size().x;
        g = p.layout_no_wrap(name, FontId::new(size, egui::FontFamily::Name(theme::MEDIUM.into())), ink);
    }
    if size >= 6.0 {
        // Turned to lie along the board, which runs down to the right.
        let angle = along.y.atan2(along.x);
        let c = iso.at((x0 + x1) / 2.0, y - 0.02, KERB + 2.86);
        let half = g.size() / 2.0;
        let (sin, cos) = angle.sin_cos();
        let pos = c - vec2(half.x * cos - half.y * sin, half.x * sin + half.y * cos);
        p.add(egui::epaint::TextShape::new(pos, g, ink).with_angle(angle));
    }
}

/// A free tile of a stand: a bistro table and two stools under a parasol.
fn bistro(p: &Painter, iso: &Iso, (x, y): (i32, i32), col: Color32) {
    let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
    let s = iso.zoom;
    let stool = faces(rgb(0x3a3d45));
    for (sx, sy) in [(cx - 0.32, cy - 0.05), (cx + 0.05, cy + 0.3)] {
        cuboid(p, iso, (sx - 0.07, sy - 0.07, sx + 0.07, sy + 0.07), KERB, KERB + 0.38, stool);
    }
    dishes::cyl(p, iso.at(cx, cy, KERB), 2.0 * s, TABLE * FLOOR * s, (rgb(0x6d7075), rgb(0x6d7075)));
    dishes::cyl(p, iso.at(cx, cy, KERB + TABLE - 0.06), 13.0 * s, 0.06 * FLOOR * s, (rgb(0xc9ccd4), rgb(0xeef0f4)));
    dishes::parasol(p, iso.at(cx, cy, KERB + TABLE), s, col);
}

/// The big eater's block (`r`: top-left tile and size): a wine-red floor, a rug and a row of
/// mustard tiles along its front.
fn den_floor(p: &Painter, iso: &Iso, r: (i32, i32, i32, i32)) {
    let (x, y, w, h) = (r.0 as f32, r.1 as f32, r.2 as f32, r.3 as f32);
    cuboid(p, iso, (x + 0.15 - 0.12, y + 0.15 - 0.12, x + w - 0.15 + 0.12, y + h - 0.15 + 0.12), 0.0, KERB, faces(rgb(0x3a1f1a)));
    poly(p, iso.quad(x + 0.3, y + 0.3, x + w - 0.3, y + h - 0.3, KERB).to_vec(), rgb(0x4a2820));
    for t in 0..7 {
        let a = x + 0.3 + t as f32 * 0.5;
        poly(p, iso.quad(a, y + h - 0.45, a + 0.25, y + h - 0.3, KERB).to_vec(), rgb(0xe9a85c));
    }
}

/// The table's corners and its top.
struct Board {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    z: f32,
}

fn board_of(r: (i32, i32, i32, i32)) -> Board {
    let (x, y) = (r.0 as f32, r.1 as f32);
    Board { x0: x + 0.45, y0: y + 1.75, x1: x + 3.55, y1: y + 2.55, z: KERB + 1.0 }
}

/// The big chair behind him: its back stands high over his head.
fn chair(p: &Painter, iso: &Iso, r: (i32, i32, i32, i32)) {
    // Straight behind him on screen: as far back along x as along y.
    let xc = r.0 as f32 + 1.55;
    let y = r.1 as f32;
    let wine = super::city::Faces { left: rgb(0x7a2e2e), right: rgb(0x5c2222), top: rgb(0x934040) };
    cuboid(p, iso, (xc - 0.62, y + 0.7, xc + 0.62, y + 0.9), KERB, KERB + 4.6, wine);
    // A gold trim along its top, and its buttoned padding above his shoulders.
    cuboid(p, iso, (xc - 0.68, y + 0.67, xc + 0.68, y + 0.93), KERB + 4.6, KERB + 4.78, super::city::Faces { left: rgb(0xd69a1e), right: rgb(0xb07c12), top: rgb(0xf2c14e) });
    for (dx, dz) in [(-0.42, 4.1), (0.0, 4.25), (0.42, 4.1), (-0.5, 3.5), (0.5, 3.5)] {
        p.circle_filled(iso.at(xc + dx, y + 0.91, KERB + dz), iso.px(1.6), rgb(0x4a1a1a));
    }
}

/// The big eater on his chair, facing the viewer, munching: a bite of burger on the silver fork
/// in his raised hand, eaten every few seconds.
fn eater(p: &Painter, iso: &Iso, r: (i32, i32, i32, i32), time: f32) {
    let z = iso.zoom;
    let seat = iso.at(r.0 as f32 + 2.0, r.1 as f32 + 1.25, KERB);
    // He chews: a small bounce, faster while the bite is in.
    let k = (time / 3.2).fract();
    let chew = (time * if k > 0.86 { 14.0 } else { 3.0 }).sin().abs();
    let e = EATER_SCALE * z;
    let o = seat - vec2(50.0, 104.0 + chew * 1.2) * e;
    super::art::eater(p, o, e);
    // The silver fork held almost upright in his raised fist, tines up, the bite of burger on
    // them (the canvas's drawings, sized as it places them at its scale of 1.1). The handle's
    // end sits in the fist and the fork turns round it; the hand bobs a little as he waves it,
    // and the bite goes for a moment as he eats it.
    let f = e / 1.1;
    let (fk, bk) = (0.22 * f, 0.32 * f);
    let fist = o + super::art::EATER_HAND * e - vec2(0.0, (time * 2.0).sin().abs() * 1.5 * f);
    let tilt = -10.0 + (time * 1.3).sin() * 2.0;
    let (sin, cos) = tilt.to_radians().sin_cos();
    let turn = |v: Vec2| vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
    // The handle's end (32, 132) of the fork's box in the fist, the tines' middle (32, 26).
    let fork_at = fist - turn(vec2(32.0, 132.0) * fk);
    super::art::draw(p, super::art::FORK_SILVER, fork_at, fk, tilt, 1.3);
    if !(0.86..0.95).contains(&k) {
        // The burger's patty (32, 35) on the tines.
        let tines = fork_at + turn(vec2(32.0, 26.0) * fk);
        super::art::draw(p, super::art::BURGER, tines - turn(vec2(32.0, 35.0) * bk), bk, tilt, 1.4);
    }
}

/// His table, its red and white cloth, and what is on it: a pizza, a stack of plates, a burger
/// and a soda.
fn eater_table(p: &Painter, iso: &Iso, r: (i32, i32, i32, i32), time: f32) {
    let b = board_of(r);
    let s = iso.zoom;
    let cream = rgb(0xf7f3e8);
    cuboid(p, iso, (b.x0, b.y0, b.x1, b.y1), KERB, b.z, super::city::Faces { left: rgb(0xc8352b), right: rgb(0xa32a22), top: cream });
    // The cloth's stripes and its hem, on both faces the viewer sees.
    let mut a = b.x0 + 0.12;
    while a < b.x1 - 0.1 {
        poly(p, on_face(iso, true, b.y1 + 0.001, (a, a + 0.14), (KERB + 0.05, b.z)), cream);
        a += 0.32;
    }
    let mut a = b.y0 + 0.1;
    while a < b.y1 - 0.1 {
        poly(p, on_face(iso, false, b.x1 + 0.001, (a, a + 0.14), (KERB + 0.05, b.z)), rgb(0xe6e0d0));
        a += 0.32;
    }
    poly(p, on_face(iso, true, b.y1 + 0.002, (b.x0, b.x1), (b.z - 0.32, b.z - 0.18)), cream);
    poly(p, on_face(iso, false, b.x1 + 0.002, (b.y0, b.y1), (b.z - 0.32, b.z - 0.18)), rgb(0xe6e0d0));
    let top = |x: f32, y: f32| iso.at(x, y, b.z);
    let xc = (b.x0 + b.x1) / 2.0;
    // The pizza on the left, the stack of plates, the burger on the right, a soda.
    dishes::plate(p, top(b.x0 + 0.5, b.y0 + 0.42), s);
    dishes::dish(p, top(b.x0 + 0.5, b.y0 + 0.42), s * 0.85, Dish::Pizza, time, true);
    let st = top(xc - 0.15, b.y0 + 0.45);
    for k in 0..6 {
        dishes::cyl(p, st - vec2(0.0, k as f32 * 3.2 * s), 15.0 * s, 2.4 * s, (rgb(0xd6d8de), rgb(0xf2f2f2)));
    }
    dishes::plate(p, top(b.x1 - 0.55, b.y0 + 0.42), s);
    dishes::dish(p, top(b.x1 - 0.55, b.y0 + 0.42), s, Dish::Burger(2), time, true);
    dishes::dish(p, top(xc + 0.55, b.y0 + 0.25), s * 0.8, Dish::Soda(0), time, true);
}

/// The pizza boxes piled on the floor beside him.
fn boxes(p: &Painter, iso: &Iso, r: (i32, i32, i32, i32)) {
    let (x, y) = (r.0 as f32, r.1 as f32);
    let card = super::city::Faces { left: rgb(0xd8cbb0), right: rgb(0xb9ab8f), top: rgb(0xefe4cc) };
    for (i, z) in [0.0, 0.14, 0.28, 0.42].iter().enumerate() {
        let o = (i % 2) as f32 * 0.08;
        cuboid(p, iso, (x + 2.95 + o, y + 2.85 - o, x + 3.7 + o, y + 3.6 - o), KERB + z, KERB + z + 0.13, card);
        poly(p, on_face(iso, true, y + 3.6 - o + 0.001, (x + 3.1 + o, x + 3.3 + o), (KERB + z + 0.03, KERB + z + 0.1)), rgb(0xff5a4e));
    }
}

/// A scooter going round its restaurant's block.
fn scooter(p: &Painter, iso: &Iso, at: OnRoad, col: Color32, number: u64) {
    // Heading +x on screen goes right, heading +y goes left.
    let right = (at.along_x && at.dir > 0.0) || (!at.along_x && at.dir < 0.0);
    dishes::scooter(p, iso.at(at.x, at.y, 0.0), iso.zoom * 0.8, right, col, Some(number));
}

impl Tab {
    /// Draws the food court of `items` (the tickets the board's filters keep) and returns the
    /// ticket clicked.
    pub(super) fn court_view(&mut self, ui: &mut Ui, items: &[&Item], detail: Option<&str>) -> Option<String> {
        let city = crate::city::layout_with(items, Some(DEN));
        let area = ui.available_rect_before_wrap().shrink2(vec2(12.0, 6.0));
        // Fitted above the daily menu bar, which covers the bottom of the window.
        let room = Rect::from_min_max(area.min, pos2(area.right(), (area.bottom() - BAR_ROOM).max(area.top() + 200.0)));
        let (resp, iso) = self.iso_view(ui, area, room, city.size);
        let p = ui.painter().with_clip_rect(area);
        let k = theme::c();
        let time = ui.input(|i| i.time) as f32;
        let (cols, rows) = (city.size.0.max(1) as f32, city.size.1.max(1) as f32);
        let fl = floor();

        // The slab and the aisles, their ketchup and mustard lines round each stand.
        cuboid(&p, &iso, (0.0, 0.0, cols, rows), -SLAB, 0.0, super::city::Faces { left: fl.slab.0, right: fl.slab.1, top: fl.aisle });
        poly(&p, iso.quad(0.0, 0.0, cols, rows, 0.0).to_vec(), fl.aisle);
        let blocks: Vec<(i32, i32, i32, i32)> = city.districts.iter().map(|d| (d.x, d.y, d.w, d.h)).chain(city.reserved).collect();
        for &(bx, by, bw, bh) in &blocks {
            let (x0, y0) = (bx as f32 - 0.5, by as f32 - 0.5);
            let (x1, y1) = (x0 + bw as f32 + 1.0, y0 + bh as f32 + 1.0);
            let mut i = 0;
            let mut s = x0 + 0.15;
            while s + 0.25 < x1 {
                let c = fl.lines[i % 2];
                poly(&p, iso.quad(s, y0 - 0.03, s + 0.2, y0 + 0.03, 0.0).to_vec(), c);
                poly(&p, iso.quad(s, y1 - 0.03, s + 0.2, y1 + 0.03, 0.0).to_vec(), c);
                s += 0.4;
                i += 1;
            }
            let mut s = y0 + 0.15;
            while s + 0.25 < y1 {
                let c = fl.lines[i % 2];
                poly(&p, iso.quad(x0 - 0.03, s, x0 + 0.03, s + 0.2, 0.0).to_vec(), c);
                poly(&p, iso.quad(x1 - 0.03, s, x1 + 0.03, s + 0.2, 0.0).to_vec(), c);
                s += 0.4;
                i += 1;
            }
        }
        // The stands: a kerb, and floor tiles in their restaurant's colours.
        for d in &city.districts {
            let (col, _) = restaurant(d.style);
            let (a, b) = tiles(col);
            let (x0, y0, x1, y1) = (d.x as f32 - 0.12, d.y as f32 - 0.12, (d.x + d.w) as f32 + 0.12, (d.y + d.h) as f32 + 0.12);
            cuboid(&p, &iso, (x0, y0, x1, y1), 0.0, KERB, faces(theme::shade(b, 0.08)));
            for i in 0..d.w {
                for j in 0..d.h {
                    let (tx, ty) = ((d.x + i) as f32, (d.y + j) as f32);
                    poly(&p, iso.quad(tx, ty, tx + 1.0, ty + 1.0, KERB).to_vec(), if (i + j) % 2 == 0 { a } else { b });
                }
            }
        }
        // The lounges where a row ends short: a carpet.
        for &(px, py, pw, ph) in &city.parks {
            let (x0, y0, x1, y1) = (px as f32 - 0.12, py as f32 - 0.12, (px + pw) as f32 + 0.12, (py + ph) as f32 + 0.12);
            cuboid(&p, &iso, (x0, y0, x1, y1), 0.0, KERB, faces(rgb(0x5a4a3a)));
            poly(&p, iso.quad(x0 + 0.14, y0 + 0.14, x1 - 0.14, y1 - 0.14, KERB).to_vec(), rgb(0x3d6b4a));
        }
        if let Some(r) = city.reserved {
            den_floor(&p, &iso, r);
        }
        // The shadows of the tables and grills.
        for pl in &city.plots {
            if pl.stage != Stage::Lot {
                let d = &city.districts[pl.district];
                table_shadow(&p, &iso, (pl.x as f32 + 0.5, pl.y as f32 + 0.45), 0.32, (d.x + d.w) as f32 + 0.12);
            }
        }

        // What stands on the stands and drives in the aisles, back to front.
        let by_id: std::collections::HashMap<&str, &Item> = items.iter().map(|it| (it.id.as_str(), *it)).collect();
        let pulls: Option<&Pulls> = self.pulls.pulls.as_deref();
        let mut things: Vec<(f32, Thing)> = Vec::new();
        for d in &city.districts {
            things.push(((d.x + d.y) as f32 - 0.5, Thing::Sign(d)));
            for (lx, ly) in [((d.x + d.w) as f32 + 0.05, d.y as f32 - 0.05), ((d.x + d.w) as f32 + 0.05, (d.y + d.h) as f32 + 0.05), (d.x as f32 - 0.05, (d.y + d.h) as f32 + 0.05)] {
                things.push((lx + ly, Thing::Lamp(lx, ly)));
            }
        }
        // Each block's scooters: one per open pull request of its tickets, evenly spaced round
        // it in its inner lane.
        let mut traffic: Vec<Vec<(Color32, u64)>> = vec![Vec::new(); city.districts.len()];
        for pl in &city.plots {
            things.push(((pl.x + pl.y) as f32 + 1.0, Thing::Plot(pl)));
            let Some(it) = by_id.get(pl.id.as_str()) else { continue };
            for pr in it.pull_requests.iter().filter(|l| l.state == "OPEN") {
                let ci = pulls.and_then(|ps| ps.list.iter().find(|q| q.number == pr.number)).and_then(|q| q.ci);
                traffic[pl.district].push((ci_colour(ci), pr.number));
            }
        }
        for (d, scooters) in city.districts.iter().zip(&mut traffic) {
            let perimeter = 2.0 * (d.w + d.h + 2) as f32;
            scooters.truncate((perimeter / 0.9) as usize);
            let n = scooters.len() as f32;
            let seed = mix(d.x, d.y);
            for (j, (c, number)) in scooters.iter().enumerate() {
                let at = in_lane(round_block(d, time * 0.3 / perimeter + (seed % 97) as f32 / 97.0 + j as f32 / n));
                things.push((at.x + at.y, Thing::Scooter(at, *c, *number)));
            }
        }
        let colour_at = |x: i32, y: i32| city.districts.iter().find(|d| x >= d.x && x < d.x + d.w && y >= d.y && y < d.y + d.h).map_or(rgb(0xff5a4e), |d| restaurant(d.style).0);
        for &(x, y) in &city.trees {
            let in_park = city.parks.iter().any(|&(px, py, pw, ph)| x >= px && x < px + pw && y >= py && y < py + ph);
            if in_park {
                let h = mix(x, y);
                if h.is_multiple_of(2) {
                    let c = [rgb(0xff5a4e), rgb(0x3b82c4), rgb(0xffc93c), rgb(0x3fb950), rgb(0xe8478b)][h / 2 % 5];
                    things.push(((x + y) as f32 + 1.0, Thing::Bistro(x, y, c)));
                } else {
                    let (tx, ty) = (x as f32 + 0.5, y as f32 + 0.5);
                    things.push((tx + ty, Thing::Plant(tx, ty, Tree::PARK[h % Tree::PARK.len()])));
                }
            } else {
                things.push(((x + y) as f32 + 1.0, Thing::Bistro(x, y, colour_at(x, y))));
            }
        }
        if let Some((rx, ry, _, _)) = city.reserved {
            let base = (rx + ry) as f32;
            things.push((base + 4.0, Thing::Chair));
            things.push((base + 4.01, Thing::Eater));
            things.push((base + 5.0, Thing::Table));
            things.push((base + 6.5, Thing::Boxes));
        }
        things.sort_by(|a, b| a.0.total_cmp(&b.0));

        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|h| area.contains(*h) && resp.hovered() && !self.board.shooting);
        let hovered: Option<&Plot> = city.plots.iter().rev().find(|pl| hover.is_some_and(|h| plot_hit(&iso, pl).contains(h)));

        for (_, thing) in &things {
            match thing {
                Thing::Sign(d) => sign(&p, &iso, d),
                Thing::Lamp(x, y) => lamp(&p, &iso, *x, *y),
                Thing::Plant(x, y, kind) => tree(&p, &iso, *x, *y, *kind),
                Thing::Bistro(x, y, c) => bistro(&p, &iso, (*x, *y), *c),
                Thing::Scooter(at, c, n) => scooter(&p, &iso, *at, *c, *n),
                Thing::Chair => chair(&p, &iso, city.reserved.unwrap_or_default()),
                Thing::Eater => eater(&p, &iso, city.reserved.unwrap_or_default(), time),
                Thing::Table => eater_table(&p, &iso, city.reserved.unwrap_or_default(), time),
                Thing::Boxes => boxes(&p, &iso, city.reserved.unwrap_or_default()),
                Thing::Plot(pl) => {
                    let Some(it) = by_id.get(pl.id.as_str()) else { continue };
                    let (x, y) = (pl.x as f32, pl.y as f32);
                    let selected = detail == Some(pl.id.as_str());
                    if selected || hovered.is_some_and(|h| h.id == pl.id) {
                        let c = if selected { k.accent } else { k.accent_text };
                        p.add(Shape::convex_polygon(iso.quad(x + 0.03, y + 0.03, x + 0.97, y + 0.97, KERB).to_vec(), c.gamma_multiply(0.35), Stroke::new(2.0, c)));
                    }
                    plot(&p, &iso, pl, it, restaurant(city.districts[pl.district].style).0, time);
                }
            }
        }

        // Each stand's name on a tag by its south-west edge, over the traffic, for when the sign
        // is too small to read.
        if iso.zoom < 0.75 {
            for d in &city.districts {
                let at = iso.at(d.x as f32 - 0.07, (d.y + d.h) as f32 + 0.54, 0.0);
                let font = FontId::proportional((theme::FS_SMALL * iso.zoom.sqrt()).max(9.0));
                let g = p.layout_no_wrap(d.name.clone(), font, rgb(0x2b2d33));
                p.rect_filled(Rect::from_min_size(at, g.size()).expand2(vec2(5.0, 2.0)), CornerRadius::same(4), Color32::from_white_alpha(235));
                p.galley(at, g, rgb(0x2b2d33));
            }
        }

        // The tally, top left; the hints just above the menu bar.
        let (served, cooking, todo) = crate::city::counts(&city);
        p.text(area.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, format!("{served} served · {cooking} cooking · {todo} on order"), theme::secondary(), k.dim);
        p.text(pos2(area.left() + 4.0, room.bottom() - 2.0), Align2::LEFT_BOTTOM, "Drag to move · pinch or ⌘ scroll to zoom · double-click to fit", theme::small(), k.faint);

        // The flames, the scooters and the big eater move while the court is on screen.
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


    fn plot(stage: Stage, building: Building, floors: u32, seed: u32) -> Plot {
        Plot { id: "I1".into(), x: 2, y: 3, stage, building, floors, district: 0, seed }
    }

    #[test]
    fn tickets_become_their_dishes() {
        assert_eq!(Dish::of(&plot(Stage::Built, Building::Tower, 6, 1)), Dish::Burger(3));
        assert_eq!(Dish::of(&plot(Stage::Built, Building::Tower, 3, 1)), Dish::Burger(1), "a feature without sub-issues still has a layer");
        assert_eq!(Dish::of(&plot(Stage::Built, Building::Tower, 9, 1)), Dish::Burger(5));
        assert!(matches!(Dish::of(&plot(Stage::Built, Building::Workshop, 2, 1)), Dish::Chili(_)));
        let houses: std::collections::HashSet<std::mem::Discriminant<Dish>> = (0..30).map(|s| std::mem::discriminant(&Dish::of(&plot(Stage::Built, Building::House, 3, s)))).collect();
        assert_eq!(houses.len(), 3, "pizzas, donuts and sodas");
        assert_eq!(Dish::of(&plot(Stage::Built, Building::House, 3, 7)), Dish::of(&plot(Stage::Staked, Building::House, 3, 7)));
    }

    #[test]
    fn neighbouring_restaurants_have_their_own_colours() {
        let colours: std::collections::HashSet<[u8; 4]> = Style::ALL.iter().map(|s| restaurant(*s).0.to_array()).collect();
        assert_eq!(colours.len(), Style::ALL.len());
    }

    #[test]
    fn a_burger_reaches_higher_with_its_layers() {
        let iso = Iso { origin: Pos2::ZERO, zoom: 1.0 };
        let low = plot_hit(&iso, &plot(Stage::Built, Building::Tower, 3, 1));
        let high = plot_hit(&iso, &plot(Stage::Built, Building::Tower, 8, 1));
        assert!(high.top() < low.top());
        assert!(low.contains(iso.at(2.5, 3.5, 0.0)));
        let slip = plot_hit(&iso, &plot(Stage::Lot, Building::Tower, 8, 1));
        assert!(slip.top() > low.top());
    }
}
