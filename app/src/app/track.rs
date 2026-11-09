//! The Runner skin's stadium (#206): the board's City view as an athletics track, seen from
//! above in flat colours. Red lanes round a green infield, one lane per zone; one athlete per
//! ticket in a singlet and headband of its zone's colour, its bib the colour of its pull
//! request's checks; the warm-up area, the scoreboard and the podium on the infield; START and
//! FINISH across the track. Drag to pan, pinch or ⌘-scroll to zoom,
//! double-click to fit; a hover shows the ticket, a click opens its detail. Read only. The
//! layout is `crate::track`.

use super::city::CityView;
use super::*;
use crate::github::Item;
use crate::pulls::Ci;
use crate::track::{self, Athlete, Spot, Stadium};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub(super) mod paint {
    use super::*;
    pub const GRASS_OUT: Color32 = rgb(0x2e4a2a);
    pub const TRACK: Color32 = rgb(0xc4543b);
    pub const LINE: Color32 = rgb(0xf3e9e4);
    pub const INFIELD: Color32 = rgb(0x4f7a3c);
    pub const RING: Color32 = rgb(0x5f8a4a);
    pub const WHITE: Color32 = rgb(0xf7f8f8);
    pub const INK: Color32 = rgb(0x111215);
    pub const WARMUP: Color32 = rgb(0x466e35);
    pub const WARMUP_EDGE: Color32 = rgb(0x7da564);
    pub const PALE: Color32 = rgb(0xe3f0d8);
    pub const BOARD_EDGE: Color32 = rgb(0x2d2f36);
    pub const PEACH: Color32 = rgb(0xffb27a);
    pub const DIM: Color32 = rgb(0xa2a5ae);
    pub const STEPS: [Color32; 3] = [rgb(0xd6cfbf), rgb(0xefe9dc), rgb(0xcbc4b3)];
    pub const STEP_INK: Color32 = rgb(0x6d5f4a);
    pub const SHOE: Color32 = rgb(0xc6f432);
    pub const HURDLE_LEG: Color32 = rgb(0x3a3d45);
    /// The lanes' colours, inside out.
    pub const ZONES: [Color32; 8] = [rgb(0x5e9cf0), rgb(0xb07ff0), rgb(0xff7a1a), rgb(0x3fb950), rgb(0xe8478b), rgb(0xe3c53a), rgb(0x3fc5c5), rgb(0x9aa0a6)];
    pub const TONES: [Color32; 4] = [rgb(0xf1c9a5), rgb(0xd9a47a), rgb(0xa8714a), rgb(0x6e4a32)];
    pub const PASSED: Color32 = rgb(0x3fb950);
    pub const RUNNING: Color32 = rgb(0xe3a008);
    pub const FAILING: Color32 = rgb(0xf85149);
}

/// How far the outer grass reaches past the last lane.
const GRASS: Vec2 = vec2(55.0, 30.0);
/// Room left at the bottom of the board for the course bar, which covers it.
const BAR_ROOM: f32 = 100.0;

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
    /// The scale of the athletes and the words: they shrink less than the stadium when it is
    /// zoomed out, so they stay readable.
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

pub(super) fn zone_colour(lane: usize) -> Color32 {
    paint::ZONES[lane.min(paint::ZONES.len() - 1)]
}

/// The bib: the checks of the ticket's open pull request, white without one.
pub(super) fn bib(it: &Item, pulls: Option<&crate::pulls::Pulls>) -> Color32 {
    let open = it.pull_requests.iter().find(|l| l.state == "OPEN");
    match open.and_then(|pr| pulls.and_then(|ps| ps.list.iter().find(|q| q.number == pr.number)).and_then(|q| q.ci)) {
        Some(Ci::Success) => paint::PASSED,
        Some(Ci::Pending) => paint::RUNNING,
        Some(Ci::Failure) => paint::FAILING,
        None => paint::WHITE,
    }
}

/// A rounded stroke along `pts` (egui's caps are flat).
fn limb(p: &Painter, pts: &[Pos2], w: f32, col: Color32) {
    p.add(Shape::line(pts.to_vec(), Stroke::new(w, col)));
    for q in pts {
        p.circle_filled(*q, w / 2.0, col);
    }
}

/// A point turned by `a` radians round `c`.
fn turn((x, y): (f32, f32), (cx, cy): (f32, f32), a: f32) -> (f32, f32) {
    let (s, c) = a.sin_cos();
    (cx + (x - cx) * c - (y - cy) * s, cy + (x - cx) * s + (y - cy) * c)
}

/// One athlete, its feet at `at`: the mockup's runner, its legs and arms swinging by `swing`
/// (-1 to 1), facing `facing`.
fn athlete(p: &Painter, v: &View, a: &Athlete, zone: Color32, bib: Color32, swing: f32) {
    figure(p, v.at(a.pos), v.fig(), a.facing, a.seed, zone, bib, swing);
}

/// The runner's figure, its feet at `foot`, `f` points per unit: also the Ultra-trail's (#207).
#[allow(clippy::too_many_arguments)]
pub(super) fn figure(p: &Painter, foot: Pos2, f: f32, facing: f32, seed: u32, zone: Color32, bib: Color32, swing: f32) {
    let skin = paint::TONES[(seed as usize) % paint::TONES.len()];
    let px = |x: f32| x * f;
    let q = |pt: (f32, f32)| foot + vec2(pt.0 * facing, pt.1) * f;
    let path = |pts: &[(f32, f32)]| pts.iter().map(|&pt| q(pt)).collect::<Vec<_>>();
    p.add(Shape::ellipse_filled(foot, vec2(px(7.0), px(2.0)), Color32::from_black_alpha(64)));
    let (hip, shoulder) = ((0.0, -11.0), (1.2, -18.0));
    let leg = 0.35 * swing;
    let back_leg: Vec<(f32, f32)> = [(0.0, -11.0), (-4.0, -6.0), (-8.0, -4.0)].iter().map(|&pt| turn(pt, hip, leg)).collect();
    let front_leg: Vec<(f32, f32)> = [(0.0, -11.0), (3.5, -5.5), (2.5, 0.0)].iter().map(|&pt| turn(pt, hip, -leg)).collect();
    limb(p, &path(&back_leg), px(2.4), theme::shade(skin, 0.72));
    limb(p, &path(&front_leg), px(2.6), skin);
    p.circle_filled(q(*front_leg.last().unwrap_or(&(2.5, 0.0))), px(1.8), paint::SHOE);
    p.circle_filled(q(*back_leg.last().unwrap_or(&(-8.0, -4.0))), px(1.6), paint::SHOE);
    limb(p, &path(&[(0.0, -11.0), (1.6, -19.0)]), px(6.0), zone);
    let b = [q((-1.4, -17.4)), q((3.0, -13.4))];
    p.rect_filled(Rect::from_two_pos(b[0], b[1]), CornerRadius::same(1), bib);
    let arm = 0.4 * swing;
    let front_arm: Vec<(f32, f32)> = [(1.2, -18.0), (4.8, -14.0), (8.0, -16.0)].iter().map(|&pt| turn(pt, shoulder, arm)).collect();
    let back_arm: Vec<(f32, f32)> = [(1.2, -18.0), (-2.0, -14.0), (-4.5, -12.0)].iter().map(|&pt| turn(pt, shoulder, -arm)).collect();
    limb(p, &path(&front_arm), px(1.9), skin);
    limb(p, &path(&back_arm), px(1.9), skin);
    p.circle_filled(q((2.6, -23.0)), px(3.2), skin);
    p.rect_filled(Rect::from_two_pos(q((-0.8, -25.3)), q((6.0, -23.7))), 0.0, zone);
}

/// A dark pill with white text, its bottom `lift` points above `at`.
fn tag(p: &Painter, v: &View, at: Pos2, lift: f32, text: &str) -> Rect {
    let g = p.layout_no_wrap(text.to_string(), v.font(10.0, true), paint::WHITE);
    let f = v.fig();
    let size = vec2(g.size().x + 14.0 * f, 16.0 * f);
    let r = Rect::from_center_size(at - vec2(0.0, lift * f + size.y / 2.0), size);
    p.rect_filled(r, CornerRadius::same((size.y / 2.0) as u8), paint::INK.gamma_multiply(0.85));
    p.galley(r.center() - g.size() / 2.0, g, paint::WHITE);
    r
}

/// A hurdle, its feet at `at`.
fn hurdle(p: &Painter, v: &View, at: Pos2) {
    let f = v.fig();
    let r = |x: f32, y: f32, w: f32, h: f32, c: Color32| p.rect_filled(Rect::from_min_size(at + vec2(x, y) * f, vec2(w, h) * f), 0.0, c);
    r(-8.0, -8.0, 1.5, 8.0, paint::HURDLE_LEG);
    r(6.5, -8.0, 1.5, 8.0, paint::HURDLE_LEG);
    r(-8.0, -11.0, 16.0, 3.0, paint::WHITE);
    r(-8.0, -11.0, 4.0, 3.0, paint::INK);
    r(0.0, -11.0, 4.0, 3.0, paint::INK);
}

/// A band across the lanes from `deg` to `deg + w` degrees, from `r0` to `r1` out of the infield.
fn band(v: &View, deg: f32, w: f32, r0: f32, r1: f32) -> Vec<Pos2> {
    vec![v.at(track::ring(r0, deg)), v.at(track::ring(r1, deg)), v.at(track::ring(r1, deg + w)), v.at(track::ring(r0, deg + w))]
}

/// The legend's card width, and the note under it.
const CARD_W: f32 = 214.0;
const NOTE: &str = "A bug jumps a hurdle. A feature shows its relay legs (sub-issues).";

fn caption(p: &Painter, at: Pos2, s: &str) -> f32 {
    let g = p.layout_no_wrap(s.to_string(), FontId::new(10.0, FontFamily::Name(theme::MEDIUM.into())), theme::c().faint);
    let w = g.size().x;
    p.galley(at - vec2(0.0, g.size().y / 2.0), g, theme::c().faint);
    w
}

/// A lane's swatch and name, its middle-left at `at`; returns its width.
fn lane_chip(p: &Painter, at: Pos2, i: usize, name: &str, max: f32) -> f32 {
    p.rect_filled(Rect::from_center_size(at + vec2(5.0, 0.0), vec2(10.0, 10.0)), CornerRadius::same(3), zone_colour(i));
    let text = format!("Lane {} · {name}", i + 1);
    let g = p.layout_no_wrap(text.clone(), FontId::proportional(12.0), theme::c().text);
    let w = g.size().x.min(max - 18.0);
    crate::ui::fit_text(p, at + vec2(18.0, 0.0), Align2::LEFT_CENTER, &text, FontId::proportional(12.0), theme::c().text, max - 18.0);
    18.0 + w
}

/// The bib's colours in a row, its middle-left at `at`; returns its width.
fn bib_row(p: &Painter, at: Pos2) -> f32 {
    let mut x = at.x;
    for (c, s) in [(paint::PASSED, "passed"), (paint::RUNNING, "running"), (paint::FAILING, "failing")] {
        p.rect_filled(Rect::from_center_size(pos2(x + 4.5, at.y), vec2(9.0, 9.0)), CornerRadius::same(2), c);
        let g = p.layout_no_wrap(s.to_string(), FontId::proportional(12.0), c);
        let gw = g.size().x;
        p.galley(pos2(x + 13.0, at.y - g.size().y / 2.0), g, c);
        x += 13.0 + gw + 12.0;
    }
    x - 12.0 - at.x
}

fn bib_row_width(p: &Painter) -> f32 {
    ["passed", "running", "failing"].iter().map(|s| 25.0 + p.layout_no_wrap(s.to_string(), FontId::proportional(12.0), Color32::WHITE).size().x).sum::<f32>() - 12.0
}

/// The mockup's legend card at the top right of `area`: the lanes, the bib's colours, the note.
fn legend_card(p: &Painter, area: Rect, st: &Stadium) {
    let k = theme::c();
    let note_g = p.layout(NOTE.to_string(), FontId::proportional(11.0), k.dim, CARD_W - 28.0);
    let h = 14.0 + 16.0 + st.lanes.len() as f32 * 18.0 + 10.0 + 16.0 + 18.0 + 8.0 + note_g.size().y + 12.0;
    let r = Rect::from_min_size(pos2(area.right() - 8.0 - CARD_W, area.top() + 8.0), vec2(CARD_W, h));
    p.add(crate::modal::card_back(r, CornerRadius::same(12)));
    p.rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0, k.border), StrokeKind::Inside);
    let x = r.left() + 14.0;
    let mut y = r.top() + 18.0;
    caption(p, pos2(x, y), "LANES · ZONES");
    y += 18.0;
    for (i, name) in st.lanes.iter().enumerate() {
        lane_chip(p, pos2(x, y), i, name, CARD_W - 28.0);
        y += 18.0;
    }
    y += 10.0;
    caption(p, pos2(x, y), "BIB · PULL REQUEST CHECKS");
    y += 18.0;
    bib_row(p, pos2(x, y));
    y += 14.0;
    p.galley(pos2(x, y), note_g, k.dim);
}

/// The legend as rows above the stadium when there's no room beside it: the lanes (wrapping),
/// the bib's colours on the right, the note under them. Returns the height it takes from `top`.
fn legend_strip(p: &Painter, area: Rect, top: f32, st: &Stadium) -> f32 {
    let bib_w = 38.0 + bib_row_width(p);
    let right = area.right() - 4.0 - bib_w;
    let bx = caption(p, pos2(right, top + 8.0), "BIB");
    bib_row(p, pos2(right + bx + 8.0, top + 8.0));
    let left = area.left() + 4.0;
    let (mut x, mut y) = (left, top + 8.0);
    for (i, name) in st.lanes.iter().enumerate() {
        let w = 18.0 + p.layout_no_wrap(format!("Lane {} · {name}", i + 1), FontId::proportional(12.0), Color32::WHITE).size().x;
        if x > left && x + w > right - 16.0 {
            (x, y) = (left, y + 18.0);
        }
        x += lane_chip(p, pos2(x, y), i, name, right - 16.0 - x) + 14.0;
    }
    p.text(pos2(left, y + 18.0), Align2::LEFT_CENTER, NOTE, theme::small(), theme::c().faint);
    y + 28.0 - top
}

impl Tab {
    /// Draws the stadium of `items` (the tickets the board's filters keep) and returns the
    /// ticket clicked.
    pub(super) fn stadium_view(&mut self, ui: &mut Ui, items: &[&Item], detail: Option<&str>) -> Option<String> {
        let st = track::layout(items);
        let area = ui.available_rect_before_wrap().shrink2(vec2(12.0, 6.0));
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        let p = ui.painter().with_clip_rect(area);
        let k = theme::c();
        let time = ui.input(|i| i.time) as f32;

        // Fitted: the stadium above the course bar, centred; then the person's pan and zoom.
        let outer = st.outer();
        let half = vec2(outer.0 + GRASS.x, outer.1 + GRASS.y);
        let world = vec2(2.0 * half.x, 2.0 * half.y);
        // The legend's card beside the stadium when the stadium, fitted to the height, leaves
        // room for it; else a strip of rows above it.
        let room = Rect::from_min_max(area.min + vec2(0.0, 40.0), area.max - vec2(0.0, BAR_ROOM));
        let by_height = room.height() / (world.y + 10.0);
        let card = room.width() - CARD_W - 16.0 >= by_height * (world.x + 20.0);
        let room = if card {
            Rect::from_min_max(room.min, room.max - vec2(CARD_W + 16.0, 0.0))
        } else {
            let strip = legend_strip(&p, area, area.top() + 36.0, &st);
            Rect::from_min_max(pos2(room.left(), area.top() + 36.0 + strip), room.max)
        };
        let fit = (room.width() / (world.x + 20.0)).min(room.height() / (world.y + 10.0)).clamp(0.3, 1.6);
        let mut view = self.board.city.unwrap_or(CityView { pan: Vec2::ZERO, zoom: fit });
        if resp.dragged() {
            view.pan += resp.drag_delta();
            self.board.city = Some(view);
        }
        if resp.hovered() {
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
            if zoom != 1.0 {
                let at = ui.input(|i| i.pointer.hover_pos()).unwrap_or(area.center());
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
        // The world's centre is the infield's.
        let v = View { origin: room.center() + view.pan, zoom: view.zoom };
        let c = v.at((0.0, 0.0));
        let ell = |rx: f32, ry: f32| vec2(v.px(rx), v.px(ry));

        // The ground: outer grass, the red track, the lane lines, the infield and its rings.
        p.add(Shape::ellipse_filled(c, ell(half.x, half.y), paint::GRASS_OUT));
        p.add(Shape::ellipse_filled(c, ell(outer.0, outer.1), paint::TRACK));
        for l in 0..=st.lanes.len().max(1) {
            let r = l as f32 * track::LANE;
            p.add(Shape::ellipse_stroke(c, ell(track::INNER.0 + r, track::INNER.1 + r), Stroke::new(v.px(1.5), paint::LINE.gamma_multiply(0.85))));
        }
        p.add(Shape::ellipse_filled(c, ell(track::INNER.0, track::INNER.1), paint::INFIELD));
        for (rx, ry) in [(380.0, 165.0), (300.0, 120.0)] {
            p.add(Shape::ellipse_stroke(c, ell(rx, ry), Stroke::new(v.px(22.0), paint::RING)));
        }

        // START and FINISH across the lanes, and the status words beside the track.
        let width = st.lanes.len().max(1) as f32 * track::LANE;
        p.add(Shape::convex_polygon(band(&v, track::START_DEG, 1.2, 0.0, width), paint::WHITE, Stroke::NONE));
        let squares = (width / 12.0).round() as usize;
        for j in 0..squares {
            let (r0, r1) = (j as f32 * 12.0, (j as f32 * 12.0 + 12.0).min(width));
            for (col, deg) in [(0, track::FINISH_DEG), (1, track::FINISH_DEG + 2.0)] {
                let fill = if (j + col) % 2 == 0 { paint::WHITE } else { paint::INK };
                p.add(Shape::convex_polygon(band(&v, deg, 2.0, r0, r1), fill, Stroke::NONE));
            }
        }
        let mut marks: Vec<(&str, f32, Color32)> = Spot::ALL.iter().filter_map(|s| s.mark()).map(|(t, d)| (t, d, paint::WHITE)).collect();
        marks.push(("FINISH · DONE", 140.0, paint::PEACH));
        for (text, deg, col) in marks {
            // Along the straights on the grass outside; by the bends on the infield, inside.
            let cos = deg.to_radians().cos();
            let (at, align) = match cos {
                c if c < -0.8 => (track::ring(-14.0, deg), Align2::LEFT_CENTER),
                c if c > 0.8 => (track::ring(-14.0, deg), Align2::RIGHT_CENTER),
                _ => (track::ring(width + 15.0, deg), Align2::CENTER_CENTER),
            };
            p.text(v.at(at), align, text, v.font(12.0, true), col);
        }

        // The infield: the warm-up area, the scoreboard, the podium.
        let rect = |(a, b): ((f32, f32), (f32, f32))| Rect::from_two_pos(v.at(a), v.at(b));
        let wr = rect(track::WARMUP);
        p.rect_filled(wr, CornerRadius::same(v.px(12.0) as u8), paint::WARMUP);
        let dash = Stroke::new(v.px(1.0), paint::WARMUP_EDGE);
        for side in [[wr.left_top(), wr.right_top()], [wr.right_top(), wr.right_bottom()], [wr.right_bottom(), wr.left_bottom()], [wr.left_bottom(), wr.left_top()]] {
            p.extend(Shape::dashed_line(&side, dash, v.px(4.0), v.px(4.0)));
        }
        p.text(pos2(wr.center().x, wr.top() + v.px(16.0)), Align2::CENTER_CENTER, "WARM-UP · BACKLOG", v.font(11.0, true), paint::PALE);

        let sb = rect(track::SCOREBOARD);
        p.rect(sb, CornerRadius::same(v.px(8.0) as u8), paint::INK, Stroke::new(v.px(1.0), paint::BOARD_EDGE), StrokeKind::Inside);
        let done = st.count(Spot::Podium);
        let name = self.name().to_uppercase();
        // Each line as big as the board's width lets it.
        let line = |y: f32, text: String, size: f32, bold: bool, col: Color32| {
            let font = v.font(size, bold);
            let w = p.layout_no_wrap(text.clone(), font.clone(), col).size().x;
            let room = sb.width() - v.px(14.0);
            let font = if w > room { FontId::new(font.size * room / w, font.family) } else { font };
            p.text(pos2(sb.center().x, sb.top() + v.px(y)), Align2::CENTER_CENTER, text, font, col);
        };
        line(18.0, format!("{name} · LAP 1"), 12.0, true, paint::PEACH);
        line(42.0, format!("{done} / {} finished", st.total()), 22.0, true, paint::WHITE);
        let per = |spots: &[(Spot, &str)]| spots.iter().map(|(s, w)| format!("{} {w}", st.count(*s))).collect::<Vec<_>>().join(" · ");
        line(66.0, per(&[(Spot::LastBend, "to test"), (Spot::Back, "in review"), (Spot::Bend, "running")]), 10.0, false, paint::DIM);
        line(81.0, per(&[(Spot::Blocks, "ready"), (Spot::Warmup, "warming up")]), 10.0, false, paint::DIM);

        for (i, step) in track::STEPS.iter().enumerate() {
            let r = rect(*step);
            p.rect_filled(r, 0.0, paint::STEPS[i]);
            let (n, size) = [("2", 12.0), ("1", 14.0), ("3", 10.0)][i];
            p.text(pos2(r.center().x, r.top() + v.px(size * 0.5 + 4.0) + v.px(if i == 1 { 8.0 } else { 3.0 })), Align2::CENTER_CENTER, n, v.font(size, true), paint::STEP_INK);
        }
        let steps_bottom = v.at((0.0, track::STEPS[1].1 .1)).y;
        let podium_x = (v.at(track::STEPS[0].0).x + v.at(track::STEPS[2].1).x) / 2.0;
        p.text(pos2(podium_x, steps_bottom + v.px(14.0)), Align2::CENTER_CENTER, "PODIUM · DONE", v.font(11.0, true), paint::PALE);

        // Hurdles, athletes and "+k" pills, back to front.
        let by_id: std::collections::HashMap<&str, &Item> = items.iter().map(|it| (it.id.as_str(), *it)).collect();
        let pulls = self.pulls.pulls.as_deref();
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|h| area.contains(*h) && resp.hovered());
        // What each athlete covers, for the hover: its body and its label.
        let hit = |a: &Athlete| {
            let (f, z) = (v.at(a.pos), v.fig());
            Rect::from_min_max(f + vec2(-12.0, -44.0) * z, f + vec2(12.0, 3.0) * z)
        };
        let hovered: Option<&Athlete> = st.athletes.iter().rev().find(|a| hover.is_some_and(|h| hit(a).contains(h)));
        enum Thing<'a> {
            Hurdle((f32, f32)),
            Athlete(&'a Athlete),
            More(&'a track::More),
        }
        let mut things: Vec<(f32, Thing)> = Vec::new();
        for a in &st.athletes {
            things.push((a.pos.1, Thing::Athlete(a)));
            if let (true, Some(deg)) = (a.hurdle, a.deg) {
                let h = track::at(a.lane, deg - 6.0);
                things.push((h.1, Thing::Hurdle(h)));
            }
        }
        for m in &st.more {
            things.push((m.pos.1, Thing::More(m)));
        }
        things.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, thing) in &things {
            match thing {
                Thing::Hurdle(at) => hurdle(&p, &v, v.at(*at)),
                Thing::More(m) => {
                    tag(&p, &v, v.at(m.pos), 10.0, &format!("+{}", m.more));
                }
                Thing::Athlete(a) => {
                    let Some(it) = by_id.get(a.id.as_str()) else { continue };
                    let selected = detail == Some(a.id.as_str());
                    if selected || hovered.is_some_and(|h| h.id == a.id) {
                        let col = if selected { k.accent } else { k.accent_text };
                        p.add(Shape::ellipse_stroke(v.at(a.pos), vec2(11.0, 4.0) * v.fig(), Stroke::new(2.0, col)));
                    }
                    let swing = if a.spot.running() { (time * 9.0 + (a.seed % 628) as f32 / 100.0).sin() } else { 0.0 };
                    athlete(&p, &v, a, zone_colour(a.lane), bib(it, pulls), swing);
                    tag(&p, &v, v.at(a.pos), 28.0, &a.label);
                }
            }
        }

        // The legend's card on the right (the strip is drawn already); the tally and the hints above.
        if card {
            legend_card(&p, area, &st);
        }
        let running = st.count(Spot::Bend) + st.count(Spot::Back) + st.count(Spot::LastBend);
        p.text(area.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, format!("{done} finished · {running} on the track · {} in the blocks · {} warming up", st.count(Spot::Blocks), st.count(Spot::Warmup)), theme::secondary(), k.dim);
        p.text(area.left_top() + vec2(4.0, 20.0), Align2::LEFT_TOP, "Drag to move · pinch or ⌘ scroll to zoom · double-click to fit", theme::small(), k.faint);

        // The runners' legs move while the stadium is on screen.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));

        let mut clicked = None;
        if let Some(a) = hovered {
            if let Some(it) = by_id.get(a.id.as_str()) {
                let n = it.number.map(|n| format!("#{n} ")).unwrap_or_default();
                let lane = st.lanes.get(a.lane).map(|l| format!(" · lane {} ({l})", a.lane + 1)).unwrap_or_default();
                let who = if it.assignees.is_empty() { String::new() } else { format!(" · {}", it.assignees.join(", ")) };
                resp.clone().on_hover_text_at_pointer(format!("{n}{}\n{}{lane}{who}", it.title, it.status));
                if resp.clicked() {
                    clicked = Some(a.id.clone());
                }
            }
        }
        clicked
    }
}
