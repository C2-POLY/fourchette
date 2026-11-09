//! The district styles from round the world (#200): Japanese, Nordic, desert and Dutch, each
//! with three variants of each kind of building, in the same flat colours as the others.

use super::*;

mod tone {
    use egui::Color32;

    const fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    // Japan: white plaster, dark wood, grey or copper-green tiles, vermilion.
    pub const PLASTER: Color32 = rgb(0xf3efe6);
    pub const DARK_WOOD: Color32 = rgb(0x4a3a30);
    pub const KAWARA: [Color32; 3] = [rgb(0x3d4652), rgb(0x5f8f7a), rgb(0x4b4f58)];
    pub const VERMILION: Color32 = rgb(0xc8402f);
    pub const NOREN: [Color32; 3] = [rgb(0x2f4a7a), rgb(0xc8402f), rgb(0x3d6b4f)];
    pub const STONE: Color32 = rgb(0x9a9a92);
    pub const NEON: [Color32; 4] = [rgb(0xff4f7b), rgb(0x3fd0e0), rgb(0xf2c230), rgb(0x9b6bff)];
    // The North: painted boards, black roofs, white trims.
    pub const BOARDS: [Color32; 5] = [rgb(0xa8322d), rgb(0xd9a441), rgb(0x4f6f9a), rgb(0x5e8a6a), rgb(0xe9e4d8)];
    pub const BLACK_ROOF: Color32 = rgb(0x2f3135);
    pub const TRIM: Color32 = rgb(0xffffff);
    pub const STAVE: Color32 = rgb(0x5a3a28);
    pub const STAVE_DARK: Color32 = rgb(0x3a2a20);
    pub const LARCH: Color32 = rgb(0xc9a36b);
    pub const LAMP_GLASS: Color32 = rgb(0xf7d35e);
    // The desert: sand and clay walls, blue tiles, dark arches.
    pub const SANDS: [Color32; 4] = [rgb(0xe2c08f), rgb(0xd79a63), rgb(0xf1e4cc), rgb(0xc98f5a)];
    pub const BLUE_TILE: Color32 = rgb(0x2a6fb0);
    pub const ARCH: Color32 = rgb(0x4a3a32);
    pub const STRIPES: [Color32; 3] = [rgb(0xd9412f), rgb(0x2a6fb0), rgb(0x3f8f5a)];
    pub const SPICES: [Color32; 4] = [rgb(0xe3a13a), rgb(0xc0392b), rgb(0x8a5a3a), rgb(0xf2c230)];
    // The Netherlands: dark bricks, white frames, green shutters, sails.
    pub const DUTCH_BRICKS: [Color32; 5] = [rgb(0x8a3b2a), rgb(0x5e3a2e), rgb(0xa0522d), rgb(0x3f3a38), rgb(0x7a4a38)];
    pub const DUTCH_ROOF: Color32 = rgb(0x3a3a3a);
    pub const GREEN_SHUTTER: Color32 = rgb(0x2f6b4a);
    pub const COPPER: Color32 = rgb(0x5f9f8a);
    pub const MILL: Color32 = rgb(0x6b5a4a);
    pub const SAIL: Color32 = rgb(0xf4f0e6);
    pub const GREENHOUSE: Color32 = Color32::from_rgba_premultiplied(150, 190, 200, 170);
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Faces of one colour, its east face shaded.
fn flat(c: Color32) -> Faces {
    Faces { left: c, right: theme::shade(c, 0.16), top: theme::lighten(c, 0.1) }
}

/// A rectangle grown by `e` on every side.
fn grow((x0, y0, x1, y1): (f32, f32, f32, f32), e: f32) -> (f32, f32, f32, f32) {
    (x0 - e, y0 - e, x1 + e, y1 + e)
}

/// Merlons along a flat roof's four edges.
fn crenels(p: &Painter, iso: &Iso, (x0, y0, x1, y1): (f32, f32, f32, f32), z: f32, f: Faces) {
    let m = 0.07;
    let step = |a0: f32, a1: f32| {
        let n = ((a1 - a0) / (m * 2.0)).floor().max(1.0) as i32;
        (0..n).map(move |k| a0 + k as f32 * (a1 - a0) / n as f32)
    };
    for a in step(x0, x1) {
        cuboid(p, iso, (a, y0, a + m, y0 + m), z, z + 0.2, f);
    }
    for a in step(y0, y1) {
        cuboid(p, iso, (x0, a, x0 + m, a + m), z, z + 0.2, f);
    }
    for a in step(y0, y1) {
        cuboid(p, iso, (x1 - m, a, x1, a + m), z, z + 0.2, f);
    }
    for a in step(x0, x1) {
        cuboid(p, iso, (a, y1 - m, a + m, y1), z, z + 0.2, f);
    }
}

/// An arched opening on a face: a dark doorway with a rounded head.
fn arch(p: &Painter, iso: &Iso, south: bool, face: f32, (a0, a1): (f32, f32), (z0, z1): (f32, f32), c: Color32) {
    let w = a1 - a0;
    let spring = z1 - w * 0.9;
    poly(p, on_face(iso, south, face, (a0, a1), (z0, spring)), c);
    let at = |a: f32, z: f32| if south { iso.at(a, face, z) } else { iso.at(face, a, z) };
    let head: Vec<Pos2> = (0..=8)
        .map(|k| {
            let t = PI * k as f32 / 8.0;
            at(a0 + w / 2.0 - t.cos() * w / 2.0, spring + t.sin() * (z1 - spring))
        })
        .collect();
    poly(p, head, c);
}

/// A building of one of the world's styles.
#[allow(clippy::too_many_arguments)]
pub(super) fn building(p: &Painter, iso: &Iso, at: (f32, f32), style: Style, kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    match style {
        Style::Japanese => japanese(p, iso, at, kind, floors, variant, d),
        Style::Nordic => nordic(p, iso, at, kind, floors, variant, d),
        Style::Desert => desert(p, iso, at, kind, floors, variant, d),
        _ => dutch(p, iso, at, kind, floors, variant, d),
    }
}

fn japanese(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    let roof = d.of(&tone::KAWARA);
    let plaster = flat(tone::PLASTER);
    let wood = flat(tone::DARK_WOOD);
    // Dark timber posts and a beam across a plaster face.
    let beams = |(x0, y0, x1, y1): (f32, f32, f32, f32), (za, zb): (f32, f32)| {
        let s = Stroke::new(iso.px(1.4), tone::DARK_WOOD);
        for t in [0.0, 0.5, 1.0] {
            let (bx, by) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            p.line_segment([iso.at(bx, y1, za), iso.at(bx, y1, zb)], s);
            p.line_segment([iso.at(x1, by, za), iso.at(x1, by, zb)], s);
        }
        let m = (za + zb) / 2.0;
        p.line_segment([iso.at(x0, y1, m), iso.at(x1, y1, m)], s);
        p.line_segment([iso.at(x1, y0, m), iso.at(x1, y1, m)], s);
    };
    match (kind, variant) {
        (Building::Tower, 0) => {
            // A pagoda: a storey under wide eaves for each floor, a spire with its rings.
            let tiers = floors.max(3);
            let mut z = z0;
            for t in 0..tiers {
                let half = 0.27 - 0.1 * t as f32 / (tiers - 1) as f32;
                let r = (x + 0.5 - half, y + 0.5 - half, x + 0.5 + half, y + 0.5 + half);
                cuboid(p, iso, r, z, z + 0.6, flat(if t == 0 { tone::VERMILION } else { tone::DARK_WOOD }));
                poly(p, on_face(iso, true, r.3, (r.0 + half * 0.6, r.2 - half * 0.6), (z + 0.1, z + 0.5)), tone::PLASTER);
                hip(p, iso, grow(r, 0.13), z + 0.6, 0.4, roof);
                z += h / tiers as f32;
            }
            let top = iso.at(x + 0.5, y + 0.5, z0 + h + 0.3);
            p.line_segment([iso.at(x + 0.5, y + 0.5, z - h / tiers as f32 + 1.0), top - vec2(0.0, iso.px(14.0))], Stroke::new(iso.px(1.4), paint::YELLOW));
            top
        }
        (Building::Tower, 1) => {
            // A castle: a sloping stone base, white storeys each under its own roof.
            let base = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            cuboid(p, iso, base, z0, z0 + 1.2, flat(tone::STONE));
            let storeys = floors.saturating_sub(1).max(2);
            let mut z = z0 + 1.2;
            for t in 0..storeys {
                let half = 0.32 - 0.16 * t as f32 / storeys as f32;
                let r = (x + 0.5 - half, y + 0.5 - half, x + 0.5 + half, y + 0.5 + half);
                let hgt = (h - 1.2) / storeys as f32;
                cuboid(p, iso, r, z, z + hgt * 0.7, plaster);
                poly(p, on_face(iso, true, r.3, (r.0 + 0.06, r.0 + 0.14), (z + 0.15, z + hgt * 0.5)), tone::DARK_WOOD);
                poly(p, on_face(iso, false, r.2, (r.1 + 0.08, r.1 + 0.16), (z + 0.15, z + hgt * 0.5)), tone::DARK_WOOD);
                hip(p, iso, grow(r, 0.08), z + hgt * 0.7, hgt * 0.3 + 0.15, roof);
                z += hgt;
            }
            iso.at(x + 0.5, y + 0.5, z + 0.2)
        }
        (Building::Tower, _) => {
            // A Tokyo tower: grey, lit signs climbing its corner, a billboard on the roof.
            let r @ (x0, y0, x1, y1) = (x + 0.2, y + 0.2, x + 0.8, y + 0.8);
            cuboid(p, iso, r, z0, z0 + h, flat(hex(0x8d939b)));
            windows(p, iso, true, y1, (x0 + 0.12, x1), (0, floors), 2, (0.25, 0.75), hex(0x2b3542), paint::LINE);
            let (a, b) = (d.of(&tone::NEON), d.of(&tone::NEON));
            poly(p, on_face(iso, true, y1, (x0 + 0.02, x0 + 0.1), (z0 + 0.5, z0 + h - 0.3)), a);
            poly(p, on_face(iso, false, x1, (y0 + 0.1, y0 + 0.4), (z0 + h * 0.4, z0 + h * 0.7)), b);
            cuboid(p, iso, (x0 + 0.1, y0 + 0.1, x1 - 0.1, y0 + 0.16), z0 + h, z0 + h + 0.8, flat(d.of(&tone::NEON)));
            iso.at(x0 + 0.35, y0 + 0.4, z0 + h)
        }
        (Building::House, 0) => {
            // A machiya: a dark wooden front with its lattice, a cloth at the door, a tile roof.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + 2.1, wood);
            let s = Stroke::new(iso.px(0.8), theme::lighten(tone::DARK_WOOD, 0.3));
            let mut a = x0 + 0.04;
            while a < x1 - 0.02 {
                p.line_segment([iso.at(a, y1, z0 + 0.9), iso.at(a, y1, z0 + 1.9)], s);
                a += 0.04;
            }
            poly(p, on_face(iso, true, y1, (x0 + 0.08, x0 + 0.3), (z0 + 0.35, z0 + 0.8)), d.of(&tone::NOREN));
            pane(p, iso, false, x1, (y0 + 0.2, y1 - 0.2), (z0 + 1.1, z0 + 1.7), tone::PLASTER, tone::DARK_WOOD);
            hip(p, iso, grow((x0, y0, x1, y1), 0.05), z0 + 0.95, 0.15, roof);
            gable(p, iso, grow(r, 0.06), z0 + 2.1, h - 2.1, roof, wood, true);
            iso.at(x0 + 0.35, y0 + 0.35, z0 + h)
        }
        (Building::House, 1) => {
            // Two floors of plaster and dark beams under a hipped roof with deep eaves.
            let r @ (x0, y0, _, _) = (x + 0.18, y + 0.18, x + 0.82, y + 0.82);
            cuboid(p, iso, r, z0, z0 + 2.2, plaster);
            beams(r, (z0, z0 + 2.2));
            hip(p, iso, grow(r, 0.12), z0 + 2.2, h - 2.0, roof);
            iso.at(x0 + 0.32, y0 + 0.32, z0 + h + 0.2)
        }
        (Building::House, _) => {
            // A tea house raised on its deck, paper screens, a high roof.
            let deck = (x + 0.1, y + 0.1, x + 0.9, y + 0.9);
            cuboid(p, iso, deck, z0, z0 + 0.3, wood);
            let r @ (x0, y0, x1, y1) = (x + 0.2, y + 0.2, x + 0.8, y + 0.8);
            cuboid(p, iso, r, z0 + 0.3, z0 + 1.5, plaster);
            for k in 0..3 {
                let a = x0 + 0.04 + k as f32 * 0.19;
                pane(p, iso, true, y1, (a, a + 0.16), (z0 + 0.4, z0 + 1.3), tone::PLASTER, tone::DARK_WOOD);
                let b = y0 + 0.04 + k as f32 * 0.19;
                pane(p, iso, false, x1, (b, b + 0.16), (z0 + 0.4, z0 + 1.3), theme::shade(tone::PLASTER, 0.08), tone::DARK_WOOD);
            }
            hip(p, iso, grow(r, 0.14), z0 + 1.5, h - 1.3, roof);
            tree(p, iso, x + 0.86, y + 0.86, Tree::Leafy(3));
            iso.at(x + 0.5, y + 0.5, z0 + h + 0.2)
        }
        (Building::Workshop, 0) => {
            // A noodle shop: a low wooden house, cloths over the door, a lantern.
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.15, x + 0.88, y + 0.85);
            cuboid(p, iso, r, z0, z0 + 1.3, wood);
            let c = d.of(&tone::NOREN);
            for k in 0..3 {
                let a = x0 + 0.12 + k as f32 * 0.13;
                poly(p, on_face(iso, true, y1, (a, a + 0.11), (z0 + 0.6, z0 + 1.1)), c);
            }
            pane(p, iso, false, x1, (y0 + 0.15, y1 - 0.15), (z0 + 0.3, z0 + 1.0), tone::PLASTER, tone::DARK_WOOD);
            p.circle_filled(iso.at(x1 - 0.1, y1 + 0.03, z0 + 0.95), iso.px(3.0), tone::VERMILION);
            gable(p, iso, grow(r, 0.05), z0 + 1.3, h - 1.1, roof, wood, true);
            iso.at(x + 0.5, y + 0.5, z0 + h)
        }
        (Building::Workshop, 1) => {
            // A shrine: a small hall under a copper roof, its vermilion gate in front.
            let r @ (x0, y0, x1, y1) = (x + 0.3, y + 0.14, x + 0.86, y + 0.62);
            cuboid(p, iso, r, z0, z0 + 1.2, wood);
            poly(p, on_face(iso, true, y1, (x0 + 0.1, x1 - 0.1), (z0 + 0.2, z0 + 1.0)), tone::PLASTER);
            gable(p, iso, grow(r, 0.06), z0 + 1.2, h - 1.0, tone::KAWARA[1], wood, true);
            let gate = Stroke::new(iso.px(2.0), tone::VERMILION);
            let (gy, ga, gb) = (y + 0.84, x + 0.3, x + 0.7);
            p.line_segment([iso.at(ga, gy, z0), iso.at(ga, gy, z0 + 1.3)], gate);
            p.line_segment([iso.at(gb, gy, z0), iso.at(gb, gy, z0 + 1.3)], gate);
            p.line_segment([iso.at(ga - 0.08, gy, z0 + 1.35), iso.at(gb + 0.08, gy, z0 + 1.35)], Stroke::new(iso.px(2.6), tone::DARK_WOOD));
            p.line_segment([iso.at(ga, gy, z0 + 1.1), iso.at(gb, gy, z0 + 1.1)], gate);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, _) => {
            // A bathhouse: plaster under a tile roof, its tall chimney.
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.18, x + 0.82, y + 0.85);
            cuboid(p, iso, r, z0, z0 + 1.3, plaster);
            beams(r, (z0, z0 + 1.3));
            poly(p, on_face(iso, true, y1, (x0 + 0.25, x0 + 0.45), (z0, z0 + 0.7)), d.of(&tone::NOREN));
            gable(p, iso, grow(r, 0.05), z0 + 1.3, h - 1.1, roof, plaster, false);
            cylinder(p, iso, (x + 0.86, y + 0.2), 0.05, (z0, z0 + h + 1.4), flat(tone::STONE));
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
    }
}

fn nordic(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    let boards = d.of(&tone::BOARDS);
    let f = flat(boards);
    let trim_frame = |south: bool, face: f32, along: (f32, f32), z: (f32, f32)| pane(p, iso, south, face, along, z, hex(0x2b3542), tone::TRIM);
    // White boards at the visible corners.
    let corners = |(x0, _, x1, y1): (f32, f32, f32, f32), (za, zb): (f32, f32)| {
        let s = Stroke::new(iso.px(1.6), tone::TRIM);
        p.line_segment([iso.at(x0, y1, za), iso.at(x0, y1, zb)], s);
        p.line_segment([iso.at(x1, y1, za), iso.at(x1, y1, zb)], s);
    };
    match (kind, variant) {
        (Building::Tower, 0) => {
            // A stave church: dark wooden tiers under steep roofs, a spire.
            let tiers = 3;
            let stave = flat(tone::STAVE);
            let mut z = z0;
            for t in 0..tiers {
                let half = 0.32 - 0.09 * t as f32;
                let r = (x + 0.5 - half, y + 0.5 - half, x + 0.5 + half, y + 0.5 + half);
                let hgt = h / (tiers as f32 + 1.0);
                cuboid(p, iso, r, z, z + hgt * 0.55, stave);
                hip(p, iso, grow(r, 0.05), z + hgt * 0.55, hgt * 0.45, tone::BLACK_ROOF);
                z += hgt;
            }
            let half = 0.08;
            hip(p, iso, (x + 0.5 - half, y + 0.5 - half, x + 0.5 + half, y + 0.5 + half), z, h - z + z0 + 0.6, tone::BLACK_ROOF);
            iso.at(x + 0.5, y + 0.5, z0 + h + 0.6)
        }
        (Building::Tower, 1) => {
            // A lighthouse: a white tower with red bands, its lamp room, the keeper's house.
            let (cx, cy) = (x + 0.4, y + 0.4);
            cuboid(p, iso, (x + 0.5, y + 0.55, x + 0.88, y + 0.88), z0, z0 + 1.0, f);
            gable(p, iso, (x + 0.5, y + 0.55, x + 0.88, y + 0.88), z0 + 1.0, 0.5, tone::BLACK_ROOF, f, true);
            for k in 0..floors {
                let c = if k % 2 == 0 { tone::TRIM } else { paint::RED };
                cylinder(p, iso, (cx, cy), 0.2 - 0.06 * k as f32 / floors as f32, (z0 + k as f32, z0 + k as f32 + 1.0), flat(c));
            }
            cylinder(p, iso, (cx, cy), 0.12, (z0 + h, z0 + h + 0.55), flat(tone::LAMP_GLASS));
            dome(p, iso, (cx, cy), 0.13, z0 + h + 0.55, paint::RED);
            iso.at(cx, cy, z0 + h + 0.8)
        }
        (Building::Tower, _) => {
            // Flats clad in larch, tall windows, balconies.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h, flat(tone::LARCH));
            ribs(p, iso, true, y1, (x0, x1), (z0, z0 + h), tone::LARCH);
            ribs(p, iso, false, x1, (y0, y1), (z0, z0 + h), theme::shade(tone::LARCH, 0.16));
            windows(p, iso, true, y1, (x0, x1), (0, floors), 2, (0.15, 0.85), hex(0x2b3542), tone::TRIM);
            windows(p, iso, false, x1, (y0, y1), (0, floors), 2, (0.15, 0.85), hex(0x2b3542), tone::TRIM);
            parapet(p, iso, r, z0 + h, tone::BLACK_ROOF);
            iso.at(x0 + 0.35, y0 + 0.35, z0 + h)
        }
        (Building::House, 0) => {
            // A painted wooden house under a steep black roof.
            let r @ (x0, y0, x1, y1) = (x + 0.18, y + 0.15, x + 0.82, y + 0.85);
            cuboid(p, iso, r, z0, z0 + 1.6, f);
            corners(r, (z0, z0 + 1.6));
            for a in [0.12, 0.42] {
                trim_frame(true, y1, (x0 + a, x0 + a + 0.14), (z0 + 0.35, z0 + 1.1));
            }
            trim_frame(false, x1, (y0 + 0.15, y0 + 0.3), (z0 + 0.35, z0 + 1.1));
            poly(p, on_face(iso, false, x1, (y0 + 0.45, y0 + 0.58), (z0, z0 + 0.9)), tone::TRIM);
            gable(p, iso, grow(r, 0.04), z0 + 1.6, h - 1.4, tone::BLACK_ROOF, f, d.roll(2) == 0);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h + 0.2)
        }
        (Building::House, 1) => {
            // Two little houses of two colours, side by side.
            let second = flat(d.of(&tone::BOARDS));
            let mut top = iso.at(x + 0.5, y + 0.5, z0 + h);
            for (r, fc) in [((x + 0.12, y + 0.18, x + 0.48, y + 0.85), f), ((x + 0.52, y + 0.18, x + 0.88, y + 0.85), second)] {
                cuboid(p, iso, r, z0, z0 + 1.4, fc);
                corners(r, (z0, z0 + 1.4));
                trim_frame(true, r.3, (r.0 + 0.1, r.2 - 0.1), (z0 + 0.4, z0 + 1.1));
                gable(p, iso, r, z0 + 1.4, h - 1.4, tone::BLACK_ROOF, fc, false);
                top = iso.at((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0, z0 + h);
            }
            top
        }
        (Building::House, _) => {
            // A farmhouse with a porch and a dormer.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.75);
            cuboid(p, iso, r, z0, z0 + 1.8, f);
            corners(r, (z0, z0 + 1.8));
            for a in [0.08, 0.3, 0.52] {
                trim_frame(true, y1, (x0 + a, x0 + a + 0.12), (z0 + 1.0, z0 + 1.55));
            }
            cuboid(p, iso, (x0 + 0.2, y1, x0 + 0.5, y1 + 0.12), z0, z0 + 0.12, flat(tone::LARCH));
            for px in [x0 + 0.2, x0 + 0.5] {
                p.line_segment([iso.at(px, y1 + 0.12, z0), iso.at(px, y1 + 0.12, z0 + 0.8)], Stroke::new(iso.px(1.4), tone::TRIM));
            }
            poly(p, vec![iso.at(x0 + 0.15, y1, z0 + 0.8), iso.at(x0 + 0.55, y1, z0 + 0.8), iso.at(x0 + 0.55, y1 + 0.14, z0 + 0.7), iso.at(x0 + 0.15, y1 + 0.14, z0 + 0.7)], tone::BLACK_ROOF);
            gable(p, iso, r, z0 + 1.8, h - 1.6, tone::BLACK_ROOF, f, true);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h + 0.2)
        }
        (Building::Workshop, 0) => {
            // A red barn, its white trims and the white cross on its door.
            let barn = flat(tone::BOARDS[0]);
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.15, x + 0.88, y + 0.85);
            cuboid(p, iso, r, z0, z0 + 1.2, barn);
            corners(r, (z0, z0 + 1.2));
            let door = (x0 + 0.25, x0 + 0.5);
            let pts = on_face(iso, true, y1, door, (z0, z0 + 0.9));
            p.add(Shape::closed_line(pts.clone(), Stroke::new(iso.px(1.4), tone::TRIM)));
            p.line_segment([pts[0], pts[2]], Stroke::new(iso.px(1.2), tone::TRIM));
            p.line_segment([pts[1], pts[3]], Stroke::new(iso.px(1.2), tone::TRIM));
            trim_frame(false, x1, (y0 + 0.2, y0 + 0.35), (z0 + 0.4, z0 + 0.9));
            gable(p, iso, grow(r, 0.03), z0 + 1.2, h - 0.8, tone::BLACK_ROOF, barn, true);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h + 0.4)
        }
        (Building::Workshop, 1) => {
            // A boathouse on the shore: long, low, its doors to the water.
            let r @ (x0, y0, x1, y1) = (x + 0.2, y + 0.1, x + 0.8, y + 0.9);
            cuboid(p, iso, r, z0, z0 + 1.1, f);
            ribs(p, iso, true, y1, (x0, x1), (z0, z0 + 1.1), boards);
            poly(p, on_face(iso, true, y1, (x0 + 0.12, x1 - 0.12), (z0, z0 + 0.85)), theme::shade(boards, 0.4));
            gable(p, iso, grow(r, 0.03), z0 + 1.1, h - 1.0, tone::BLACK_ROOF, f, false);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, _) => {
            // A sauna cabin, its stove pipe and its woodpile.
            let r @ (x0, y0, x1, y1) = (x + 0.2, y + 0.2, x + 0.7, y + 0.7);
            let log = flat(tone::LARCH);
            cuboid(p, iso, r, z0, z0 + 1.1, log);
            ribs(p, iso, true, y1, (x0, x1), (z0, z0 + 1.1), tone::LARCH);
            poly(p, on_face(iso, true, y1, (x0 + 0.08, x0 + 0.2), (z0, z0 + 0.8)), tone::STAVE);
            gable(p, iso, grow(r, 0.04), z0 + 1.1, h - 1.0, tone::BLACK_ROOF, log, true);
            cylinder(p, iso, (x0 + 0.38, y0 + 0.12), 0.03, (z0 + 1.3, z0 + h + 0.6), flat(paint::WEIGHT));
            for k in 0..3 {
                for j in 0..3 - k {
                    p.circle_filled(iso.at(x + 0.78, y + 0.3 + j as f32 * 0.1 + k as f32 * 0.05, z0 + 0.12 + k as f32 * 0.2), iso.px(2.4), tone::LARCH);
                }
            }
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h + 0.1)
        }
    }
}

fn desert(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    let sand = d.of(&tone::SANDS);
    let f = flat(sand);
    let lip = flat(theme::lighten(sand, 0.12));
    // Small dark windows, a few to a floor.
    let slots = |(x0, y0, x1, y1): (f32, f32, f32, f32), (from, to): (u32, u32)| {
        windows(p, iso, true, y1, (x0, x1), (from, to), 2, (0.35, 0.7), tone::ARCH, tone::ARCH);
        windows(p, iso, false, x1, (y0, y1), (from, to), 2, (0.35, 0.7), tone::ARCH, tone::ARCH);
    };
    match (kind, variant) {
        (Building::Tower, 0) => {
            // A mosque: its hall under a great dome, the minaret at its corner.
            let hall @ (x0, y0, x1, y1) = (x + 0.3, y + 0.3, x + 0.88, y + 0.88);
            let m @ (mx0, my0, mx1, my1) = (x + 0.1, y + 0.1, x + 0.3, y + 0.3);
            cuboid(p, iso, m, z0, z0 + h, lip);
            cuboid(p, iso, grow(m, 0.04), z0 + h - 1.0, z0 + h - 0.85, flat(tone::BLUE_TILE));
            cuboid(p, iso, (mx0 + 0.04, my0 + 0.04, mx1 - 0.04, my1 - 0.04), z0 + h, z0 + h + 0.5, lip);
            dome(p, iso, ((mx0 + mx1) / 2.0, (my0 + my1) / 2.0), 0.08, z0 + h + 0.5, tone::BLUE_TILE);
            cuboid(p, iso, hall, z0, z0 + 1.6, f);
            for k in 0..3 {
                let a = x0 + 0.06 + k as f32 * 0.18;
                arch(p, iso, true, y1, (a, a + 0.12), (z0, z0 + 1.0), tone::ARCH);
            }
            crenels(p, iso, hall, z0 + 1.6, f);
            dome(p, iso, ((x0 + x1) / 2.0, (y0 + y1) / 2.0), 0.24, z0 + 1.6, d.of(&[tone::BLUE_TILE, hex(0x3f8f5a), hex(0xd9b23a)]));
            iso.at((mx0 + mx1) / 2.0, (my0 + my1) / 2.0, z0 + h + 0.9)
        }
        (Building::Tower, 1) => {
            // A kasbah: a tall clay block, a tower at each corner, crenellated.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h - 0.6, f);
            slots(r, (1, floors));
            arch(p, iso, true, y1, (x0 + 0.26, x0 + 0.44), (z0, z0 + 0.9), tone::ARCH);
            for (cx, cy) in [(x0, y0), (x1 - 0.16, y0), (x1 - 0.16, y1 - 0.16), (x0, y1 - 0.16)] {
                let t = (cx, cy, cx + 0.16, cy + 0.16);
                cuboid(p, iso, t, z0, z0 + h, f);
                crenels(p, iso, t, z0 + h, f);
            }
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h - 0.6)
        }
        (Building::Tower, _) => {
            // A merchant's house cooled by two wind towers.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h - 1.0, f);
            slots(r, (0, floors - 1));
            crenels(p, iso, r, z0 + h - 1.0, f);
            for (bx, by) in [(x0 + 0.1, y0 + 0.1), (x0 + 0.38, y0 + 0.3)] {
                let t @ (tx0, _, _, ty1) = (bx, by, bx + 0.16, by + 0.16);
                cuboid(p, iso, t, z0 + h - 1.0, z0 + h + 0.8, lip);
                for k in 0..3 {
                    let a = tx0 + 0.02 + k as f32 * 0.05;
                    poly(p, on_face(iso, true, ty1, (a, a + 0.03), (z0 + h + 0.1, z0 + h + 0.6)), tone::ARCH);
                }
            }
            iso.at(x1 - 0.2, y1 - 0.2, z0 + h - 1.0)
        }
        (Building::House, 0) => {
            // A flat-roofed house with crenellations, an arched door, a palm on its terrace.
            let r @ (x0, y0, x1, y1) = (x + 0.15, y + 0.15, x + 0.85, y + 0.85);
            cuboid(p, iso, r, z0, z0 + h, f);
            slots(r, (1, floors));
            arch(p, iso, true, y1, (x0 + 0.24, x0 + 0.42), (z0, z0 + 0.95), d.of(&[tone::BLUE_TILE, tone::ARCH, paint::DOOR]));
            crenels(p, iso, r, z0 + h, f);
            tree(p, iso, x0 + 0.4, y0 + 0.4, Tree::Palm);
            iso.at(x1 - 0.18, y1 - 0.18, z0 + h)
        }
        (Building::House, 1) => {
            // A white cube under a dome.
            let r @ (x0, y0, x1, y1) = (x + 0.18, y + 0.18, x + 0.82, y + 0.82);
            let white = flat(tone::SANDS[2]);
            cuboid(p, iso, r, z0, z0 + h - 1.0, white);
            slots(r, (0, floors - 1));
            arch(p, iso, false, x1, (y0 + 0.24, y0 + 0.4), (z0, z0 + 0.9), tone::ARCH);
            dome(p, iso, ((x0 + x1) / 2.0, (y0 + y1) / 2.0), 0.26, z0 + h - 1.0, d.of(&[tone::SANDS[2], tone::BLUE_TILE]));
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h + 0.6)
        }
        (Building::House, _) => {
            // A courtyard house: walls round a palm, the living wing at the back.
            let (x0, y0, x1, y1) = (x + 0.12, y + 0.12, x + 0.88, y + 0.88);
            let back = (x0, y0, x1, y0 + 0.3);
            cuboid(p, iso, back, z0, z0 + h, f);
            crenels(p, iso, back, z0 + h, f);
            let side = (x0, y0 + 0.3, x0 + 0.22, y1);
            cuboid(p, iso, side, z0, z0 + h - 1.0, f);
            poly(p, iso.quad(x0 + 0.22, y0 + 0.3, x1, y1, z0).to_vec(), theme::lighten(sand, 0.2));
            tree(p, iso, x0 + 0.5, y0 + 0.55, Tree::Palm);
            cuboid(p, iso, (x0 + 0.22, y1 - 0.05, x1, y1), z0, z0 + 0.7, f);
            cuboid(p, iso, (x1 - 0.05, y0 + 0.3, x1, y1), z0, z0 + 0.7, f);
            arch(p, iso, true, y1, (x0 + 0.42, x0 + 0.58), (z0, z0 + 0.65), tone::ARCH);
            iso.at(x0 + 0.4, y0 + 0.15, z0 + h)
        }
        (Building::Workshop, 0) => {
            // A souk: stalls under striped awnings, sacks of spice in front.
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.12, x + 0.88, y + 0.7);
            cuboid(p, iso, r, z0, z0 + h, f);
            crenels(p, iso, r, z0 + h, f);
            let stripe = d.of(&tone::STRIPES);
            for k in 0..3 {
                let a = x0 + 0.04 + k as f32 * 0.25;
                arch(p, iso, true, y1, (a, a + 0.18), (z0, z0 + 1.0), tone::ARCH);
                poly(p, vec![iso.at(a - 0.02, y1, z0 + 1.15), iso.at(a + 0.2, y1, z0 + 1.15), iso.at(a + 0.2, y1 + 0.14, z0 + 0.9), iso.at(a - 0.02, y1 + 0.14, z0 + 0.9)], if k % 2 == 0 { stripe } else { paint::LINE });
                for j in 0..2 {
                    p.circle_filled(iso.at(a + 0.05 + j as f32 * 0.08, y1 + 0.12, z0 + 0.12), iso.px(2.6), d.of(&tone::SPICES));
                }
            }
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, 1) => {
            // A bathhouse under three little domes.
            let r @ (x0, y0, _, y1) = (x + 0.12, y + 0.2, x + 0.88, y + 0.8);
            cuboid(p, iso, r, z0, z0 + h - 0.6, f);
            arch(p, iso, true, y1, (x0 + 0.3, x0 + 0.46), (z0, z0 + 0.9), tone::ARCH);
            for k in 0..3 {
                dome(p, iso, (x0 + 0.13 + k as f32 * 0.25, (y0 + y1) / 2.0), 0.11, z0 + h - 0.6, lip.left);
            }
            iso.at(x0 + 0.38, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, _) => {
            // A caravanserai: an arcade under a band of blue tiles.
            let r @ (x0, y0, x1, y1) = (x + 0.1, y + 0.12, x + 0.9, y + 0.88);
            cuboid(p, iso, r, z0, z0 + h, f);
            for k in 0..4 {
                let a = x0 + 0.04 + k as f32 * 0.19;
                arch(p, iso, true, y1, (a, a + 0.13), (z0, z0 + 1.0), tone::ARCH);
            }
            for k in 0..3 {
                let a = y0 + 0.08 + k as f32 * 0.24;
                arch(p, iso, false, x1, (a, a + 0.14), (z0, z0 + 1.0), tone::ARCH);
            }
            poly(p, on_face(iso, true, y1, (x0, x1), (z0 + h - 0.45, z0 + h - 0.3)), tone::BLUE_TILE);
            poly(p, on_face(iso, false, x1, (y0, y1), (z0 + h - 0.45, z0 + h - 0.3)), theme::shade(tone::BLUE_TILE, 0.16));
            crenels(p, iso, r, z0 + h, f);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
    }
}

/// A narrow canal house, its front (south) gable stepped or crowned; `walls` floors of brick.
fn canal(p: &Painter, iso: &Iso, (x0, y0, x1, y1): (f32, f32, f32, f32), walls: f32, rise: f32, brick: Color32, stepped: bool) -> Pos2 {
    let z0 = KERB;
    let f = flat(brick);
    cuboid(p, iso, (x0, y0, x1, y1), z0, z0 + walls, f);
    let w = x1 - x0;
    for fl in 0..walls.floor() as u32 {
        let z = z0 + fl as f32;
        pane(p, iso, true, y1, (x0 + w * 0.22, x0 + w * 0.78), (z + 0.2, z + 0.8), hex(0x2b3542), tone::TRIM);
        pane(p, iso, false, x1, (y0 + 0.12, y0 + 0.28), (z + 0.25, z + 0.78), hex(0x2b3542), tone::TRIM);
        pane(p, iso, false, x1, (y1 - 0.28, y1 - 0.12), (z + 0.25, z + 0.78), hex(0x2b3542), tone::TRIM);
    }
    gable(p, iso, (x0, y0, x1, y1), z0 + walls, rise, tone::DUTCH_ROOF, f, false);
    let z = z0 + walls;
    if stepped {
        for (k, frac) in [0.8, 0.56, 0.32].into_iter().enumerate() {
            let m = (x0 + x1) / 2.0;
            poly(p, on_face(iso, true, y1, (m - w * frac / 2.0, m + w * frac / 2.0), (z + k as f32 * rise / 3.0, z + (k + 1) as f32 * rise / 3.0 + 0.05)), brick);
        }
    } else {
        let m = (x0 + x1) / 2.0;
        poly(p, on_face(iso, true, y1, (m - w * 0.3, m + w * 0.3), (z, z + rise * 0.85)), brick);
        poly(p, vec![iso.at(m - w * 0.36, y1, z + rise * 0.85), iso.at(m + w * 0.36, y1, z + rise * 0.85), iso.at(m, y1, z + rise * 1.15)], tone::TRIM);
    }
    pane(p, iso, true, y1, (x0 + w * 0.38, x0 + w * 0.62), (z + 0.1, z + 0.45), hex(0x2b3542), tone::TRIM);
    p.line_segment([iso.at(x0, y1, z), iso.at(x1, y1, z)], Stroke::new(iso.px(1.4), tone::TRIM));
    iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z + rise)
}

fn dutch(p: &Painter, iso: &Iso, (x, y): (f32, f32), kind: Building, floors: u32, variant: usize, d: &mut Dice) -> Pos2 {
    let z0 = KERB;
    let h = floors as f32;
    match (kind, variant) {
        (Building::Tower, 0) => {
            // A windmill: a tapering body, its stage, its cap and four sails.
            let (cx, cy) = (x + 0.5, y + 0.5);
            cuboid(p, iso, (x + 0.15, y + 0.15, x + 0.85, y + 0.85), z0, z0 + 1.0, flat(d.of(&tone::DUTCH_BRICKS)));
            let body = flat(tone::MILL);
            let n = floors.max(3);
            for k in 1..n {
                let r = 0.26 - 0.1 * k as f32 / n as f32;
                cylinder(p, iso, (cx, cy), r, (z0 + k as f32, z0 + k as f32 + 1.0), body);
            }
            p.line_segment([iso.at(x + 0.15, y + 0.85, z0 + 1.0), iso.at(x + 0.85, y + 0.85, z0 + 1.0)], Stroke::new(iso.px(1.4), tone::TRIM));
            dome(p, iso, (cx, cy), 0.17, z0 + h, tone::DUTCH_ROOF);
            let hub = iso.at(cx, cy + 0.2, z0 + h - 0.2);
            for k in 0..4 {
                let a = 0.4 + k as f32 * FRAC_PI_2;
                let dir = vec2(a.cos(), a.sin());
                let tip = hub + dir * iso.px(44.0);
                let side = vec2(-dir.y, dir.x) * iso.px(5.0);
                poly(p, vec![hub + dir * iso.px(8.0), tip, tip + side, hub + dir * iso.px(8.0) + side], tone::SAIL);
                p.line_segment([hub, tip], Stroke::new(iso.px(1.4), tone::STAVE_DARK));
            }
            p.circle_filled(hub, iso.px(2.5), tone::STAVE_DARK);
            iso.at(cx, cy, z0 + h + 0.4)
        }
        (Building::Tower, 1) => {
            // A church tower of brick, its clock, a copper spire.
            let r @ (x0, y0, x1, y1) = (x + 0.25, y + 0.25, x + 0.75, y + 0.75);
            let brick = flat(d.of(&tone::DUTCH_BRICKS));
            cuboid(p, iso, r, z0, z0 + h - 1.0, brick);
            for fl in 0..floors - 1 {
                arch(p, iso, true, y1, (x0 + 0.18, x1 - 0.18), (z0 + fl as f32 + 0.2, z0 + fl as f32 + 0.85), hex(0x2b3542));
            }
            p.circle_filled(iso.at((x0 + x1) / 2.0, y1, z0 + h - 1.5), iso.px(5.0), tone::TRIM);
            p.line_segment([iso.at((x0 + x1) / 2.0, y1, z0 + h - 1.5), iso.at((x0 + x1) / 2.0, y1, z0 + h - 1.3)], Stroke::new(iso.px(1.0), tone::DUTCH_ROOF));
            hip(p, iso, grow(r, 0.03), z0 + h - 1.0, 2.2, tone::COPPER);
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h + 1.2)
        }
        (Building::Tower, _) => {
            // Two tall canal houses side by side.
            let a = canal(p, iso, (x + 0.15, y + 0.15, x + 0.5, y + 0.85), h - 1.0, 1.0, d.of(&tone::DUTCH_BRICKS), true);
            let _ = canal(p, iso, (x + 0.5, y + 0.2, x + 0.85, y + 0.85), h - 1.6, 0.9, d.of(&tone::DUTCH_BRICKS), false);
            a
        }
        (Building::House, 0) => {
            // Three narrow canal houses, each its own brick and gable.
            let mut top = iso.at(x + 0.5, y + 0.5, z0 + h);
            for k in 0..3 {
                let x0 = x + 0.12 + k as f32 * 0.255;
                let walls = h - 1.0 + (d.roll(2) as f32) * 0.5;
                let t = canal(p, iso, (x0, y + 0.15, x0 + 0.25, y + 0.85), walls, 0.9, d.of(&tone::DUTCH_BRICKS), d.roll(2) == 0);
                if k == 1 {
                    top = t;
                }
            }
            top
        }
        (Building::House, 1) => {
            // A merchant's warehouse: green shutters on its loading doors, a hoist beam.
            let brick = d.of(&tone::DUTCH_BRICKS);
            let r @ (x0, _, x1, y1) = (x + 0.2, y + 0.15, x + 0.8, y + 0.85);
            let top = canal(p, iso, r, h - 1.0, 1.0, brick, false);
            let m = (x0 + x1) / 2.0;
            for fl in 0..floors - 1 {
                let z = z0 + fl as f32;
                poly(p, on_face(iso, true, y1, (m - 0.18, m - 0.1), (z + 0.2, z + 0.8)), tone::GREEN_SHUTTER);
                poly(p, on_face(iso, true, y1, (m + 0.1, m + 0.18), (z + 0.2, z + 0.8)), tone::GREEN_SHUTTER);
            }
            p.line_segment([iso.at(m, y1, z0 + h - 0.4), iso.at(m, y1 + 0.15, z0 + h - 0.4)], Stroke::new(iso.px(2.0), tone::STAVE_DARK));
            top
        }
        (Building::House, _) => {
            // Two canal houses and a bicycle by the door.
            let a = canal(p, iso, (x + 0.12, y + 0.15, x + 0.5, y + 0.85), h - 1.0, 0.9, d.of(&tone::DUTCH_BRICKS), true);
            let _ = canal(p, iso, (x + 0.5, y + 0.15, x + 0.88, y + 0.85), h - 1.5, 0.9, d.of(&tone::DUTCH_BRICKS), false);
            let (bx, by) = (x + 0.3, y + 0.92);
            let s = Stroke::new(iso.px(1.0), tone::STAVE_DARK);
            let (w0, w1) = (iso.at(bx, by, z0 + 0.12), iso.at(bx + 0.14, by, z0 + 0.12));
            p.circle_stroke(w0, iso.px(2.6), s);
            p.circle_stroke(w1, iso.px(2.6), s);
            p.line_segment([w0, iso.at(bx + 0.07, by, z0 + 0.3)], s);
            p.line_segment([w1, iso.at(bx + 0.07, by, z0 + 0.3)], s);
            a
        }
        (Building::Workshop, 0) => {
            // A greenhouse: a glass roof on a low wall, rows of plants inside.
            let r @ (x0, y0, x1, y1) = (x + 0.1, y + 0.12, x + 0.9, y + 0.88);
            for k in 0..4 {
                let py = y0 + 0.12 + k as f32 * 0.18;
                for j in 0..5 {
                    p.circle_filled(iso.at(x0 + 0.1 + j as f32 * 0.15, py, z0 + 0.1), iso.px(2.0), paint::FLOWERS[(k + j) % paint::FLOWERS.len()]);
                }
            }
            let glass = Faces { left: tone::GREENHOUSE, right: tone::GREENHOUSE, top: tone::GREENHOUSE };
            cuboid(p, iso, r, z0, z0 + 1.0, glass);
            gable(p, iso, r, z0 + 1.0, h - 1.0, tone::GREENHOUSE, glass, true);
            let s = Stroke::new(iso.px(0.8), tone::TRIM);
            let mut a = x0;
            while a <= x1 + 0.01 {
                p.line_segment([iso.at(a, y1, z0), iso.at(a, y1, z0 + 1.0)], s);
                p.line_segment([iso.at(a, y1, z0 + 1.0), iso.at(a, (y0 + y1) / 2.0, z0 + h)], s);
                a += 0.16;
            }
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, 1) => {
            // A flower shop: a low brick house, buckets of tulips under its awning.
            let brick = flat(d.of(&tone::DUTCH_BRICKS));
            let r @ (x0, y0, x1, y1) = (x + 0.12, y + 0.15, x + 0.88, y + 0.7);
            cuboid(p, iso, r, z0, z0 + h - 0.5, brick);
            pane(p, iso, true, y1, (x0 + 0.06, x1 - 0.3), (z0 + 0.1, z0 + 0.9), hex(0x2b3542), tone::TRIM);
            poly(p, on_face(iso, true, y1, (x1 - 0.24, x1 - 0.1), (z0, z0 + 0.85)), tone::GREEN_SHUTTER);
            gable(p, iso, r, z0 + h - 0.5, 0.6, tone::DUTCH_ROOF, brick, true);
            poly(p, vec![iso.at(x0, y1, z0 + 1.1), iso.at(x1, y1, z0 + 1.1), iso.at(x1, y1 + 0.15, z0 + 0.85), iso.at(x0, y1 + 0.15, z0 + 0.85)], tone::GREEN_SHUTTER);
            for k in 0..6 {
                tree(p, iso, x0 + 0.06 + k as f32 * 0.12, y1 + 0.12, Tree::Flowers(k));
            }
            iso.at((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0 + h)
        }
        (Building::Workshop, _) => {
            // A low brick house with a bell gable and a dormer.
            canal(p, iso, (x + 0.15, y + 0.15, x + 0.85, y + 0.85), h - 0.8, 0.8, d.of(&tone::DUTCH_BRICKS), false)
        }
    }
}

/// A free tile of a world district: a stone lantern under cherries and maples, birches and
/// spruces, palms on sand, or rows of tulips.
pub(super) fn yard(p: &Painter, iso: &Iso, (x, y): (i32, i32), style: Style) {
    let h = mix(x, y);
    let (fx, fy) = (x as f32, y as f32);
    match style {
        Style::Japanese => {
            tree(p, iso, fx + 0.3, fy + 0.3, Tree::Leafy(5));
            tree(p, iso, fx + 0.7, fy + 0.35, Tree::Leafy(if h.is_multiple_of(2) { 3 } else { 5 }));
            let (lx, ly) = (fx + 0.4, fy + 0.68);
            let stone = flat(tone::STONE);
            cuboid(p, iso, (lx - 0.03, ly - 0.03, lx + 0.03, ly + 0.03), KERB, KERB + 0.45, stone);
            cuboid(p, iso, (lx - 0.07, ly - 0.07, lx + 0.07, ly + 0.07), KERB + 0.45, KERB + 0.6, stone);
            hip(p, iso, (lx - 0.09, ly - 0.09, lx + 0.09, ly + 0.09), KERB + 0.6, 0.15, tone::STONE);
            tree(p, iso, fx + 0.75, fy + 0.75, Tree::Bush);
        }
        Style::Nordic => {
            tree(p, iso, fx + 0.25, fy + 0.3, Tree::Spruce);
            tree(p, iso, fx + 0.65, fy + 0.25, Tree::Birch);
            tree(p, iso, fx + 0.45, fy + 0.6, Tree::Birch);
            tree(p, iso, fx + 0.78, fy + 0.72, if h.is_multiple_of(2) { Tree::Spruce } else { Tree::Flowers(4) });
        }
        Style::Desert => {
            tree(p, iso, fx + 0.3, fy + 0.3, Tree::Palm);
            tree(p, iso, fx + 0.7, fy + 0.5, Tree::Palm);
            tree(p, iso, fx + 0.4, fy + 0.75, Tree::Bush);
        }
        _ => {
            for row in 0..4 {
                for k in 0..4 {
                    tree(p, iso, fx + 0.18 + k as f32 * 0.2, fy + 0.18 + row as f32 * 0.2, Tree::Flowers(row + h % 3));
                }
            }
        }
    }
}

/// The pavement and the ground of a world district.
pub(super) fn colours(style: Style) -> (Color32, Color32) {
    match style {
        Style::Japanese => (Color32::from_rgb(0xdd, 0xd8, 0xce), Color32::from_rgb(0x8f, 0xbf, 0x6a)),
        Style::Nordic => (Color32::from_rgb(0xd4, 0xdc, 0xe0), Color32::from_rgb(0x6f, 0xae, 0x5a)),
        Style::Desert => (Color32::from_rgb(0xe8, 0xd2, 0xa6), Color32::from_rgb(0xdc, 0xc0, 0x88)),
        _ => (Color32::from_rgb(0xcd, 0xbf, 0xb2), Color32::from_rgb(0x5f, 0xb0, 0x4f)),
    }
}

