//! The Runner skin's race of the day (#206), over the window: the course bar at the bottom (the
//! 8,000 m as one straight lane of 20 laps, the part run in orange, a hurdle at each 1,000 m
//! split, the finish and the runner), the toolbar's "N / 8,000 m" pill, the coach's toasts and
//! the Finisher dialog. The bar is painted on a layer of its own and never takes a click. The
//! race is `crate::course`; the tabs record what moved the runner (`Tab::strides`) and
//! `course_strides` counts it. Under the Ultra-trail skin (#207) the same race is 160 km: the bar
//! draws the mountain's profile, aligned under the Mountain view when it shows, the pill reads
//! km, the coach talks of climbs and checkpoints and the medal says 160 KM.

use super::*;
use crate::course::{self, Course, Race, Stride, LAP, LENGTH, SPLIT_EVERY};
use crate::trail;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Id, LayerId, Order, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui};

/// How long the bar shows a split once reached, in seconds.
const SPLIT_SECS: f32 = 30.0;
/// The runner's orange.
const ORANGE: Color32 = Color32::from_rgb(0xff, 0x7a, 0x1a);
const PEACH: Color32 = Color32::from_rgb(0xff, 0xb2, 0x7a);

/// What the coach says and in which colour. The Big eater skin's announcer (#212) has its
/// own: a bite, a combo, an Undo spat out, an indigestion and a stop on the menu; so do the
/// Gamer skin's achievements (#218): a commit, a push, a merge, a streak, a rewind, a boss
/// fight and a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Push,
    Merge,
    Undo,
    Cramp,
    Split,
    Bite,
    Combo,
    Spit,
    Sick,
    Stop,
    Achieve,
    Pushed,
    Joined,
    Streak,
    Rewind,
    Boss,
    LevelUp,
}

impl Tone {
    /// The announcer's tones, which show the cutlery rather than the runner.
    fn eater(self) -> bool {
        matches!(self, Tone::Bite | Tone::Combo | Tone::Spit | Tone::Sick | Tone::Stop)
    }

    /// The Gamer's tones, which show the game pad (or the boss).
    fn player(self) -> bool {
        matches!(self, Tone::Achieve | Tone::Pushed | Tone::Joined | Tone::Streak | Tone::Rewind | Tone::Boss | Tone::LevelUp)
    }

    /// The icon tile's fill and the ink of its icon and metres.
    pub(super) fn colours(self) -> (Color32, Color32) {
        let light = theme::is_light();
        let (dark_ink, light_ink) = match self {
            Tone::Push | Tone::Split => (0xff8a33, 0xd35d0a),
            Tone::Merge => (0xc6f432, 0x4f8a00),
            Tone::Undo => (0x7fb8f0, 0x2f78c4),
            Tone::Cramp => (0xff7b72, 0xcf3d33),
            Tone::Bite => (0xffc93c, 0xa8770a),
            Tone::Combo => (0xff8a80, 0xd0352b),
            Tone::Spit => (0x7fb8f0, 0x2f78c4),
            Tone::Sick => (0x7ee787, 0x2f8a3f),
            Tone::Stop => (0xc3a6ff, 0x7c4dff),
            Tone::Achieve => (0xc6f432, 0x4f8a00),
            Tone::Pushed => (0x38bdf8, 0x0b7ab8),
            Tone::Joined => (0xffd66b, 0xa8770a),
            Tone::Streak => (0xff8a80, 0xd0352b),
            Tone::Rewind => (0xc9ccd4, 0x6b6f78),
            Tone::Boss => (0xff8a80, 0xcf3d33),
            Tone::LevelUp => (0xc3a6ff, 0x7c4dff),
        };
        let hex = |h: u32| Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8);
        let ink = hex(if light { light_ink } else { dark_ink });
        (theme::tint(ink, theme::c().elevated, if light { 0.14 } else { 0.2 }), ink)
    }
}

/// A toast in the coach's words.
#[derive(Clone, Debug, PartialEq)]
pub struct Coach {
    pub title: String,
    /// "+300 m", "−100 m", "paused".
    pub gain: String,
    pub tone: Tone,
    /// The Ultra-trail's coach: a mountain rather than the runner on its tile.
    pub mountain: bool,
}

/// The race of this launch, and what the bar and the dialog show of it.
pub struct CourseState {
    pub course: Course,
    started: Instant,
    /// The time at the keyboard a screenshot shows, instead of the time since launch.
    pub sample_secs: Option<u64>,
    /// The last split reached, and when: the bar shows it a while.
    pub split: Option<(u32, Instant)>,
    /// The Finisher dialog is open.
    pub finisher: bool,
}

impl Default for CourseState {
    fn default() -> Self {
        CourseState { course: Course::default(), started: Instant::now(), sample_secs: None, split: None, finisher: false }
    }
}

impl CourseState {
    /// Time at the keyboard since launch.
    fn secs(&self) -> u64 {
        self.sample_secs.unwrap_or_else(|| self.started.elapsed().as_secs())
    }

    /// The split the bar shows now, if one was reached lately.
    fn split_now(&self) -> Option<u32> {
        self.split.filter(|(_, at)| self.sample_secs.is_some() || at.elapsed().as_secs_f32() < SPLIT_SECS).map(|(m, _)| m)
    }

    /// FOURCHETTE_SKIN=runner[,split|,finish]: the race at 2,300 m, at the 3,000 m split, or
    /// finished with the Finisher dialog. FOURCHETTE_SKIN=trail[,split|,finish]: the ultra at
    /// 46 km, at the km 60 checkpoint, or finished at 160 km.
    pub fn sample(&mut self, v: &str) {
        let race = if v.starts_with("trail") { Race::Ultra } else { Race::Track };
        let (m, secs) = if v.contains("finish") {
            (8000, 9 * 3600 + 12 * 60)
        } else if v.contains("split") {
            (3000, 2 * 3600)
        } else {
            (2300, 3600 + 25 * 60)
        };
        self.course = Course::sample(race, m * race.scale() as u32);
        self.sample_secs = Some(secs);
        self.finisher = self.course.finished;
        if m == 3000 {
            self.split = Some((self.course.metres, Instant::now()));
        }
    }

    /// Runs `race` from now on: a different race starts again at 0 (Runner ↔ Ultra-trail).
    pub fn set_race(&mut self, race: Race) {
        if self.course.race != race {
            *self = CourseState { course: Course { race, ..Course::default() }, ..CourseState::default() };
        }
    }
}

fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(theme::MEDIUM.into()))
}

impl App {
    /// Counts what the tabs did since the last frame (`App::take_strides`), under the Runner skin, and lets the coach
    /// speak: a push, a merge, an Undo and a cramp have their toast, which replaces Git's; an
    /// split has its own; the finish opens the Finisher dialog.
    pub(super) fn course_strides(&mut self, strides: Vec<(Stride, String)>) {
        let race = self.race.course.race;
        let ultra = race == Race::Ultra;
        for (stride, msg) in strides {
            let moved = self.race.course.stride(&stride);
            let coach = match (&stride, ultra) {
                (Stride::Commit, _) => None,
                (Stride::Push, false) => Some(("Pace held.", Tone::Push)),
                (Stride::Push, true) => Some(("Pace held on the climb.", Tone::Push)),
                (Stride::Merge, false) => Some(("Strong finish down the straight.", Tone::Merge)),
                (Stride::Merge, true) => Some(("A big climb done.", Tone::Merge)),
                (Stride::Undo, false) => Some(("Turned back.", Tone::Undo)),
                (Stride::Undo, true) => Some(("Back down the trail.", Tone::Undo)),
                (Stride::Cramp { .. }, false) => Some(("Cramp?", Tone::Cramp)),
                (Stride::Cramp { .. }, true) => Some(("Cramp on the climb?", Tone::Cramp)),
            };
            let gain = match (&stride, moved.metres) {
                (Stride::Cramp { .. }, _) => "paused".to_string(),
                (_, m) if m < 0 => format!("−{}", race.distance(-m as u32)),
                (_, m) => format!("+{}", race.distance(m as u32)),
            };
            let body = match &stride {
                Stride::Undo => format!("Undo: {msg}"),
                _ => msg,
            };
            if let Some(mark) = moved.split {
                self.race.split = Some((mark, Instant::now()));
                let time = course::keyboard_time(self.race.secs());
                if ultra {
                    self.coach(&format!("Checkpoint · km {}", mark / 1000), format!("{time} at the keyboard. Refill your flasks."), gain, Tone::Split);
                } else {
                    self.coach(&format!("Split · {}", course::metres(mark)), format!("{time} at the keyboard. Drink some water."), gain, Tone::Split);
                }
            } else if let Some((title, tone)) = coach {
                self.coach(title, body, gain, tone);
            }
            if moved.finished {
                self.race.finisher = true;
            }
        }
    }

    /// The coach's toast of a screenshot: the split at 3,000 m, else a push.
    pub(super) fn coach_sample(&mut self) {
        if self.race.course.race == Race::Ultra {
            if self.race.split.is_some() {
                self.coach("Checkpoint · km 60", "2 h at the keyboard. Refill your flasks.".into(), "+6 km".into(), Tone::Split);
            } else if !self.race.course.finished {
                self.coach("Pace held on the climb.", "Pushed feat/52-linked-issue to origin.".into(), "+6 km".into(), Tone::Push);
            }
        } else if self.race.split.is_some() {
            self.coach("Split · 3,000 m", "2 h at the keyboard. Drink some water.".into(), "+300 m".into(), Tone::Split);
        } else if !self.race.course.finished {
            self.coach("Pace held.", "Pushed feat/52-linked-issue to origin.".into(), "+300 m".into(), Tone::Push);
        }
    }

    pub(super) fn coach(&mut self, title: &str, body: String, gain: String, tone: Tone) {
        let mountain = self.library.settings.skin == crate::settings::Skin::Trail;
        self.toast = Some(tab::Notice { msg: body, error: false, at: Instant::now(), undo: false, action: None, coach: Some(Coach { title: title.into(), gain, tone, mountain }) });
    }

    /// The bar floats centred at the bottom of the central view (between the sidebar and the details), just above
    /// the status bar, over everything but the dialogs and toasts (#215). Without a recorded central view, the window.
    pub(super) fn course_bar(&self, ctx: &egui::Context) {
        let area = self.centre.unwrap_or_else(|| ctx.content_rect());
        if self.race.course.race == Race::Ultra {
            self.ultra_bar(ctx, area);
            return;
        }
        let w = (area.width() - 32.0).min(1088.0);
        if w < 420.0 {
            return;
        }
        let rect = Rect::from_min_size(pos2(area.center().x - w / 2.0, area.bottom() - 12.0 - BAR_H), vec2(w, BAR_H));
        let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("course_bar")));
        let k = theme::c();
        let c = &self.race.course;
        p.add(crate::modal::card_back(rect, CornerRadius::same(12)));
        p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, k.border), StrokeKind::Inside);

        // Left: the day's race, the metres and the count of each action.
        let x = rect.left() + 16.0;
        p.text(pos2(x, rect.top() + 14.0), Align2::LEFT_TOP, "RACE OF THE DAY", medium(10.0), k.faint);
        let g = p.layout_no_wrap(course::thousands(c.metres), medium(22.0), k.strong);
        let mw = g.size().x;
        p.galley(pos2(x, rect.top() + 28.0), g, k.strong);
        p.text(pos2(x + mw + 5.0, rect.top() + 49.0), Align2::LEFT_BOTTOM, format!("of {}", course::metres(LENGTH)), FontId::proportional(13.0), k.dim);
        p.text(pos2(x, rect.top() + 58.0), Align2::LEFT_TOP, c.tally(), FontId::proportional(11.0), k.dim);

        // Right: the next split, or the one just reached, or the finish.
        const RIGHT_W: f32 = 170.0;
        let rx = rect.right() - 16.0 - RIGHT_W;
        let laps = LENGTH / LAP;
        let lines: [(String, FontId, Color32); 3] = if c.finished {
            [
                ("FINISHER".into(), medium(10.0), Tone::Merge.colours().1),
                (format!("{} in {}", course::metres(LENGTH), course::keyboard_time(self.race.secs())), FontId::proportional(12.0), k.text),
                ("at the keyboard.".into(), FontId::proportional(12.0), k.text),
            ]
        } else if let Some(mark) = self.race.split_now() {
            [
                (format!("SPLIT · {}", course::metres(mark).to_uppercase()), medium(10.0), Tone::Split.colours().1),
                (format!("{} at the keyboard.", course::keyboard_time(self.race.secs())), FontId::proportional(12.0), k.text),
                ("Drink some water.".into(), FontId::proportional(12.0), k.text),
            ]
        } else {
            let next = c.next_split();
            let name = if next == LENGTH { format!("{} (finish)", course::metres(LENGTH)) } else { course::metres(next) };
            [
                ("NEXT SPLIT".into(), medium(10.0), k.faint),
                (name, medium(14.0), k.strong),
                (format!("{} to go · lap {} of {laps}", course::metres(next - c.metres), c.lap()), FontId::proportional(11.0), k.dim),
            ]
        };
        let mut y = rect.top() + 18.0;
        for (i, (s, f, col)) in lines.into_iter().enumerate() {
            let h = f.size;
            p.text(pos2(rx, y), Align2::LEFT_TOP, s, f, col);
            y += h + if i == 0 { 7.0 } else { 5.0 };
        }

        // The lane between them.
        let lane = Rect::from_min_max(pos2(rect.left() + 16.0 + 168.0 + 16.0, rect.top() + 8.0), pos2(rx - 16.0, rect.bottom() - 8.0));
        lane_strip(&p, lane, c.metres);

        // At a split, a pill over the bar says so.
        if let (Some(mark), false) = (self.race.split_now(), c.finished) {
            let sx = lane.left() + mark as f32 / LENGTH as f32 * (lane.width() - 14.0);
            split_pill(&p, pos2(sx, rect.top() - 8.0), &format!("Split · {} in {}", course::metres(mark), course::keyboard_time(self.race.secs())));
        }
    }

    /// The ultra's bar (#207): the 160 km as the mountain's profile, run in orange, the aid
    /// station, the ridge and the summit marked, the runner at its km. When the Mountain view is
    /// on screen its km 0 and km 160 (`Tab::mountain_x`) set the profile's ends, so the bar's km
    /// stand right under the mountain's; otherwise it takes its own width.
    fn ultra_bar(&self, ctx: &egui::Context, area: Rect) {
        let own = {
            let w = (area.width() - 32.0).min(1088.0);
            if w < 420.0 {
                return;
            }
            let r = Rect::from_min_size(pos2(area.center().x - w / 2.0, area.bottom() - 12.0 - ULTRA_H), vec2(w, ULTRA_H));
            (r, r.left() + ULTRA_LEFT, r.right() - 24.0)
        };
        // Aligned, the left block shows only when there is room left of km 0; its km then go on
        // the top line.
        let aligned = self.tab().mountain_x.and_then(|(x0, x1)| {
            let left = if x0 - ULTRA_LEFT >= area.left() + 4.0 { x0 - ULTRA_LEFT } else { x0 - 14.0 };
            let r = Rect::from_min_max(pos2(left, area.bottom() - 12.0 - ULTRA_H), pos2(x1 + 24.0, area.bottom() - 12.0));
            (r.left() >= area.left() + 4.0 && r.right() <= area.right() - 4.0 && x1 - x0 > 240.0).then_some((r, x0, x1))
        });
        let (rect, x0, x1) = aligned.unwrap_or(own);
        let block = x0 - rect.left() >= ULTRA_LEFT - 1.0;
        let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("course_bar")));
        let k = theme::c();
        let light = theme::is_light();
        let c = &self.race.course;
        let km = c.metres as f32 / 1000.0;
        p.add(crate::modal::card_back(rect, CornerRadius::same(12)));
        p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, k.border), StrokeKind::Inside);

        // Left: the ultra, the km and the climbing so far.
        let x = rect.left() + 14.0;
        let dplus = course::thousands(trail::climb(km).round() as u32);
        if block {
            p.text(pos2(x, rect.top() + 14.0), Align2::LEFT_TOP, "ULTRA OF THE DAY", medium(9.0), k.faint);
            p.text(pos2(x, rect.top() + 28.0), Align2::LEFT_TOP, course::km(c.metres), medium(22.0), k.strong);
            p.text(pos2(x, rect.top() + 58.0), Align2::LEFT_TOP, format!("of 160 · D+ {dplus} m"), FontId::proportional(11.0), k.dim);
        }

        // The profile: grey ahead, orange behind, its ridge line over both.
        let (top, base) = (rect.top() + 26.0, rect.bottom() - 18.0);
        let bx = |k: f32| x0 + k / trail::LENGTH_KM * (x1 - x0);
        let by = |k: f32| base - trail::profile(k) * (base - top);
        let rest = if light { Color32::from_rgb(0xe3, 0xe4, 0xe8) } else { Color32::from_rgb(0x2a, 0x2c, 0x34) };
        let line = if light { Color32::from_rgb(0xa2, 0xa5, 0xae) } else { Color32::from_rgb(0x5c, 0x5f, 0x69) };
        let passed = if light { Tone::Split.colours().1 } else { PEACH };
        let steps = ((x1 - x0) / 3.0).clamp(80.0, 400.0) as usize;
        let mut ridge = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let k = trail::LENGTH_KM * i as f32 / steps as f32;
            ridge.push(pos2(bx(k), by(k)));
        }
        // One mesh of thin vertical slices, so the concave ridge fills right without seams.
        let mut mesh = egui::epaint::Mesh::default();
        for w in ridge.windows(2) {
            let mid = trail::LENGTH_KM * ((w[0].x + w[1].x) / 2.0 - x0) / (x1 - x0);
            let fill = if mid <= km { ORANGE.gamma_multiply(0.85) } else { rest };
            let i = mesh.vertices.len() as u32;
            for q in [w[0], w[1], pos2(w[1].x, base), pos2(w[0].x, base)] {
                mesh.colored_vertex(q, fill);
            }
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        }
        p.add(Shape::mesh(mesh));
        p.add(Shape::line(ridge, Stroke::new(1.2, line)));
        p.hline(x0..=x1, base, Stroke::new(1.0, line));
        // A tick at each checkpoint.
        for mark in (1..8).map(|i| i as f32 * 20.0) {
            p.vline(bx(mark), (base - 3.0)..=base, Stroke::new(1.0, if mark <= km { passed } else { line }));
        }
        // The places: start, aid station, ridge, summit.
        for (mk, label, align) in [(0.0, "Start", Align2::LEFT_TOP), (trail::AID_KM, "Aid · 92", Align2::CENTER_TOP), (130.0, "Ridge", Align2::CENTER_TOP), (trail::LENGTH_KM, "Summit", Align2::RIGHT_TOP)] {
            let col = if mk <= km { passed } else { line };
            let at = pos2(bx(mk), by(mk));
            p.extend(Shape::dashed_line(&[at, pos2(at.x, base)], Stroke::new(1.0, col), 2.0, 2.0));
            p.circle_filled(at, 3.5, col);
            let tx = match align {
                Align2::LEFT_TOP => at.x - 2.0,
                Align2::RIGHT_TOP => at.x + 2.0,
                _ => at.x,
            };
            p.text(pos2(tx, base + 3.0), align, label, medium(9.0), k.dim);
        }
        // The runner.
        let (rx, ry) = (bx(km), by(km));
        p.vline(rx, (rect.top() + 24.0)..=base, Stroke::new(1.5, PEACH));
        p.circle(pos2(rx, ry), 6.0, ORANGE, Stroke::new(2.0, k.elevated));

        // Over the profile's low start: what comes next.
        let time = course::keyboard_time(self.race.secs());
        let (head, rest_text) = if c.finished {
            ("Finisher".to_string(), format!(" · 160 km in {time} at the keyboard"))
        } else if let Some(mark) = self.race.split_now() {
            (format!("Checkpoint · km {}", mark / 1000), format!(" · {time} at the keyboard. Refill your flasks."))
        } else {
            let (at, name) = [(trail::AID_KM, "aid station"), (130.0, "the ridge"), (trail::LENGTH_KM, "summit")].into_iter().find(|(m, _)| *m > km).unwrap_or((trail::LENGTH_KM, "summit"));
            (format!("{name}, km {}", at as u32), format!(" · {} km to go", (at - km).ceil() as u32))
        };
        let mut job = egui::text::LayoutJob::default();
        let fmt = |font: FontId, color: Color32| egui::TextFormat { font_id: font, color, ..Default::default() };
        if !block {
            job.append(&format!("{} · D+ {dplus} m · ", course::km(c.metres)), 0.0, fmt(medium(11.0), Tone::Push.colours().1));
        }
        if !c.finished && self.race.split_now().is_none() {
            job.append("Next: ", 0.0, fmt(FontId::proportional(11.0), k.dim));
        }
        job.append(&head, 0.0, fmt(medium(11.0), k.strong));
        job.append(&rest_text, 0.0, fmt(FontId::proportional(11.0), k.dim));
        p.galley(pos2(x0 + 8.0, rect.top() + 8.0), p.fonts_mut(|f| f.layout_job(job)), k.dim);

        // At a checkpoint, a pill over the bar says so.
        if let (Some(mark), false) = (self.race.split_now(), c.finished) {
            split_pill(&p, pos2(bx(mark as f32 / 1000.0), rect.top() - 8.0), &format!("Checkpoint · km {} in {time}", mark / 1000));
        }
    }

    /// The Finisher dialog at 8,000 m: the medal, the day's counts, Run it again and Recover.
    pub(super) fn finisher_dialog(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        use crate::modal::{text, Modal, Width};
        if !self.race.finisher {
            return;
        }
        let k = theme::c();
        let c = self.race.course.clone();
        let time = course::keyboard_time(self.race.secs());
        let ultra = c.race == Race::Ultra;
        let status = if ultra { format!("Ultra of the day · {}", course::km(c.length())) } else { format!("Race of the day · {}", course::metres(LENGTH)) };
        let out = Modal::new("finisher", Width::Medium, "medal", vec![text("Finisher")])
            .status(vec![text(status)])
            .cancel(None)
            .extra("Run it again")
            .primary("Recover", false, true)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    let (r, _) = ui.allocate_exact_size(vec2(120.0, 156.0), Sense::hover());
                    medal(ui.painter(), r, c.race);
                    ui.add_space(10.0);
                    let line = if ultra {
                        let dplus = course::thousands(trail::climb(trail::LENGTH_KM).round() as u32);
                        format!("{} today, D+ {dplus} m: {}, {time} at the keyboard.", course::km(c.length()), c.tally().replace(" ·", ","))
                    } else {
                        format!("{} today, {} laps: {}, {time} at the keyboard.", course::metres(LENGTH), LENGTH / LAP, c.tally().replace(" ·", ","))
                    };
                    ui.add(egui::Label::new(egui::RichText::new(line).color(k.dim)).wrap());
                    ui.add_space(12.0);
                });
                let gap = 10.0;
                let w = (ui.available_width() - 2.0 * gap) / 3.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (n, what) in [(c.commits, "commits"), (c.pushes, "pushes"), (c.merges, "merges")] {
                        tile(ui, w, n, what);
                    }
                });
            });
        if out.extra == Some(0) {
            cmds.push(Cmd::Finisher { again: true });
        } else if out.primary || out.close {
            cmds.push(Cmd::Finisher { again: false });
        }
    }

    /// The Finisher dialog's buttons: Run it again starts the course over, Recover only closes it.
    pub(super) fn finisher_done(&mut self, again: bool) {
        self.race.finisher = false;
        if again {
            self.race.course.restart();
            self.race.split = None;
            self.race.started = Instant::now();
            self.race.sample_secs = None;
        }
    }
}

/// The bar's height.
const BAR_H: f32 = 88.0;
/// The ultra's bar: taller, for the profile, and the width of its left block.
const ULTRA_H: f32 = 92.0;
const ULTRA_LEFT: f32 = 124.0;

/// One of the Finisher dialog's counts.
fn tile(ui: &mut Ui, w: f32, n: u32, what: &str) {
    let k = theme::c();
    let (r, _) = ui.allocate_exact_size(vec2(w, 56.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(10), k.sidebar);
    let ink = if theme::is_light() { Tone::Push.colours().1 } else { PEACH };
    p.text(pos2(r.left() + 12.0, r.top() + 9.0), Align2::LEFT_TOP, n.to_string(), medium(18.0), ink);
    p.text(pos2(r.left() + 12.0, r.bottom() - 9.0), Align2::LEFT_BOTTOM, what, FontId::proportional(11.0), k.faint);
}

/// The race as one straight lane in `r`: grey ahead, orange behind the runner, a tick at each
/// lap, a hurdle at each split with its distance under it, the start and the chequered finish.
fn lane_strip(p: &Painter, r: Rect, run: u32) {
    let k = theme::c();
    let light = theme::is_light();
    let x = |m: u32| r.left() + m as f32 / LENGTH as f32 * (r.width() - 14.0);
    let lane = Rect::from_min_max(pos2(r.left(), r.center().y - 4.0), pos2(x(LENGTH), r.center().y + 12.0));
    let rest = if light { Color32::from_rgb(0xe3, 0xe4, 0xe8) } else { Color32::from_rgb(0x2a, 0x2c, 0x34) };
    let line = if light { Color32::from_rgb(0xa2, 0xa5, 0xae) } else { Color32::from_rgb(0x5c, 0x5f, 0x69) };
    p.rect_filled(lane, CornerRadius::same(3), rest);
    p.rect_filled(Rect::from_min_max(lane.min, pos2(x(run), lane.bottom())), CornerRadius::same(3), ORANGE.gamma_multiply(0.85));
    // The lane's lines, and a tick at each lap.
    for y in [lane.top(), lane.bottom()] {
        p.hline(lane.x_range(), y, Stroke::new(1.0, line));
    }
    for lap in 1..LENGTH / LAP {
        let lx = x(lap * LAP);
        p.vline(lx, (lane.top() + 4.0)..=(lane.bottom() - 4.0), Stroke::new(1.0, Color32::from_white_alpha(if light { 200 } else { 40 })));
    }
    // The start line.
    p.vline(lane.left() + 1.0, lane.y_range(), Stroke::new(2.0, if light { line } else { Color32::WHITE }));
    // A hurdle at each split, its distance under the lane.
    for mark in (SPLIT_EVERY..LENGTH).step_by(SPLIT_EVERY as usize) {
        let sx = x(mark);
        let passed = mark <= run;
        let col = if passed { if light { Tone::Split.colours().1 } else { PEACH } } else { line };
        hurdle_icon(p, pos2(sx, lane.top() - 1.0), col);
        p.text(pos2(sx, lane.bottom() + 3.0), Align2::CENTER_TOP, format!("{}k", mark / 1000), FontId::proportional(9.0), k.faint);
    }
    // The chequered finish.
    let fx = x(LENGTH);
    for i in 0..4 {
        for j in 0..2 {
            let dark = (i + j) % 2 == 1;
            let cell = Rect::from_min_size(pos2(fx + j as f32 * 4.0, lane.top() + i as f32 * 4.0), vec2(4.0, 4.0));
            p.rect_filled(cell, 0.0, if dark { Color32::from_rgb(0x11, 0x12, 0x15) } else { Color32::WHITE });
        }
    }
    p.text(pos2(fx + 4.0, lane.bottom() + 3.0), Align2::CENTER_TOP, "8k", FontId::proportional(9.0), k.faint);
    // The runner on the lane, a line down to it from its icon.
    let rx = x(run);
    runner_icon(p, pos2(rx, r.top() + 9.0), 16.0, if light { Tone::Push.colours().1 } else { PEACH });
    p.line_segment([pos2(rx, r.top() + 18.0), pos2(rx, lane.center().y)], Stroke::new(1.5, PEACH));
    p.circle(pos2(rx, lane.center().y), 5.5, ORANGE, Stroke::new(2.0, k.elevated));
}

/// A hurdle standing on `at`, 8 points tall.
fn hurdle_icon(p: &Painter, at: Pos2, col: Color32) {
    p.vline(at.x - 5.0, (at.y - 8.0)..=at.y, Stroke::new(1.2, col));
    p.vline(at.x + 5.0, (at.y - 8.0)..=at.y, Stroke::new(1.2, col));
    p.rect_filled(Rect::from_min_max(pos2(at.x - 6.0, at.y - 9.0), pos2(at.x + 6.0, at.y - 6.5)), 0.0, col);
}

/// "Split · 3,000 m in 2 h" (or the ultra's checkpoint), centred above `at`.
fn split_pill(p: &Painter, at: Pos2, text: &str) {
    let light = theme::is_light();
    let (fill, edge, ink) = if light {
        (Color32::from_rgb(0xff, 0xee, 0xe0), Color32::from_rgb(0xf3, 0xb0, 0x80), Tone::Split.colours().1)
    } else {
        (Color32::from_rgb(0x3a, 0x21, 0x0c), Color32::from_rgb(0x7a, 0x41, 0x16), PEACH)
    };
    let g = p.layout_no_wrap(text.to_string(), medium(11.0), ink);
    let size = vec2(g.size().x + 34.0, 24.0);
    let r = Rect::from_center_size(at - vec2(0.0, size.y / 2.0), size);
    p.rect(r, CornerRadius::same(12), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    stopwatch(p, pos2(r.left() + 15.0, r.center().y), ink);
    p.galley(pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0), g, ink);
}

/// A stopwatch, about 14 points tall, centred on `c`.
fn stopwatch(p: &Painter, c: Pos2, col: Color32) {
    let st = Stroke::new(1.5, col);
    let o = c + vec2(0.0, 1.0);
    p.circle_stroke(o, 5.5, st);
    p.line_segment([o, o + vec2(2.2, -2.8)], st);
    p.line_segment([o + vec2(-1.8, -8.2), o + vec2(1.8, -8.2)], st);
    p.line_segment([o + vec2(0.0, -8.2), o + vec2(0.0, -5.5)], st);
}

/// A mountain with a flag on its summit, `size` points wide, centred on `c`: the mockup's
/// 16-by-14-unit icon.
pub(super) fn mountain_icon(p: &Painter, c: Pos2, size: f32, col: Color32) {
    let s = size / 16.0;
    let q = |x: f32, y: f32| c + vec2((x - 8.0) * s, (y - 7.0) * s);
    let st = Stroke::new(1.5 * s.max(0.8), col);
    p.add(Shape::closed_line(vec![q(1.0, 13.0), q(6.0, 4.0), q(8.5, 8.0), q(10.5, 5.0), q(15.0, 13.0)], st));
    p.vline(q(6.0, 0.0).x, q(6.0, 1.0).y..=q(6.0, 4.0).y, Stroke::new(1.2 * s.max(0.8), col));
    p.add(Shape::convex_polygon(vec![q(6.0, 1.0), q(8.8, 2.0), q(6.0, 3.0)], col, Stroke::NONE));
}

/// A runner mid-stride, `size` points tall, centred on `c`: the mockup's 14-unit icon.
pub(super) fn runner_icon(p: &Painter, c: Pos2, size: f32, col: Color32) {
    let s = size / 14.0;
    let q = |x: f32, y: f32| c + vec2((x - 7.0) * s, (y - 7.5) * s);
    p.circle_filled(q(8.5, 2.5), 1.6 * s, col);
    let st = Stroke::new(1.5 * s.max(0.8), col);
    for line in [&[(7.5, 5.0), (5.5, 8.5), (8.0, 9.5), (7.0, 13.0)][..], &[(5.5, 8.5), (3.0, 7.5)], &[(7.5, 5.0), (10.5, 7.0), (12.0, 6.0)]] {
        let pts: Vec<Pos2> = line.iter().map(|&(x, y)| q(x, y)).collect();
        for w in pts.windows(2) {
            p.line_segment([w[0], w[1]], st);
        }
        for pt in pts {
            p.circle_filled(pt, st.width / 2.0, col);
        }
    }
}

/// The finisher medal in `r` (64 by 84 units scaled to fit): two ribbons and a gold disc, with
/// the race's distance.
fn medal(p: &Painter, r: Rect, race: Race) {
    let s = (r.width() / 64.0).min(r.height() / 84.0);
    let o = r.center() - vec2(32.0, 42.0) * s;
    let q = |x: f32, y: f32| o + vec2(x, y) * s;
    p.add(Shape::convex_polygon(vec![q(18.0, 0.0), q(30.0, 34.0), q(22.0, 36.0), q(8.0, 0.0)], Color32::from_rgb(0x3b, 0x82, 0xc4), Stroke::NONE));
    p.add(Shape::convex_polygon(vec![q(46.0, 0.0), q(56.0, 0.0), q(42.0, 36.0), q(34.0, 34.0)], ORANGE, Stroke::NONE));
    p.circle_filled(q(32.0, 56.0), 24.0 * s, Color32::from_rgb(0xe8, 0xb9, 0x3a));
    p.circle_stroke(q(32.0, 56.0), 18.0 * s, Stroke::new(2.0 * s, Color32::from_rgb(0xb9, 0x8a, 0x14)));
    let ink = Color32::from_rgb(0x5a, 0x3f, 0x00);
    let (n, unit) = match race {
        Race::Track => (LENGTH.to_string(), "METRES"),
        Race::Ultra => ((race.length() / 1000).to_string(), "KM"),
    };
    p.text(q(32.0, 51.0), Align2::CENTER_CENTER, n, medium(11.0 * s), ink);
    p.text(q(32.0, 64.5), Align2::CENTER_CENTER, unit, medium(5.5 * s), ink);
}

/// The toolbar's "2,300 / 8,000 m" pill: the runner and the metres, in orange; on the ultra,
/// "46 / 160 km" by a mountain. It only shows.
pub(super) fn metres_pill(ui: &mut Ui, metres: u32, race: Race) {
    let light = theme::is_light();
    let k = theme::c();
    let ultra = race == Race::Ultra;
    let text = if ultra {
        format!("{} / {}", metres / 1000, course::km(race.length()))
    } else {
        format!("{} / {}", course::thousands(metres), course::metres(LENGTH))
    };
    let ink = if light { Tone::Push.colours().1 } else { PEACH };
    let g = ui.painter().layout_no_wrap(text, medium(12.0), ink);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 42.0, 28.0), Sense::hover());
    let p = ui.painter();
    let (fill, edge) = if light {
        (theme::tint(ORANGE, k.sidebar, 0.12), theme::tint(ORANGE, k.sidebar, 0.4))
    } else {
        (Color32::from_rgb(0x2a, 0x1a, 0x0c), Color32::from_rgb(0x5c, 0x34, 0x12))
    };
    p.rect(rect, CornerRadius::same(14), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    if ultra {
        mountain_icon(p, pos2(rect.left() + 18.0, rect.center().y), 16.0, Tone::Push.colours().1);
    } else {
        runner_icon(p, pos2(rect.left() + 18.0, rect.center().y), 14.0, Tone::Push.colours().1);
    }
    p.galley(pos2(rect.left() + 30.0, rect.center().y - g.size().y / 2.0), g, ink);
    resp.on_hover_text(if ultra {
        "Ultra of the day, 160 km up the mountain: a commit +2 km, a push +6, a merge +10, Undo −2"
    } else {
        "Race of the day, 20 laps: a commit +100 m, a push +300, a merge +500, Undo −100"
    });
}

/// The coach's toast, under the toolbar on the right: the icon tile, the title, Git's message
/// and the metres. Clicks go through it.
pub(super) fn coach_toast(ctx: &egui::Context, coach: &Coach, msg: &str, age: f64) {
    let k = theme::c();
    let t = crate::modal::eased(age * 1.4);
    if t < 1.0 {
        ctx.request_repaint();
    }
    egui::Area::new(Id::unique("coach_toast")).order(Order::Tooltip).interactable(false).anchor(Align2::RIGHT_TOP, vec2(-20.0, 112.0 - 12.0 * (1.0 - t))).show(ctx, |ui| {
        ui.multiply_opacity(t);
        crate::modal::card(ui, 12, egui::Margin { left: 14, right: 14, top: 10, bottom: 10 }, |ui| {
            ui.set_width(308.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let (bg, ink) = coach.tone.colours();
                let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(8), bg);
                if coach.tone == Tone::Split {
                    stopwatch(ui.painter(), r.center(), ink);
                } else if coach.tone.eater() {
                    super::meal::tone_icon(ui.painter(), r.center(), coach.tone);
                } else if coach.tone.player() {
                    super::xp::tone_icon(ui.painter(), r.center(), coach.tone);
                } else if coach.mountain {
                    mountain_icon(ui.painter(), r.center(), 18.0, ink);
                } else {
                    runner_icon(ui.painter(), r.center(), 16.0, ink);
                }
                let gk = ui.painter().layout_no_wrap(coach.gain.clone(), medium(12.0), ink);
                let text_w = ui.available_width() - gk.size().x - 12.0;
                ui.allocate_ui_with_layout(vec2(text_w, 30.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(egui::RichText::new(&coach.title).font(medium(theme::FS_BODY)).color(k.strong));
                    ui.add(egui::Label::new(egui::RichText::new(msg).font(theme::secondary()).color(k.dim)).wrap());
                });
                let (r, _) = ui.allocate_exact_size(gk.size(), Sense::hover());
                ui.painter().galley(r.min, gk, ink);
            });
        });
    });
}
