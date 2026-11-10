//! The Gamer skin's session (#218), over the window: the XP bar at the bottom of the central view
//! (a segmented track from Noob to Legend, the gem at the XP, the last actions as orbs behind it,
//! the streak and its countdown, or the boss fight), the toolbar's "Lv 3 · 2,840 XP ×3" pill,
//! the achievement toasts and the GG card at Legend. The bar is painted on a layer of its own and
//! never takes a click. The session is `crate::xp`; `App::take_strides` hands it what the tabs
//! did, and the room (`app/room.rs`) shows it through `Look`. Every icon is a drawing of `art`.

use super::art::gamer::{self as g};
use super::art::{self};
use super::course::Tone;
use super::room::{Look, React};
use super::*;
use crate::course::{self, Stride};
use crate::xp::{self, Session, LEGEND, LEVELS};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Id, LayerId, Order, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};

/// How long an orb takes to drop in behind the gem, in seconds.
const DROP_SECS: f32 = 0.6;
/// The last actions the bar shows behind the gem.
const RECENT: usize = 6;
/// The track's segments.
const SEGMENTS: usize = 48;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const LIME: Color32 = rgb(0xc6f432);
const RED: Color32 = rgb(0xff4d6d);
const GOLD: Color32 = rgb(0xffd66b);

/// An action, as an orb behind the gem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Orb {
    Commit,
    Push,
    Merge,
    Undo,
}

impl Orb {
    fn of(s: &Stride) -> Option<Orb> {
        match s {
            Stride::Commit => Some(Orb::Commit),
            Stride::Push => Some(Orb::Push),
            Stride::Merge => Some(Orb::Merge),
            Stride::Undo => Some(Orb::Undo),
            Stride::Cramp { .. } => None,
        }
    }

    /// Its label, its ink and its fill.
    fn look(self) -> (&'static str, Color32, Color32) {
        let light = theme::is_light();
        match self {
            Orb::Commit => ("+100", if light { rgb(0x4f8a00) } else { LIME }, if light { rgb(0xeef9cf) } else { rgb(0x1e2808) }),
            Orb::Push => ("+300", if light { rgb(0x0b7ab8) } else { rgb(0x38bdf8) }, if light { rgb(0xdff2fc) } else { rgb(0x0b2233) }),
            Orb::Merge => ("+500", if light { rgb(0xa8770a) } else { GOLD }, if light { rgb(0xfdf2d2) } else { rgb(0x2e2508) }),
            Orb::Undo => ("−100", if light { rgb(0x6b6f78) } else { rgb(0xa2a5ae) }, if light { rgb(0xeceef1) } else { rgb(0x1f2026) }),
        }
    }
}

/// The session of this launch, and what the bar, the card and the room show of it.
pub struct XpState {
    pub session: Session,
    started: Instant,
    /// The time at the keyboard a screenshot shows, instead of the time since launch.
    pub sample_secs: Option<u64>,
    /// The last actions, the latest first, and when they were made.
    recent: Vec<(Orb, Instant)>,
    /// The GG card is open.
    pub gg: bool,
    /// The last action the gamer reacts to, when, and the XP that floats up.
    react: Option<(React, Instant, String)>,
    /// When the last level was reached: the room stays gold a while.
    level_at: Option<Instant>,
    /// When the room was last tidied.
    tidied_at: Option<Instant>,
    /// Touch grass: the screens are dimmed until the next action.
    dim: bool,
}

impl Default for XpState {
    fn default() -> Self {
        XpState { session: Session::default(), started: Instant::now(), sample_secs: None, recent: Vec::new(), gg: false, react: None, level_at: None, tidied_at: None, dim: false }
    }
}

impl XpState {
    /// Time at the keyboard since launch.
    pub fn secs(&self) -> u64 {
        self.sample_secs.unwrap_or_else(|| self.started.elapsed().as_secs())
    }

    /// What the room shows of the session now.
    pub(super) fn look(&self) -> Look {
        let s = &self.session;
        Look {
            mess: s.mess.clone(),
            boss: s.boss.is_some(),
            gold: self.level_at.is_some_and(|t| t.elapsed().as_secs_f32() < super::room::GOLD_SECS),
            react: self.react.as_ref().map(|(r, at, text)| (*r, at.elapsed().as_secs_f32(), text.clone())),
            level: s.level(),
            tidy: self.tidied_at.map(|t| t.elapsed().as_secs_f32()),
            dim: self.dim,
        }
    }

    /// FOURCHETTE_SKIN=player: the session at 2,840 XP with a streak ×3 running and a push just
    /// made; `player,boss` in a boss fight at 3,300; `player,gg` at Legend with the GG card;
    /// `player,mess` at 5,200 with a crowded room and a merge just made.
    pub fn sample(&mut self, v: &str) {
        let (xp, secs, boss, react) = if v.contains("gg") {
            (LEGEND, 7 * 3600 + 55 * 60, false, None)
        } else if v.contains("boss") {
            (3300, 3 * 3600, true, None)
        } else if v.contains("mess") {
            (5200, 6 * 3600, false, Some((React::Merge, "+1,500 XP")))
        } else {
            (2840, 2 * 3600 + 5 * 60, false, Some((React::Push, "+900 XP")))
        };
        self.session = Session::sample(xp, secs, boss);
        if v.contains("mess") {
            // A long session's mess, past the point where the piles grow.
            let more = [xp::Piece::Can, xp::Piece::Bag, xp::Piece::Can, xp::Piece::Pizza, xp::Piece::Can, xp::Piece::Can, xp::Piece::Pizza, xp::Piece::Can, xp::Piece::Bag, xp::Piece::Can, xp::Piece::Pizza, xp::Piece::Can];
            self.session.mess.extend(more);
        }
        self.sample_secs = Some(secs);
        self.gg = self.session.legend;
        let now = Instant::now();
        self.react = react.map(|(r, t)| (r, now, t.to_string()));
        let orbs = if boss { vec![Orb::Commit, Orb::Commit] } else { vec![Orb::Push, Orb::Commit, Orb::Commit, Orb::Undo, Orb::Commit] };
        self.recent = orbs.into_iter().map(|o| (o, now - std::time::Duration::from_secs(5))).collect();
    }
}

fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(theme::MEDIUM.into()))
}

/// A colour for dark mode, or its darker twin on light.
fn ink(dark: Color32, light: Color32) -> Color32 {
    if theme::is_light() { light } else { dark }
}

/// The lime of text, readable on light.
fn lime_ink() -> Color32 {
    ink(rgb(0xd9f99d), rgb(0x4f8a00))
}

/// The line under a level's toast.
fn level_words(i: usize) -> &'static str {
    match i {
        1 => "the room turns gold.",
        2 => "the cans start to pile up.",
        _ => "one more push to Legend.",
    }
}

impl App {
    /// Counts what the tabs did since the last frame, under the Gamer skin: each action has its
    /// toast, which replaces Git's, the gamer his reaction and the room its piece of mess; a
    /// level has its own toast and turns the room gold; Legend opens the GG card.
    pub(super) fn xp_plays(&mut self, strides: Vec<(Stride, String)>) {
        for (stride, msg) in strides {
            let secs = self.xp.secs();
            let s = self.xp.session.play(&stride, secs);
            self.xp.dim = false;
            if let Some(orb) = Orb::of(&stride) {
                self.xp.recent.insert(0, (orb, Instant::now()));
                self.xp.recent.truncate(RECENT);
            }
            let gain = match (&stride, s.xp) {
                (Stride::Cramp { .. }, _) => "paused".to_string(),
                (_, x) if x < 0 => format!("−{} XP", -x),
                (Stride::Undo, _) => "0 XP".to_string(),
                (_, x) => format!("+{} XP", course::thousands(x as u32)),
            };
            let react = match &stride {
                _ if s.beaten => Some(React::Beaten),
                Stride::Commit => Some(React::Commit),
                Stride::Push => Some(React::Push),
                Stride::Merge => Some(React::Merge),
                Stride::Undo => Some(React::Undo),
                Stride::Cramp { .. } => None,
            };
            self.xp.react = react.map(|r| (r, Instant::now(), gain.clone()));
            if s.legend {
                self.xp.gg = true;
            }
            if let Some(i) = s.level {
                self.xp.level_at = Some(Instant::now());
                self.xp.react = Some((React::Level, Instant::now(), gain.clone()));
                let (at, name) = LEVELS[i];
                self.coach(&format!("LEVEL UP · Lv {} {name}", i + 1), format!("{} XP: {}", course::thousands(at), level_words(i)), name.into(), Tone::LevelUp);
                continue;
            }
            let (title, body, tone) = match &stride {
                Stride::Undo => ("Rewind.".to_string(), format!("Undo: {msg}"), Tone::Rewind),
                Stride::Cramp { op, conflicts } => {
                    let n = if *conflicts == 1 { "1 conflict".to_string() } else { format!("{conflicts} conflicts") };
                    ("BOSS FIGHT!".to_string(), format!("The {op} stopped on {n}. XP waits until the next action beats it."), Tone::Boss)
                }
                _ if s.beaten => ("Boss beaten.".to_string(), msg, Tone::Achieve),
                _ if s.streak >= 2 => (format!("STREAK ×{}!", s.streak), msg, Tone::Streak),
                Stride::Commit if s.first => ("Achievement: First commit".to_string(), msg, Tone::Achieve),
                Stride::Push if s.first => ("Achievement: First push".to_string(), msg, Tone::Pushed),
                Stride::Merge if s.first => ("Achievement: First merge".to_string(), msg, Tone::Joined),
                Stride::Commit => ("Commit landed.".to_string(), msg, Tone::Achieve),
                Stride::Push => ("Pushed.".to_string(), msg, Tone::Pushed),
                Stride::Merge => ("Branches joined.".to_string(), msg, Tone::Joined),
            };
            self.coach(&title, body, gain, tone);
        }
    }

    /// The toast of a screenshot.
    pub(super) fn xp_sample_toast(&mut self, v: &str) {
        if self.xp.gg {
            return;
        }
        if v.contains("boss") {
            self.coach("BOSS FIGHT!", "The rebase stopped on 2 conflicts. XP waits until the next action beats it.".into(), "paused".into(), Tone::Boss);
        } else if v.contains("mess") {
            self.coach("STREAK ×3!", "Merged fix/55-login-timeout into main.".into(), "+1,500 XP".into(), Tone::Streak);
        } else {
            self.coach("STREAK ×3!", "Pushed feat/52-linked-issue to origin.".into(), "+900 XP".into(), Tone::Streak);
        }
    }

    /// The bar floating centred at the bottom of the central view, where the Big eater's menu
    /// sits, over everything but the dialogs and toasts.
    pub(super) fn xp_bar(&self, ctx: &egui::Context) {
        let Some(rect) = super::meal::bar_rect(self.centre.unwrap_or_else(|| ctx.content_rect())) else { return };
        let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("xp_bar")));
        let k = theme::c();
        let s = &self.xp.session;
        let secs = self.xp.secs();
        let boss = s.boss.as_ref();
        p.add(crate::modal::card_back(rect, CornerRadius::same(12)));
        let edge = if boss.is_some() { ink(rgb(0x7a2433), rgb(0xf0a3b0)) } else if s.legend { ink(rgb(0x6b5412), rgb(0xe8c96a)) } else { k.border };
        p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, edge), StrokeKind::Inside);

        // Left: the level, the XP and the count of each action.
        let x = rect.left() + 16.0;
        let lv = s.level();
        p.text(pos2(x, rect.top() + 14.0), Align2::LEFT_TOP, format!("SESSION · LV {} {}", lv + 1, LEVELS[lv].1.to_uppercase()), medium(10.0), k.faint);
        let gal = p.layout_no_wrap(course::thousands(s.xp), medium(22.0), k.strong);
        let xw = gal.size().x;
        p.galley(pos2(x, rect.top() + 28.0), gal, k.strong);
        p.text(pos2(x + xw + 5.0, rect.top() + 49.0), Align2::LEFT_BOTTOM, "XP", FontId::proportional(13.0), k.dim);
        p.text(pos2(x, rect.top() + 58.0), Align2::LEFT_TOP, s.tally(), FontId::proportional(11.0), k.dim);

        // Right: Legend, the boss fight, the streak running, or the next level.
        const RIGHT_W: f32 = 170.0;
        let rx = rect.right() - 16.0 - RIGHT_W;
        let streak = s.streak(secs);
        let mut y = rect.top() + 16.0;
        let line = |y: &mut f32, t: String, f: FontId, col: Color32, gap: f32| {
            let h = f.size;
            crate::ui::fit_text(&p, pos2(rx, *y), Align2::LEFT_TOP, &t, f, col, RIGHT_W);
            *y += h + gap;
        };
        if s.legend {
            line(&mut y, "LEGEND · GG".into(), medium(10.0), ink(GOLD, rgb(0xa8770a)), 7.0);
            line(&mut y, format!("{} XP in {}", course::thousands(s.xp), course::keyboard_time(secs)), medium(14.0), k.strong, 5.0);
            line(&mut y, "Time to touch some grass.".into(), FontId::proportional(11.0), k.dim, 0.0);
        } else if let Some(b) = boss {
            line(&mut y, format!("BOSS FIGHT · {}", b.op.to_uppercase()), medium(10.0), ink(rgb(0xff6b6b), rgb(0xcf3d33)), 7.0);
            let n = if b.conflicts == 1 { "1 conflict".to_string() } else { format!("{} conflicts", b.conflicts) };
            line(&mut y, format!("Stopped on {n}."), medium(14.0), k.strong, 5.0);
            line(&mut y, "XP waits until it is beaten.".into(), FontId::proportional(11.0), k.dim, 0.0);
        } else if let (true, Some(left)) = (streak >= 2, s.streak_left(secs)) {
            line(&mut y, format!("STREAK ×{streak}"), medium(10.0), ink(rgb(0xff8a80), rgb(0xd0352b)), 6.0);
            let next = s.next_level().map_or("Legend", |(_, n)| n);
            line(&mut y, format!("Next: {next}"), medium(14.0), k.strong, 6.0);
            let track = Rect::from_min_size(pos2(rx, y), vec2(RIGHT_W - 10.0, 5.0));
            p.rect_filled(track, CornerRadius::same(3), ink(rgb(0x2a2c34), rgb(0xe3e4e8)));
            let f = left as f32 / xp::STREAK_SECS as f32;
            p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * f, 5.0)), CornerRadius::same(3), rgb(0xff8a80));
            y += 10.0;
            let next = s.next_streak(secs);
            let keep = if next > streak { format!("{left} s for another ×{next}") } else { format!("{left} s to keep the streak") };
            line(&mut y, keep, FontId::proportional(11.0), k.dim, 0.0);
        } else {
            line(&mut y, "NEXT LEVEL".into(), medium(10.0), k.faint, 7.0);
            let (at, name) = s.next_level().unwrap_or(LEVELS[LEVELS.len() - 1]);
            line(&mut y, format!("{name} · {}", course::thousands(at)), medium(14.0), k.strong, 5.0);
            line(&mut y, format!("{} XP to go", course::thousands(at - s.xp.min(at))), FontId::proportional(11.0), k.dim, 0.0);
        }

        // The track between them.
        let lane = Rect::from_min_max(pos2(rect.left() + 16.0 + 176.0 + 16.0, rect.top() + 6.0), pos2(rx - 18.0, rect.bottom() - 6.0));
        self.xp_track(&p, lane);
        if boss.is_some() {
            boss_pill(&p, pos2(lane.center().x, rect.top() - 8.0));
        }
    }

    /// The segmented track in `r`: the levels, the XP lit in lime (red in a boss fight, gold at
    /// Legend), the gem at the XP and the last actions as orbs behind it.
    fn xp_track(&self, p: &Painter, r: Rect) {
        let s = &self.xp.session;
        let boss = s.boss.is_some();
        let x = |v: u32| r.left() + 8.0 + (v.min(LEGEND) as f32 / LEGEND as f32) * (r.width() - 16.0);
        let cy = r.top() + 40.0;
        let gap = 2.4;
        let seg = ((r.width() - (SEGMENTS - 1) as f32 * gap) / SEGMENTS as f32).max(1.0);
        let lit = ((s.xp.min(LEGEND) as f32 / LEGEND as f32) * SEGMENTS as f32).round() as usize;
        let off = ink(rgb(0x2a2c34), rgb(0xe3e4e8));
        for i in 0..SEGMENTS {
            let c = if i >= lit {
                off
            } else if boss {
                RED
            } else if s.legend {
                if i % 2 == 1 { GOLD } else { rgb(0xffc93c) }
            } else if i + 3 >= lit {
                rgb(0xe9ffa1)
            } else {
                LIME
            };
            let sx = r.left() + i as f32 * (seg + gap);
            p.rect_filled(Rect::from_min_size(pos2(sx, cy - 7.0), vec2(seg, 14.0)), CornerRadius::same(2), c);
        }
        let k = theme::c();
        for (i, (at, name)) in LEVELS.iter().enumerate().skip(1) {
            let sx = x(*at);
            let on = *at <= s.xp;
            let c = if on { if boss { RED } else { LIME } } else { ink(rgb(0x4a4d57), rgb(0xc9ccd4)) };
            p.rect_filled(Rect::from_min_size(pos2(sx - 1.0, cy - 13.0), vec2(2.0, 26.0)), CornerRadius::ZERO, c);
            let last = i == LEVELS.len() - 1;
            let align = if last { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
            let label = if on { lime_ink() } else { k.faint };
            p.text(pos2(sx, cy + 15.0), align, format!("{name} {}", course::thousands(*at)), medium(9.5), label);
        }
        // The gem above the XP, the last actions dropping in behind it.
        let gx = x(s.xp);
        let colours = if boss { g::RED } else if s.legend { g::GOLD } else { g::LIME };
        g::gem_at(p, pos2(gx, r.top() + 15.0), 26.0, colours);
        for (i, (orb, at)) in self.xp.recent.iter().enumerate() {
            let t = (at.elapsed().as_secs_f32() / DROP_SECS).min(1.0);
            let rest = pos2(gx - 34.0 - i as f32 * 40.0, r.top() + 13.0 + (i % 2) as f32 * 6.0);
            if rest.x < r.left() + 18.0 {
                break;
            }
            let from = rest - vec2(0.0, 24.0);
            let c = from.lerp(rest, crate::modal::eased(t as f64));
            let scale = 1.0 - i as f32 * 0.1;
            let (label, fg, bg) = orb.look();
            let pill = Rect::from_center_size(c, vec2(34.0, 18.0) * scale);
            p.rect(pill, CornerRadius::same(9), bg, Stroke::new(1.4, fg), StrokeKind::Inside);
            p.text(pill.center(), Align2::CENTER_CENTER, label, medium(9.5 * scale), fg);
        }
    }

    /// The GG card at Legend: the trophy, GG, the session's feats, New game+ and Touch grass.
    pub(super) fn gg_dialog(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        use crate::modal::{text, Modal, Width};
        if !self.xp.gg {
            return;
        }
        let k = theme::c();
        let s = self.xp.session.clone();
        let time = course::keyboard_time(self.xp.secs());
        let tile = if theme::is_light() { theme::tint(LIME, k.elevated, 0.25) } else { rgb(0x1e2808) };
        let out = Modal::new("gg", Width::Medium, "medal", vec![text("GG")])
            .icon_art(tile, |p, c| g::gem_at(p, c, 22.0, g::LIME))
            .status(vec![text(format!("Lv 5 · Legend · {} XP", course::thousands(s.xp)))])
            .cancel(None)
            .extra("New game+")
            .primary("Touch grass", false, true)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    let (r, _) = ui.allocate_exact_size(vec2(124.0, 112.0), Sense::hover());
                    trophy(ui.painter(), r.min);
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("GG").font(medium(46.0)).color(ink(LIME, rgb(0x4f8a00))));
                    ui.add_space(6.0);
                    let line = format!("{} XP in {time} at the keyboard. Either button tidies the room.", course::thousands(s.xp));
                    ui.add(egui::Label::new(egui::RichText::new(line).color(k.dim)).wrap());
                    ui.add_space(10.0);
                });
                // The full bar, in gold.
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
                let n = 30;
                let w = (r.width() - (n - 1) as f32 * 3.0) / n as f32;
                for i in 0..n {
                    let c = if i % 2 == 1 { GOLD } else { rgb(0xffc93c) };
                    ui.painter().rect_filled(Rect::from_min_size(pos2(r.left() + i as f32 * (w + 3.0), r.top()), vec2(w, 8.0)), CornerRadius::same(2), c);
                }
                ui.add_space(10.0);
                let bosses = if s.bosses == 1 { "1 boss beaten".to_string() } else { format!("{} bosses beaten", s.bosses) };
                for (title, value, c) in [
                    ("Boss slayer", bosses, ink(rgb(0xff8a80), rgb(0xd0352b))),
                    ("On a roll", format!("{} in a row", s.best.max(1)), ink(rgb(0x38bdf8), rgb(0x0b7ab8))),
                    ("The grind", s.tally(), ink(LIME, rgb(0x4f8a00))),
                ] {
                    feat(ui, title, &value, c);
                    ui.add_space(6.0);
                }
            });
        if out.extra == Some(0) {
            cmds.push(Cmd::GgDone { new_game: true });
        } else if out.primary || out.close {
            cmds.push(Cmd::GgDone { new_game: false });
        }
    }

    /// The card's buttons: both tidy the room into the bin bag; New game+ also starts again at 0,
    /// Touch grass keeps the XP and dims the screens until the next action.
    pub(super) fn gg_done(&mut self, new_game: bool) {
        let x = &mut self.xp;
        x.gg = false;
        x.tidied_at = Some(Instant::now());
        x.react = None;
        if new_game {
            x.session.new_game();
            x.recent.clear();
            x.started = Instant::now();
            x.sample_secs = None;
            x.dim = false;
        } else {
            x.session.tidy();
            x.dim = true;
        }
    }
}

/// The Legend trophy in a 124 × 112 box at `o`: the gem in the gold cup, three sparkles.
fn trophy(p: &Painter, o: Pos2) {
    art::draw(p, &g::CUP[..6], o + vec2(28.0, 34.0), 1.7, 0.0, 1.0);
    art::draw(p, &g::gem(g::LIME), o + vec2(37.0, 4.0), 1.25, 0.0, 1.0);
    for (x, y, k) in [(104.0, 16.0, 1.1), (16.0, 30.0, 0.8), (104.0, 70.0, 0.6)] {
        art::draw(p, art::STAR, o + vec2(x, y), k, 0.0, 1.0);
    }
}

/// One of the card's feats: a diamond, its name and its value.
fn feat(ui: &mut Ui, title: &str, value: &str, c: Color32) {
    let k = theme::c();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(9), k.sidebar);
    let d = pos2(r.left() + 14.0, r.center().y);
    p.add(egui::Shape::convex_polygon(vec![d - vec2(0.0, 5.0), d + vec2(5.0, 0.0), d + vec2(0.0, 5.0), d - vec2(5.0, 0.0)], c, Stroke::NONE));
    p.text(pos2(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, title, medium(13.0), k.strong);
    crate::ui::fit_text(p, pos2(r.right() - 12.0, r.center().y), Align2::RIGHT_CENTER, value, medium(12.0), c, r.width() - 150.0);
}

/// "Paused while the boss lives", centred above `at`, with the boss.
fn boss_pill(p: &Painter, at: Pos2) {
    let (fill, edge, text) = if theme::is_light() { (rgb(0xfde4e8), rgb(0xf0a3b0), rgb(0xcf3d33)) } else { (rgb(0x2c0d14), rgb(0x7a2433), rgb(0xff8a80)) };
    let gal = p.layout_no_wrap("Paused while the boss lives".into(), medium(11.0), text);
    let size = vec2(gal.size().x + 36.0, 24.0);
    let r = Rect::from_center_size(at - vec2(0.0, size.y / 2.0), size);
    p.rect(r, CornerRadius::same(12), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    art::draw(p, g::BOSS, pos2(r.left() + 6.0, r.center().y - 8.5), 0.3, 0.0, 1.4);
    p.galley(pos2(r.left() + 28.0, r.center().y - gal.size().y / 2.0), gal, text);
}

/// The icon of a toast: the game pad, its light in the toast's colour; the boss for a boss fight.
pub(super) fn tone_icon(p: &Painter, c: Pos2, tone: Tone) {
    if tone == Tone::Boss {
        art::draw(p, g::BOSS, c - vec2(30.0, 27.0) * 0.4, 0.4, 0.0, 1.4);
    } else {
        let (_, led) = tone.colours();
        art::draw(p, &g::pad(led), c - vec2(32.0, 23.0) * 0.4, 0.4, 0.0, 1.5);
    }
}

/// The toolbar's "Lv 3 · 2,840 XP ×3" pill: the gem, the level, the XP and the streak running,
/// red with BOSS in a boss fight. It only shows.
pub(super) fn xp_pill(ui: &mut Ui, level: usize, xp: u32, streak: u32, boss: bool) {
    let light = theme::is_light();
    let k = theme::c();
    let fg = if boss { ink(rgb(0xff8a80), rgb(0xcf3d33)) } else { lime_ink() };
    let gal = ui.painter().layout_no_wrap(format!("Lv {} · {} XP", level + 1, course::thousands(xp)), medium(12.0), fg);
    let extra = if boss { Some("BOSS".to_string()) } else { (streak >= 2).then(|| format!("×{streak}")) };
    let ge = extra.map(|e| ui.painter().layout_no_wrap(e, medium(12.0), ink(rgb(0xff8a80), rgb(0xd0352b))));
    let more = ge.as_ref().map_or(0.0, |g| g.size().x + 6.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(gal.size().x + more + 42.0, 28.0), Sense::hover());
    let p = ui.painter();
    let (fill, edge) = match (boss, light) {
        (true, false) => (rgb(0x2c0d14), rgb(0x7a2433)),
        (true, true) => (theme::tint(RED, k.sidebar, 0.12), theme::tint(RED, k.sidebar, 0.45)),
        (false, false) => (rgb(0x1e2808), rgb(0x4f6b12)),
        (false, true) => (theme::tint(LIME, k.sidebar, 0.22), theme::tint(rgb(0x7aa21a), k.sidebar, 0.55)),
    };
    p.rect(rect, CornerRadius::same(14), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    g::gem_at(p, pos2(rect.left() + 16.0, rect.center().y), 17.0, if boss { g::RED } else { g::LIME });
    let gx = rect.left() + 30.0;
    let gw = gal.size().x;
    p.galley(pos2(gx, rect.center().y - gal.size().y / 2.0), gal, fg);
    if let Some(ge) = ge {
        p.galley(pos2(gx + gw + 6.0, rect.center().y - ge.size().y / 2.0), ge, fg);
    }
    resp.on_hover_text("Session XP, to Legend at 6,000: a commit +100, a push +300, a merge +500, ×2 then ×3 when they follow within a minute; Undo −100; conflicts are a boss fight worth +200");
}
