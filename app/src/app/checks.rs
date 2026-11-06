//! The checks of a pull request, in a card under its badge (#34): a click on a branch's badge in
//! the history or the sidebar opens it. The checks are read with `gh` in the background, the
//! failing ones first, and read again every few seconds while some still run. A check opens its
//! log on GitHub, the footer the pull request. Esc or a click elsewhere closes it.
use super::*;
use crate::pulls::{Check, CheckState, Pull};
use egui::{pos2, vec2, Align, Align2, CornerRadius, CursorIcon, Key, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// How often running checks are read again while the card is open.
const RELOAD: Duration = Duration::from_secs(15);
/// The card's width.
const WIDTH: f32 = 380.0;
/// A click on the badge of the card just closed by that same press does not open it again.
const REOPEN_GUARD: Duration = Duration::from_millis(400);

pub struct ChecksState {
    pub pull: Pull,
    /// Where the card's top left corner goes, under the badge. None until the badge is drawn
    /// (a screenshot opens it before).
    pub anchor: Option<Pos2>,
    pub checks: Option<Result<Vec<Check>, String>>,
    pub loading: Option<Receiver<Result<Vec<Check>, String>>>,
    /// When the checks were last read, to read running ones again.
    pub read_at: Option<Instant>,
}

/// The card, open or just closed.
#[derive(Default)]
pub struct ChecksCard {
    pub open: Option<ChecksState>,
    /// The pull request whose card was closed last, and when.
    closed: Option<(u64, Instant)>,
}

/// Seconds since 1970, for the running checks' durations.
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// The hit area of a badge drawn at `left`, in a row from `top` to `bottom`: a little wider than
/// the badge, so it is easy to click.
pub(super) fn badge_hit(left: f32, top: f32, bottom: f32, pull: &Pull) -> Rect {
    Rect::from_min_max(pos2(left - 3.0, top), pos2(left + crate::ui::pr_badge_width(pull) + 3.0, bottom))
}

/// Under the pointer, the badge of `badges` it is on; shows a hand while it is there.
pub(super) fn badge_under<'a>(ui: &Ui, badges: &[(Rect, &'a Pull)]) -> Option<(Rect, &'a Pull)> {
    let q = ui.input(|i| i.pointer.hover_pos())?;
    let hit = badges.iter().find(|(r, _)| r.contains(q)).copied();
    if hit.is_some() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    hit
}

/// The Cmd that opens `pull`'s card under the badge `rect`.
pub(super) fn open_cmd(rect: Rect, pull: &Pull) -> Cmd {
    Cmd::Checks(pull.clone(), Some(pos2(rect.left(), rect.bottom() + 2.0)))
}

impl Tab {
    /// Opens the card of `pull` under `anchor`, and reads its checks.
    pub(super) fn checks_open(&mut self, pull: Pull, anchor: Option<Pos2>) {
        if let Some(c) = &mut self.checks.open {
            if c.pull.number == pull.number {
                c.anchor = anchor.or(c.anchor);
                return;
            }
        }
        if self.checks.closed.is_some_and(|(n, at)| n == pull.number && at.elapsed() < REOPEN_GUARD) {
            return;
        }
        self.checks.open = Some(ChecksState { pull, anchor, checks: None, loading: None, read_at: None });
        self.checks_load();
    }

    pub(super) fn checks_close(&mut self) {
        if let Some(c) = self.checks.open.take() {
            self.checks.closed = Some((c.pull.number, Instant::now()));
        }
    }

    /// Reads the open card's checks in the background; the sample project's are made up.
    fn checks_load(&mut self) {
        let slug = self.github_slug();
        let demo = self.is_demo();
        let Some(c) = &mut self.checks.open else { return };
        c.read_at = Some(Instant::now());
        if demo {
            c.checks = Some(Ok(crate::pulls::sample_checks(&c.pull, now())));
            return;
        }
        let Some(slug) = slug else {
            c.checks = Some(Err("This repository isn't on GitHub.".into()));
            return;
        };
        let (tx, rx) = channel();
        let number = c.pull.number;
        std::thread::spawn(move || {
            let _ = tx.send(crate::pulls::load_checks(&slug, number));
        });
        c.loading = Some(rx);
    }

    /// Takes the checks read, and reads them again while some run.
    pub(super) fn checks_poll(&mut self, ctx: &egui::Context) {
        let demo = self.is_demo();
        let Some(c) = &mut self.checks.open else { return };
        if let Some(rx) = &c.loading {
            match rx.try_recv() {
                Ok(res) => {
                    // A failed read again keeps the list there was.
                    if res.is_ok() || !matches!(c.checks, Some(Ok(_))) {
                        c.checks = Some(res);
                    }
                    c.loading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => c.loading = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let running = matches!(&c.checks, Some(Ok(l)) if l.iter().any(|k| k.state == CheckState::Pending));
        if running {
            // The running durations count up each second.
            ctx.request_repaint_after(Duration::from_secs(1));
            if !demo && c.read_at.is_none_or(|t| t.elapsed() >= RELOAD) {
                self.checks_load();
            }
        }
    }

    /// The card, over everything but dialogs. Esc or a press outside it closes it.
    pub(super) fn checks_ui(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let Some(c) = &self.checks.open else { return };
        let Some(anchor) = c.anchor else { return };
        // The latest read of the pull request, for its state and CI.
        let pull = self.pulls.pulls.as_deref().and_then(|p| p.list.iter().find(|q| q.number == c.pull.number)).unwrap_or(&c.pull);
        let k = theme::c();
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        let area = egui::Area::new(egui::Id::new(("checks", self.key))).order(egui::Order::Foreground).fixed_pos(anchor).constrain(true).show(ctx, |ui| {
            crate::modal::card(ui, 10, Margin { left: 12, right: 12, top: 11, bottom: 10 }, |ui| {
                ui.set_width(WIDTH);
                ui.spacing_mut().item_spacing.y = 6.0;
                // Header: the badge, "#n title", then where it stands.
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(crate::ui::pr_badge_width(pull), 16.0), Sense::hover());
                    crate::ui::pr_badge(ui.painter(), r.left(), r.center().y, pull);
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
                    crate::ui::fit_text(ui.painter(), r.left_center(), Align2::LEFT_CENTER, &format!("#{} {}", pull.number, pull.title), theme::body(), k.strong, r.width());
                });
                let summary = match &c.checks {
                    Some(Ok(l)) if !l.is_empty() => crate::pulls::checks_summary(l),
                    _ => pull.ci.map(crate::pulls::Ci::label).unwrap_or("No checks").to_string(),
                };
                ui.label(RichText::new(format!("{} · {summary}", pull.state.label())).font(theme::secondary()).color(k.dim));
                ui.separator();
                match &c.checks {
                    None => {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(12.0));
                            ui.label(RichText::new("Reading the checks…").color(k.dim));
                        });
                    }
                    Some(Err(e)) => {
                        ui.add(egui::Label::new(RichText::new(e).color(k.deleted)).wrap());
                    }
                    Some(Ok(l)) if l.is_empty() => {
                        ui.label(RichText::new("No checks on the last commit.").color(k.dim));
                    }
                    Some(Ok(l)) => {
                        let now = now();
                        egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for check in l {
                                if check_row(ui, check, now).clicked() && !check.url.is_empty() {
                                    cmds.push(Cmd::OpenUrl(check.url.clone()));
                                }
                            }
                        });
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if c.loading.is_some() && c.checks.is_some() {
                        ui.add(egui::Spinner::new().size(10.0));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !pull.url.is_empty() && crate::modal::button(ui, "Open the pull request", crate::modal::Kind::Plain, true).clicked() {
                            cmds.push(Cmd::OpenUrl(pull.url.clone()));
                        }
                    });
                });
            });
        });
        let rect = area.response.rect;
        if ctx.input(|i| i.pointer.any_pressed() && i.pointer.press_origin().is_some_and(|q| !rect.contains(q))) {
            close = true;
        }
        if close {
            self.checks_close();
        }
    }
}

/// One check: its state icon, name, workflow, and duration on the right. A click opens its log.
fn check_row(ui: &mut Ui, check: &Check, now: i64) -> egui::Response {
    let k = theme::c();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), k.hover);
        if !check.url.is_empty() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
    }
    let y = rect.center().y;
    state_icon(p, pos2(rect.left() + 12.0, y), check.state);
    let mut right = rect.right() - 8.0;
    if let Some(d) = check.duration(now) {
        let g = p.layout_no_wrap(crate::pulls::format_duration(d), theme::secondary(), k.dim);
        right -= g.size().x;
        p.galley(pos2(right, y - g.size().y / 2.0), g, k.dim);
        right -= 10.0;
    }
    let left = rect.left() + 26.0;
    let name = crate::ui::fit_text(p, pos2(left, y), Align2::LEFT_CENTER, &check.name, theme::body(), k.text, right - left);
    if !check.workflow.is_empty() && name.right() + 8.0 < right {
        crate::ui::fit_text(p, pos2(name.right() + 8.0, y), Align2::LEFT_CENTER, &check.workflow, theme::secondary(), k.faint, right - name.right() - 8.0);
    }
    let tip = if check.url.is_empty() { check.state.label().to_string() } else { format!("{} — open its log on GitHub", check.state.label()) };
    resp.on_hover_text(tip)
}

/// A check's state drawn with the painter: a red cross, an amber ring, a green tick, a grey dash.
fn state_icon(p: &egui::Painter, c: Pos2, state: CheckState) {
    let k = theme::c();
    match state {
        CheckState::Failure => {
            p.circle_filled(c, 7.0, k.deleted);
            let s = Stroke::new(1.6_f32, k.elevated);
            for d in [vec2(2.6, 2.6), vec2(2.6, -2.6)] {
                p.line_segment([c - d, c + d], s);
            }
        }
        CheckState::Pending => {
            p.circle_stroke(c, 5.8, Stroke::new(1.6_f32, k.modified));
            p.circle_filled(c, 2.0, k.modified);
        }
        CheckState::Success => {
            p.circle_filled(c, 7.0, k.added);
            p.add(egui::Shape::line(vec![c + vec2(-3.2, 0.2), c + vec2(-1.0, 2.4), c + vec2(3.2, -2.4)], Stroke::new(1.6_f32, k.elevated)));
        }
        CheckState::Skipped => {
            p.circle_stroke(c, 5.8, Stroke::new(1.4_f32, k.faint));
            p.line_segment([c - vec2(2.6, 0.0), c + vec2(2.6, 0.0)], Stroke::new(1.4_f32, k.faint));
        }
    }
}
