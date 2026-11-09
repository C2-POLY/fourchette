//! The Big eater skin's daily menu (#212), over the window: the bar at the bottom of the central
//! view (the kcal of the day on a track from the starter to the Golden Fork, the fork
//! at the total, the last bites flying into it, the combo and its countdown), the toolbar's
//! "N kcal ×k" pill, the announcer's toasts and the Golden Fork card. The bar is painted on a
//! layer of its own and never takes a click. The menu is `crate::meal`; `App::take_strides`
//! hands it what the tabs did. Every icon is a drawing of `art`.

use super::course::Tone;
use super::art;
use super::*;
use crate::course::{self, Stride};
use crate::meal::{self, Meal, GOAL, STOPS};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Id, LayerId, Order, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui};

/// The bar's height.
const BAR_H: f32 = 88.0;
/// How long a bite takes to fly into the fork, in seconds.
const FLY_SECS: f32 = 0.6;
/// The last bites the bar shows behind the fork.
const RECENT: usize = 6;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The menu's mustard, and its gold.
const MUSTARD: Color32 = rgb(0xffc93c);
const GOLD: Color32 = rgb(0xffd66b);
const KETCHUP: Color32 = rgb(0xff5a4e);
const GREEN: Color32 = rgb(0x3fb950);

/// What a bite was, for its icon and its wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Food {
    Chips,
    Burger,
    Pizza,
}

impl Food {
    fn of(s: &Stride) -> Option<Food> {
        match s {
            Stride::Commit => Some(Food::Chips),
            Stride::Push => Some(Food::Burger),
            Stride::Merge => Some(Food::Pizza),
            _ => None,
        }
    }

    /// Its drawing.
    fn art(self) -> &'static [art::Part] {
        match self {
            Food::Chips => art::CHIPS,
            Food::Burger => art::BURGER,
            Food::Pizza => art::PIZZA,
        }
    }
}

/// The menu of this launch, and what the bar and the card show of it.
pub struct MealState {
    pub meal: Meal,
    started: Instant,
    /// The time at the keyboard a screenshot shows, instead of the time since launch.
    pub sample_secs: Option<u64>,
    /// The last bites, the latest first, and when they were taken.
    recent: Vec<(Food, Instant)>,
    /// The Golden Fork card is open.
    pub golden: bool,
}

impl Default for MealState {
    fn default() -> Self {
        MealState { meal: Meal::default(), started: Instant::now(), sample_secs: None, recent: Vec::new(), golden: false }
    }
}

impl MealState {
    /// Time at the keyboard since launch.
    pub fn secs(&self) -> u64 {
        self.sample_secs.unwrap_or_else(|| self.started.elapsed().as_secs())
    }

    /// FOURCHETTE_SKIN=eater: the menu at 1,240 kcal with a combo ×3 running; eater,golden: at
    /// 3,000 with the Golden Fork card.
    pub fn sample(&mut self, v: &str) {
        let golden = v.contains("golden");
        let secs = if golden { 8 * 3600 + 40 * 60 } else { 2 * 3600 + 5 * 60 };
        self.meal = Meal::sample(if golden { GOAL } else { 1240 }, secs);
        self.sample_secs = Some(secs);
        self.golden = golden;
        let now = Instant::now();
        self.recent = [Food::Burger, Food::Chips, Food::Chips, Food::Pizza, Food::Chips].iter().map(|f| (*f, now - std::time::Duration::from_secs(5))).collect();
    }
}

fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(theme::MEDIUM.into()))
}

/// A colour for dark mode, or its darker twin on light.
fn ink(dark: Color32, light: Color32) -> Color32 {
    if theme::is_light() { light } else { dark }
}

/// The title and the line of the stop at `STOPS[i]`.
fn stop_words(i: usize) -> (&'static str, &'static str) {
    match i {
        0 => ("Starter served!", "warming up the jaw."),
        1 => ("Main course!", "the floor is getting sticky."),
        2 => ("Cheese board!", "it smells like a dorm in here."),
        _ => ("Dessert time!", "one more push to the Golden Fork."),
    }
}

impl App {
    /// Counts what the tabs did since the last frame, under the Big eater skin, and lets the
    /// announcer speak: each bite, combo, Undo and indigestion has its toast, which replaces
    /// Git's; a stop has its own; the Golden Fork opens its card.
    pub(super) fn meal_bites(&mut self, strides: Vec<(Stride, String)>) {
        for (stride, msg) in strides {
            let secs = self.meal.secs();
            let b = self.meal.meal.bite(&stride, secs);
            if let Some(food) = Food::of(&stride) {
                self.meal.recent.insert(0, (food, Instant::now()));
                self.meal.recent.truncate(RECENT);
            }
            let gain = match (&stride, b.kcal) {
                (Stride::Cramp { .. }, _) => "paused".to_string(),
                (_, k) if k < 0 => format!("−{} kcal", -k),
                (_, k) => format!("+{} kcal", course::thousands(k as u32)),
            };
            if b.golden {
                self.meal.golden = true;
            }
            let (title, body, tone) = match &stride {
                _ if b.stop.is_some() => {
                    let i = b.stop.unwrap_or(0);
                    let (title, line) = stop_words(i);
                    self.coach(title, format!("{} kcal: {line}", course::thousands(STOPS[i].0)), STOPS[i].1.into(), Tone::Stop);
                    continue;
                }
                Stride::Undo => ("Spat it out.".to_string(), format!("Undo: {msg}"), Tone::Spit),
                Stride::Cramp { op, conflicts } => {
                    let s = if *conflicts == 1 { "" } else { "s" };
                    ("Indigestion.".to_string(), format!("The {op} stopped on {conflicts} conflict{s}. Nothing counts until the next bite."), Tone::Sick)
                }
                _ if b.combo >= 2 => (format!("COMBO ×{}!", b.combo), msg, Tone::Combo),
                _ if b.cured => ("Digested.".to_string(), msg, Tone::Bite),
                Stride::Commit => ("Crunch.".to_string(), msg, Tone::Bite),
                Stride::Push => ("Big bite.".to_string(), msg, Tone::Bite),
                Stride::Merge => ("Pizza party.".to_string(), msg, Tone::Bite),
            };
            self.coach(&title, body, gain, tone);
        }
    }

    /// The announcer's toast of a screenshot: a combo ×3 on a push.
    pub(super) fn meal_sample_toast(&mut self) {
        if !self.meal.golden {
            self.coach("COMBO ×3!", "Pushed feat/52-linked-issue to origin.".into(), "+750 kcal".into(), Tone::Combo);
        }
    }

    /// The bar floating centred at the bottom of the central view (between the sidebar and the
    /// details), where the Runner's course bar sits, over everything but the dialogs and toasts.
    pub(super) fn meal_bar(&self, ctx: &egui::Context) {
        let Some(rect) = bar_rect(self.centre.unwrap_or_else(|| ctx.content_rect())) else { return };
        let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("meal_bar")));
        let k = theme::c();
        let m = &self.meal.meal;
        let secs = self.meal.secs();
        let sick = m.sick();
        p.add(crate::modal::card_back(rect, CornerRadius::same(12)));
        let edge = if sick { ink(rgb(0x2c6b3f), rgb(0x9fd8ab)) } else if m.golden { ink(rgb(0x6b5412), rgb(0xe8c96a)) } else { k.border };
        p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, edge), StrokeKind::Inside);

        // Left: the day's menu, the kcal and the count of each bite.
        let x = rect.left() + 16.0;
        p.text(pos2(x, rect.top() + 14.0), Align2::LEFT_TOP, "DAILY MENU", medium(10.0), k.faint);
        let g = p.layout_no_wrap(course::thousands(m.kcal), medium(22.0), k.strong);
        let mw = g.size().x;
        p.galley(pos2(x, rect.top() + 28.0), g, k.strong);
        p.text(pos2(x + mw + 5.0, rect.top() + 49.0), Align2::LEFT_BOTTOM, format!("of {} kcal", course::thousands(GOAL)), FontId::proportional(13.0), k.dim);
        p.text(pos2(x, rect.top() + 58.0), Align2::LEFT_TOP, m.tally(), FontId::proportional(11.0), k.dim);

        // Right: the Golden Fork, the indigestion, the combo running, or the next stop.
        const RIGHT_W: f32 = 170.0;
        let rx = rect.right() - 16.0 - RIGHT_W;
        let combo = m.combo(secs);
        let mut y = rect.top() + 16.0;
        let line = |y: &mut f32, s: String, f: FontId, col: Color32, gap: f32| {
            let h = f.size;
            crate::ui::fit_text(&p, pos2(rx, *y), Align2::LEFT_TOP, &s, f, col, RIGHT_W);
            *y += h + gap;
        };
        if m.golden {
            line(&mut y, "GOLDEN FORK".into(), medium(10.0), ink(GOLD, rgb(0xa8770a)), 7.0);
            line(&mut y, format!("{} kcal in {}", course::thousands(m.kcal), course::keyboard_time(secs)), FontId::proportional(12.0), k.text, 5.0);
            line(&mut y, "at the keyboard. Burp.".into(), FontId::proportional(12.0), k.text, 0.0);
        } else if sick {
            line(&mut y, "INDIGESTION".into(), medium(10.0), ink(rgb(0x7ee787), rgb(0x2f8a3f)), 7.0);
            line(&mut y, "Stopped on conflicts.".into(), FontId::proportional(12.0), k.text, 5.0);
            line(&mut y, "Nothing counts until digested.".into(), FontId::proportional(11.0), k.dim, 0.0);
        } else if let (true, Some(left)) = (combo >= 2, m.combo_left(secs)) {
            line(&mut y, format!("COMBO ×{combo}"), medium(10.0), ink(rgb(0xff8a80), rgb(0xd0352b)), 6.0);
            let next = m.next_stop().map_or("Golden fork", |(_, n)| n);
            line(&mut y, format!("Next: {next}"), medium(14.0), k.strong, 6.0);
            let track = Rect::from_min_size(pos2(rx, y), vec2(RIGHT_W - 10.0, 5.0));
            p.rect_filled(track, CornerRadius::same(3), ink(rgb(0x2a2c34), rgb(0xe3e4e8)));
            let f = left as f32 / meal::COMBO_SECS as f32;
            p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * f, 5.0)), CornerRadius::same(3), KETCHUP);
            y += 10.0;
            line(&mut y, format!("{left} s for another ×{}", m.streak(secs)), FontId::proportional(11.0), k.dim, 0.0);
        } else {
            line(&mut y, "NEXT STOP".into(), medium(10.0), k.faint, 7.0);
            let (at, name) = m.next_stop().unwrap_or(STOPS[STOPS.len() - 1]);
            line(&mut y, format!("{name} · {}", course::thousands(at)), medium(14.0), k.strong, 5.0);
            line(&mut y, format!("{} kcal to go", course::thousands(at - m.kcal.min(at))), FontId::proportional(11.0), k.dim, 0.0);
        }

        // The track between them.
        let lane = Rect::from_min_max(pos2(rect.left() + 16.0 + 168.0 + 16.0, rect.top() + 6.0), pos2(rx - 18.0, rect.bottom() - 6.0));
        self.meal_track(&p, lane, sick);

        // While sick, a pill over the bar says the menu waits.
        if sick {
            sick_pill(&p, pos2(lane.center().x, rect.top() - 8.0));
        }
    }

    /// The kcal track in `r`: the stops, the part eaten in mustard (green while sick), the
    /// fork at the total and the last bites flying into it.
    fn meal_track(&self, p: &Painter, r: Rect, sick: bool) {
        let k = theme::c();
        let m = &self.meal.meal;
        let x = |kcal: u32| r.left() + 8.0 + (kcal.min(GOAL) as f32 / GOAL as f32) * (r.width() - 16.0);
        let cy = r.top() + 38.0;
        let track = Rect::from_min_max(pos2(r.left(), cy - 6.0), pos2(r.right(), cy + 6.0));
        let fill = if sick { GREEN } else { MUSTARD };
        p.rect_filled(track, CornerRadius::same(6), ink(rgb(0x2a2c34), rgb(0xe3e4e8)));
        if m.kcal > 0 {
            p.rect_filled(Rect::from_min_max(track.min, pos2(x(m.kcal).max(track.left() + 12.0), track.bottom())), CornerRadius::same(6), fill);
        }
        // Ketchup and mustard dots along the part still to eat.
        let mut dx = x(m.kcal) + 14.0;
        while dx < track.right() - 8.0 {
            p.circle_filled(pos2(dx, cy), 1.4, ink(rgb(0x3a3d45), rgb(0xc9ccd4)));
            dx += 14.0;
        }
        for (i, (kcal, name)) in STOPS.iter().enumerate() {
            let sx = x(*kcal);
            let on = *kcal <= m.kcal;
            let c = if on { fill } else { ink(rgb(0x3a3d45), rgb(0xc9ccd4)) };
            p.circle(pos2(sx, cy), 7.0, c, Stroke::new(2.0, k.elevated));
            if i == STOPS.len() - 1 {
                art::fork_small(p, pos2(sx, cy), 13.0, if on { art::GOLD } else { art::SILVER });
            }
            let label = if on { ink(GOLD, rgb(0xa8770a)) } else { k.faint };
            let align = if i == STOPS.len() - 1 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
            let lx = if i == STOPS.len() - 1 { sx + 8.0 } else { sx };
            p.text(pos2(lx, cy + 11.0), align, *name, medium(9.5), label);
            p.text(pos2(lx, cy + 24.0), align, course::thousands(*kcal), FontId::proportional(8.5), k.faint);
        }
        // The fork above the total, the last bites flying in behind it.
        let fx = x(m.kcal);
        let top = r.top() + 2.0;
        art::fork(p, pos2(fx, (top + cy - 8.0) / 2.0), cy - 8.0 - top, art::GOLD);
        for (i, (food, at)) in self.meal.recent.iter().enumerate() {
            let t = (at.elapsed().as_secs_f32() / FLY_SECS).min(1.0);
            let rest = pos2(fx - 26.0 - i as f32 * 22.0, top + 9.0 + (i % 2) as f32 * 5.0);
            if rest.x < r.left() + 8.0 {
                break;
            }
            // A new bite drops in from above, then settles in line.
            let from = rest - vec2(0.0, 30.0);
            let c = from.lerp(rest, crate::modal::eased(t as f64));
            let s = 1.0 - i as f32 * 0.1;
            art::food(p, c, 18.0 * s, food.art());
        }
    }

    /// The Golden Fork card at 3,000 kcal: the trophy, the day's bites, Second helping and Nap time.
    pub(super) fn golden_fork_dialog(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        use crate::modal::{text, Modal, Width};
        if !self.meal.golden {
            return;
        }
        let k = theme::c();
        let m = self.meal.meal.clone();
        let time = course::keyboard_time(self.meal.secs());
        let gold_tile = if theme::is_light() { theme::tint(MUSTARD, k.elevated, 0.2) } else { rgb(0x2e2508) };
        let out = Modal::new("golden_fork", Width::Medium, "medal", vec![text("Golden Fork")])
            .icon_art(gold_tile, |p, c| art::fork_small(p, c, 26.0, art::GOLD))
            .status(vec![text(format!("Daily menu · {} kcal", course::thousands(GOAL)))])
            .cancel(None)
            .extra("Second helping")
            .primary("Nap time", false, true)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    let (r, _) = ui.allocate_exact_size(vec2(128.0, 160.0), Sense::hover());
                    art::golden_fork(ui.painter(), r);
                    ui.add_space(10.0);
                    let n = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
                    let line = format!(
                        "{} kcal today in {time} at the keyboard: {}, {}, {}, best combo ×{}.",
                        course::thousands(m.kcal),
                        n(m.chips, "commit", "commits"),
                        n(m.burgers, "push", "pushes"),
                        n(m.pizzas, "merge", "merges"),
                        m.best.max(1)
                    );
                    ui.add(egui::Label::new(egui::RichText::new(line).color(k.dim)).wrap());
                    ui.add_space(12.0);
                });
                let gap = 10.0;
                let w = (ui.available_width() - 2.0 * gap) / 3.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (n, what, food) in [(m.chips, "chips", Food::Chips), (m.burgers, "burgers", Food::Burger), (m.pizzas, "pizzas", Food::Pizza)] {
                        tile(ui, w, n, what, food);
                    }
                });
            });
        if out.extra == Some(0) {
            cmds.push(Cmd::GoldenFork { again: true });
        } else if out.primary || out.close {
            cmds.push(Cmd::GoldenFork { again: false });
        }
    }

    /// The card's buttons: Second helping empties the plate, Nap time only closes it.
    pub(super) fn golden_fork_done(&mut self, again: bool) {
        self.meal.golden = false;
        if again {
            self.meal.meal.restart();
            self.meal.recent.clear();
            self.meal.started = Instant::now();
            self.meal.sample_secs = None;
        }
    }
}

/// Where the bar sits in `area`, the central view: centred at its bottom, as the Runner's course
/// bar (#215). None when it is too narrow.
pub(super) fn bar_rect(area: Rect) -> Option<Rect> {
    let w = (area.width() - 32.0).min(1088.0);
    (w >= 460.0).then(|| Rect::from_min_size(pos2(area.center().x - w / 2.0, area.bottom() - 12.0 - BAR_H), vec2(w, BAR_H)))
}

/// One of the card's counts, its food beside the number.
fn tile(ui: &mut Ui, w: f32, n: u32, what: &str, food: Food) {
    let k = theme::c();
    let (r, _) = ui.allocate_exact_size(vec2(w, 56.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(10), k.sidebar);
    p.text(pos2(r.left() + 12.0, r.top() + 9.0), Align2::LEFT_TOP, n.to_string(), medium(18.0), ink(GOLD, rgb(0xa8770a)));
    p.text(pos2(r.left() + 12.0, r.bottom() - 9.0), Align2::LEFT_BOTTOM, what, FontId::proportional(11.0), k.faint);
    art::food(p, pos2(r.right() - 24.0, r.center().y), 28.0, food.art());
}

/// "Paused while conflicts last", centred above `at`, with a queasy face.
fn sick_pill(p: &Painter, at: Pos2) {
    let (fill, edge, text) = if theme::is_light() { (rgb(0xe3f5e6), rgb(0x9fd8ab), rgb(0x2f8a3f)) } else { (rgb(0x10291a), rgb(0x2c6b3f), rgb(0x7ee787)) };
    let g = p.layout_no_wrap("Paused while conflicts last".into(), medium(11.0), text);
    let size = vec2(g.size().x + 34.0, 24.0);
    let r = Rect::from_center_size(at - vec2(0.0, size.y / 2.0), size);
    p.rect(r, CornerRadius::same(12), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    let c = pos2(r.left() + 15.0, r.center().y);
    p.circle_filled(c, 6.5, text);
    let st = Stroke::new(1.3, fill);
    p.line_segment([c + vec2(-3.0, -1.5), c + vec2(-1.5, -1.5)], st);
    p.line_segment([c + vec2(1.5, -1.5), c + vec2(3.0, -1.5)], st);
    p.add(Shape::line(vec![c + vec2(-2.5, 3.0), c + vec2(-1.0, 1.8), c + vec2(1.0, 3.2), c + vec2(2.5, 2.2)], st));
    p.galley(pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0), g, text);
}

/// The icon of a toast's tone: a burger for a combo, else the cutlery.
pub(super) fn tone_icon(p: &Painter, c: Pos2, tone: Tone) {
    if tone == Tone::Combo {
        art::food(p, c, 22.0, art::BURGER);
    } else {
        art::cutlery(p, c, 20.0);
    }
}

/// The toolbar's "1,240 kcal ×3" pill: the fork, the kcal and the combo running. It only shows.
pub(super) fn kcal_pill(ui: &mut Ui, kcal: u32, combo: u32) {
    let light = theme::is_light();
    let k = theme::c();
    let text_ink = ink(GOLD, rgb(0x8a6200));
    let g = ui.painter().layout_no_wrap(format!("{} kcal", course::thousands(kcal)), medium(12.0), text_ink);
    let gc = (combo >= 2).then(|| ui.painter().layout_no_wrap(format!("×{combo}"), medium(12.0), ink(rgb(0xff8a80), rgb(0xd0352b))));
    let extra = gc.as_ref().map_or(0.0, |g| g.size().x + 6.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + extra + 42.0, 28.0), Sense::hover());
    let p = ui.painter();
    let (fill, edge) = if light { (theme::tint(MUSTARD, k.sidebar, 0.16), theme::tint(MUSTARD, k.sidebar, 0.55)) } else { (rgb(0x2e2508), rgb(0x6b5412)) };
    p.rect(rect, CornerRadius::same(14), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    art::fork_small(p, pos2(rect.left() + 16.0, rect.center().y), 17.0, art::GOLD);
    let gx = rect.left() + 30.0;
    let gw = g.size().x;
    p.galley(pos2(gx, rect.center().y - g.size().y / 2.0), g, text_ink);
    if let Some(gc) = gc {
        p.galley(pos2(gx + gw + 6.0, rect.center().y - gc.size().y / 2.0), gc, text_ink);
    }
    resp.on_hover_text("Daily menu, to the Golden Fork at 3,000 kcal: a commit +50, a push +250, a merge +400, ×2 then ×3 when they follow within a minute; Undo −50");
}
