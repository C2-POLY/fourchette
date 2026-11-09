//! The messy gamer skin (#202): kawaii chips, crisp bags, cans, spills and pizza slices fall
//! where the pointer lingers and pile up on their own as time goes by, up to `MAX`. One blocks a
//! click on what it covers. The robot vacuum parked in the bottom-right corner is picked up with
//! a click: it then follows the pointer and sucks up the mess it passes over, which gasps as it
//! goes; a click or Esc puts it back. Nothing is kept: Normal or a new launch starts clean.
//!
//! The drawings come from the design canvas, as paths in a 120 × 96 box (`Pen`).

use egui::{pos2, vec2, Align2, Color32, CursorIcon, FontId, Id, LayerId, Order, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// The most litter on screen at once.
pub(super) const MAX: usize = 40;
/// Seconds the pointer lingers before something falls there.
const HOVER_SECS: f32 = 2.5;
/// Seconds between two things falling anywhere on their own.
const AMBIENT_SECS: f32 = 20.0;
/// How far round its nozzle the vacuum sucks, in points.
const NOZZLE: f32 = 28.0;
/// How long a piece of litter takes to disappear into the vacuum.
const SUCK_SECS: f32 = 0.35;
/// How long litter shakes when a click hits it.
const SHAKE_SECS: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Kind {
    Chips,
    Bag,
    Can,
    Spill,
    Pizza,
}

impl Kind {
    /// How far from its middle a click hits it.
    fn reach(self) -> f32 {
        match self {
            Kind::Chips => 22.0,
            Kind::Bag => 18.0,
            Kind::Can => 22.0,
            Kind::Spill => 24.0,
            Kind::Pizza => 20.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Litter {
    pub at: Pos2,
    pub kind: Kind,
    /// Its angle, in radians.
    pub turn: f32,
    /// Which of its kind's colours.
    pub tint: usize,
    /// When the vacuum caught it.
    pub sucked: Option<f32>,
    /// When a click last hit it.
    pub shaken: Option<f32>,
}

#[derive(Debug)]
pub(super) struct Mess {
    pub litter: Vec<Litter>,
    /// The vacuum follows the pointer.
    pub carried: bool,
    /// How much it has sucked up since the skin was put on.
    pub cleaned: u32,
    /// Where the screenshot mode puts the pointer.
    pub pin: Option<Pos2>,
    /// The screenshot mode fills the screen on the first frame.
    pub sample: bool,
    hover: f32,
    ambient: f32,
    seed: u32,
    /// A press taken from the interface, whose release goes too.
    swallow: bool,
}

impl Default for Mess {
    fn default() -> Self {
        Mess { litter: Vec::new(), carried: false, cleaned: 0, pin: None, sample: false, hover: 0.0, ambient: 0.0, seed: 0x2545_f491, swallow: false }
    }
}

/// Where the vacuum is parked: the bottom-right corner, above the status bar.
pub(super) fn parking(screen: Rect) -> Rect {
    Rect::from_min_size(screen.right_bottom() - vec2(80.0, 110.0), vec2(64.0, 64.0))
}

impl Mess {
    /// A number from 0 to 1 (xorshift).
    fn rand(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Back to clean, the vacuum parked.
    pub fn clear(&mut self) {
        *self = Mess { seed: self.seed, ..Mess::default() };
    }

    /// The litter still on screen.
    pub fn live(&self) -> usize {
        self.litter.iter().filter(|l| l.sucked.is_none()).count()
    }

    /// Drops a random piece of litter near `at`, unless the screen is full.
    pub fn drop_at(&mut self, at: Pos2) {
        if self.live() >= MAX {
            return;
        }
        let kind = [Kind::Chips, Kind::Bag, Kind::Can, Kind::Spill, Kind::Pizza][(self.rand() * 5.0) as usize % 5];
        let jitter = vec2(self.rand() - 0.5, self.rand() - 0.5) * 24.0;
        // Tilted, never upside down: they have faces.
        let turn = (self.rand() - 0.5) * 0.6;
        let tint = (self.rand() * 4.0) as usize;
        self.litter.push(Litter { at: at + jitter, kind, turn, tint, sucked: None, shaken: None });
    }

    /// The litter a click at `at` hits, the one on top first.
    pub fn hit(&self, at: Pos2) -> Option<usize> {
        self.litter.iter().rposition(|l| l.sucked.is_none() && l.at.distance(at) <= l.kind.reach())
    }

    /// One frame, `dt` seconds after the last, the pointer at `pointer` (None outside the
    /// window): litter falls where it lingers and on its own, the carried vacuum sucks.
    pub fn step(&mut self, dt: f32, now: f32, pointer: Option<Pos2>, screen: Rect) {
        if self.sample {
            self.sample = false;
            for k in 0..22 {
                let at = pos2(screen.left() + 120.0 + (k * 211 % 1100) as f32, screen.top() + 110.0 + (k * 137 % 600) as f32);
                self.drop_at(at);
            }
        }
        if self.carried {
            if let Some(p) = pointer {
                for l in self.litter.iter_mut().filter(|l| l.sucked.is_none() && l.at.distance(p) <= NOZZLE) {
                    l.sucked = Some(now);
                    self.cleaned += 1;
                }
            }
        } else if let Some(p) = pointer.filter(|p| !parking(screen).contains(*p)) {
            self.hover += dt;
            if self.hover >= HOVER_SECS {
                self.hover = 0.0;
                self.drop_at(p);
            }
        }
        self.ambient += dt;
        if self.ambient >= AMBIENT_SECS {
            self.ambient = 0.0;
            let at = pos2(screen.left() + 40.0 + self.rand() * (screen.width() - 80.0), screen.top() + 60.0 + self.rand() * (screen.height() - 120.0));
            self.drop_at(at);
        }
        self.litter.retain(|l| l.sucked.is_none_or(|t| now - t < SUCK_SECS));
    }

    /// A press of the main button at `at`: the vacuum taken or put back, or litter hit. True
    /// when the press is taken from the interface underneath.
    pub fn press(&mut self, at: Pos2, now: f32, screen: Rect) -> bool {
        let taken = if self.carried {
            self.carried = false;
            true
        } else if parking(screen).contains(at) {
            self.carried = true;
            true
        } else if let Some(i) = self.hit(at) {
            self.litter[i].shaken = Some(now);
            true
        } else {
            false
        };
        self.swallow = taken;
        taken
    }

    /// Takes from `events` the clicks the mess catches, and Esc while the vacuum is carried.
    pub fn filter(&mut self, events: &mut Vec<egui::Event>, now: f32, screen: Rect) {
        events.retain(|e| match e {
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } => !self.press(*pos, now, screen),
            egui::Event::PointerButton { button: egui::PointerButton::Primary, pressed: false, .. } if self.swallow => {
                self.swallow = false;
                false
            }
            egui::Event::Key { key: egui::Key::Escape, pressed, .. } if self.carried => {
                if *pressed {
                    self.carried = false;
                }
                false
            }
            _ => true,
        });
    }

    /// Draws the litter over the whole window, then the vacuum, parked or under the pointer.
    pub fn paint(&self, ctx: &egui::Context, now: f32, pointer: Option<Pos2>, screen: Rect) {
        let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("mess")));
        let nozzle = if self.carried { pointer } else { None };
        for l in &self.litter {
            let mut at = l.at;
            let mut scale = 1.0;
            if let (Some(t), Some(n)) = (l.sucked, nozzle) {
                let k = ((now - t) / SUCK_SECS).clamp(0.0, 1.0);
                at = at.lerp(n, k);
                scale = 1.0 - k;
            }
            if let Some(t) = l.shaken.filter(|t| now - t < SHAKE_SECS) {
                at.x += ((now - t) * 60.0).sin() * 3.0;
            }
            litter(&p, at, l, scale);
        }
        let top = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("vacuum")));
        match nozzle {
            Some(n) => {
                top.circle_stroke(n, NOZZLE, Stroke::new(1.0, Color32::from_rgba_unmultiplied(160, 120, 255, 90)));
                vacuum(&top, n, now, true);
                ctx.set_cursor_icon(CursorIcon::None);
            }
            None => {
                let r = parking(screen);
                vacuum(&top, r.center() + vec2(0.0, 6.0), now, false);
            }
        }
        if self.cleaned > 0 || self.carried {
            let at = parking(screen).center_top() - vec2(0.0, 8.0);
            let g = top.layout_no_wrap(format!("{} cleaned", self.cleaned), FontId::proportional(11.0), Color32::WHITE);
            let pill = Rect::from_center_size(at, g.size() + vec2(12.0, 4.0));
            top.rect_filled(pill, 8.0, Color32::from_rgb(0x7c, 0x4d, 0xff));
            top.galley(pill.center() - g.size() / 2.0, g, Color32::WHITE);
        }
    }
}

/// A point `v` from `c`, turned by `turn` radians and scaled by `s`.
fn turned(c: Pos2, v: Vec2, turn: f32, s: f32) -> Pos2 {
    let (sin, cos) = turn.sin_cos();
    c + vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos) * s
}

/// Points per design unit: litter about 50 points wide, the robot about 53.
const LITTER_SCALE: f32 = 0.42;
const ROBOT_SCALE: f32 = 0.6;

const INK: Color32 = Color32::from_rgb(0x3b, 0x2f, 0x4a);
const EYE: Color32 = Color32::from_rgb(0x2a, 0x24, 0x33);
const BLUSH: Color32 = Color32::from_rgb(0xff, 0x8f, 0xb1);
const TEAR: Color32 = Color32::from_rgb(0x9f, 0xd8, 0xff);
const SPARK: Color32 = Color32::from_rgb(0xff, 0xf6, 0xc2);
/// A can's body and its shadow: sky, pink, mint, lilac.
const CANS: [(Color32, Color32); 4] = [
    (Color32::from_rgb(0x8f, 0xd3, 0xff), Color32::from_rgb(0x6b, 0xbc, 0xf0)),
    (Color32::from_rgb(0xff, 0x9a, 0xc1), Color32::from_rgb(0xf0, 0x78, 0xa4)),
    (Color32::from_rgb(0x9b, 0xe6, 0xb4), Color32::from_rgb(0x6f, 0xcf, 0x93)),
    (Color32::from_rgb(0xc3, 0xa6, 0xff), Color32::from_rgb(0xa0, 0x82, 0xf0)),
];
/// A spill and its shadow: mint, strawberry, blue, grape.
const SPILLS: [(Color32, Color32); 4] = [
    (Color32::from_rgb(0x9b, 0xe6, 0xb4), Color32::from_rgb(0x6f, 0xcf, 0x93)),
    (Color32::from_rgb(0xff, 0xb0, 0xcb), Color32::from_rgb(0xf7, 0x8f, 0xb0)),
    (Color32::from_rgb(0xa9, 0xdc, 0xff), Color32::from_rgb(0x7c, 0xc2, 0xf2)),
    (Color32::from_rgb(0xd9, 0xc8, 0xff), Color32::from_rgb(0xb3, 0x9c, 0xf0)),
];

/// The parts of an SVG path's `d` this skin uses: absolute M, L, H, V, Q, C and Z, curves
/// flattened. One list of points per subpath.
fn path(d: &str) -> Vec<Vec<Vec2>> {
    enum Tok {
        Cmd(char),
        Num(f32),
    }
    let mut toks = Vec::new();
    let mut num = String::new();
    let flush = |num: &mut String, toks: &mut Vec<Tok>| {
        if !num.is_empty() {
            toks.push(Tok::Num(num.parse().unwrap_or(0.0)));
            num.clear();
        }
    };
    for ch in d.chars() {
        match ch {
            '0'..='9' | '.' => num.push(ch),
            '-' => {
                flush(&mut num, &mut toks);
                num.push(ch);
            }
            c if c.is_ascii_alphabetic() => {
                flush(&mut num, &mut toks);
                toks.push(Tok::Cmd(c.to_ascii_uppercase()));
            }
            _ => flush(&mut num, &mut toks),
        }
    }
    flush(&mut num, &mut toks);

    let mut out: Vec<Vec<Vec2>> = Vec::new();
    let mut at = Vec2::ZERO;
    let mut cmd = 'M';
    let mut i = 0;
    while i < toks.len() {
        if let Tok::Cmd(c) = toks[i] {
            cmd = c;
            i += 1;
            if c == 'Z' {
                if let Some(first) = out.last().and_then(|s| s.first()) {
                    at = *first;
                }
            }
            continue;
        }
        let arity = match cmd {
            'H' | 'V' => 1,
            'M' | 'L' => 2,
            'Q' => 4,
            'C' => 6,
            _ => {
                i += 1;
                continue;
            }
        };
        let v: Vec<f32> = toks[i..].iter().take(arity).map_while(|t| if let Tok::Num(n) = t { Some(*n) } else { None }).collect();
        if v.len() < arity {
            break;
        }
        i += arity;
        let p = |k: usize| vec2(v[k], v[k + 1]);
        if cmd == 'M' {
            at = p(0);
            out.push(vec![at]);
            cmd = 'L';
            continue;
        }
        if out.is_empty() {
            out.push(vec![at]);
        }
        let seg = out.last_mut().expect("a subpath");
        match cmd {
            'L' => at = p(0),
            'H' => at = vec2(v[0], at.y),
            'V' => at = vec2(at.x, v[0]),
            'Q' => {
                for k in 1..8 {
                    let t = k as f32 / 8.0;
                    let u = 1.0 - t;
                    seg.push(at * (u * u) + p(0) * (2.0 * u * t) + p(2) * (t * t));
                }
                at = p(2);
            }
            _ => {
                for k in 1..10 {
                    let t = k as f32 / 10.0;
                    let u = 1.0 - t;
                    seg.push(at * (u * u * u) + p(0) * (3.0 * u * u * t) + p(2) * (3.0 * u * t * t) + p(4) * (t * t * t));
                }
                at = p(4);
            }
        }
        seg.push(at);
    }
    out
}

/// The triangles that fill a simple polygon, convex or not (ear clipping).
fn triangulate(pts: &[Pos2]) -> Vec<[usize; 3]> {
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b - a).x * (c - b).y - (b - a).y * (c - b).x;
    let area: f32 = (0..pts.len()).map(|k| {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        a.x * b.y - b.x * a.y
    }).sum();
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let inside = |p: Pos2, a: Pos2, b: Pos2, c: Pos2| cross(a, b, p) > 0.0 && cross(b, c, p) > 0.0 && cross(c, a, p) > 0.0;
    let mut tris = Vec::new();
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&k| {
            let (a, b, c) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            cross(pts[a], pts[b], pts[c]) >= 0.0 && !idx.iter().any(|&j| j != a && j != b && j != c && inside(pts[j], pts[a], pts[b], pts[c]))
        });
        let Some(k) = ear else { break };
        tris.push([idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]]);
        idx.remove(k);
    }
    if idx.len() == 3 {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    tris
}

/// Draws the canvas's paths round a centre: its point (60, 48) lands on `c`.
#[derive(Clone, Copy)]
struct Pen<'a> {
    p: &'a Painter,
    c: Pos2,
    turn: f32,
    s: f32,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        turned(self.c, vec2(x - 60.0, y - 48.0), self.turn, self.s)
    }

    /// The same pen turned `by` radians more.
    fn turn(self, by: f32) -> Self {
        Pen { turn: self.turn + by, ..self }
    }

    fn subpaths(&self, d: &str) -> Vec<Vec<Pos2>> {
        path(d).into_iter().map(|s| {
            let mut pts: Vec<Pos2> = s.into_iter().map(|v| self.at(v.x, v.y)).collect();
            pts.dedup_by(|a, b| a.distance(*b) < 0.01);
            if pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) < 0.01 {
                pts.pop();
            }
            pts
        }).collect()
    }

    /// A closed path filled, outlined in ink `w` units wide when `w` is above zero.
    fn fill(&self, d: &str, fill: Color32, w: f32) {
        for pts in self.subpaths(d) {
            if pts.len() < 3 {
                continue;
            }
            let mut mesh = Mesh::default();
            for q in &pts {
                mesh.colored_vertex(*q, fill);
            }
            for [a, b, c] in triangulate(&pts) {
                mesh.add_triangle(a as u32, b as u32, c as u32);
            }
            self.p.add(Shape::mesh(mesh));
            if w > 0.0 {
                self.p.add(Shape::closed_line(pts, Stroke::new(w * self.s, INK)));
            }
        }
    }

    /// A closed path's ink outline alone.
    fn outline(&self, d: &str, w: f32) {
        for pts in self.subpaths(d) {
            self.p.add(Shape::closed_line(pts, Stroke::new(w * self.s, INK)));
        }
    }

    /// An open path stroked.
    fn line(&self, d: &str, w: f32, colour: Color32) {
        for pts in self.subpaths(d) {
            self.p.add(Shape::line(pts, Stroke::new(w * self.s, colour)));
        }
    }

    fn ellipse(&self, cx: f32, cy: f32, rx: f32, ry: f32, fill: Color32, w: f32) {
        let pts = (0..24).map(|k| {
            let a = k as f32 / 24.0 * std::f32::consts::TAU;
            self.at(cx + rx * a.cos(), cy + ry * a.sin())
        }).collect();
        let edge = if w > 0.0 { Stroke::new(w * self.s, INK) } else { Stroke::NONE };
        self.p.add(Shape::convex_polygon(pts, fill, edge));
    }

    /// A round eye with its glint.
    fn eye(&self, x: f32, y: f32, r: f32) {
        self.ellipse(x, y, r, r, EYE, 0.0);
        self.ellipse(x - r * 0.33, y - r * 0.38, r * 0.36, r * 0.36, Color32::WHITE, 0.0);
    }

    fn blush(&self, x: f32, y: f32, rx: f32, ry: f32) {
        self.ellipse(x, y, rx, ry, BLUSH.gamma_multiply(0.8), 0.0);
    }

    /// The mouth drawn by `d`, or a round "O" at (x, y) while the vacuum takes it.
    fn mouth(&self, d: &str, x: f32, y: f32, gasp: bool) {
        if gasp {
            self.ellipse(x, y, 2.6, 3.2, EYE, 0.0);
        } else {
            self.line(d, 2.4, EYE);
        }
    }

    /// A four-pointed sparkle of radius `r`.
    fn sparkle(&self, x: f32, y: f32, r: f32, colour: Color32) {
        let d = format!("M{x} {} Q{x} {y} {} {y} Q{x} {y} {x} {} Q{x} {y} {} {y} Q{x} {y} {x} {}Z", y - r, x + r, y + r, x - r, y - r);
        self.fill(&d, colour, 0.0);
    }
}

fn litter(p: &Painter, c: Pos2, l: &Litter, s: f32) {
    if s <= 0.02 {
        return;
    }
    let pen = Pen { p, c, turn: l.turn, s: s * LITTER_SCALE };
    let gasp = l.sucked.is_some();
    match l.kind {
        Kind::Chips => {
            let main = "M18 60 Q20 32 48 30 Q76 30 78 56 Q76 80 48 82 Q20 82 18 60Z";
            pen.fill(main, Color32::from_rgb(0xff, 0xe0, 0x8a), 0.0);
            pen.fill("M78 56 Q76 80 48 82 Q28 82 21 71 Q42 77 60 69 Q73 63 78 56Z", Color32::from_rgb(0xff, 0xc9, 0x5c), 0.0);
            pen.line("M27 46 Q32 37 42 36", 4.0, Color32::WHITE);
            pen.outline(main, 3.2);
            pen.eye(39.0, 55.0, 4.2);
            pen.eye(57.0, 55.0, 4.2);
            pen.blush(31.0, 63.0, 4.5, 2.6);
            pen.blush(65.0, 63.0, 4.5, 2.6);
            pen.mouth("M44 62 Q48 67 52 62", 48.0, 64.0, gasp);
            // Its little friend, worried.
            pen.fill("M84 68 Q86 54 98 54 Q110 56 107 69 Q103 80 94 80 Q84 78 84 68Z", Color32::from_rgb(0xff, 0xe0, 0x8a), 3.0);
            pen.ellipse(91.0, 65.0, 2.8, 2.8, EYE, 0.0);
            pen.ellipse(100.0, 65.0, 2.8, 2.8, EYE, 0.0);
            pen.ellipse(95.5, 72.0, 1.8, 1.8, EYE, 0.0);
            pen.fill("M108 46 Q112 52 108 55 Q104 52 108 46Z", TEAR, 1.8);
            pen.ellipse(14.0, 86.0, 2.6, 2.6, Color32::from_rgb(0xff, 0xc9, 0x5c), 1.8);
            pen.sparkle(86.0, 30.0, 8.0, SPARK);
        }
        Kind::Bag => {
            let tilted = pen.turn(-0.14);
            let body = "M32 24 L37 17 L42 24 L47 17 L52 24 L57 17 L62 24 L67 17 L72 24 L77 17 L82 24 Q90 54 85 84 L35 84 Q28 54 32 24Z";
            tilted.fill(body, Color32::from_rgb(0xff, 0x9a, 0xa2), 0.0);
            tilted.fill("M72 30 L83 30 Q90 54 85 84 L72 84 Q78 54 72 30Z", Color32::from_rgb(0xf0, 0x7a, 0x86), 0.0);
            tilted.fill("M32 24 L82 24 L83 31 L31 31Z", Color32::from_rgb(0xe8, 0x68, 0x7a), 0.0);
            tilted.line("M38 38 Q35 56 38 72", 4.0, Color32::WHITE.gamma_multiply(0.6));
            tilted.outline(body, 3.2);
            // Sad, its eyes shut.
            tilted.line("M44 52 Q48 56 52 52", 2.6, EYE);
            tilted.line("M62 52 Q66 56 70 52", 2.6, EYE);
            tilted.blush(42.0, 61.0, 4.5, 2.6);
            tilted.blush(73.0, 61.0, 4.5, 2.6);
            tilted.mouth("M51 64 Q54 61 57 64 Q60 67 63 64", 57.0, 64.0, gasp);
            tilted.fill("M44 56 Q42 62 45 64 Q48 62 44 56Z", TEAR, 1.5);
            let lilac = Color32::from_rgb(0xc3, 0xa6, 0xff);
            p.text(pen.at(96.0, 16.0), Align2::CENTER_CENTER, "z", FontId::proportional(13.0 * pen.s), lilac);
            p.text(pen.at(103.0, 8.0), Align2::CENTER_CENTER, "z", FontId::proportional(10.0 * pen.s), lilac);
        }
        Kind::Can => {
            let (body, shadow) = CANS[l.tint % CANS.len()];
            pen.fill("M94 74 Q101 66 110 72 Q115 80 105 82 Q94 83 94 74Z", Color32::from_rgb(0xff, 0xb0, 0xcb), 2.6);
            let tilted = pen.turn(-0.17);
            let shape = "M28 32 H84 Q94 32 94 42 V58 Q94 68 84 68 H28 Q18 68 18 58 V42 Q18 32 28 32Z";
            tilted.fill(shape, body, 0.0);
            tilted.fill("M18 54 H94 V58 Q94 68 84 68 H28 Q18 68 18 58Z", shadow, 0.0);
            tilted.line("M26 38 H52", 3.5, Color32::WHITE.gamma_multiply(0.8));
            tilted.outline(shape, 3.2);
            tilted.ellipse(94.0, 50.0, 9.0, 18.0, Color32::from_rgb(0xee, 0xf0, 0xf6), 3.2);
            tilted.ellipse(95.0, 43.0, 2.6, 4.4, INK, 0.0);
            // Knocked out: its eyes squeezed shut.
            tilted.line("M40 44 L46 48 L40 52", 2.8, EYE);
            tilted.line("M66 44 L60 48 L66 52", 2.8, EYE);
            tilted.blush(36.0, 57.0, 4.5, 2.6);
            tilted.blush(70.0, 57.0, 4.5, 2.6);
            if gasp {
                tilted.ellipse(53.0, 58.0, 2.6, 3.2, EYE, 0.0);
            } else {
                tilted.fill("M48 56 Q53 62 58 56Z", EYE, 0.0);
            }
            pen.sparkle(30.0, 20.0, 6.0, Color32::from_rgb(0xff, 0xd6, 0x6b));
            pen.sparkle(46.0, 12.0, 4.0, SPARK);
            pen.sparkle(62.0, 18.0, 4.0, Color32::from_rgb(0xff, 0xd6, 0x6b));
        }
        Kind::Spill => {
            let (fill, shadow) = SPILLS[l.tint % SPILLS.len()];
            let main = "M22 58 C14 36, 44 26, 58 36 C70 22, 104 30, 98 52 C112 66, 86 84, 66 74 C46 88, 22 80, 22 58Z";
            pen.fill(main, fill, 0.0);
            pen.fill("M98 52 C112 66, 86 84, 66 74 C46 88, 22 80, 22 58 C28 72, 48 78, 66 68 C84 76, 102 64, 98 52Z", shadow, 0.0);
            pen.line("M32 48 Q38 40 48 41", 4.0, Color32::WHITE);
            pen.outline(main, 3.2);
            // Delighted, its eyes curved up.
            pen.line("M46 54 Q50 48 54 54", 2.8, EYE);
            pen.line("M66 54 Q70 48 74 54", 2.8, EYE);
            pen.blush(42.0, 62.0, 5.0, 2.8);
            pen.blush(78.0, 62.0, 5.0, 2.8);
            if gasp {
                pen.ellipse(60.0, 63.0, 3.0, 3.6, EYE, 0.0);
            } else {
                pen.fill("M54 60 Q60 68 66 60Z", EYE, 0.0);
            }
            pen.ellipse(108.0, 78.0, 4.0, 4.0, fill, 2.4);
            pen.ellipse(12.0, 42.0, 3.0, 3.0, fill, 2.0);
            pen.sparkle(98.0, 22.0, 8.0, SPARK);
            pen.sparkle(24.0, 17.0, 5.0, Color32::from_rgb(0x7f, 0xd8, 0xff));
        }
        Kind::Pizza => {
            let slice = "M24 28 Q60 18 98 30 L60 90Z";
            pen.fill(slice, Color32::from_rgb(0xff, 0xe0, 0x8a), 0.0);
            pen.fill("M80 26 Q90 28 98 30 L60 90 Q74 56 80 26Z", Color32::from_rgb(0xff, 0xcf, 0x66), 0.0);
            pen.outline(slice, 3.2);
            pen.fill("M19 28 Q60 11 103 29 Q105 38 97 38 Q60 23 25 36 Q15 36 19 28Z", Color32::from_rgb(0xf2, 0xb2, 0x6b), 3.2);
            pen.line("M28 25 Q60 15 94 26", 3.0, Color32::from_rgb(0xff, 0xd9, 0xa8));
            // Heart-shaped pepperoni.
            let heart = Color32::from_rgb(0xff, 0x6b, 0x81);
            pen.fill("M40 50 C31 43, 36 35, 40 40 C44 35, 49 43, 40 50Z", heart, 2.2);
            pen.fill("M84 46 C77 40, 81 34, 84 38 C87 34, 91 40, 84 46Z", heart, 2.2);
            pen.fill("M60 80 C55 75, 57 71, 60 74 C63 71, 65 75, 60 80Z", heart, 2.0);
            pen.eye(54.0, 52.0, 3.8);
            pen.eye(68.0, 52.0, 3.8);
            pen.blush(49.0, 59.0, 3.8, 2.2);
            pen.blush(73.0, 59.0, 3.8, 2.2);
            pen.mouth("M57 59 Q61 63 65 59", 61.0, 61.0, gasp);
        }
    }
}

/// The robot vacuum, its middle at `n`: a lilac disc on an RGB ring, a face on a screen and an
/// antenna. Its ring cycles faster and it smiles with its eyes while it runs.
fn vacuum(p: &Painter, n: Pos2, now: f32, on: bool) {
    let pen = Pen { p, c: n - vec2(0.0, 12.0 * ROBOT_SCALE), turn: 0.0, s: ROBOT_SCALE };
    let arc = |cx: f32, cy: f32, rx: f32, ry: f32, from: f32, to: f32, k: usize| -> Vec<Pos2> {
        (0..=k).map(|i| {
            let a = from + (to - from) * i as f32 / k as f32;
            pen.at(cx + rx * a.cos(), cy + ry * a.sin())
        }).collect()
    };
    pen.ellipse(60.0, 88.0, 40.0, 5.0, Color32::from_black_alpha(60), 0.0);
    pen.line("M20 74 L10 78 M20 74 L12 70 M20 74 L16 82", 2.4, INK);
    pen.ellipse(60.0, 64.0, 44.0, 22.0, Color32::from_rgb(0xa9, 0x8b, 0xf0), 3.2);
    // The RGB ring along its front.
    let ring = arc(60.0, 66.0, 44.0, 22.0, 0.15, std::f32::consts::PI - 0.15, 24);
    let speed = if on { 0.5 } else { 0.08 };
    for (k, w) in ring.windows(2).enumerate() {
        let hue = (k as f32 / 24.0 + now * speed).fract();
        let c: Color32 = egui::ecolor::Hsva::new(hue, 0.55, 1.0, 1.0).into();
        p.line_segment([w[0], w[1]], Stroke::new(4.5 * pen.s, c));
    }
    pen.ellipse(60.0, 56.0, 44.0, 22.0, Color32::from_rgb(0xd9, 0xc8, 0xff), 3.2);
    p.add(Shape::line(arc(60.0, 56.0, 36.0, 16.0, 3.5, 4.5, 10), Stroke::new(3.5 * pen.s, Color32::WHITE.gamma_multiply(0.8))));
    pen.line("M60 36 L60 22", 3.0, INK);
    pen.ellipse(60.0, 19.0, 5.0, 5.0, BLUSH, 2.6);
    pen.fill("M50 44 H70 Q80 44 80 54 Q80 64 70 64 H50 Q40 64 40 54 Q40 44 50 44Z", EYE, 2.6);
    let cyan = Color32::from_rgb(0x7f, 0xf0, 0xff);
    if on {
        pen.line("M48.6 55 Q52 49 55.4 55", 2.4, cyan);
        pen.line("M64.6 55 Q68 49 71.4 55", 2.4, cyan);
    } else if now % 4.0 < 0.15 {
        // A blink every few seconds.
        pen.line("M48.6 53 H55.4", 2.2, cyan);
        pen.line("M64.6 53 H71.4", 2.2, cyan);
    } else {
        pen.ellipse(52.0, 53.0, 3.4, 4.4, cyan, 0.0);
        pen.ellipse(68.0, 53.0, 3.4, 4.4, cyan, 0.0);
    }
    pen.line("M56 58 Q60 62 64 58", 2.2, cyan);
    pen.blush(33.0, 58.0, 4.5, 2.6);
    pen.blush(87.0, 58.0, 4.5, 2.6);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1360.0, 840.0))
    }

    #[test]
    fn litter_falls_where_the_pointer_lingers_up_to_the_cap() {
        let mut m = Mess::default();
        let at = pos2(400.0, 300.0);
        m.step(HOVER_SECS - 0.1, 0.0, Some(at), screen());
        assert_eq!(m.live(), 0);
        m.step(0.2, 1.0, Some(at), screen());
        assert_eq!(m.live(), 1);
        assert!(m.litter[0].at.distance(at) <= 18.0);
        for k in 0..200 {
            m.step(HOVER_SECS, k as f32, Some(at), screen());
        }
        assert_eq!(m.live(), MAX);
    }

    #[test]
    fn litter_piles_up_on_its_own_with_time() {
        let mut m = Mess::default();
        m.step(AMBIENT_SECS + 0.1, 0.0, None, screen());
        assert_eq!(m.live(), 1);
        assert!(screen().contains(m.litter[0].at));
    }

    #[test]
    fn a_click_on_litter_is_taken_and_elsewhere_passes() {
        let mut m = Mess::default();
        m.litter.push(Litter { at: pos2(100.0, 100.0), kind: Kind::Can, turn: 0.0, tint: 0, sucked: None, shaken: None });
        let click = |at: Pos2, pressed: bool| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let mut events = vec![click(pos2(104.0, 100.0), true), click(pos2(104.0, 100.0), false), click(pos2(600.0, 400.0), true), click(pos2(600.0, 400.0), false)];
        m.filter(&mut events, 1.0, screen());
        assert_eq!(events.len(), 2, "the click elsewhere goes through: {events:?}");
        assert!(m.litter[0].shaken.is_some());
    }

    #[test]
    fn the_vacuum_is_taken_sucks_and_is_put_back() {
        let mut m = Mess::default();
        let at = pos2(500.0, 500.0);
        m.litter.push(Litter { at, kind: Kind::Chips, turn: 0.0, tint: 0, sucked: None, shaken: None });
        assert!(m.press(parking(screen()).center(), 0.0, screen()));
        assert!(m.carried);
        m.step(0.016, 1.0, Some(at + vec2(10.0, 0.0)), screen());
        assert_eq!((m.live(), m.cleaned), (0, 1));
        // Gone once the animation is over.
        m.step(0.016, 1.0 + SUCK_SECS + 0.1, Some(at), screen());
        assert!(m.litter.is_empty());
        // Esc puts it back.
        let mut events = vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }];
        m.filter(&mut events, 2.0, screen());
        assert!(!m.carried && events.is_empty());
    }

    #[test]
    fn clearing_parks_the_vacuum_and_leaves_nothing() {
        let mut m = Mess::default();
        m.drop_at(pos2(10.0, 10.0));
        m.carried = true;
        m.cleaned = 4;
        m.clear();
        assert!(m.litter.is_empty() && !m.carried && m.cleaned == 0);
    }

    #[test]
    fn a_path_parses_with_its_curves_flattened() {
        let subs = path("M0 0 L10 0 H20 V5 Q20 10 15 10Z M1-2 L3 4");
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0][..4], [vec2(0.0, 0.0), vec2(10.0, 0.0), vec2(20.0, 0.0), vec2(20.0, 5.0)]);
        assert_eq!(*subs[0].last().unwrap(), vec2(15.0, 10.0));
        assert_eq!(subs[0].len(), 4 + 8);
        assert_eq!(subs[1], [vec2(1.0, -2.0), vec2(3.0, 4.0)]);
    }

    #[test]
    fn a_concave_shape_fills_with_two_triangles_less_than_its_points() {
        // An L, whichever way round it is drawn.
        let mut l = vec![pos2(0.0, 0.0), pos2(2.0, 0.0), pos2(2.0, 1.0), pos2(1.0, 1.0), pos2(1.0, 3.0), pos2(0.0, 3.0)];
        for _ in 0..2 {
            let tris = triangulate(&l);
            assert_eq!(tris.len(), l.len() - 2);
            let area: f32 = tris.iter().map(|[a, b, c]| ((l[*b] - l[*a]).x * (l[*c] - l[*a]).y - (l[*b] - l[*a]).y * (l[*c] - l[*a]).x).abs() / 2.0).sum();
            assert!((area - 4.0).abs() < 1e-4, "{area}");
            l.reverse();
        }
    }
}
