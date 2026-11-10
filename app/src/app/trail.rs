//! The Ultra-trail skin's mountain (#207): the board's City view as one long climb of 160 km,
//! seen from the side at dusk (by day in light mode). The sky, far ranges, the mountain whose
//! ridge is the course's profile, its forest, rock and snow, the trail with its ribbons; the
//! registration tent in the valley, the start arch, the aid station of km 92 and the summit's
//! finish arch. One runner per ticket (the stadium's athlete, singlet and headband of its zone,
//! its bib the colour of its pull request's checks), a rock ahead of a bug, and "You" at the
//! course's km. Drag to pan, pinch or ⌘-scroll to zoom, double-click to fit; a hover shows the
//! ticket, a click opens its detail. Read only. The layout is `crate::trail`; the screen x of
//! km 0 and km 160 is kept in `Tab::mountain_x` so the course bar lines up under it.

use super::city::CityView;
use super::track::{bib, figure, paint, zone_colour};
use super::*;
use crate::github::Item;
use crate::trail::{self, Mountain, Runner, Spot, KM};
use egui::epaint::Mesh;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The scenery's colours, at dusk (dark mode) or by day (light mode).
struct Scene {
    sky: [Color32; 3],
    stars: bool,
    sun: Color32,
    far: [Color32; 2],
    mountain: Color32,
    meadow: Color32,
    rock: Color32,
    snow: Color32,
    trees: [Color32; 2],
    trail: Color32,
}

fn scene() -> Scene {
    if theme::is_light() {
        Scene {
            sky: [rgb(0x7fb6e6), rgb(0xb9d8f1), rgb(0xf4d6bd)],
            stars: false,
            sun: rgb(0xfff0c2),
            far: [rgb(0xa5b6d0), rgb(0x8a9fbf)],
            mountain: rgb(0x4d7a50),
            meadow: rgb(0x5c8a55),
            rock: rgb(0x8b919b),
            snow: rgb(0xf8fafc),
            trees: [rgb(0x2f5a36), rgb(0x386a3f)],
            trail: rgb(0xf3e2bf),
        }
    } else {
        Scene {
            sky: [rgb(0x121a2b), rgb(0x2b3552), rgb(0xb47a63)],
            stars: true,
            sun: rgb(0xffd49a),
            far: [rgb(0x3a4766), rgb(0x2d3a55)],
            mountain: rgb(0x264232),
            meadow: rgb(0x2f5038),
            rock: rgb(0x59606c),
            snow: rgb(0xe9eef4),
            trees: [rgb(0x1d3828), rgb(0x22402e)],
            trail: rgb(0xd9b98a),
        }
    }
}

/// The world box fitted on screen (the valley's tent to past the summit, the sky to the valley
/// floor), and the room left at the bottom for the course bar, which covers it.
const WORLD: (f32, f32, f32, f32) = (-190.0, -540.0, 990.0, 62.0);
const BAR_ROOM: f32 = 106.0;
/// How far the scenery reaches past the world box, for a zoomed-out view.
const FAR: f32 = 4000.0;

/// World points to the screen.
struct View {
    origin: Pos2,
    zoom: f32,
}

impl View {
    fn at(&self, (x, y): (f32, f32)) -> Pos2 {
        self.origin + vec2(x, y) * self.zoom
    }
    fn px(&self, v: f32) -> f32 {
        v * self.zoom
    }
    /// The runners' and the words' scale: they shrink less than the mountain when zoomed out.
    fn fig(&self) -> f32 {
        if self.zoom < 1.0 { self.zoom.sqrt() } else { self.zoom }
    }
    fn font(&self, size: f32, bold: bool) -> FontId {
        let s = (size * self.fig()).max(5.0);
        if bold {
            FontId::new(s, FontFamily::Name(theme::MEDIUM.into()))
        } else {
            FontId::proportional(s)
        }
    }
}

/// The ridge from `k0` to `k1` km, in world points, every half km.
fn ridge(k0: f32, k1: f32) -> Vec<(f32, f32)> {
    let n = ((k1 - k0) * 2.0).round().max(1.0) as usize;
    (0..=n).map(|i| trail::ground(k0 + (k1 - k0) * i as f32 / n as f32)).collect()
}

/// The mountain's far side, from the summit down to the right, in world points.
const DESCENT: [(f32, f32); 8] = [(896.0, -430.0), (928.0, -378.0), (956.0, -310.0), (980.0, -270.0), (1060.0, -180.0), (1180.0, -60.0), (1320.0, 40.0), (1500.0, 120.0)];

/// Fills between two x-ordered lines of the same length, `top` and `bottom`, as one mesh.
fn strip(p: &Painter, top: &[Pos2], bottom: &[Pos2], col: Color32) {
    let mut m = Mesh::default();
    for (a, b) in top.iter().zip(bottom) {
        m.colored_vertex(*a, col);
        m.colored_vertex(*b, col);
    }
    for i in 0..top.len().saturating_sub(1) as u32 {
        let j = 2 * i;
        m.add_triangle(j, j + 1, j + 2);
        m.add_triangle(j + 1, j + 3, j + 2);
    }
    p.add(Shape::mesh(m));
}

/// Fills under an x-ordered line down to `y`.
fn fill_under(p: &Painter, line: &[Pos2], y: f32, col: Color32) {
    let bottom: Vec<Pos2> = line.iter().map(|q| pos2(q.x, y.max(q.y))).collect();
    strip(p, line, &bottom, col);
}

/// The sky over `r`: three stops at the world's heights `ys`, flat past them.
fn sky(p: &Painter, r: Rect, stops: [(f32, Color32); 3]) {
    let mut rows: Vec<(f32, Color32)> = vec![(r.top(), stops[0].1)];
    rows.extend(stops.iter().map(|(y, c)| (y.clamp(r.top(), r.bottom()), *c)));
    rows.push((r.bottom(), stops[2].1));
    let mut m = Mesh::default();
    for (y, c) in &rows {
        m.colored_vertex(pos2(r.left(), *y), *c);
        m.colored_vertex(pos2(r.right(), *y), *c);
    }
    for i in 0..rows.len() as u32 - 1 {
        let j = 2 * i;
        m.add_triangle(j, j + 1, j + 2);
        m.add_triangle(j + 1, j + 3, j + 2);
    }
    p.add(Shape::mesh(m));
}

/// A polygon of world points (`o` added), filled.
fn poly(p: &Painter, v: &View, o: (f32, f32), pts: &[(f32, f32)], col: Color32) {
    p.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| v.at((o.0 + x, o.1 + y))).collect(), col, Stroke::NONE));
}

/// A rectangle of `s`-scaled units at `o` (world), filled.
fn block(p: &Painter, v: &View, o: (f32, f32), s: f32, (x, y, w, h): (f32, f32, f32, f32), col: Color32) {
    p.rect_filled(Rect::from_min_size(v.at((o.0 + x * s, o.1 + y * s)), vec2(v.px(w * s), v.px(h * s))), 0.0, col);
}

/// An arch over the trail at `o`, `half` units each side and `tall` high: two posts and a
/// banner of its colour with its word in its ink.
fn arch(p: &Painter, v: &View, o: (f32, f32), (half, tall): (f32, f32), (banner, text, ink): (Color32, &str, Color32)) {
    let s = 1.3;
    let post = rgb(0x2b2e35);
    block(p, v, o, s, (-half, -tall, 4.0, tall), post);
    block(p, v, o, s, (half - 4.0, -tall, 4.0, tall), post);
    block(p, v, o, s, (-half - 2.0, -tall - 10.0, 2.0 * half + 4.0, 13.0), banner);
    let font = FontId::new(v.px(8.5 * s).max(4.0), FontFamily::Name(theme::MEDIUM.into()));
    p.text(v.at((o.0, o.1 + (-tall - 3.5) * s)), Align2::CENTER_CENTER, text, font, ink);
}

/// The aid station at `o`: an orange tent roof on poles, a table with cups and a jug.
fn aid_station(p: &Painter, v: &View, o: (f32, f32)) {
    let s = 1.4;
    let pole = rgb(0xe9e6dc);
    block(p, v, o, s, (-22.0, -24.0, 2.0, 24.0), pole);
    block(p, v, o, s, (20.0, -24.0, 2.0, 24.0), pole);
    poly(p, v, o, &[(-27.0 * s, -23.0 * s), (0.0, -37.0 * s), (27.0 * s, -23.0 * s)], rgb(0xff7a1a));
    block(p, v, o, s, (-27.0, -24.0, 54.0, 3.0), paint::WHITE);
    block(p, v, o, s, (-17.0, -10.0, 34.0, 3.5), rgb(0xf4f2ea));
    block(p, v, o, s, (-15.0, -7.0, 1.5, 7.0), rgb(0xc9c5b8));
    block(p, v, o, s, (13.5, -7.0, 1.5, 7.0), rgb(0xc9c5b8));
    for (x, c) in [(-10.0, rgb(0x7fd1ff)), (-3.0, rgb(0xff7a1a)), (4.0, rgb(0x7fd1ff))] {
        p.circle_filled(v.at((o.0 + x * s, o.1 - 12.0 * s)), v.px(2.0 * s), c);
    }
    block(p, v, o, s, (9.0, -16.0, 5.0, 6.0), rgb(0x3b82c4));
}

/// A rock on the trail at `at` (world): what a bug stumbles on.
fn rock(p: &Painter, v: &View, at: Pos2) {
    let f = v.fig();
    let pts: Vec<Pos2> = [(-9.0, 0.0), (-6.0, -8.0), (1.0, -11.0), (8.0, -5.0), (8.0, 0.0)].iter().map(|&(x, y)| at + vec2(x, y) * f).collect();
    p.add(Shape::convex_polygon(pts, rgb(0x6b6f78), Stroke::new(f, rgb(0x3a3d45))));
}

/// A pill of `text` centred on `at.x` with its top at `at.y`: dark with white text, or a white sign.
fn pill(p: &Painter, v: &View, at: Pos2, text: &str, sign: bool) -> Rect {
    let (fill, ink, font) = if sign { (paint::WHITE.gamma_multiply(0.92), paint::INK, v.font(10.5, true)) } else { (paint::INK.gamma_multiply(0.9), paint::WHITE, v.font(11.0, true)) };
    let g = p.layout_no_wrap(text.to_string(), font, ink);
    let f = v.fig();
    let size = vec2(g.size().x + 16.0 * f, 20.0 * f);
    let r = Rect::from_min_size(pos2(at.x - size.x / 2.0, at.y), size);
    p.rect_filled(r, CornerRadius::same((size.y / 2.0) as u8), fill);
    p.galley(r.center() - g.size() / 2.0, g, ink);
    r
}

/// "2,290 m": the altitude at `km`, less `down` metres, rounded to 10 m.
fn alt(km: f32, down: f32) -> String {
    format!("{} m", crate::course::thousands((((trail::altitude(km) - down) / 10.0).round() * 10.0) as u32))
}

/// The singlets' and bibs' legend, a row at `at` (wrapping before `right`); returns its height.
fn legend(p: &Painter, at: Pos2, right: f32, m: &Mountain) -> f32 {
    let k = theme::c();
    let small = FontId::proportional(11.0);
    let caption = FontId::new(10.0, FontFamily::Name(theme::MEDIUM.into()));
    let mut items: Vec<(Color32, String)> = m.zones.iter().enumerate().map(|(i, z)| (zone_colour(i), z.clone())).collect();
    items.push((Color32::TRANSPARENT, "BIB".into()));
    items.extend([(paint::PASSED, "passed".to_string()), (paint::RUNNING, "running".into()), (paint::FAILING, "failing".into())]);
    let w = p.text(at, Align2::LEFT_CENTER, "SINGLET · ZONE", caption.clone(), k.faint).width();
    let (mut x, mut y) = (at.x + w + 10.0, at.y);
    for (c, name) in items {
        let is_caption = c == Color32::TRANSPARENT;
        let font = if is_caption { caption.clone() } else { small.clone() };
        let gw = p.layout_no_wrap(name.clone(), font.clone(), k.text).size().x + if is_caption { 4.0 } else { 16.0 };
        if x + gw > right && x > at.x + w + 10.0 {
            (x, y) = (at.x + w + 10.0, y + 16.0);
        }
        if is_caption {
            x += 6.0;
            p.text(pos2(x, y), Align2::LEFT_CENTER, name, font, k.faint);
        } else {
            p.rect_filled(Rect::from_center_size(pos2(x + 4.5, y), vec2(9.0, 9.0)), CornerRadius::same(2), c);
            p.text(pos2(x + 13.0, y), Align2::LEFT_CENTER, name, font, k.text);
        }
        x += gw + 8.0;
    }
    y - at.y + 16.0
}

impl Tab {
    /// Draws the mountain of `items` (the tickets the board's filters keep) and returns the
    /// ticket clicked.
    pub(super) fn mountain_view(&mut self, ui: &mut Ui, items: &[&Item], detail: Option<&str>) -> Option<String> {
        let m = trail::layout(items);
        let area = ui.available_rect_before_wrap().shrink2(vec2(12.0, 6.0));
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        let p = ui.painter().with_clip_rect(area);
        let k = theme::c();
        let sc = scene();
        let time = ui.input(|i| i.time) as f32;

        // Fitted: the world box above the course bar, under the tally and the legend; then the
        // person's pan and zoom.
        let (x0, y0, x1, y1) = WORLD;
        let world = vec2(x1 - x0, y1 - y0);
        let room = Rect::from_min_max(area.min + vec2(0.0, 58.0), area.max - vec2(0.0, BAR_ROOM));
        let fit = (room.width() / world.x).min(room.height() / world.y).clamp(0.3, 1.6);
        let mut view = self.board.city.unwrap_or(CityView { pan: Vec2::ZERO, zoom: fit });
        let centre_w = vec2((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let origin = |view: &CityView| room.center() + view.pan - centre_w * view.zoom;
        if resp.dragged() {
            view.pan += resp.drag_delta();
            self.board.city = Some(view);
        }
        if resp.hovered() {
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
            if zoom != 1.0 {
                let at = ui.input(|i| i.pointer.hover_pos()).unwrap_or(area.center());
                let z = (view.zoom * zoom).clamp(0.25, 3.0);
                // The point under the pointer stays where it is.
                let o = at - (at - origin(&view)) * (z / view.zoom);
                view.pan = o - room.center() + centre_w * z;
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
        let v = View { origin: origin(&view), zoom: view.zoom };
        let at = |w: (f32, f32)| v.at(w);
        let pts = |w: &[(f32, f32)]| w.iter().map(|&q| v.at(q)).collect::<Vec<_>>();

        // The sky, its stars and the low sun; two far ranges.
        sky(&p, area, [(at((0.0, -540.0)).y, sc.sky[0]), (at((0.0, -287.0)).y, sc.sky[1]), (at((0.0, -80.0)).y, sc.sky[2])]);
        if sc.stars {
            for i in 0..40u32 {
                let w = (-130.0 + ((i * 263) % 1100) as f32, -532.0 + ((i * 97) % 150) as f32);
                p.circle_filled(at(w), if i % 5 == 0 { 1.4 } else { 0.9 }, rgb(0xe8ecf5).gamma_multiply(0.25 + (i % 4) as f32 * 0.15));
            }
        }
        p.circle_filled(at((160.0, -248.0)), v.px(46.0), sc.sun.gamma_multiply(0.9));
        let far1 = [(-FAR, -220.0), (-140.0, -220.0), (-80.0, -278.0), (0.0, -248.0), (90.0, -340.0), (180.0, -288.0), (270.0, -358.0), (380.0, -298.0), (480.0, -378.0), (580.0, -318.0), (680.0, -388.0), (780.0, -338.0), (880.0, -408.0), (980.0, -348.0), (FAR, -348.0)];
        fill_under(&p, &pts(&far1), area.bottom(), sc.far[0]);
        let far2 = [(-FAR, -170.0), (-140.0, -170.0), (-50.0, -222.0), (40.0, -188.0), (140.0, -258.0), (220.0, -208.0), (320.0, -242.0), (410.0, -194.0), (500.0, -150.0), (700.0, -20.0)];
        fill_under(&p, &pts(&far2), area.bottom(), sc.far[1]);

        // The mountain: its ridge is the course, flat in the valley, down again past the summit.
        let mut line = vec![(-FAR, trail::ground(0.0).1)];
        line.extend(ridge(0.0, trail::LENGTH_KM));
        line.extend(DESCENT.iter().skip(1));
        line.push((FAR, DESCENT[DESCENT.len() - 1].1));
        let ridge_line = pts(&line);
        fill_under(&p, &ridge_line, area.bottom(), sc.mountain);
        // Rock from km 56 up, snow from km 128, deeper towards the summit; the far side in shade.
        let band = |k0: f32, k1: f32, off: f32, col: Color32| {
            let top = ridge(k0, k1);
            let bottom: Vec<(f32, f32)> = top
                .iter()
                .map(|&(x, y)| {
                    let kk = x / KM;
                    let t = (kk - k0) / (k1 - k0);
                    (x, y + off * (std::f32::consts::FRAC_PI_2 * t).sin() * (0.75 + 0.25 * (kk * 0.85).sin()))
                })
                .collect();
            strip(&p, &pts(&top), &pts(&bottom), col);
        };
        band(56.0, trail::LENGTH_KM, 190.0, sc.rock);
        let far_rock: Vec<(f32, f32)> = DESCENT[..5].to_vec();
        let far_rock_low: Vec<(f32, f32)> = far_rock.iter().enumerate().map(|(i, &(x, y))| (x, y + [190.0, 200.0, 230.0, 260.0, 220.0][i])).collect();
        strip(&p, &pts(&far_rock), &pts(&far_rock_low), sc.rock);
        band(128.0, trail::LENGTH_KM, 70.0, sc.snow);
        let cap: Vec<(f32, f32)> = DESCENT[..3].to_vec();
        let cap_low: Vec<(f32, f32)> = cap.iter().enumerate().map(|(i, &(x, y))| (x, y + [70.0, 40.0, 4.0][i])).collect();
        strip(&p, &pts(&cap), &pts(&cap_low), sc.snow);
        let shade: Vec<Pos2> = pts(&DESCENT);
        fill_under(&p, &shade, area.bottom(), Color32::from_black_alpha(56));
        // The meadow in the valley before the start.
        p.rect_filled(Rect::from_min_max(at((-FAR, trail::ground(0.0).1)), pos2(at((0.0, 0.0)).x, area.bottom())), 0.0, sc.meadow);

        // The forest on the lower slopes and round the village.
        for i in 0..46u32 {
            let kk = 5.0 + i as f32 * 1.6;
            let (gx, gy) = trail::ground(kk);
            let d = 22.0 + ((i * 37) % 5) as f32 * 20.0;
            let (x, y) = (gx + (((i * 13) % 7) as f32 - 3.0) * 3.0, gy + d);
            if y > 60.0 {
                continue;
            }
            let (th, tw) = (15.0 + (i % 3) as f32 * 6.0, 5.0 + (i % 2) as f32 * 2.0);
            poly(&p, &v, (x, y), &[(0.0, -th), (tw, 0.0), (-tw, 0.0)], sc.trees[if i % 3 == 0 { 1 } else { 0 }]);
        }
        for (j, (x, y)) in [(-118.0, 20.0), (-98.0, 50.0), (-24.0, 28.0), (-130.0, 60.0), (-8.0, 60.0)].into_iter().enumerate() {
            poly(&p, &v, (x, y), &[(0.0, -24.0), (8.0, 0.0), (-8.0, 0.0)], sc.trees[j % 2]);
        }

        // The trail along the ridge, from the tent, and its marking ribbons.
        let mut path = vec![(-80.0, trail::ground(0.0).1)];
        path.extend(ridge(0.0, trail::LENGTH_KM));
        p.add(Shape::line(pts(&path), Stroke::new(v.px(2.5).max(1.2), sc.trail.gamma_multiply(0.9))));
        for kk in [10.0, 24.0, 40.0, 52.0, 72.0, 104.0, 114.0, 130.0, 148.0] {
            let (x, y) = trail::ground(kk);
            let o = (x + 6.0, y + 2.0);
            block(&p, &v, o, 1.0, (-0.8, -16.0, 1.6, 16.0), rgb(0xf2efe6));
            poly(&p, &v, o, &[(0.8, -16.0), (10.0, -12.5), (0.8, -9.0)], rgb(0xff4fa3));
        }

        // The village and the registration tent (Backlog), the start arch (Ready), the aid station
        // (In review) and the summit's arch (Done).
        let floor = trail::ground(0.0).1;
        block(&p, &v, (-132.0, floor - 25.0), 1.0, (0.0, 0.0, 30.0, 25.0), rgb(0xe9dfcc));
        poly(&p, &v, (0.0, floor), &[(-136.0, -23.0), (-117.0, -41.0), (-98.0, -23.0)], rgb(0xb5583f));
        block(&p, &v, (-122.0, floor - 15.0), 1.0, (0.0, 0.0, 8.0, 15.0), rgb(0x6d4a32));
        poly(&p, &v, (0.0, floor), &[(-90.0, 0.0), (-62.0, -35.0), (-34.0, 0.0)], paint::WHITE);
        poly(&p, &v, (0.0, floor), &[(-62.0, -35.0), (-34.0, 0.0), (-48.0, 0.0)], rgb(0xd4d0c6));
        poly(&p, &v, (0.0, floor), &[(-68.0, 0.0), (-62.0, -19.0), (-56.0, 0.0)], rgb(0x3a3d45));
        arch(&p, &v, (0.0, floor), (30.0, 40.0), (rgb(0x3b82c4), "START", paint::WHITE));
        let aid = trail::ground(trail::AID_KM);
        aid_station(&p, &v, (aid.0, aid.1 + 2.0));
        let top = trail::ground(trail::LENGTH_KM);
        arch(&p, &v, (top.0, top.1 + 2.0), (34.0, 44.0), (rgb(0xff7a1a), "FINISH", paint::INK));

        // Rocks, runners and "+k" pills, back to front.
        let by_id: std::collections::HashMap<&str, &Item> = items.iter().map(|it| (it.id.as_str(), *it)).collect();
        let pulls = self.pulls.pulls.as_deref();
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|h| area.contains(*h) && resp.hovered() && !self.board.shooting);
        let f = v.fig() * 1.5;
        let hit = |r: &Runner| {
            let foot = at(r.pos);
            Rect::from_min_max(foot + vec2(-12.0, -40.0) * f, foot + vec2(12.0, 3.0) * f)
        };
        let hovered: Option<&Runner> = m.runners.iter().rev().find(|r| hover.is_some_and(|h| hit(r).contains(h)));
        for r in m.runners.iter().filter(|r| r.rock) {
            let (x, y) = trail::ground(r.km + 3.2);
            rock(&p, &v, at((x, y + 2.0)));
        }
        for r in &m.runners {
            let Some(it) = by_id.get(r.id.as_str()) else { continue };
            let foot = at(r.pos);
            let selected = detail == Some(r.id.as_str());
            if selected || hovered.is_some_and(|h| h.id == r.id) {
                let col = if selected { k.accent } else { k.accent_text };
                p.add(Shape::ellipse_stroke(foot + vec2(0.0, -16.0) * f / 1.5, vec2(22.0, 26.0) * f / 1.5, Stroke::new(2.0, col)));
            }
            p.add(Shape::ellipse_filled(foot, vec2(9.0, 2.5) * f / 1.5, Color32::from_black_alpha(76)));
            let swing = if r.spot.running() { (time * 7.0 + (r.seed % 628) as f32 / 100.0).sin() } else { 0.0 };
            figure(&p, foot, f, 1.0, r.seed, zone_colour(r.zone), bib(it, pulls), swing);
        }
        // Each runner on the trail has its label on a stem, two heights apart from its neighbour.
        for r in m.runners.iter().filter(|r| r.spot.running()) {
            let foot = at(r.pos);
            let lift = if r.high { 88.0 } else { 66.0 } * v.fig();
            let label = pill(&p, &v, foot - vec2(0.0, lift), &r.label, false);
            p.vline(foot.x, label.bottom()..=(foot.y - 40.0 * v.fig()), Stroke::new(1.0, paint::INK.gamma_multiply(0.6)));
        }
        for more in m.more.iter().filter(|x| x.spot.running()) {
            let foot = at(more.pos);
            pill(&p, &v, foot - vec2(0.0, 44.0 * v.fig()), &format!("+{}", more.more), false);
        }

        // The groups at the tent, the start and the summit; the places' signs.
        // Each group's pill stands 6 points over what it names (the tent, an arch's banner).
        let group = |spot: Spot, w: (f32, f32)| {
            let (_, status) = spot.sign();
            let shown = m.at(spot);
            let text = if shown.len() < m.count(spot) || spot == Spot::Tent {
                format!("{status} · {}", m.count(spot))
            } else {
                let nums: Vec<&str> = shown.iter().map(|r| r.label.split(' ').next().unwrap_or("")).collect();
                format!("{status} · {}", nums.join(" "))
            };
            if m.count(spot) > 0 {
                pill(&p, &v, at(w) - vec2(0.0, 26.0 * v.fig()), &text, false);
            }
        };
        group(Spot::Tent, (-76.0, floor - 35.0));
        group(Spot::Start, (0.0, floor - 64.0));
        group(Spot::Summit, (top.0, top.1 + 2.0 - 70.0));
        let sign = |w: (f32, f32), text: String| {
            pill(&p, &v, at(w), &text, true);
        };
        let place = |kk: f32, below: f32| {
            let (x, y) = trail::ground(kk);
            (x, y + below)
        };
        sign((-62.0, 32.0), format!("Registration · {}", alt(0.0, 40.0)));
        sign(place(32.0, 46.0), "The climb · In progress".into());
        sign(place(trail::AID_KM, 26.0), format!("Aid station · In review · {}", alt(trail::AID_KM, 0.0)));
        sign(place(130.0, 40.0), format!("Ridge · To test · {}", alt(130.0, 0.0)));
        sign((870.0, -340.0), format!("Summit · {}", alt(trail::LENGTH_KM, 0.0)));

        // You: the course's km, the same as the bar's below.
        let you_km = self.trail_km.clamp(0.0, trail::LENGTH_KM);
        let (yx, yy) = trail::ground(you_km);
        let you = at((yx, yy + 2.0));
        let fy = v.fig();
        p.extend(Shape::dashed_line(&[you, pos2(you.x, area.bottom() - BAR_ROOM + 8.0)], Stroke::new(1.2, paint::PEACH.gamma_multiply(0.55)), 3.0, 4.0));
        p.circle(you, 7.0 * fy, rgb(0xff7a1a), Stroke::new(2.5 * fy, paint::INK));
        let g = p.layout_no_wrap(format!("You · km {}", you_km.floor() as u32), v.font(11.0, true), paint::INK);
        // Under the marker on the last km, so the summit's arch keeps its banner.
        let lift = if you_km >= 154.0 { -23.0 } else { 23.0 };
        let r = Rect::from_center_size(you - vec2(0.0, lift * fy), vec2(g.size().x + 18.0 * fy, 22.0 * fy));
        p.rect_filled(r, CornerRadius::same((11.0 * fy) as u8), rgb(0xff7a1a));
        p.galley(r.center() - g.size() / 2.0, g, paint::INK);

        // The bar lines up under km 0 and km 160.
        self.mountain_x = Some((at((0.0, 0.0)).x, at((trail::LENGTH_KM * KM, 0.0)).x));

        // The tally, the hints and the legend above.
        let on_trail = m.count(Spot::Climb) + m.count(Spot::Aid) + m.count(Spot::Ridge);
        let bg = Rect::from_min_size(area.min, vec2(area.width(), 56.0));
        p.rect_filled(bg, 0.0, k.bg);
        p.text(area.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, format!("{} of {} at the summit · {on_trail} on the trail · {} at the start · {} registered", m.count(Spot::Summit), m.total(), m.count(Spot::Start), m.count(Spot::Tent)), theme::secondary(), k.dim);
        // Not in a photo (#232).
        if !self.board.shooting {
            p.text(area.left_top() + vec2(4.0, 20.0), Align2::LEFT_TOP, "Drag to move · pinch or ⌘ scroll to zoom · double-click to fit", theme::small(), k.faint);
        }
        legend(&p, area.left_top() + vec2(4.0, 45.0), area.right() - 8.0, &m);

        // The runners' legs move while the mountain is on screen.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));

        let mut clicked = None;
        if let Some(r) = hovered {
            if let Some(it) = by_id.get(r.id.as_str()) {
                let n = it.number.map(|n| format!("#{n} ")).unwrap_or_default();
                let (name, _) = r.spot.sign();
                let place = match r.spot {
                    Spot::Tent => "at registration".to_string(),
                    _ => format!("{}, km {}", name.to_lowercase().replace("the ", ""), r.km.round() as i32),
                };
                let zone = m.zones.get(r.zone).map(|z| format!(" · {z}")).unwrap_or_default();
                let who = if it.assignees.is_empty() { String::new() } else { format!(" · {}", it.assignees.join(", ")) };
                resp.clone().on_hover_text_at_pointer(format!("{n}{}\n{} · {place}{zone}{who}", it.title, it.status));
                if resp.clicked() {
                    clicked = Some(r.id.clone());
                }
            }
        }
        clicked
    }
}
