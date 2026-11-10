//! The Gamer skin's drawings (#218), from the "Icons" and "Mess" artboards of the design
//! canvas: SVG paths in their own boxes, drawn by `art::draw` (the litter's `Pen`) in the same
//! kawaii style. The gamer from behind on his chair in his poses, the game pad, the XP gem, the
//! bug, the trophy, the boss, the cans, the pizza box, the spill, the fly, the bin bag, and the
//! emblems and stickers of the game boxes. The crisp bag is the Big eater's `CHIPS`.

use super::{draw, Part, BLUSH, EYE, INK};
use egui::{vec2, Color32, Painter, Pos2};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// A colour seen through: `hex` at `a` of 255.
const fn rgba(hex: u32, a: u8) -> Color32 {
    let a32 = a as u32;
    Color32::from_rgba_premultiplied((((hex >> 16) & 0xff) * a32 / 255) as u8, (((hex >> 8) & 0xff) * a32 / 255) as u8, ((hex & 0xff) * a32 / 255) as u8, a)
}

/// The XP's lime, the boss's red, Legend's gold: each a fill, a light, a shade and a side.
pub(in crate::app) const LIME: [Color32; 4] = [rgb(0xc6f432), rgb(0xeaff9e), rgb(0x9fcf1a), rgb(0xb2e021)];
pub(in crate::app) const RED: [Color32; 4] = [rgb(0xff4d6d), rgb(0xffb3c0), rgb(0xd63a57), rgb(0xec4463)];
pub(in crate::app) const GOLD: [Color32; 4] = [rgb(0xffc93c), rgb(0xfff1c2), rgb(0xe0a21a), rgb(0xf2b52e)];

const SKIN: Color32 = rgb(0xf1c9a5);
const HOODIE: Color32 = rgb(0x7b68ee);
const SEAT: Color32 = rgb(0x24222c);
const BLACK: Color32 = rgb(0x1b1c20);

/// What the gamer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Pose {
    /// Typing: his rest pose, and a commit.
    Type,
    /// Both arms up: a push, a merge, a level.
    Up,
    /// A hand on his head: Undo.
    Palm,
    /// Leaning in, sweating: a boss fight.
    Lean,
    /// The bin bag held up: tidying.
    Lift,
}

/// The chair's legs, wheels, post and seat, behind him.
const CHAIR_BASE: &[Part] = &[
    Part("M24 160 L60 150 L96 160 M60 150 L60 166", None, Some(INK), 4.0),
    Part("M18 162 a4 4 0 1 0 8 0 a4 4 0 1 0 -8 0 Z M94 162 a4 4 0 1 0 8 0 a4 4 0 1 0 -8 0 Z M56 168 a4 4 0 1 0 8 0 a4 4 0 1 0 -8 0 Z", Some(BLACK), None, 0.0),
    Part("M55 128 H65 V151 H55 Z", Some(rgb(0x3a3d45)), Some(INK), 2.0),
    Part("M18 120 Q60 110 102 120 L100 132 Q60 142 20 132 Z", Some(SEAT), Some(INK), 2.4),
];
/// Both arms down to the keyboard.
const ARMS_DOWN: &[Part] = &[
    Part("M28 78 Q8 92 12 110 Q15 119 26 116 L33 104 Q24 96 35 86 Z", Some(HOODIE), Some(INK), 2.4),
    Part("M92 78 Q112 92 108 110 Q105 119 94 116 L87 104 Q96 96 85 86 Z", Some(HOODIE), Some(INK), 2.4),
];
/// Both arms up, fists out.
const ARMS_UP: &[Part] = &[
    Part("M32 80 Q14 60 18 32 Q21 24 29 27 Q31 50 44 70 Z", Some(HOODIE), Some(INK), 2.4),
    Part("M88 80 Q106 60 102 32 Q99 24 91 27 Q89 50 76 70 Z", Some(HOODIE), Some(INK), 2.4),
    Part("M17 25 a7 7 0 1 0 14 0 a7 7 0 1 0 -14 0 Z M89 25 a7 7 0 1 0 14 0 a7 7 0 1 0 -14 0 Z", Some(SKIN), Some(INK), 2.0),
];
/// The right arm up with the hand on his head.
const PALM: &[Part] = &[
    Part("M92 80 Q108 62 88 38 Q82 33 77 39 Q90 56 80 72 Z", Some(HOODIE), Some(INK), 2.4),
    Part("M68 34 a8 7 0 1 0 16 0 a8 7 0 1 0 -16 0 Z", Some(SKIN), Some(INK), 2.0),
];
/// The right arm up, holding the bin bag.
const LIFT: &[Part] = &[
    Part("M88 80 Q106 60 102 32 Q99 24 91 27 Q89 50 76 70 Z", Some(HOODIE), Some(INK), 2.4),
    Part("M89 25 a7 7 0 1 0 14 0 a7 7 0 1 0 -14 0 Z", Some(SKIN), Some(INK), 2.0),
];
/// Both arms forward, leaning in.
const ARMS_LEAN: &[Part] = &[
    Part("M30 86 Q6 100 12 120 Q15 128 26 124 L34 112 Q24 104 36 94 Z", Some(HOODIE), Some(INK), 2.4),
    Part("M90 86 Q114 100 108 120 Q105 128 94 124 L86 112 Q96 104 84 94 Z", Some(HOODIE), Some(INK), 2.4),
];
/// His back, hood, hair and headset.
const BODY: &[Part] = &[
    Part("M28 76 Q60 58 92 76 L96 104 L24 104 Z", Some(HOODIE), Some(INK), 2.4),
    Part("M44 62 Q60 70 76 62 Q82 70 76 78 Q60 86 44 78 Q38 70 44 62 Z", Some(rgb(0x5a48b8)), Some(INK), 2.0),
    Part("M38 42 a22 22 0 1 1 44 0 Q84 58 74 64 Q60 70 46 64 Q36 58 38 42 Z", Some(rgb(0x3a2a22)), Some(INK), 2.4),
    Part("M48 28 Q55 22 64 24", None, Some(rgb(0x6b4a38)), 3.0),
    Part("M37 42 Q38 16 60 16 Q82 16 83 42", None, Some(BLACK), 5.0),
    Part("M29 36 Q29 32 33 32 H37 Q41 32 41 36 V52 Q41 56 37 56 H33 Q29 56 29 52 Z", Some(BLACK), Some(INK), 2.0),
    Part("M79 36 Q79 32 83 32 H87 Q91 32 91 36 V52 Q91 56 87 56 H83 Q79 56 79 52 Z", Some(BLACK), Some(INK), 2.0),
];
/// The headset's lights, in the room's colour.
const LEDS_D: &str = "M32.5 39 V49 M87.5 39 V49";
/// The chair's back over him, its red stripes and seam.
const CHAIR_BACK: &[Part] = &[
    Part("M30 84 Q30 76 40 75 Q60 72 80 75 Q90 76 90 84 L92 122 Q60 128 28 122 Z", Some(SEAT), Some(INK), 2.4),
    Part("M44 78 L42 122 M76 78 L78 122", None, Some(rgb(0xff4d6d)), 4.0),
];
/// The diamond on the chair's back, in the room's colour.
const DIAMOND_D: &str = "M60 92 l6 6 l-6 6 l-6 -6 Z";
const SEAM: &[Part] = &[Part("M35 86 Q34 100 35 112", None, Some(rgb(0x3a3846)), 3.0)];
/// Two drops of sweat by his head.
const SWEAT: &[Part] = &[Part("M96 30 Q100 37 96 40 Q92 37 96 30 Z M104 44 Q107 49 104 51 Q101 49 104 44 Z", Some(rgb(0x7fd1ff)), Some(INK), 1.4)];

/// The gamer from behind on his chair, his box's (0, 0) at `o`, scaled `k` (his box is 120 × 170,
/// his chair's wheels at its bottom middle); his headset and the chair's diamond in `light`.
pub(in crate::app) fn gamer(p: &Painter, o: Pos2, k: f32, pose: Pose, light: Color32) {
    let at = |parts: &[Part], dy: f32| draw(p, parts, o + vec2(0.0, dy) * k, k, 0.0, 1.0);
    at(CHAIR_BASE, 0.0);
    let lean = if pose == Pose::Lean { 8.0 } else { 0.0 };
    match pose {
        Pose::Type => {
            at(ARMS_DOWN, 0.0);
            at(BODY, 0.0);
        }
        Pose::Up => {
            at(BODY, 0.0);
            at(ARMS_UP, 0.0);
        }
        Pose::Palm => {
            at(&ARMS_DOWN[..1], 0.0);
            at(BODY, 0.0);
            at(PALM, 0.0);
        }
        Pose::Lean => {
            at(ARMS_LEAN, 0.0);
            at(BODY, lean);
        }
        Pose::Lift => {
            at(&ARMS_DOWN[..1], 0.0);
            at(BODY, 0.0);
            at(LIFT, 0.0);
        }
    }
    at(&[Part(LEDS_D, None, Some(light), 2.6)], lean);
    at(CHAIR_BACK, 0.0);
    at(&[Part(DIAMOND_D, Some(light), Some(INK), 1.4)], 0.0);
    at(SEAM, 0.0);
    if pose == Pose::Lean {
        at(SWEAT, 0.0);
    }
}

/// Where the gamer's raised right fist is, in his box (the bin bag hangs from it).
pub(in crate::app) const FIST: egui::Vec2 = vec2(96.0, 25.0);

/// A game pad, its light bar in `led`, in a 64 × 42 box.
pub(in crate::app) fn pad(led: Color32) -> [Part; 8] {
    [
        Part("M14 8 H50 Q60 8 62 22 Q64 36 56 38 Q50 39 46 30 H18 Q14 39 8 38 Q0 36 2 22 Q4 8 14 8 Z", Some(rgb(0xe9ecf2)), Some(INK), 2.4),
        Part("M46 30 H18 Q14 39 8 38 Q4 37 3 32 Q8 34 12 30 Q16 25 24 26 H40 Q48 25 52 30 Q56 34 61 32 Q60 37 56 38 Q50 39 46 30 Z", Some(rgb(0xc3c8d2)), None, 0.0),
        Part("M10 13 Q14 11 20 11", None, Some(rgb(0xffffff)), 2.2),
        Part("M24 9 H40", None, Some(led), 3.4),
        Part("M13 20 H21 M17 16 V24", None, Some(INK), 3.0),
        Part("M44 16 a2.2 2.2 0 1 0 4.4 0 a2.2 2.2 0 1 0 -4.4 0 Z M50 21 a2.2 2.2 0 1 0 4.4 0 a2.2 2.2 0 1 0 -4.4 0 Z", Some(rgb(0xff5a6e)), Some(INK), 1.2),
        Part("M28 19 Q30 17 32 19 M34 19 Q36 17 38 19", None, Some(EYE), 1.6),
        Part("M31 23 Q33 25 35 23", None, Some(EYE), 1.4),
    ]
}

/// The XP gem's outline, in a 40 × 40 box.
const GEM_D: &str = "M10 6 H30 L38 16 L20 37 L2 16 Z";

/// The XP gem with its face and its sparkle, in `c` (`LIME`, `RED`, `GOLD`).
pub(in crate::app) fn gem(c: [Color32; 4]) -> [Part; 12] {
    [
        Part(GEM_D, Some(c[0]), None, 0.0),
        Part("M10 6 H30 L26 16 H14 Z", Some(c[1]), None, 0.0),
        Part("M26 16 H38 L20 37 Z", Some(c[2]), None, 0.0),
        Part("M2 16 H14 L20 37 Z", Some(c[3]), None, 0.0),
        Part("M2 16 H38 M14 16 L10 6 M26 16 L30 6", None, Some(INK), 1.0),
        Part("M13 9.5 H18.5", None, Some(rgb(0xffffff)), 2.0),
        Part("M15.4 22 a1.7 1.7 0 1 0 3.4 0 a1.7 1.7 0 1 0 -3.4 0 Z M21.2 22 a1.7 1.7 0 1 0 3.4 0 a1.7 1.7 0 1 0 -3.4 0 Z", Some(EYE), None, 0.0),
        Part("M16.6 21.4 a0.5 0.5 0 1 0 1 0 a0.5 0.5 0 1 0 -1 0 Z M22.4 21.4 a0.5 0.5 0 1 0 1 0 a0.5 0.5 0 1 0 -1 0 Z", Some(rgb(0xffffff)), None, 0.0),
        Part("M11.6 25 a2 1.1 0 1 0 4 0 a2 1.1 0 1 0 -4 0 Z M24.4 25 a2 1.1 0 1 0 4 0 a2 1.1 0 1 0 -4 0 Z", Some(BLUSH), None, 0.0),
        Part("M18.4 25.4 Q20 27 21.6 25.4", None, Some(EYE), 1.2),
        Part(GEM_D, None, Some(INK), 2.2),
        Part("M36 0 Q36.6 3.4 40 4 Q36.6 4.6 36 8 Q35.4 4.6 32 4 Q35.4 3.4 36 0 Z", Some(rgb(0xffffff)), None, 0.0),
    ]
}

/// The gem without its face, below 20 points: the toolbar pill's.
pub(in crate::app) fn gem_small(c: [Color32; 4]) -> [Part; 4] {
    [Part(GEM_D, Some(c[0]), None, 0.0), Part("M10 6 H30 L26 16 H14 Z", Some(c[1]), None, 0.0), Part("M26 16 H38 L20 37 Z", Some(c[2]), None, 0.0), Part(GEM_D, None, Some(INK), 3.0)]
}

/// The gem, `h` points tall, centred on `c`: with its face from 20 points.
pub(in crate::app) fn gem_at(p: &Painter, c: Pos2, h: f32, colours: [Color32; 4]) {
    let k = h / 37.0;
    let o = c - vec2(20.0, 21.5) * k;
    if h < 20.0 {
        draw(p, &gem_small(colours), o, k, 0.0, 1.0);
    } else {
        draw(p, &gem(colours), o, k, 0.0, 1.0);
    }
}

/// A little bug, in a 30 × 28 box.
pub(in crate::app) const BUG: &[Part] = &[
    Part("M6 18 L1 21 M8 21 L4 26 M24 18 L29 21 M22 21 L26 26 M11 9 L8 3 M19 9 L22 3", None, Some(INK), 1.6),
    Part("M5 16 a10 8 0 1 0 20 0 a10 8 0 1 0 -20 0 Z", Some(rgb(0x7ee787)), Some(INK), 1.8),
    Part("M15 8.5 V23.5", None, Some(rgb(0x3fa35a)), 1.4),
    Part("M9.5 14 a1.6 1.6 0 1 0 3.2 0 a1.6 1.6 0 1 0 -3.2 0 Z M17.3 14 a1.6 1.6 0 1 0 3.2 0 a1.6 1.6 0 1 0 -3.2 0 Z", Some(EYE), None, 0.0),
];
/// The trophy with its face, in a 40 × 38 box; its first six parts are the bowl alone.
pub(in crate::app) const CUP: &[Part] = &[
    Part("M10 9 Q3 9 4 15 Q5 20 11 20 M30 9 Q37 9 36 15 Q35 20 29 20", None, Some(INK), 2.2),
    Part("M10 5 H30 V15 Q30 26 20 26 Q10 26 10 15 Z", Some(rgb(0xffc93c)), Some(INK), 2.0),
    Part("M25 7 V15 Q25 22 20 24 Q28 23 28 15 V7 Z", Some(rgb(0xe0a21a)), None, 0.0),
    Part("M14 9 Q14 15 16 19", None, Some(rgb(0xfff1c2)), 2.0),
    Part("M17 26 H23 V31 H17 Z", Some(rgb(0xe0a21a)), Some(INK), 1.6),
    Part("M11 31 H29 V37 H11 Z", Some(rgb(0x6d5bd0)), Some(INK), 1.8),
    Part("M15 14 Q17 12 19 14 M21 14 Q23 12 25 14", None, Some(EYE), 1.2),
    Part("M13 17 a1.8 1 0 1 0 3.6 0 a1.8 1 0 1 0 -3.6 0 Z M23.4 17 a1.8 1 0 1 0 3.6 0 a1.8 1 0 1 0 -3.6 0 Z", Some(BLUSH), None, 0.0),
    Part("M18.5 18 Q20 19.5 21.5 18", None, Some(EYE), 1.1),
];
/// The boss, horned, in a 60 × 54 box.
pub(in crate::app) const BOSS: &[Part] = &[
    Part("M14 14 L9 1 L22 10 Z M46 14 L51 1 L38 10 Z", Some(rgb(0xffd66b)), Some(INK), 1.8),
    Part("M6 52 Q2 10 30 8 Q58 10 54 52 Q48 46 42 52 Q36 46 30 52 Q24 46 18 52 Q12 46 6 52 Z", Some(rgb(0xff4d6d)), Some(INK), 2.2),
    Part("M44 14 Q56 24 54 50 Q50 47 46 50 Q50 30 40 16 Z", Some(rgb(0xd63a57)), None, 0.0),
    Part("M14 20 Q18 14 24 13", None, Some(rgb(0xffc2cc)), 2.4),
    Part("M17 22 L26 26 M43 22 L34 26", None, Some(INK), 2.6),
    Part("M19 29 a3 3 0 1 0 6 0 a3 3 0 1 0 -6 0 Z M35 29 a3 3 0 1 0 6 0 a3 3 0 1 0 -6 0 Z", Some(EYE), None, 0.0),
    Part("M22 38 Q30 32 38 38 Q30 42 22 38 Z", Some(rgb(0xffffff)), Some(INK), 1.6),
    Part("M12 34 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z M42 34 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z", Some(BLUSH), None, 0.0),
];

/// An energy drink can standing, in `c` with its `dark` side, in a 30 × 54 box.
pub(in crate::app) fn can(c: Color32, dark: Color32) -> [Part; 9] {
    const BODY_D: &str = "M5 8 Q5 4 15 4 Q25 4 25 8 V46 Q25 50 15 50 Q5 50 5 46 Z";
    [
        Part(BODY_D, Some(c), Some(INK), 2.4),
        Part("M20 6 Q25 6 25 10 V46 Q25 50 20 50 Z", Some(dark), None, 0.0),
        Part("M5 32 H25 V39 H5 Z", Some(rgb(0xffffff)), None, 0.0),
        Part("M9 14 V27", None, Some(rgba(0xffffff, 153)), 2.4),
        Part("M5 8 Q15 12 25 8 Q15 4 5 8 Z", Some(rgb(0xc9ccd4)), Some(INK), 1.6),
        Part(BODY_D, None, Some(INK), 2.4),
        Part("M9.2 20 a1.8 1.8 0 1 0 3.6 0 a1.8 1.8 0 1 0 -3.6 0 Z M17.2 20 a1.8 1.8 0 1 0 3.6 0 a1.8 1.8 0 1 0 -3.6 0 Z", Some(EYE), None, 0.0),
        Part("M5 23.5 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z M19 23.5 a3 1.6 0 1 0 6 0 a3 1.6 0 1 0 -6 0 Z", Some(BLUSH), None, 0.0),
        Part("M12 22.5 Q15 25.5 18 22.5", None, Some(EYE), 1.6),
    ]
}

/// A can lying on its side, a little dented, asleep, in a 50 × 26 box.
pub(in crate::app) fn can_down(c: Color32, dark: Color32) -> [Part; 6] {
    [
        Part("M6 6 Q2 6 2 15 Q2 24 6 24 H44 Q48 24 48 15 Q48 6 44 6 Z", Some(c), Some(INK), 2.4),
        Part("M6 19 H44 Q48 19 48 15 V20 Q47 24 44 24 H6 Q3 24 2 20 Z", Some(dark), None, 0.0),
        Part("M28 6 V24 M34 6 V24", None, Some(rgb(0xffffff)), 3.0),
        Part("M44 6 Q48 6 48 15 Q48 24 44 24 Q40 24 40 15 Q40 6 44 6 Z", Some(rgb(0xc9ccd4)), Some(INK), 1.6),
        Part("M10 12 Q12 10 14 12 M18 12 Q20 10 22 12", None, Some(EYE), 1.6),
        Part("M14.5 17 Q16 18 17.5 17", None, Some(EYE), 1.3),
    ]
}

/// The cans' colours and their dark sides: blue, lime, red.
pub(in crate::app) const CANS: [(Color32, Color32); 3] = [(rgb(0x3b82c4), rgb(0x2c64a0)), (rgb(0xc6f432), rgb(0x9fcf1a)), (rgb(0xff4d6d), rgb(0xd63a57))];

/// A flat pizza box seen from the front, in a 60 × 16 box.
pub(in crate::app) const BOX: &[Part] = &[
    Part("M2 4 H58 V14 H2 Z", Some(rgb(0xefe4cc)), Some(INK), 2.0),
    Part("M2 9 H58", None, Some(rgb(0xb9ab8f)), 1.4),
    Part("M22 6 H38", None, Some(rgb(0xe84a3d)), 2.4),
];
/// A lime spill, in a 52 × 18 box.
pub(in crate::app) const SPILL: &[Part] = &[
    Part("M4 10 Q2 2 16 3 Q26 0 36 4 Q50 3 48 11 Q46 17 30 15 Q18 18 8 16 Q3 15 4 10 Z", Some(rgba(0xc6f432, 153)), Some(rgb(0x7aa21a)), 1.4),
    Part("M14 6 Q20 4 26 6", None, Some(rgb(0xf4ffd0)), 2.0),
];
/// A fly, in a 16 × 12 box.
pub(in crate::app) const FLY: &[Part] = &[
    Part("M2 4 a3 2 0 1 0 6 0 a3 2 0 1 0 -6 0 Z M8 4 a3 2 0 1 0 6 0 a3 2 0 1 0 -6 0 Z", Some(rgba(0xdcebff, 217)), None, 0.0),
    Part("M5 8 a3 3 0 1 0 6 0 a3 3 0 1 0 -6 0 Z", Some(BLACK), None, 0.0),
];
/// The bin bag, in a 56 × 64 box.
pub(in crate::app) const BINBAG: &[Part] = &[
    Part("M10 16 Q2 30 6 50 Q10 62 28 62 Q46 62 50 50 Q54 30 46 16 Z", Some(BLACK), Some(INK), 2.4),
    Part("M14 18 Q10 32 13 46", None, Some(rgb(0x3a3d45)), 3.0),
    Part("M20 16 Q28 4 36 16 M24 10 L20 2 M32 10 L36 2", None, Some(BLACK), 3.4),
    Part("M20 36 Q22 34 24 36 M32 36 Q34 34 36 36", None, Some(rgb(0x8a8e98)), 1.6),
    Part("M25 42 Q28 44 31 42", None, Some(rgb(0x8a8e98)), 1.4),
];
/// A lime sparkle round its centre.
pub(in crate::app) const SPARK: &[Part] = &[Part("M0 -8 Q1 -1 8 0 Q1 1 0 8 Q-1 1 -8 0 Q-1 -1 0 -8 Z", Some(rgb(0xeaff9e)), None, 0.0)];

/// The game boxes' fronts, in a 100 × 100 box: the frame of a wishlisted one, the heart on it.
pub(in crate::app) const FRAME_D: &str = "M2 2 H98 V98 H2 Z";
pub(in crate::app) const HEART_D: &str = "M50 66 C34 54 38 40 50 48 C62 40 66 54 50 66 Z";
/// The band at the top where the number goes.
pub(in crate::app) const BAND_D: &str = "M0 0 H100 V26 H0 Z";
/// The emblems: a castle for a feature, a beetle for a bug, a star for the rest.
pub(in crate::app) const CASTLE_D: &str = "M28 86 V52 H35 V43 H42 V52 H58 V43 H65 V52 H72 V86 Z M45 86 V72 Q50 64 55 72 V86 Z";
pub(in crate::app) const BEETLE_D: &str = "M34 60 a16 14 0 1 0 32 0 a16 14 0 1 0 -32 0 Z M50 46 V74 M34 56 L24 50 M66 56 L76 50 M34 66 L24 72 M66 66 L76 72";
pub(in crate::app) const STAR_D: &str = "M50 36 L56 50 L71 51 L59 60 L63 75 L50 66 L37 75 L41 60 L29 51 L44 50 Z";
/// The shrink wrap's shine on a new one.
pub(in crate::app) const SHINE_D: &str = "M8 100 L44 0 H58 L22 100 Z M62 100 L82 44 H88 L68 100 Z";
/// Being played: the lime glow round it, the loading bar's track.
pub(in crate::app) const GLOW_D: &str = "M1 1 H99 V99 H1 Z";
pub(in crate::app) const LOADING_D: &str = "M12 78 H88 V86 H12 Z";
/// Paused: the badge and its two bars.
pub(in crate::app) const PAUSE_D: &str = "M58 52 a16 16 0 1 0 32 0 a16 16 0 1 0 -32 0 Z";
pub(in crate::app) const PAUSE_BARS_D: &str = "M68 44 V60 M80 44 V60";
/// In beta: the sticker's band.
pub(in crate::app) const BETA_D: &str = "M0 68 H100 V90 H0 Z";

#[cfg(test)]
mod tests {
    use crate::app::mess::path;
    use super::*;

    fn every() -> Vec<&'static str> {
        let mut all: Vec<&'static str> = [CHAIR_BASE, ARMS_DOWN, ARMS_UP, PALM, LIFT, ARMS_LEAN, BODY, CHAIR_BACK, SEAM, SWEAT, BUG, CUP, BOSS, BOX, SPILL, FLY, BINBAG, SPARK]
            .iter()
            .flat_map(|parts| parts.iter().map(|Part(d, ..)| *d))
            .collect();
        all.extend(pad(LIME[0]).iter().map(|Part(d, ..)| *d));
        all.extend(gem(LIME).iter().map(|Part(d, ..)| *d));
        all.extend(can(LIME[0], LIME[2]).iter().map(|Part(d, ..)| *d));
        all.extend(can_down(LIME[0], LIME[2]).iter().map(|Part(d, ..)| *d));
        all.extend([LEDS_D, DIAMOND_D, FRAME_D, HEART_D, BAND_D, CASTLE_D, BEETLE_D, STAR_D, SHINE_D, GLOW_D, LOADING_D, PAUSE_D, PAUSE_BARS_D, BETA_D]);
        all
    }

    #[test]
    fn every_drawing_parses_into_shapes() {
        for d in every() {
            let subs = path(d);
            assert!(!subs.is_empty() && subs.iter().all(|s| s.len() >= 2), "{d}");
        }
    }

    #[test]
    fn the_drawings_stay_in_their_boxes() {
        let inside = |parts: &[Part], w: f32, h: f32| {
            parts.iter().flat_map(|Part(d, ..)| path(d)).flatten().all(|v| (-1.0..=w + 1.0).contains(&v.x) && (-1.0..=h + 1.0).contains(&v.y))
        };
        for parts in [CHAIR_BASE, ARMS_DOWN, ARMS_UP, PALM, LIFT, ARMS_LEAN, BODY, CHAIR_BACK, SEAM, SWEAT] {
            assert!(inside(parts, 120.0, 172.0));
        }
        assert!(inside(&pad(LIME[0]), 64.0, 42.0));
        assert!(inside(&gem(LIME), 40.0, 40.0));
        assert!(inside(BUG, 30.0, 28.0));
        assert!(inside(CUP, 40.0, 38.0));
        assert!(inside(BOSS, 60.0, 54.0));
        assert!(inside(&can(LIME[0], LIME[2]), 30.0, 54.0));
        assert!(inside(&can_down(LIME[0], LIME[2]), 50.0, 26.0));
        assert!(inside(BOX, 60.0, 16.0));
        assert!(inside(SPILL, 52.0, 18.0));
        assert!(inside(FLY, 16.0, 12.0));
        assert!(inside(BINBAG, 56.0, 64.0));
        let front: Vec<Part> = [FRAME_D, HEART_D, BAND_D, CASTLE_D, BEETLE_D, STAR_D, SHINE_D, GLOW_D, LOADING_D, PAUSE_D, BETA_D].iter().map(|d| Part(d, None, None, 0.0)).collect();
        assert!(inside(&front, 100.0, 100.0));
    }
}
