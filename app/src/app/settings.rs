//! Settings window (⌘,): the appearance and the automatic fetch. Every choice applies and is saved
//! at once, so there is no OK button; Esc, the close button or a click outside closes it.
use super::*;
use crate::settings::{minutes_label, Settings, FETCH_MINUTES};
use crate::ui::appearance_icon;
use egui::{pos2, vec2, Align2, CornerRadius, Key, Margin, Painter, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

const SETTINGS_W: f32 = 500.0;
const HEADER_H: f32 = 30.0;
const SECTION_H: f32 = 26.0;
const ROW_H: f32 = 50.0;
const SEG_H: f32 = 28.0;

/// A section title, with a hairline above it unless it is the first.
fn section(ui: &mut Ui, title: &str, first: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), SECTION_H), Sense::hover());
    if !first {
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0_f32, theme::c().border));
    }
    ui.painter().text(pos2(r.left(), r.bottom() - 4.0), Align2::LEFT_BOTTOM, title, theme::small(), theme::c().faint);
}

/// A setting's row: its name and a line of help on the left. Returns the rect of the control,
/// `ctrl` wide, against the right edge.
fn row(ui: &mut Ui, title: &str, help: &str, ctrl: Vec2, enabled: bool) -> Rect {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let p = ui.painter();
    let (tc, hc) = if enabled { (theme::c().text, theme::c().dim) } else { (theme::c().faint, theme::c().faint) };
    p.text(pos2(r.left(), r.center().y - 8.0), Align2::LEFT_CENTER, title, theme::body(), tc);
    p.text(pos2(r.left(), r.center().y + 9.0), Align2::LEFT_CENTER, help, theme::secondary(), hc);
    Rect::from_center_size(pos2(r.right() - ctrl.x * 0.5, r.center().y), ctrl)
}

/// A segmented control of `n` options in `rect`, the chosen one raised. `draw` paints option `i`
/// in its rect with the given text colour and background. Returns the option clicked.
fn segmented(ui: &mut Ui, rect: Rect, n: usize, chosen: usize, enabled: bool, id: &str, mut draw: impl FnMut(&Painter, Rect, usize, Color32, Color32)) -> Option<usize> {
    let k = theme::c();
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    let p = ui.painter().clone();
    p.rect(rect, CornerRadius::same(7), k.input, Stroke::new(1.0_f32, fade(k.border_strong)), StrokeKind::Inside);
    let w = rect.width() / n as f32;
    let mut clicked = None;
    for i in 0..n {
        let seg = Rect::from_min_size(pos2(rect.left() + w * i as f32, rect.top()), vec2(w, rect.height())).shrink(3.0);
        let resp = ui.interact(seg, ui.id().with((id, i)), if enabled { Sense::click() } else { Sense::hover() });
        let on = i == chosen;
        let hovered = enabled && resp.hovered();
        let bg = if on {
            p.rect(seg, CornerRadius::same(5), k.widget_active, Stroke::new(1.0_f32, fade(k.widget_active_border)), StrokeKind::Inside);
            k.widget_active
        } else if hovered {
            p.rect_filled(seg, CornerRadius::same(5), k.hover);
            k.hover
        } else {
            k.input
        };
        let fg = if on { k.strong } else if hovered { k.text } else { k.dim };
        draw(&p, seg, i, fade(fg), bg);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// An on/off switch in `rect`. Returns true when clicked.
fn toggle(ui: &mut Ui, rect: Rect, id: &str, on: bool) -> bool {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let t = ui.ctx().animate_bool(ui.id().with((id, "anim")), on);
    let k = theme::c();
    let track = theme::tint(k.accent, k.border_strong, t);
    let track = if resp.hovered() { theme::lighten(track, 0.08) } else { track };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same((rect.height() * 0.5) as u8), track);
    let r = rect.height() * 0.5 - 2.5;
    let x = egui::lerp((rect.left() + r + 2.5)..=(rect.right() - r - 2.5), t);
    p.circle_filled(pos2(x, rect.center().y), r, k.on_accent);
    resp.clicked()
}

impl App {
    pub(super) fn settings_ui(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        if !self.settings_open {
            return;
        }
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let appearance = self.library.appearance;
        let mut s: Settings = self.library.settings;

        let screen = ctx.screen_rect();
        let left = screen.center().x - SETTINGS_W * 0.5 - 16.0;
        let top = (screen.top() + 70.0).max(screen.center().y - 240.0);
        egui::Area::new(egui::Id::new("settings_backdrop")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::ZERO, theme::shadow(0.8));
            if resp.clicked() {
                close = true;
            }
        });
        egui::Area::new(egui::Id::new("settings")).order(egui::Order::Foreground).fixed_pos(pos2(left, top)).show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::c().elevated)
                .stroke(Stroke::new(1.0_f32, theme::c().border_strong))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(16, 12))
                .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: theme::shadow(1.0) })
                .show(ui, |ui| {
                    ui.set_width(SETTINGS_W);
                    ui.spacing_mut().item_spacing.y = 0.0;

                    // Title and a drawn close button.
                    let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
                    ui.painter().text(pos2(hr.left(), hr.center().y), Align2::LEFT_CENTER, "Settings", egui::FontId::proportional(theme::FS_TITLE), theme::c().strong);
                    let x = Rect::from_center_size(pos2(hr.right() - 11.0, hr.center().y), vec2(22.0, 22.0));
                    let xr = ui.interact(x, ui.id().with("settings_close"), Sense::click()).on_hover_text("Close (Esc)");
                    if xr.hovered() {
                        ui.painter().rect_filled(x, CornerRadius::same(5), theme::c().hover);
                    }
                    let xc = if xr.hovered() { theme::c().text } else { theme::c().dim };
                    for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
                        ui.painter().line_segment([x.center() - d, x.center() + d], Stroke::new(1.4_f32, xc));
                    }
                    if xr.clicked() {
                        close = true;
                    }
                    ui.add_space(4.0);

                    section(ui, "Appearance", true);
                    let ctrl = row(ui, "Mode", "System follows macOS", vec2(270.0, SEG_H), true);
                    let modes = theme::Appearance::ALL;
                    let chosen = modes.iter().position(|a| *a == appearance).unwrap_or(0);
                    let picked = segmented(ui, ctrl, modes.len(), chosen, true, "appearance", |p, r, i, fg, bg| {
                        let a = modes[i];
                        let w = 14.0 + 6.0 + p.layout_no_wrap(a.label().into(), theme::body(), fg).size().x;
                        let x0 = r.center().x - w * 0.5;
                        appearance_icon(p, pos2(x0 + 7.0, r.center().y), a, fg, bg);
                        p.text(pos2(x0 + 20.0, r.center().y), Align2::LEFT_CENTER, a.label(), theme::body(), fg);
                    });
                    if let Some(i) = picked {
                        if modes[i] != appearance {
                            cmds.push(Cmd::SetAppearance(modes[i]));
                        }
                    }

                    ui.add_space(6.0);
                    section(ui, "Fetch", false);
                    let ctrl = row(ui, "Fetch automatically", "Gets new commits from the remote in the background", vec2(34.0, 20.0), true);
                    if toggle(ui, ctrl, "auto_fetch", s.auto_fetch) {
                        s.auto_fetch = !s.auto_fetch;
                    }
                    let ctrl = row(ui, "Every", "While a repository is open", vec2(270.0, SEG_H), s.auto_fetch);
                    let chosen = FETCH_MINUTES.iter().position(|m| *m == s.fetch_minutes).unwrap_or(1);
                    let picked = segmented(ui, ctrl, FETCH_MINUTES.len(), chosen, s.auto_fetch, "fetch_minutes", |p, r, i, fg, _| {
                        p.text(r.center(), Align2::CENTER_CENTER, minutes_label(FETCH_MINUTES[i]), theme::body(), fg);
                    });
                    if let Some(i) = picked {
                        s.fetch_minutes = FETCH_MINUTES[i];
                    }

                    ui.add_space(10.0);
                    let (fr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
                    ui.painter().text(pos2(fr.left(), fr.center().y), Align2::LEFT_CENTER, "Changes apply and are saved right away · Esc to close", theme::small(), theme::c().faint);
                });
        });
        if s != self.library.settings {
            cmds.push(Cmd::SetSettings(s));
        }
        if close {
            self.settings_open = false;
        }
    }
}
