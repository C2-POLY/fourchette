//! The Gamer skin's room (#218), drawn in place of the board's City view with the city's
//! isometric painter (`Tab::iso_view`: drag pans, pinch or ⌘-scroll zooms, double-click fits; a
//! hover shows the ticket, a click opens its detail). The gamer's room at night: violet walls, an
//! RGB strip along their top, a neon sign over each shelf (`crate::room::layout`, one per zone),
//! a game box per ticket in the state its status gives (a ghost box on the wishlist for Backlog,
//! shrink-wrapped for Ready, a lime glow and a loading bar for In progress, a pause badge for In
//! review, a BETA sticker for To test, a trophy on top for Done; a castle for a feature with a pip
//! per sub-issue, a beetle and a little bug crawling on it for a bug, a star for the rest). In
//! the middle, the gamer from behind at his desk: the code screen, the game screen, the RGB
//! keyboard, his chips. A game pad per open pull request, its light in the colour of its checks.
//! The session (`crate::xp`) shows through `Look`: his reaction to the last action for 1.5 s, the
//! room lime, red during a boss fight and gold for 3 s after a level, and the mess
//! (`crate::room::litter`), tidied into a bin bag by the GG card. Read only.

use super::art::gamer::{self as g, Pose};
use super::art::{self, Part};
use super::city::{Faces, Iso, FLOOR, TILE_W};
use super::*;
use crate::city::{Building, Stage};
use crate::github::Item;
use crate::pulls::Ci;
use crate::room::{self, GameBox, Place, Shelf, Wall};
use crate::xp::Piece;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2};

/// The canvas draws the room with tiles 76 points wide and floors 17 points high: its heights
/// in the city's floors, and its points in the city's.
const ZK: f32 = 17.0 / FLOOR * (TILE_W / 2.0) / 38.0;
const PX: f32 = (TILE_W / 2.0) / 38.0;
/// Room left at the bottom of the board for the XP bar, which covers it.
const BAR_ROOM: f32 = 100.0;
/// How long the gamer reacts to an action, the room stays gold after a level, and the tidy-up lasts.
pub(super) const REACT_SECS: f32 = 1.5;
pub(super) const GOLD_SECS: f32 = 3.0;
pub(super) const TIDY_SECS: f32 = 2.0;
/// The walls' height, in the canvas's floors.
const WALL: f32 = 7.0;
/// The desk's top.
const DESK: f32 = 2.42;
/// The pads shown at most; the rest are counted.
const PADS: usize = 6;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const LIME: Color32 = rgb(0xc6f432);
const RED: Color32 = rgb(0xff4d6d);
const GOLD: Color32 = rgb(0xffd66b);
const RAINBOW: [Color32; 9] = [rgb(0xff4d6d), rgb(0xf97316), rgb(0xfacc15), rgb(0xc6f432), rgb(0x22c55e), rgb(0x38bdf8), rgb(0x6366f1), rgb(0xa78bfa), rgb(0xec4899)];
/// The shelves' neon, one per shelf, and whether its emblems are dark on it.
const NEON: [(Color32, bool); 6] = [(rgb(0xff5a4e), false), (rgb(0x38bdf8), true), (rgb(0xec4899), false), (rgb(0xfacc15), true), (rgb(0xa78bfa), false), (rgb(0x22c55e), true)];

/// What an action does in the room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum React {
    Commit,
    Push,
    Merge,
    Undo,
    /// A boss beaten.
    Beaten,
    /// A new level.
    Level,
}

/// What the session shows in the room, copied to the tab each frame (`App::xp_look`).
#[derive(Clone, Debug, Default)]
pub(super) struct Look {
    pub mess: Vec<Piece>,
    /// A boss fight is going on.
    pub boss: bool,
    /// A level was reached less than `GOLD_SECS` ago.
    pub gold: bool,
    /// The last action, how long ago in seconds, and the XP that floats up.
    pub react: Option<(React, f32, String)>,
    /// The level, for the game screen.
    pub level: usize,
    /// The tidy-up, how long ago in seconds.
    pub tidy: Option<f32>,
    /// Touch grass: the screens are dimmed until the next action.
    pub dim: bool,
}

impl Look {
    /// The reaction going on, if it is still on.
    fn reacting(&self) -> Option<React> {
        self.react.as_ref().filter(|(_, age, _)| *age < REACT_SECS).map(|(r, ..)| *r)
    }

    /// The gamer's pose.
    fn pose(&self) -> Pose {
        if self.tidy.is_some_and(|t| t < TIDY_SECS) {
            return Pose::Lift;
        }
        if self.boss {
            return Pose::Lean;
        }
        match self.reacting() {
            Some(React::Push | React::Merge | React::Beaten | React::Level) => Pose::Up,
            Some(React::Undo) => Pose::Palm,
            _ => Pose::Type,
        }
    }

    /// The room's light: lime, red during a boss, gold after a level.
    fn accent(&self) -> Color32 {
        if self.boss {
            RED
        } else if self.gold {
            GOLD
        } else {
            LIME
        }
    }
}

/// The walls and the floor, at night in dark mode and in a lilac daylight in light mode.
struct Walls {
    floor: [Color32; 2],
    left: Color32,
    right: Color32,
    rim: Color32,
    rug: (Color32, Color32),
    wood: Faces,
}

fn walls(look: &Look) -> Walls {
    let mut w = if theme::is_light() {
        Walls {
            floor: [rgb(0xe4ddf1), rgb(0xddd5ec)],
            left: rgb(0xcfc6e6),
            right: rgb(0xc3b9de),
            rim: rgb(0xb3a6d4),
            rug: (rgb(0xb9aaea), rgb(0xa897e0)),
            wood: Faces { left: rgb(0x9a89bd), right: rgb(0x8877ad), top: rgb(0xb2a3d2) },
        }
    } else {
        Walls {
            floor: [rgb(0x2f2a3d), rgb(0x2b2638)],
            left: rgb(0x262035),
            right: rgb(0x1f1a2d),
            rim: rgb(0x3a3150),
            rug: (rgb(0x4a3a75), rgb(0x3b2f5c)),
            wood: Faces { left: rgb(0x4a3d5e), right: rgb(0x3a2f4c), top: rgb(0x5d4d75) },
        }
    };
    // The room warms to red in a boss fight, to gold after a level.
    if look.boss || look.gold {
        let c = look.accent();
        w.left = theme::tint(c, w.left, 0.12);
        w.right = theme::tint(c, w.right, 0.12);
    }
    w
}

fn ci_colour(ci: Option<Ci>) -> Color32 {
    match ci {
        Some(Ci::Success) => rgb(0x3fb950),
        Some(Ci::Failure) => rgb(0xf85149),
        Some(Ci::Pending) => rgb(0xe3a008),
        None => rgb(0x9aa0a6),
    }
}

/// The room's painter: the canvas's coordinates on the city's isometric view.
struct Rm<'a> {
    p: &'a Painter,
    iso: Iso,
}

impl Rm<'_> {
    /// A point of the room, in tiles and the canvas's floors.
    fn at(&self, x: f32, y: f32, z: f32) -> Pos2 {
        self.iso.at(x, y, z * ZK)
    }

    /// The canvas's points at this zoom.
    fn s(&self) -> f32 {
        self.iso.zoom * PX
    }

    fn quad(&self, x0: f32, y0: f32, x1: f32, y1: f32, z: f32) -> Vec<Pos2> {
        vec![self.at(x0, y0, z), self.at(x1, y0, z), self.at(x1, y1, z), self.at(x0, y1, z)]
    }

    fn poly(&self, pts: Vec<Pos2>, c: Color32) {
        super::city::poly(self.p, pts, c);
    }

    fn cuboid(&self, (x0, y0, x1, y1): (f32, f32, f32, f32), z0: f32, z1: f32, f: Faces) {
        super::city::cuboid(self.p, &self.iso, (x0, y0, x1, y1), z0 * ZK, z1 * ZK, f);
    }

    /// A drawing with its box's (0, 0) at `o`, `k` times the canvas's size.
    fn art(&self, parts: &[Part], o: Pos2, k: f32) {
        art::draw(self.p, parts, o, k * self.s(), 0.0, 1.0);
    }

    /// The step of one tile along the right wall (x) or toward the viewer on the left wall (−y).
    fn step(&self, wall: Wall) -> Vec2 {
        match wall {
            Wall::Right => self.at(1.0, 0.0, 0.0) - self.at(0.0, 0.0, 0.0),
            Wall::Left => self.at(0.0, -1.0, 0.0) - self.at(0.0, 0.0, 0.0),
        }
    }

    /// A face of the room as a 100 × 100 box: `o` its top-left corner on screen, `w` tiles along
    /// `u`, `h` floors high.
    fn face(&self, o: Pos2, u: Vec2, w: f32, h: f32) -> impl Fn(Vec2) -> Pos2 {
        let down = ZK * FLOOR * self.iso.zoom;
        move |v: Vec2| o + u * (w * v.x / 100.0) + vec2(0.0, down * h * v.y / 100.0)
    }

    /// Text centred on `c`, `size` canvas points, turned along `u`; left out when too small to read.
    fn text(&self, c: Pos2, s: &str, size: f32, col: Color32, u: Vec2, room: Option<f32>) {
        let mut size = size * self.s();
        let font = |size: f32| FontId::new(size, FontFamily::Name(theme::MEDIUM.into()));
        let mut gal = self.p.layout_no_wrap(s.into(), font(size), col);
        if let Some(room) = room.filter(|r| gal.size().x > *r) {
            size *= room / gal.size().x;
            gal = self.p.layout_no_wrap(s.into(), font(size), col);
        }
        if size < 5.5 {
            return;
        }
        let angle = u.y.atan2(u.x);
        let half = gal.size() / 2.0;
        let (sin, cos) = angle.sin_cos();
        let pos = c - vec2(half.x * cos - half.y * sin, half.x * sin + half.y * cos);
        self.p.add(egui::epaint::TextShape::new(pos, gal, col).with_angle(angle));
    }
}

/// Something to draw in front of the walls, at its depth: the farther, the sooner.
enum Thing {
    Desk,
    Gamer,
    Table,
    Pad(Pos2, Color32, u64),
    Piece(usize, Piece, Place),
    Flies,
}

/// The shelf's corners along its wall and in depth, for a box at `along` `w` tiles wide.
fn box_rect(wall: Wall, along: f32, w: f32) -> (f32, f32, f32, f32) {
    match wall {
        Wall::Right => (along, 0.08, along + w, 0.42),
        Wall::Left => (0.08, along, 0.42, along + w),
    }
}

/// Where a box stands: its rect on the floor plan, its board's height, its width and height.
fn box_place(shelf: &Shelf, b: &GameBox) -> ((f32, f32, f32, f32), f32, f32, f32) {
    let (board, j) = b.board();
    let feature = b.building == Building::Tower;
    let (w, h) = if feature { (0.95, 1.45) } else { (0.8, 1.2) };
    let bz = [0.45, 2.15, 3.85][board];
    let offset = 0.08 + j as f32 * 1.0 + (0.95 - w) / 2.0;
    // Slot 0 on the screen's left: the right wall's back end, the left wall's front end.
    let along = match shelf.wall {
        Wall::Right => shelf.along.0 + offset,
        Wall::Left => shelf.along.1 - offset - w,
    };
    (box_rect(shelf.wall, along, w), bz, w, h)
}

/// The screen box a game box takes, for hovering.
fn box_hit(r: &Rm, shelf: &Shelf, b: &GameBox) -> Rect {
    let ((x0, y0, x1, y1), bz, _, h) = box_place(shelf, b);
    let pts: Vec<Pos2> = [bz, bz + h].iter().flat_map(|z| r.quad(x0, y0, x1, y1, *z)).collect();
    Rect::from_points(&pts)
}

/// A shelf: its neon sign, its three boards and its boxes, and the `+k` of the rest.
fn shelf(r: &Rm, i: usize, s: &Shelf, wood: Faces, time: f32) {
    let (neon, dark_ink) = NEON[i % NEON.len()];
    let (a0, a1) = s.along;
    let mid = (a0 + a1) / 2.0;
    let u = r.step(s.wall);
    let on_wall = |a: f32, z: f32| match s.wall {
        Wall::Right => r.at(a, 0.0, z),
        Wall::Left => r.at(0.0, a, z),
    };
    // The sign: a dark plate with a neon rim and the zone's name, flickering now and then.
    let plate = vec![on_wall(mid - 1.2, 6.35), on_wall(mid + 1.2, 6.35), on_wall(mid + 1.2, 5.6), on_wall(mid - 1.2, 5.6)];
    r.poly(plate.clone(), rgb(0x14111d));
    let flicker = if (time * 0.7 + i as f32 * 1.3).sin() > 0.985 { 0.45 } else { 1.0 };
    let glow = neon.gamma_multiply(flicker);
    r.p.add(Shape::closed_line(plate.clone(), Stroke::new(2.0 * r.s(), glow)));
    let width = (plate[1] - plate[0]).length() - 14.0 * r.s();
    r.text(on_wall(mid, 5.98), &s.name.to_uppercase(), 11.0, glow, u, Some(width));
    for bz in [0.45, 2.15, 3.85] {
        let rect = match s.wall {
            Wall::Right => (a0, 0.0, a1, 0.6),
            Wall::Left => (0.0, a0, 0.6, a1),
        };
        r.cuboid(rect, bz - 0.14, bz, wood);
        let mut on: Vec<&GameBox> = s.boxes.iter().filter(|b| [0.45, 2.15, 3.85][b.board().0] == bz).collect();
        // Back to front: along the right wall by x, along the left one by y.
        on.sort_by(|a, b| box_place(s, a).0 .0.total_cmp(&box_place(s, b).0 .0).then(box_place(s, a).0 .1.total_cmp(&box_place(s, b).0 .1)));
        for b in on {
            game_box(r, s, b, neon, dark_ink, time);
        }
    }
    if s.more > 0 {
        // The `+k` in the last place of the bottom board.
        let along = match s.wall {
            Wall::Right => a0 + 2.6,
            Wall::Left => a1 - 2.6,
        };
        let c = match s.wall {
            Wall::Right => r.at(along, 0.3, 0.45 + 0.5),
            Wall::Left => r.at(0.3, along, 0.45 + 0.5),
        };
        let font = FontId::new((10.0 * r.s()).max(8.0), FontFamily::Name(theme::MEDIUM.into()));
        let gal = r.p.layout_no_wrap(format!("+{}", s.more), font, rgb(0x1b1c20));
        let pill = Rect::from_center_size(c, gal.size() + vec2(10.0, 4.0));
        r.p.rect_filled(pill, CornerRadius::same(8), neon);
        r.p.galley(pill.center() - gal.size() / 2.0, gal, rgb(0x1b1c20));
    }
}

/// One ticket's game box on its board.
fn game_box(r: &Rm, s: &Shelf, b: &GameBox, cover: Color32, dark_ink: bool, time: f32) {
    let ((x0, y0, x1, y1), bz, w, h) = box_place(s, b);
    let u = r.step(s.wall);
    let ghost = b.stage == Stage::Lot;
    let faces = if ghost {
        let c = if theme::is_light() { Color32::from_black_alpha(18) } else { Color32::from_white_alpha(18) };
        Faces { left: c, right: c, top: c }
    } else {
        let (front, side) = (cover, theme::shade(cover, 0.35));
        match s.wall {
            Wall::Right => Faces { left: front, right: side, top: theme::lighten(cover, 0.3) },
            Wall::Left => Faces { left: side, right: front, top: theme::lighten(cover, 0.3) },
        }
    };
    r.cuboid((x0, y0, x1, y1), bz, bz + h, faces);
    // The front, as the canvas's 100 × 100 box.
    let o = match s.wall {
        Wall::Right => r.at(x0, y1, bz + h),
        Wall::Left => r.at(x1, y1, bz + h),
    };
    let f = r.face(o, u, w, h);
    let k = (u.length() * w / 100.0).max(0.2);
    let number = b.number.map_or_else(|| "draft".to_string(), |n| format!("#{n}"));
    let grey = rgb(0x8a8e98);
    if ghost {
        art::draw_mapped(r.p, &[Part(g::FRAME_D, None, Some(grey), 1.2)], &f, k);
        art::draw_mapped(r.p, &[Part(g::HEART_D, Some(Color32::from_rgba_unmultiplied(255, 107, 129, 140)), None, 0.0)], &f, k);
        r.text(f(vec2(50.0, 18.0)), &number, 7.0, grey, u, Some(w * u.length()));
    } else {
        let ink = if dark_ink { rgb(0x1b1c20) } else { rgb(0xffffff) };
        art::draw_mapped(r.p, &[Part(g::BAND_D, Some(Color32::from_black_alpha(72)), None, 0.0)], &f, k);
        r.text(f(vec2(50.0, 13.0)), &number, 7.0, rgb(0xffffff), u, Some(w * u.length() * 0.9));
        let emblem = match b.building {
            Building::Tower => Part(g::CASTLE_D, Some(ink), None, 0.0),
            Building::Workshop => Part(g::BEETLE_D, Some(ink), Some(ink), 3.0),
            Building::House => Part(g::STAR_D, Some(ink), None, 0.0),
        };
        art::draw_mapped(r.p, &[emblem], &f, k);
        // A pip per sub-issue along the bottom, lit when done.
        let (done, total) = b.subs;
        let n = total.min(7);
        for i in 0..n {
            let px = 50.0 + (i as f32 - (n as f32 - 1.0) / 2.0) * 13.0;
            let col = if i < done { LIME } else { Color32::from_black_alpha(90) };
            r.p.circle_filled(f(vec2(px, 90.0)), (4.0 * k).max(1.0), col);
        }
        match b.stage {
            Stage::Staked => art::draw_mapped(r.p, &[Part(g::SHINE_D, Some(Color32::from_white_alpha(66)), None, 0.0)], &f, k),
            Stage::Foundations => {
                art::draw_mapped(r.p, &[Part(g::GLOW_D, None, Some(LIME), 2.4)], &f, k);
                art::draw_mapped(r.p, &[Part(g::LOADING_D, Some(rgb(0x111215)), None, 0.0)], &f, k);
                // Its sub-issues done, else a bar that keeps loading.
                let t = if total > 0 { done as f32 / total as f32 } else { 0.25 + 0.5 * (time * 0.6 + (b.slot as f32)).sin().abs() };
                let a = f(vec2(12.0, 78.0));
                let bb = f(vec2(12.0 + 76.0 * t, 86.0));
                let (p0, p1) = (f(vec2(12.0 + 76.0 * t, 78.0)), f(vec2(12.0, 86.0)));
                r.p.add(Shape::convex_polygon(vec![a, p0, bb, p1], LIME, Stroke::NONE));
            }
            Stage::Frame => {
                art::draw_mapped(r.p, &[Part(g::PAUSE_D, Some(rgb(0xffc93c)), Some(super::mess::INK), 1.0), Part(g::PAUSE_BARS_D, None, Some(super::mess::INK), 4.0)], &f, k);
            }
            Stage::Testing => {
                art::draw_mapped(r.p, &[Part(g::BETA_D, Some(LIME), None, 0.0)], &f, k);
                r.text(f(vec2(50.0, 79.0)), "BETA", 7.0, rgb(0x1b1c20), u, Some(w * u.length() * 0.9));
            }
            _ => {}
        }
    }
    let top = f(vec2(50.0, 0.0));
    if b.stage == Stage::Built {
        r.art(g::CUP, top - vec2(20.0, 37.0) * 0.36 * r.s(), 0.36);
    }
    if b.building == Building::Workshop && b.stage != Stage::Built {
        // The bug crawls along the top edge.
        let along = 60.0 + 25.0 * (time * 0.8 + b.slot as f32).sin();
        let at = f(vec2(along, 0.0));
        r.art(g::BUG, at - vec2(15.0, 22.0) * 0.55 * r.s(), 0.55);
    }
}

/// The desk: its legs and top with the lit strip, the two screens, the keyboard and the mouse,
/// the chips.
fn desk(r: &Rm, look: &Look, crowded: bool, time: f32) {
    let dark = Faces { left: rgb(0x1d1a26), right: rgb(0x16141e), top: rgb(0x2a2536) };
    let top = Faces { left: rgb(0x2b2638), right: rgb(0x221e2d), top: rgb(0x3d3550) };
    for (x, y) in [(3.1, 4.3), (8.1, 4.3), (3.1, 5.1), (8.1, 5.1)] {
        r.cuboid((x, y, x + 0.2, y + 0.2), 0.0, 2.2, dark);
    }
    r.cuboid((3.0, 4.2, 8.4, 5.4), 2.2, DESK, top);
    let accent = look.accent();
    r.poly(vec![r.at(3.0, 5.4, 2.2), r.at(8.4, 5.4, 2.2), r.at(8.4, 5.4, 2.26), r.at(3.0, 5.4, 2.26)], accent);
    for (i, (m0, m1)) in [(3.3, 5.6), (5.8, 8.1)].into_iter().enumerate() {
        let mid = (m0 + m1) / 2.0;
        r.cuboid((mid - 0.15, 4.3, mid + 0.15, 4.42), DESK, 2.9, dark);
        r.cuboid((m0, 4.36, m1, 4.5), 2.8, 5.2, Faces { left: rgb(0x16151c), right: rgb(0x0f0e14), top: rgb(0x24222c) });
        let u = r.step(Wall::Right);
        let f = r.face(r.at(m0 + 0.1, 4.5, 5.08), u, m1 - m0 - 0.2, 2.16);
        let k = u.length() * (m1 - m0 - 0.2) / 100.0;
        if i == 0 {
            code_screen(r, &f, look, time);
        } else {
            game_screen(r, &f, k, u, look, time);
        }
    }
    // The RGB keyboard: a wave of colours, red in a boss fight, gold after a level; a commit
    // flashes its keys one after the other.
    r.poly(r.quad(4.6, 4.8, 6.9, 5.3, 2.44), rgb(0x15141b));
    let typed = look.reacting() == Some(React::Commit);
    let wave = (time * 6.0) as usize;
    for row in 0..3 {
        for key in 0..11 {
            let (kx, ky) = (4.68 + key as f32 * 0.2, 4.86 + row as f32 * 0.14);
            let c = if look.boss {
                if key % 2 == 1 { RED } else { rgb(0xc2253f) }
            } else if look.gold {
                if key % 2 == 1 { GOLD } else { rgb(0xe0a21a) }
            } else if typed && (key + row * 4) % 11 == wave % 11 {
                rgb(0xffffff)
            } else {
                RAINBOW[(key + row + (time * 2.0) as usize) % RAINBOW.len()]
            };
            r.poly(r.quad(kx, ky, kx + 0.16, ky + 0.1, 2.5), c);
        }
    }
    // The mouse pad and the mouse.
    r.poly(r.quad(7.1, 4.75, 8.0, 5.3, 2.43), rgb(0x2a2440));
    let m = r.at(7.55, 5.02, 2.5);
    let pts: Vec<Pos2> = (0..16).map(|i| m + vec2(6.0 * (i as f32 / 16.0 * std::f32::consts::TAU).cos(), 3.6 * (i as f32 / 16.0 * std::f32::consts::TAU).sin()) * r.s()).collect();
    r.poly(pts, rgb(0xe9ecf2));
    r.p.circle_filled(r.at(7.5, 5.0, 2.62), 1.4 * r.s(), if look.boss { RED } else { rgb(0x38bdf8) });
    // His chips, unless the can pyramid stands there.
    if !crowded {
        let c = r.at(3.55, 4.9, DESK);
        r.art(art::CHIPS, c - vec2(22.0, 52.0) * 0.46 * r.s(), 0.46);
    }
}

/// The code screen: lines of code, a green one landing on a commit, conflict bands in a boss fight.
fn code_screen(r: &Rm, f: &dyn Fn(Vec2) -> Pos2, look: &Look, time: f32) {
    let rect = |x: f32, y: f32, w: f32, h: f32, c: Color32| {
        r.p.add(Shape::convex_polygon(vec![f(vec2(x, y)), f(vec2(x + w, y)), f(vec2(x + w, y + h)), f(vec2(x, y + h))], c, Stroke::NONE));
    };
    let dim = look.dim;
    rect(0.0, 0.0, 100.0, 100.0, if dim { rgb(0x090b0f) } else { rgb(0x0d1117) });
    if look.boss {
        rect(0.0, 44.0, 100.0, 18.0, Color32::from_rgba_unmultiplied(248, 81, 73, 70));
        rect(0.0, 66.0, 100.0, 14.0, Color32::from_rgba_unmultiplied(56, 139, 253, 64));
    } else if !dim {
        rect(0.0, 44.0, 100.0, 18.0, Color32::from_rgba_unmultiplied(63, 185, 80, 56));
        rect(0.0, 72.0, 100.0, 9.0, Color32::from_rgba_unmultiplied(248, 81, 73, 56));
    }
    const CODE: [(f32, f32, u32); 10] = [(0.0, 30.0, 0xff7b72), (1.0, 44.0, 0x79c0ff), (2.0, 36.0, 0xc9d1d9), (2.0, 22.0, 0xd2a8ff), (1.0, 18.0, 0xc9d1d9), (1.0, 40.0, 0x7ee787), (2.0, 30.0, 0x7ee787), (1.0, 26.0, 0xc9d1d9), (2.0, 34.0, 0xffa198), (0.0, 12.0, 0xc9d1d9)];
    for (li, (indent, w, c)) in CODE.iter().enumerate() {
        let y = 6.0 + li as f32 * 9.0;
        rect(2.0, y, 5.0, 4.0, rgb(0x30363d));
        rect(12.0 + indent * 7.0, y, *w, 4.0, if dim { rgb(0x30363d) } else { rgb(*c) });
    }
    // A commit: the new line lands, green, at the bottom.
    if look.reacting() == Some(React::Commit) {
        rect(0.0, 88.0, 100.0, 10.0, Color32::from_rgba_unmultiplied(63, 185, 80, 90));
        rect(12.0, 91.0, 60.0, 4.0, rgb(0x7ee787));
    }
    // The cursor blinks.
    if (time * 2.0).fract() < 0.5 && !dim {
        rect(38.0, 87.0, 5.0, 6.0, LIME);
    }
}

/// The game screen: the hero, coins and a bug in a level; a boss, a rewind, an arrow, a merge or
/// LEVEL UP when the room reacts.
fn game_screen(r: &Rm, f: &dyn Fn(Vec2) -> Pos2, k: f32, u: Vec2, look: &Look, time: f32) {
    let rect = |x: f32, y: f32, w: f32, h: f32, c: Color32| {
        r.p.add(Shape::convex_polygon(vec![f(vec2(x, y)), f(vec2(x + w, y)), f(vec2(x + w, y + h)), f(vec2(x, y + h))], c, Stroke::NONE));
    };
    let shape = |d: &'static str, fill: Option<Color32>, stroke: Option<Color32>, w: f32| art::draw_mapped(r.p, &[Part(d, fill, stroke, w)], f, k);
    let react = look.reacting();
    let rewind = react == Some(React::Undo);
    let (sky, grass) = if look.dim {
        (rgb(0x0b0a1c), rgb(0x1f4a28))
    } else if look.boss {
        (rgb(0x2a0f1c), rgb(0x7a2e3a))
    } else if rewind {
        (rgb(0x2a2c34), rgb(0x4a4d57))
    } else {
        (rgb(0x23205a), rgb(0x3fb950))
    };
    rect(0.0, 0.0, 100.0, 100.0, sky);
    shape("M10 16 h1.5 v1.5 h-1.5 Z M30 8 h1.5 v1.5 h-1.5 Z M78 22 h1.5 v1.5 h-1.5 Z M56 12 h1.5 v1.5 h-1.5 Z", Some(rgb(0xf7f8f8)), None, 0.0);
    rect(0.0, 82.0, 100.0, 18.0, if look.dim { rgb(0x2a1a12) } else { rgb(0x7a4528) });
    rect(0.0, 80.0, 100.0, 6.0, grass);
    if look.dim {
        return;
    }
    if look.boss {
        shape("M50 80 Q46 34 70 32 Q94 34 90 80 Z", Some(RED), None, 0.0);
        shape("M60 50 L68 54 M80 50 L72 54", None, Some(rgb(0x1b1c20)), 3.0);
        shape("M64 66 Q70 60 76 66", None, Some(rgb(0x1b1c20)), 3.0);
        rect(8.0, 8.0, 40.0, 6.0, rgb(0x111215));
        rect(8.0, 8.0, 22.0, 6.0, RED);
        shape("M22 80 Q20 66 28 64 Q36 66 34 80 Z", Some(LIME), None, 0.0);
        r.text(f(vec2(28.0, 24.0)), "BOSS", 7.0, rgb(0xff8a80), u, None);
        return;
    }
    match react {
        Some(React::Undo) => {
            shape("M56 30 L36 50 L56 70 Z M78 30 L58 50 L78 70 Z", Some(rgb(0xf7f8f8)), None, 0.0);
            r.text(f(vec2(50.0, 90.0)), "−100", 7.0, rgb(0xf7f8f8), u, None);
        }
        Some(React::Push) => shape("M50 74 V28 M34 44 L50 26 L66 44", None, Some(rgb(0x38bdf8)), 6.0),
        Some(React::Merge) => shape("M34 74 V54 Q34 38 50 36 Q66 38 66 54 V74 M50 36 V20 M42 28 L50 18 L58 28", None, Some(GOLD), 5.0),
        Some(React::Level) => {
            rect(14.0, 26.0, 72.0, 36.0, rgb(0x2e2508));
            r.text(f(vec2(50.0, 38.0)), "LEVEL UP", 9.0, GOLD, u, None);
            let name = crate::xp::LEVELS[look.level.min(crate::xp::LEVELS.len() - 1)].1.to_uppercase();
            r.text(f(vec2(50.0, 52.0)), &format!("LV {} · {name}", look.level + 1), 7.0, rgb(0xf7f8f8), u, None);
        }
        _ => {
            // The level: the hero hops, coins float, a bug walks.
            let hop = (time * 3.0).sin().abs() * 8.0;
            shape("M40 56 H56 V64 H40 Z M64 44 H72 V52 H64 Z", Some(rgb(0xffc93c)), None, 0.0);
            let hf = |v: Vec2| f(v - vec2(0.0, hop));
            art::draw_mapped(r.p, &[Part("M18 80 Q16 64 25 62 Q34 64 32 80 Z", Some(LIME), None, 0.0), Part("M22 70 a1.5 1.5 0 1 0 3 0 a1.5 1.5 0 1 0 -3 0 Z M27 70 a1.5 1.5 0 1 0 3 0 a1.5 1.5 0 1 0 -3 0 Z", Some(rgb(0x1b1c20)), None, 0.0)], &hf, k);
            shape("M46 40 a3 3 0 1 0 6 0 a3 3 0 1 0 -6 0 Z M84 34 a3 3 0 1 0 6 0 a3 3 0 1 0 -6 0 Z", Some(GOLD), None, 0.0);
            let walk = 62.0 + 14.0 * (time * 0.5).sin();
            let bk = k * 0.8;
            let bo = f(vec2(walk, 66.0));
            art::draw(r.p, g::BUG, bo, bk, 0.0, 1.0);
            r.text(f(vec2(16.0, 11.0)), &format!("LV {}", look.level + 1), 7.0, rgb(0xf7f8f8), u, None);
            rect(64.0, 8.0, 30.0, 6.0, rgb(0x111215));
            rect(64.0, 8.0, 20.0, 6.0, LIME);
        }
    }
}

/// A piece of the mess where it lies; `n` its place in the session, for its colour.
fn piece(r: &Rm, n: usize, what: Piece, place: Place, time: f32) {
    let s = r.s();
    let (c, dark) = g::CANS[n % g::CANS.len()];
    match (what, place) {
        (Piece::Can, Place::Desk(x, y)) => {
            let at = r.at(x, y, DESK);
            r.art(&g::can(c, dark), at - vec2(15.0, 50.0) * 0.4 * s, 0.4);
        }
        (Piece::Can, Place::Pyramid(i)) => {
            // Rows of 5, 4, 3, 2 and 1 on the desk's left end.
            let (mut row, mut first, mut size) = (0, 0, 5);
            while i >= first + size {
                first += size;
                size -= 1;
                row += 1;
            }
            let col = (i - first) as f32 - (size as f32 - 1.0) / 2.0;
            let base = r.at(3.6, 4.9, DESK);
            let at = base + vec2(col * 22.0 * 0.36, -(row as f32) * 44.0 * 0.36) * s;
            r.art(&g::can(c, dark), at - vec2(15.0, 50.0) * 0.36 * s, 0.36);
        }
        (Piece::Can, Place::Floor(x, y)) => {
            let at = r.at(x, y, 0.0);
            r.art(&g::can_down(c, dark), at - vec2(25.0, 22.0) * 0.5 * s, 0.5);
        }
        (Piece::Bag, Place::Floor(x, y)) => {
            let at = r.at(x, y, 0.0);
            r.art(art::CHIPS, at - vec2(24.0, 52.0) * 0.5 * s, 0.5);
        }
        (Piece::Bag, Place::Heap(i)) => {
            let at = r.at(0.9 + (i % 3) as f32 * 0.35, 9.4 + (i % 3) as f32 * 0.1, 0.0) - vec2(0.0, (i / 3) as f32 * 20.0 * s);
            art::draw(r.p, art::CHIPS, at - vec2(24.0, 52.0) * 0.48 * s, 0.48 * s, ((i * 37) % 30) as f32 - 15.0, 1.0);
        }
        (Piece::Spill, Place::Floor(x, y)) => {
            let at = r.at(x, y, 0.0);
            r.art(g::SPILL, at - vec2(26.0, 9.0) * 0.9 * s, 0.9);
        }
        (Piece::Pizza, Place::Stack(x, y, i)) => {
            let o = (i % 2) as f32 * 0.06;
            let (z0, z1) = (i as f32 * 0.16, (i + 1) as f32 * 0.16);
            let card = Faces { left: rgb(0xd8cbb0), right: rgb(0xb9ab8f), top: rgb(0xefe4cc) };
            // A tower sways a little.
            let sway = if i >= room::STACK { (time * 1.5).sin() * 0.02 * (i - room::STACK + 1) as f32 } else { 0.0 };
            let (x0, y0) = (x + o + sway, y - o);
            r.cuboid((x0, y0, x0 + 1.1, y0 + 1.1), z0, z1, card);
            r.poly(vec![r.at(x0 + 0.4, y0 + 1.1, z0 + 0.03), r.at(x0 + 0.7, y0 + 1.1, z0 + 0.03), r.at(x0 + 0.7, y0 + 1.1, z1 - 0.03), r.at(x0 + 0.4, y0 + 1.1, z1 - 0.03)], rgb(0xe84a3d));
        }
        _ => {}
    }
}

/// The depth of a piece of the mess, to draw it among the furniture.
fn piece_depth(place: Place) -> f32 {
    match place {
        Place::Desk(..) | Place::Pyramid(_) => 10.6,
        Place::Floor(x, y) => x + y,
        Place::Stack(x, y, i) => x + y + 1.1 + i as f32 * 0.001,
        Place::Heap(i) => 10.5 + i as f32 * 0.001,
    }
}

impl Tab {
    /// Draws the gamer's room of `items` (the tickets the board's filters keep) and returns the
    /// ticket clicked.
    pub(super) fn room_view(&mut self, ui: &mut Ui, items: &[&Item], detail: Option<&str>) -> Option<String> {
        let layout = room::layout(items);
        let look = self.room_look.clone();
        let litter = room::litter(&look.mess);
        let area = ui.available_rect_before_wrap().shrink2(vec2(12.0, 6.0));
        // Fitted above the XP bar, which covers the bottom of the window.
        let fit = Rect::from_min_max(area.min + vec2(0.0, 36.0), pos2(area.right(), (area.bottom() - BAR_ROOM).max(area.top() + 200.0)));
        let n = room::SIZE as i32;
        let (resp, iso) = self.iso_view(ui, area, fit, (n, n));
        // The room stands on its floor: up a little, since its walls rise high at the back.
        let iso = Iso { origin: iso.origin + vec2(0.0, 2.0 * FLOOR * iso.zoom), zoom: iso.zoom };
        let p = ui.painter().with_clip_rect(area);
        let r = Rm { p: &p, iso };
        let k = theme::c();
        let time = ui.input(|i| i.time) as f32;
        let w = walls(&look);
        let accent = look.accent();
        let nn = room::SIZE;

        // The floor, the two walls, their rim and the RGB strip along their top.
        for i in 0..n {
            let x = i as f32;
            r.poly(r.quad(x, 0.0, x + 1.0, nn, 0.0), w.floor[(i % 2) as usize]);
        }
        r.poly(vec![r.at(0.0, 0.0, 0.0), r.at(0.0, nn, 0.0), r.at(0.0, nn, WALL), r.at(0.0, 0.0, WALL)], w.left);
        r.poly(vec![r.at(0.0, 0.0, 0.0), r.at(nn, 0.0, 0.0), r.at(nn, 0.0, WALL), r.at(0.0, 0.0, WALL)], w.right);
        r.poly(vec![r.at(0.0, 0.0, WALL), r.at(0.0, nn, WALL), r.at(-0.25, nn, WALL), r.at(-0.25, -0.25, WALL)], w.rim);
        r.poly(vec![r.at(-0.25, -0.25, WALL), r.at(nn, -0.25, WALL), r.at(nn, 0.0, WALL), r.at(0.0, 0.0, WALL)], w.rim);
        let tidying = look.tidy.is_some_and(|t| t < TIDY_SECS);
        let shift = (time * 3.0) as usize;
        let mut s = 0.0;
        let mut i = 0;
        while s < nn {
            let c = if look.boss {
                if i % 2 == 1 { RED } else { rgb(0xb3263f) }
            } else if look.gold {
                if i % 2 == 1 { GOLD } else { rgb(0xe0a21a) }
            } else if tidying {
                if ((time * 8.0) as usize).is_multiple_of(2) { LIME } else { rgb(0x9fcf1a) }
            } else {
                RAINBOW[(i + shift) % RAINBOW.len()]
            };
            r.poly(vec![r.at(0.0, s, 6.6), r.at(0.0, s + 0.5, 6.6), r.at(0.0, s + 0.5, 6.75), r.at(0.0, s, 6.75)], c);
            r.poly(vec![r.at(s, 0.0, 6.6), r.at(s + 0.5, 0.0, 6.6), r.at(s + 0.5, 0.0, 6.75), r.at(s, 0.0, 6.75)], c);
            s += 0.5;
            i += 1;
        }
        // The rug under the chair, its lime stitches.
        r.poly(r.quad(3.2, 5.8, 8.4, 9.4, 0.0), w.rug.0);
        r.poly(r.quad(3.4, 6.0, 8.2, 9.2, 0.0), w.rug.1);
        let mut x = 3.6;
        while x < 8.1 {
            r.poly(r.quad(x, 6.1, x + 0.3, 6.2, 0.0), accent);
            x += 0.6;
        }
        // The spills lie flat on the floor, under everything.
        for (n, (what, place)) in litter.pieces.iter().enumerate() {
            if *what == Piece::Spill {
                piece(&r, n, *what, *place, time);
            }
        }
        // The shelves along the walls.
        for (i, sh) in layout.shelves.iter().enumerate() {
            shelf(&r, i, sh, w.wood, time);
        }

        // What stands in the room, back to front.
        let by_id: std::collections::HashMap<&str, &Item> = items.iter().map(|it| (it.id.as_str(), *it)).collect();
        let pulls = self.pulls.pulls.as_deref();
        let mut prs: Vec<(u64, Color32)> = Vec::new();
        for it in items {
            for pr in it.pull_requests.iter().filter(|l| l.state == "OPEN") {
                if !prs.iter().any(|(n, _)| *n == pr.number) {
                    let ci = pulls.and_then(|ps| ps.list.iter().find(|q| q.number == pr.number)).and_then(|q| q.ci);
                    prs.push((pr.number, ci_colour(ci)));
                }
            }
        }
        prs.sort_by_key(|(n, _)| *n);
        let mut things: Vec<(f32, Thing)> = vec![(10.5, Thing::Desk), (12.1, Thing::Gamer), (16.2, Thing::Table)];
        const PAD_SPOTS: [(f32, f32, f32); PADS] = [(9.45, 6.5, 1.32), (9.95, 6.5, 1.32), (3.4, 8.6, 0.0), (7.9, 9.6, 0.0), (6.4, 9.0, 0.0), (9.9, 8.9, 0.0)];
        for ((number, c), (x, y, z)) in prs.iter().zip(PAD_SPOTS) {
            things.push((x + y + if z > 0.0 { 1.0 } else { 0.0 }, Thing::Pad(r.at(x, y, z), *c, *number)));
        }
        for (n, (what, place)) in litter.pieces.iter().enumerate() {
            if *what != Piece::Spill {
                things.push((piece_depth(*place), Thing::Piece(n, *what, *place)));
            }
        }
        if litter.flies {
            things.push((30.0, Thing::Flies));
        }
        things.sort_by(|a, b| a.0.total_cmp(&b.0));
        let crowded = litter.pieces.iter().any(|(_, p)| matches!(p, Place::Pyramid(_)));
        let pose = look.pose();
        let gamer_o = r.at(5.7, 6.4, 0.0) - vec2(60.0, 170.0) * 0.72 * r.s();
        for (_, thing) in &things {
            match thing {
                Thing::Desk => desk(&r, &look, crowded, time),
                Thing::Table => {
                    let dark = Faces { left: rgb(0x1d1a26), right: rgb(0x16141e), top: rgb(0x2a2536) };
                    r.cuboid((9.1, 6.0, 10.3, 7.0), 0.0, 1.3, dark);
                    r.poly(r.quad(9.2, 6.15, 10.2, 6.85, 1.32), rgb(0x3a3150));
                }
                Thing::Gamer => {
                    // He types: a small bob of his shoulders.
                    let bob = if pose == Pose::Type { (time * 9.0).sin() * 0.6 } else { 0.0 };
                    g::gamer(&p, gamer_o + vec2(0.0, bob), 0.72 * r.s(), pose, accent);
                    if pose == Pose::Lift {
                        let fist = gamer_o + g::FIST * 0.72 * r.s();
                        let bag = fist - vec2(28.0, 6.0) * 0.62 * r.s();
                        r.art(g::BINBAG, bag, 0.62);
                        // The last pieces fly into it, turning.
                        let t = look.tidy.unwrap_or(0.0) / TIDY_SECS;
                        let (c, dark) = g::CANS[0];
                        let can = g::can(c, dark);
                        let flying: [(&[Part], f32); 3] = [(art::CHIPS, 0.4), (&can, 0.32), (g::BOX, 0.3)];
                        for (j, (parts, k)) in flying.iter().enumerate() {
                            let f = (t * 1.6 - j as f32 * 0.25).clamp(0.0, 1.0);
                            if f <= 0.0 || f >= 1.0 {
                                continue;
                            }
                            let from = r.at(4.0 + j as f32 * 2.0, 9.5, 0.0);
                            let to = fist + vec2(0.0, 20.0 * r.s());
                            let at = from.lerp(to, f) - vec2(0.0, (f * std::f32::consts::PI).sin() * 60.0 * r.s());
                            art::draw(&p, parts, at, k * r.s(), f * 300.0 + j as f32 * 40.0, 1.0);
                        }
                    }
                }
                Thing::Pad(at, c, number) => {
                    r.art(&g::pad(*c), *at - vec2(32.0, 38.0) * 0.48 * r.s(), 0.48);
                    let font = FontId::new((9.0 * r.s()).max(8.0), FontFamily::Name(theme::MEDIUM.into()));
                    p.text(*at - vec2(0.0, 26.0 * r.s()), Align2::CENTER_CENTER, format!("#{number}"), font, if theme::is_light() { Color32::from_rgb(c.r() / 2, c.g() / 2, c.b() / 2) } else { *c });
                }
                Thing::Piece(n, what, place) => piece(&r, *n, *what, *place, time),
                Thing::Flies => {
                    // Over the pizza tower, else over the heap of bags.
                    let over = litter.pieces.iter().rev().find_map(|(_, pl)| match pl {
                        Place::Stack(x, y, i) => Some(r.at(x + 0.55, y + 0.55, (*i + 1) as f32 * 0.16 + 1.2)),
                        _ => None,
                    });
                    let c = over.unwrap_or_else(|| r.at(1.2, 9.6, 1.6));
                    for j in 0..3 {
                        let a = time * (2.0 + j as f32 * 0.7) + j as f32 * 2.1;
                        let at = c + vec2(a.cos() * 18.0, (a * 1.3).sin() * 9.0) * r.s();
                        r.art(g::FLY, at - vec2(8.0, 6.0) * 0.8 * r.s(), 0.8);
                    }
                }
            }
        }
        if prs.len() > PADS {
            let at = r.at(10.4, 9.9, 0.0);
            p.text(at, Align2::CENTER_CENTER, format!("+{} pads", prs.len() - PADS), theme::small(), k.dim);
        }
        // The tidy-up: sparkles on the clean floor.
        if let Some(t) = look.tidy.filter(|t| *t < TIDY_SECS) {
            for (j, (x, y)) in [(2.0, 9.0), (8.0, 10.0), (4.6, 10.4), (10.0, 8.0), (1.2, 7.2)].iter().enumerate() {
                let pulse = ((t * 4.0 + j as f32).sin() * 0.5 + 0.5) * 1.2;
                art::draw(&p, g::SPARK, r.at(*x, *y, 0.4), pulse * r.s(), 0.0, 1.0);
            }
        }
        // The XP an action earned floats up over his head.
        if let Some((_, age, text)) = look.react.as_ref().filter(|(_, age, _)| *age < REACT_SECS) {
            let t = age / REACT_SECS;
            let at = r.at(5.7, 6.4, 0.0) - vec2(0.0, (170.0 * 0.72 + 14.0 + 30.0 * t) * r.s());
            let col = match (text.starts_with('−'), theme::is_light()) {
                (true, false) => rgb(0xc9ccd4),
                (true, true) => rgb(0x6b7280),
                (false, false) => accent,
                // A darker twin, readable on the light floor.
                (false, true) => Color32::from_rgb(accent.r() / 2, accent.g() / 2, accent.b() / 2),
            };
            let font = FontId::new((16.0 * r.s()).max(11.0), FontFamily::Name(theme::MEDIUM.into()));
            p.text(at, Align2::CENTER_BOTTOM, text, font, col.gamma_multiply(1.0 - t * t));
        }
        // Confetti over the desk on a merge.
        if look.reacting() == Some(React::Merge) {
            let age = look.react.as_ref().map_or(0.0, |r| r.1);
            let top = r.at(3.0, 4.2, 8.0);
            let span = r.at(8.4, 5.4, 0.0).x - r.at(3.0, 5.4, 0.0).x;
            for c in 0..34 {
                let x = top.x + ((c * 53) % 100) as f32 / 100.0 * span.abs().max(120.0);
                let y = top.y + ((c * 37) % 60) as f32 * r.s() + age * 60.0 * r.s();
                let q = pos2(x, y);
                p.add(Shape::convex_polygon(vec![q, q + vec2(5.0, 2.0) * r.s(), q + vec2(4.0, 6.0) * r.s(), q + vec2(-1.0, 4.0) * r.s()], RAINBOW[c % RAINBOW.len()], Stroke::NONE));
            }
        }

        // The hovered box, and the open one.
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|h| area.contains(*h) && resp.hovered());
        let mut hovered: Option<(&Shelf, &GameBox)> = None;
        for sh in &layout.shelves {
            for b in &sh.boxes {
                let hit = box_hit(&r, sh, b);
                let selected = detail == Some(b.id.as_str());
                if hover.is_some_and(|h| hit.contains(h)) {
                    hovered = Some((sh, b));
                }
                if selected {
                    p.rect_stroke(hit.expand(2.0), CornerRadius::same(4), Stroke::new(2.0, k.accent), StrokeKind::Outside);
                }
            }
        }
        if let Some((sh, b)) = hovered {
            p.rect_stroke(box_hit(&r, sh, b).expand(2.0), CornerRadius::same(4), Stroke::new(2.0, k.accent_text), StrokeKind::Outside);
        }

        // The tally and the hints, top left (the boss pill sits above the XP bar).
        let count = |f: fn(Stage) -> bool| items.iter().filter(|it| f(Stage::of(&it.status))).count();
        let done = count(|s| s.built());
        let playing = count(|s| s.crane());
        p.text(area.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, format!("{done} finished · {playing} playing · {} on the wishlist", items.len() - done - playing), theme::secondary(), k.dim);
        // Not in a photo (#232).
        if !self.board.shooting {
            p.text(area.left_top() + vec2(4.0, 20.0), Align2::LEFT_TOP, "Drag to move · pinch or ⌘ scroll to zoom · double-click to fit", theme::small(), k.faint);
        }

        // The LEDs, the keyboard and the gamer move while the room is on screen.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));

        let mut clicked = None;
        if let Some((_, b)) = hovered {
            if let Some(it) = by_id.get(b.id.as_str()) {
                let n = it.number.map(|n| format!("#{n} ")).unwrap_or_default();
                let prs = it.pull_requests.iter().filter(|l| l.state == "OPEN").count();
                let who = if it.assignees.is_empty() { String::new() } else { format!(" · {}", it.assignees.join(", ")) };
                let more = if prs > 0 { format!(" · {prs} open pull request{}", if prs > 1 { "s" } else { "" }) } else { String::new() };
                resp.clone().on_hover_text_at_pointer(format!("{n}{}\n{}{who}{more}", it.title, it.status));
                if resp.clicked() {
                    clicked = Some(b.id.clone());
                }
            }
        }
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gbox(slot: usize, building: Building) -> GameBox {
        GameBox { id: "I1".into(), number: Some(1), stage: Stage::Staked, building, subs: (0, 0), slot }
    }

    #[test]
    fn boxes_stand_on_their_shelf_and_apart() {
        let sh = Shelf { name: "a".into(), wall: Wall::Right, along: (0.5, 3.6), boxes: vec![], more: 0 };
        let places: Vec<_> = (0..room::PER_SHELF).map(|i| box_place(&sh, &gbox(i, Building::Tower))).collect();
        for ((x0, _, x1, _), ..) in &places {
            assert!(*x0 >= sh.along.0 && *x1 <= sh.along.1);
        }
        assert!(places[0].1 > places[3].1, "the first board is the top one");
        assert!(places[1].0 .0 - places[0].0 .2 > 0.0, "side by side, apart");
        // On the left wall the first box is the front one, on the screen's left too.
        let left = Shelf { wall: Wall::Left, ..sh };
        let (a, ..) = box_place(&left, &gbox(0, Building::House));
        let (b, ..) = box_place(&left, &gbox(1, Building::House));
        assert!(a.1 > b.1);
    }

    #[test]
    fn the_gamer_reacts_then_types_again() {
        let mut look = Look { react: Some((React::Push, 0.2, "+300 XP".into())), ..Look::default() };
        assert_eq!(look.pose(), Pose::Up);
        look.react = Some((React::Undo, 0.5, "−100 XP".into()));
        assert_eq!(look.pose(), Pose::Palm);
        look.react = Some((React::Undo, REACT_SECS + 0.1, "−100 XP".into()));
        assert_eq!(look.pose(), Pose::Type);
        look.boss = true;
        assert_eq!((look.pose(), look.accent()), (Pose::Lean, RED));
        look.tidy = Some(0.5);
        assert_eq!(look.pose(), Pose::Lift);
        assert_eq!(Look { gold: true, ..Look::default() }.accent(), GOLD);
    }
}
