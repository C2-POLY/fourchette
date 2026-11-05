//! The frame every dialog shares: the app darkened behind, a centred card with a soft shadow and
//! 14 pt corners, a header (icon tile, title, status line, close cross), the body, and a footer
//! (a hint on the left, Cancel then the main action on the right, red when it destroys
//! something). Enter runs the main action, Esc closes, and the card appears with a short scale
//! and fade. Also the pieces dialogs share: choice cards, check rows, pills, keycaps and notes,
//! and the shadow, backdrop and animation the palette, the switcher, Settings and the toasts use.
use crate::theme;
use egui::emath::TSTransform;
use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Id, Key, LayerId, Layout, Margin, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};

/// Corners of every window.
pub const RADIUS: u8 = 14;
/// Side padding of the header, the body and the footer.
pub const PAD: f32 = 20.0;
/// How long a window takes to appear, in seconds.
pub const APPEAR_SECS: f64 = 0.2;

/// The three widths a dialog can have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    /// A name to type, a question.
    Small,
    /// A few options or a short text.
    Medium,
    /// Choices next to an explanation, or a list.
    Large,
}

impl Width {
    pub fn px(self) -> f32 {
        match self {
            Width::Small => 420.0,
            Width::Medium => 520.0,
            Width::Large => 760.0,
        }
    }
}

/// A piece of a title or status line.
pub enum Seg {
    Text(String),
    /// A local branch, a tag or a commit: a mono pill on the accent's soft colour.
    Pill(String),
    /// A remote branch: a mono pill outlined in green.
    Remote(String),
}

pub fn text(s: impl Into<String>) -> Seg {
    Seg::Text(s.into())
}

pub fn pill(s: impl Into<String>) -> Seg {
    Seg::Pill(s.into())
}

pub fn remote(s: impl Into<String>) -> Seg {
    Seg::Remote(s.into())
}

/// A key drawn as a small keycap in a hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    Up,
    Down,
    Text(&'static str),
}

/// A piece of the footer's hint.
pub enum Hint {
    Key(Cap),
    Text(String),
}

/// The hint of a list window: Up and Down move, Enter `run`s, Esc closes.
pub fn list_hint(run: &str) -> Vec<Hint> {
    vec![
        Hint::Key(Cap::Up),
        Hint::Key(Cap::Down),
        Hint::Text("to move".into()),
        Hint::Key(Cap::Text("Enter")),
        Hint::Text(format!("to {run}")),
        Hint::Key(Cap::Text("Esc")),
        Hint::Text("to close".into()),
    ]
}

/// "⌘Z takes it back", the hint of every dialog whose action Undo takes back.
pub fn undo_hint() -> Vec<Hint> {
    vec![Hint::Key(Cap::Text("⌘Z")), Hint::Text("takes it back".into())]
}

/// The footer's main button.
pub struct Primary {
    pub text: String,
    pub danger: bool,
    pub enabled: bool,
}

/// What the person did with a dialog this frame.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The main button, or Enter.
    pub primary: bool,
    /// Cancel, the cross or Esc.
    pub close: bool,
    /// One of the extra buttons left of Cancel.
    pub extra: Option<usize>,
}

/// A dialog, built then shown with its body.
pub struct Modal {
    id: Id,
    width: Width,
    icon: &'static str,
    danger: bool,
    title: Vec<Seg>,
    status: Vec<Seg>,
    hint: Vec<Hint>,
    cancel: Option<String>,
    extra: Vec<String>,
    primary: Option<Primary>,
    closable: bool,
}

impl Modal {
    /// `icon` is one of `ui::glyph`'s names.
    pub fn new(id: &str, width: Width, icon: &'static str, title: Vec<Seg>) -> Self {
        Modal { id: Id::new(("modal", id)), width, icon, danger: false, title, status: Vec::new(), hint: Vec::new(), cancel: Some("Cancel".into()), extra: Vec::new(), primary: None, closable: true }
    }

    /// The icon tile in red, for a dialog that deletes something.
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    pub fn status(mut self, status: Vec<Seg>) -> Self {
        self.status = status;
        self
    }

    pub fn hint(mut self, hint: Vec<Hint>) -> Self {
        self.hint = hint;
        self
    }

    /// The Cancel button's text ("Later", "Close"), or none.
    pub fn cancel(mut self, cancel: Option<&str>) -> Self {
        self.cancel = cancel.map(str::to_string);
        self
    }

    pub fn extra(mut self, text: impl Into<String>) -> Self {
        self.extra.push(text.into());
        self
    }

    pub fn primary(mut self, text: impl Into<String>, danger: bool, enabled: bool) -> Self {
        self.primary = Some(Primary { text: text.into(), danger, enabled });
        self
    }

    /// Whether Esc and the cross close it (not while an update installs).
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    pub fn show(self, ctx: &egui::Context, body: impl FnOnce(&mut Ui)) -> Outcome {
        let t = appear(ctx, self.id);
        let layer = LayerId::new(egui::Order::Foreground, self.id);
        let resp = egui::Modal::new(self.id).backdrop_color(Color32::TRANSPARENT).frame(egui::Frame::NONE).show(ctx, |ui| {
            ui.multiply_opacity(t);
            // Larger than the window, so it still covers it while the layer is scaled down.
            let screen = ui.ctx().screen_rect().expand(200.0);
            ui.painter().with_clip_rect(screen).rect_filled(screen, CornerRadius::ZERO, backdrop());
            self.card(ui, body)
        });
        animate_layer(ctx, layer, resp.response.rect.center(), t);
        let mut out = resp.inner;
        let (enter, esc) = ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        if enter && self.primary.as_ref().is_some_and(|p| p.enabled) {
            out.primary = true;
        }
        if esc && self.closable {
            out.close = true;
        }
        out
    }

    fn card(&self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) -> Outcome {
        let k = theme::c();
        let w = self.width.px();
        let p = ui.painter().clone();
        let shadow_idx = p.add(Shape::Noop);
        let mut out = Outcome::default();
        let whole = ui.vertical(|ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing.y = 0.0;
            // Header: icon tile, title and status line; the cross is placed once its row is known.
            let head = egui::Frame::new().inner_margin(Margin { left: PAD as i8, right: 14, top: 18, bottom: 16 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    icon_tile(ui, self.icon, self.danger);
                    ui.add_space(14.0);
                    let text_w = w - PAD - 14.0 - 38.0 - 14.0 - 40.0;
                    ui.allocate_ui_with_layout(vec2(text_w, 38.0), Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        if self.status.is_empty() {
                            ui.add_space(9.0);
                        }
                        line(ui, &self.title, FontId::proportional(16.0), k.strong);
                        if !self.status.is_empty() {
                            line(ui, &self.status, theme::body(), k.dim);
                        }
                    });
                });
            });
            if self.closable {
                let r = head.response.rect;
                let x = Rect::from_min_size(pos2(r.left() + w - 14.0 - 26.0, r.top() + 14.0), vec2(26.0, 26.0));
                if close_cross(ui, x).clicked() {
                    out.close = true;
                }
            }
            // Body.
            egui::Frame::new().inner_margin(Margin { left: PAD as i8, right: PAD as i8, top: 2, bottom: 18 }).show(ui, |ui| {
                ui.set_width(w - 2.0 * PAD);
                ui.spacing_mut().item_spacing.y = 4.0;
                body(ui);
            });
            // Footer: a hairline, a darker fill, the hint, then the buttons from the right.
            let fill = theme::tint(k.sidebar, k.elevated, 0.6);
            let r = RADIUS;
            let foot = egui::Frame::new().fill(fill).corner_radius(CornerRadius { nw: 0, ne: 0, sw: r, se: r }).inner_margin(Margin::symmetric(PAD as i8, 12)).show(ui, |ui| {
                ui.set_width(w - 2.0 * PAD);
                ui.horizontal(|ui| {
                    ui.set_min_height(30.0);
                    // Buttons from the right first; the hint takes what is left, cut short if need be.
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(pr) = &self.primary {
                            let kind = if pr.danger { Kind::Danger } else { Kind::Primary };
                            if button(ui, &pr.text, kind, pr.enabled).clicked() {
                                out.primary = true;
                            }
                        }
                        if let Some(c) = &self.cancel {
                            if button(ui, c, Kind::Plain, true).clicked() {
                                out.close = true;
                            }
                        }
                        for (i, e) in self.extra.iter().enumerate().rev() {
                            if button(ui, e, Kind::Plain, true).clicked() {
                                out.extra = Some(i);
                            }
                        }
                        let room = ui.available_width() - 12.0;
                        if room > 40.0 {
                            let left = ui.max_rect().left();
                            let r = Rect::from_min_size(pos2(left, ui.max_rect().top()), vec2(room, 30.0));
                            let mut h = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)));
                            h.set_clip_rect(r.intersect(h.clip_rect()));
                            hint_row(&mut h, &self.hint);
                        }
                    });
                });
            });
            let fr = foot.response.rect;
            ui.painter().hline(fr.x_range(), fr.top(), Stroke::new(1.0_f32, k.border));
        });
        let rect = whole.response.rect;
        let rr = CornerRadius::same(RADIUS);
        p.set(shadow_idx, card_back(rect, rr));
        if !theme::is_light() {
            // A faint light edge along the top, as if lit from above.
            ui.painter().hline(rect.shrink2(vec2(RADIUS as f32, 0.0)).x_range(), rect.top() + 1.0, Stroke::new(1.0_f32, Color32::from_white_alpha(10)));
        }
        ui.painter().rect_stroke(rect, rr, Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
        out
    }
}

/// The darkened app behind a window.
pub fn backdrop() -> Color32 {
    // Light shadows are faint (45), so the light backdrop takes more of it: about 22 %.
    theme::shadow(if theme::is_light() { 1.25 } else { 0.8 })
}

/// A window's two shadows (a wide soft one and a close one) and its fill, as one shape.
pub fn card_back(rect: Rect, rr: CornerRadius) -> Shape {
    let k = theme::c();
    Shape::Vec(vec![
        egui::epaint::Shadow { offset: [0, 24], blur: 64, spread: 0, color: theme::shadow(1.0) }.as_shape(rect, rr).into(),
        egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: theme::shadow(0.6) }.as_shape(rect, rr).into(),
        egui::epaint::RectShape::filled(rect, rr, k.elevated).into(),
    ])
}

/// A window's card around `add`: shadows, fill, `radius` corners and its edge.
pub fn card<R>(ui: &mut Ui, radius: u8, margin: Margin, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let idx = ui.painter().add(Shape::Noop);
    let r = egui::Frame::new().inner_margin(margin).show(ui, add);
    let rect = r.response.rect;
    let rr = CornerRadius::same(radius);
    ui.painter().set(idx, card_back(rect, rr));
    ui.painter().rect_stroke(rect, rr, Stroke::new(1.0_f32, theme::c().border_strong), StrokeKind::Inside);
    r
}

/// A toast at the bottom of the window: a green tick or a red cross, then the message. It rises
/// into place as it appears; `age` is its age in seconds. With `undo` (the share of its time left),
/// a second line says Cmd+Z takes it back, and a bar along its foot shrinks as the time runs out.
pub fn toast(ctx: &egui::Context, msg: &str, error: bool, age: f64, undo: Option<f32>) {
    let k = theme::c();
    let t = eased(age * 1.4);
    if t < 1.0 {
        ctx.request_repaint();
    }
    egui::Area::new(Id::new("toast")).order(egui::Order::Tooltip).interactable(false).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -40.0 + 16.0 * (1.0 - t))).show(ctx, |ui| {
        ui.multiply_opacity(t);
        let bottom = if undo.is_some() { 12 } else { 9 };
        let r = card(ui, 10, Margin { left: 12, right: 16, top: 9, bottom }, |ui| {
            ui.set_max_width(560.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                let col = if error { k.deleted } else { k.added };
                let p = ui.painter();
                p.circle_filled(r.center(), 9.0, theme::tint(col, k.elevated, 0.18));
                if error {
                    for d in [vec2(3.0, 3.0), vec2(3.0, -3.0)] {
                        p.line_segment([r.center() - d, r.center() + d], Stroke::new(1.6_f32, col));
                    }
                } else {
                    crate::ui::check_mark(p, r.center() + vec2(0.0, 0.5), col);
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.add(egui::Label::new(egui::RichText::new(msg).color(k.strong)).wrap());
                    if undo.is_some() {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(egui::RichText::new("Not what you wanted?").font(theme::secondary()).color(k.dim));
                            keycap(ui, Cap::Text("⌘Z"));
                            ui.label(egui::RichText::new("undoes it").font(theme::secondary()).color(k.dim));
                        });
                    }
                });
            });
        });
        if let Some(left) = undo {
            // The time left: a thin bar along the foot, inside the corners, shrinking to the left.
            let rect = r.response.rect;
            let track = Rect::from_min_max(pos2(rect.left() + 12.0, rect.bottom() - 6.0), pos2(rect.right() - 12.0, rect.bottom() - 4.0));
            let p = ui.painter();
            p.rect_filled(track, CornerRadius::same(1), k.border);
            p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * left.clamp(0.0, 1.0), track.height())), CornerRadius::same(1), k.accent);
        }
    });
}

/// Paints a window's shadows and fill, for windows laid out by hand.
pub fn paint_card(p: &egui::Painter, rect: Rect) {
    p.add(card_back(rect, CornerRadius::same(RADIUS)));
}

pub fn paint_edge(p: &egui::Painter, rect: Rect) {
    p.rect_stroke(rect, CornerRadius::same(RADIUS), Stroke::new(1.0_f32, theme::c().border_strong), StrokeKind::Inside);
}

/// How far a window is into its appearing, 0 to 1, eased out: `elapsed` seconds since it opened.
pub fn eased(elapsed: f64) -> f32 {
    let x = (elapsed / APPEAR_SECS).clamp(0.0, 1.0) as f32;
    1.0 - (1.0 - x).powi(3)
}

/// How far the window `id` is into its appearing. It starts again when the window was not shown
/// the frame before.
pub fn appear(ctx: &egui::Context, id: Id) -> f32 {
    let now = ctx.input(|i| i.time);
    let pass = ctx.cumulative_pass_nr();
    let key = id.with("appear");
    let start = match ctx.data(|d| d.get_temp::<(f64, u64)>(key)) {
        Some((start, last)) if last + 1 >= pass => start,
        _ => now,
    };
    ctx.data_mut(|d| d.insert_temp(key, (start, pass)));
    let t = eased(now - start);
    if t < 1.0 {
        ctx.request_repaint();
    }
    t
}

/// Scales a window's layer from 96 % around `centre`, and lifts it 8 points, as it appears.
pub fn animate_layer(ctx: &egui::Context, layer: LayerId, centre: Pos2, t: f32) {
    let s = 0.96 + 0.04 * t;
    let lift = vec2(0.0, 8.0 * (1.0 - t));
    let to = TSTransform::from_translation(centre.to_vec2() + lift) * TSTransform::from_scaling(s) * TSTransform::from_translation(-centre.to_vec2());
    ctx.set_transform_layer(layer, if t >= 1.0 { TSTransform::IDENTITY } else { to });
}

/// A title or status line: text and pills in a row that wraps.
fn line(ui: &mut Ui, segs: &[Seg], font: FontId, color: Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(5.0, 2.0);
        for s in segs {
            match s {
                Seg::Text(t) => {
                    ui.label(egui::RichText::new(t).font(font.clone()).color(color));
                }
                Seg::Pill(t) => {
                    pill_ui(ui, t, false, font.size);
                }
                Seg::Remote(t) => {
                    pill_ui(ui, t, true, font.size);
                }
            }
        }
    });
}

/// A branch name as a small mono pill; `remote` outlines it in green.
pub fn pill_ui(ui: &mut Ui, text: &str, remote: bool, beside: f32) -> Response {
    let k = theme::c();
    let size = if beside >= 15.0 { 13.0 } else { 11.5 };
    let (fg, fill, stroke) = if remote { (k.added, Color32::TRANSPARENT, Stroke::new(1.0_f32, theme::tint(k.added, k.elevated, 0.55))) } else { (k.accent_text, k.accent_soft, Stroke::NONE) };
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::monospace(size), fg);
    let h = if beside >= 15.0 { 22.0 } else { 18.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 14.0, h), Sense::hover());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(5), fill, stroke, StrokeKind::Inside);
    p.galley(pos2(rect.left() + 7.0, rect.center().y - g.size().y / 2.0), g, fg);
    resp
}

/// The rounded square at the left of the header, with the dialog's icon.
fn icon_tile(ui: &mut Ui, icon: &str, danger: bool) {
    let k = theme::c();
    let (rect, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
    let (fill, fg) = if danger { (theme::tint(k.deleted, k.elevated, 0.16), k.deleted) } else { (k.accent_soft, k.accent_text) };
    ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
    crate::ui::glyph(ui.painter(), rect.center(), icon, 20.0, 1.7, fg);
}

/// The header's close cross.
pub fn close_cross(ui: &mut Ui, x: Rect) -> Response {
    let k = theme::c();
    let r = ui.interact(x, ui.id().with(("modal_close", x.min.x as i32, x.min.y as i32)), Sense::click()).on_hover_text("Close (Esc)");
    if r.hovered() {
        ui.painter().rect_filled(x, CornerRadius::same(7), k.widget_hover);
    }
    let col = if r.hovered() { k.text } else { k.faint };
    for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
        ui.painter().line_segment([x.center() - d, x.center() + d], Stroke::new(1.5_f32, col));
    }
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The footer's hint: text and keycaps.
pub fn hint_row(ui: &mut Ui, h: &[Hint]) {
    let k = theme::c();
    ui.spacing_mut().item_spacing.x = 5.0;
    for (i, part) in h.iter().enumerate() {
        match part {
            Hint::Key(c) => {
                keycap(ui, *c);
            }
            Hint::Text(t) => {
                ui.add(egui::Label::new(egui::RichText::new(t).size(theme::FS_SECONDARY).color(k.faint)).truncate());
                if i + 1 < h.len() {
                    ui.add_space(6.0);
                }
            }
        }
    }
}

/// A key as a small keycap, its lower edge thicker.
pub fn keycap(ui: &mut Ui, c: Cap) -> Response {
    let k = theme::c();
    let font = FontId::proportional(theme::FS_SMALL);
    let g = match c {
        Cap::Text(t) => Some(ui.painter().layout_no_wrap(t.to_string(), font, k.dim)),
        _ => None,
    };
    let w = g.as_ref().map_or(16.0, |g| g.size().x + 10.0).max(16.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), k.border_strong);
    p.rect(Rect::from_min_max(rect.min, rect.max - vec2(0.0, 2.0)), CornerRadius::same(4), theme::tint(k.sidebar, k.elevated, 0.6), Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
    let c0 = rect.center() - vec2(0.0, 1.0);
    match (c, g) {
        (_, Some(g)) => {
            p.galley(c0 - g.size() / 2.0, g, k.dim);
        }
        (Cap::Up | Cap::Down, None) => {
            let s = if c == Cap::Up { -1.0 } else { 1.0 };
            let st = Stroke::new(1.3_f32, k.dim);
            p.line_segment([c0 + vec2(0.0, -4.0 * s), c0 + vec2(0.0, 4.0 * s)], st);
            p.add(Shape::line(vec![c0 + vec2(-3.0, 1.0 * s), c0 + vec2(0.0, 4.0 * s), c0 + vec2(3.0, 1.0 * s)], st));
        }
        _ => {}
    }
    resp
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Primary,
    Danger,
}

/// A footer button, 30 points high.
pub fn button(ui: &mut Ui, text: &str, kind: Kind, enabled: bool) -> Response {
    let k = theme::c();
    let fg = if kind == Kind::Plain { k.text } else { k.on_accent };
    let g = ui.painter().layout_no_wrap(text.to_string(), theme::body(), fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 28.0, 30.0), if enabled { Sense::click() } else { Sense::hover() });
    let mut p = ui.painter().clone();
    if !enabled {
        p.multiply_opacity(0.45);
    }
    let hot = enabled && resp.hovered();
    let rr = CornerRadius::same(8);
    match kind {
        Kind::Plain => {
            let stroke = if hot { k.widget_hover_border } else { k.border_strong };
            p.rect(rect, rr, if hot { k.widget_active } else { k.widget_hover }, Stroke::new(1.0_f32, stroke), StrokeKind::Inside);
        }
        Kind::Primary | Kind::Danger => {
            let base = if kind == Kind::Danger { k.deleted } else { k.accent };
            let fill = if hot { theme::lighten(base, 0.08) } else { base };
            if enabled {
                p.add(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: base.gamma_multiply(0.3) }.as_shape(rect, rr));
            }
            p.rect_filled(rect, rr, fill);
            p.hline(rect.shrink2(vec2(6.0, 0.0)).x_range(), rect.top() + 0.5, Stroke::new(1.0_f32, Color32::from_white_alpha(28)));
        }
    }
    p.galley(rect.center() - g.size() / 2.0, g, fg);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// One choice of a dialog: a card with a radio dot, a title, an explanation line and its number
/// key. The selected card is in the accent (red when `danger`); a disabled one is dimmed and says
/// why instead.
pub fn choice(ui: &mut Ui, title: &str, line: &str, key: Option<usize>, selected: bool, danger: bool, disabled: Option<&str>) -> Response {
    let k = theme::c();
    let w = ui.available_width();
    let tc = if danger { k.deleted } else { k.strong };
    let title_g = ui.painter().layout_no_wrap(title.to_string(), theme::body(), tc);
    let (sub, sc) = match disabled {
        Some(why) => (why, k.faint),
        None => (line, k.dim),
    };
    let sub_g = ui.painter().layout(sub.to_string(), theme::secondary(), sc, w - 38.0 - 12.0);
    let h = 10.0 + title_g.size().y + 2.0 + sub_g.size().y + 10.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if disabled.is_some() { Sense::hover() } else { Sense::click() });
    let mut p = ui.painter().clone();
    if disabled.is_some() {
        p.multiply_opacity(0.45);
    }
    let col = if danger { k.deleted } else { k.accent };
    let rr = CornerRadius::same(10);
    let hot = disabled.is_none() && resp.hovered();
    if selected {
        p.rect_stroke(rect.expand(3.0), CornerRadius::same(13), Stroke::new(3.0_f32, col.gamma_multiply(0.18)), StrokeKind::Inside);
        p.rect(rect, rr, theme::tint(col, k.input, 0.12), Stroke::new(1.0_f32, col), StrokeKind::Inside);
    } else {
        p.rect(rect, rr, k.input, Stroke::new(1.0_f32, if hot { k.widget_hover_border } else { k.border_strong }), StrokeKind::Inside);
    }
    let dot = pos2(rect.left() + 20.0, rect.top() + 10.0 + title_g.size().y / 2.0);
    if selected {
        p.circle_filled(dot, 7.0, col);
        p.circle_filled(dot, 5.5, k.elevated);
        p.circle_filled(dot, 3.0, col);
    } else {
        p.circle_stroke(dot, 6.5, Stroke::new(1.5_f32, k.faint));
    }
    let tx = rect.left() + 38.0;
    p.galley(pos2(tx, rect.top() + 10.0), title_g.clone(), tc);
    p.galley(pos2(tx, rect.top() + 12.0 + title_g.size().y), sub_g, sc);
    if let Some(n) = key {
        let g = p.layout_no_wrap(n.to_string(), theme::small(), k.faint);
        let kr = Rect::from_center_size(pos2(rect.right() - 20.0, dot.y), vec2(g.size().x + 10.0, 16.0));
        p.rect_stroke(kr, CornerRadius::same(4), Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
        p.galley(kr.center() - g.size() / 2.0, g, k.faint);
    }
    if disabled.is_none() {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// One choice of a list, `h` points tall, flat like a row of the sidebar: a radio dot, its name and
/// its number key. The selected one on the accent's soft colour (its name red when `danger`), a
/// disabled one dimmed, saying why in a tooltip. Rows the same height as a graph's rows line up
/// with it.
pub fn list_row(ui: &mut Ui, title: &str, key: Option<usize>, selected: bool, danger: bool, disabled: Option<&str>, h: f32) -> Response {
    let k = theme::c();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if disabled.is_some() { Sense::hover() } else { Sense::click() });
    let mut p = ui.painter().clone();
    if disabled.is_some() {
        p.multiply_opacity(0.45);
    }
    let col = if danger { k.deleted } else { k.accent };
    let hot = disabled.is_none() && resp.hovered();
    let row = rect.shrink2(vec2(0.0, 1.0));
    if selected {
        p.rect_filled(row, CornerRadius::same(7), theme::tint(col, k.bg, 0.16));
    } else if hot {
        p.rect_filled(row, CornerRadius::same(7), k.hover);
    }
    let dot = pos2(rect.left() + 18.0, rect.center().y);
    if selected {
        p.circle_filled(dot, 7.0, col);
        p.circle_filled(dot, 4.0, theme::tint(col, k.bg, 0.16));
    } else {
        p.circle_stroke(dot, 6.0, Stroke::new(1.5_f32, k.faint));
    }
    let tc = if danger { k.deleted } else if selected { k.strong } else { k.dim };
    let mut right = rect.right() - 10.0;
    if let Some(n) = key {
        let g = p.layout_no_wrap(n.to_string(), theme::small(), k.faint);
        let kr = Rect::from_center_size(pos2(rect.right() - 18.0, dot.y), vec2(g.size().x + 10.0, 16.0));
        p.rect_stroke(kr, CornerRadius::same(4), Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
        p.galley(kr.center() - g.size() / 2.0, g, k.faint);
        right = kr.left() - 6.0;
    }
    crate::ui::fit_text(&p, pos2(rect.left() + 34.0, dot.y), Align2::LEFT_CENTER, title, theme::body(), tc, right - rect.left() - 34.0);
    match disabled {
        None => resp.on_hover_cursor(egui::CursorIcon::PointingHand),
        Some(why) => resp.on_hover_text(why),
    }
}

/// A row of options of which one is on, like a segmented control. `why[i]`: why option `i` is
/// greyed out, shown when hovered. Returns the option clicked.
pub fn segmented(ui: &mut Ui, options: &[&str], selected: usize, why: &[Option<String>]) -> Option<usize> {
    let k = theme::c();
    // Smaller and tighter when the room is short.
    let fit = |size: f32, pad: f32| -> Vec<f32> { options.iter().map(|o| ui.painter().layout_no_wrap(o.to_string(), FontId::proportional(size), k.text).size().x + pad).collect() };
    let room = ui.available_width();
    let (mut size, mut widths) = (12.0, fit(12.0, 16.0));
    while widths.iter().sum::<f32>() + 6.0 > room && size > 10.0 {
        size -= 0.5;
        widths = fit(size, 10.0);
    }
    let font = FontId::proportional(size);
    let total = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 24.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(6), k.input, Stroke::new(1.0_f32, k.border), StrokeKind::Inside);
    let mut x = rect.left() + 3.0;
    let mut clicked = None;
    for (i, o) in options.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(widths[i], 18.0));
        x += widths[i];
        let off = why.get(i).and_then(|w| w.as_deref());
        let r = ui.interact(cell, ui.id().with(("segmented", options.join("|"), i)), if off.is_some() { Sense::hover() } else { Sense::click() });
        let r = match off {
            Some(w) => r.on_hover_text(w),
            None => r.on_hover_cursor(egui::CursorIcon::PointingHand),
        };
        if i == selected {
            ui.painter().rect_filled(cell, CornerRadius::same(4), k.select);
        } else if r.hovered() && off.is_none() {
            ui.painter().rect_filled(cell, CornerRadius::same(4), k.hover);
        }
        let col = if off.is_some() { k.faint.gamma_multiply(0.6) } else if i == selected { k.text } else { k.faint };
        ui.painter().text(cell.center(), Align2::CENTER_CENTER, o, font.clone(), col);
        if r.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// The choice `delta` steps from `cur`, going round and skipping disabled ones.
pub fn step(enabled: &[bool], cur: usize, delta: i32) -> usize {
    let n = enabled.len() as i32;
    if n == 0 || !enabled.iter().any(|e| *e) {
        return cur;
    }
    let mut i = cur as i32;
    let dir = delta.signum();
    for _ in 0..delta.abs() {
        loop {
            i = (i + dir).rem_euclid(n);
            if enabled[i as usize] {
                break;
            }
        }
    }
    i as usize
}

/// The choice number key `n` (1 for the first) picks, when it is enabled.
pub fn numbered(enabled: &[bool], n: usize) -> Option<usize> {
    (n >= 1 && enabled.get(n - 1) == Some(&true)).then(|| n - 1)
}

/// The choice the keyboard picks this frame: Up and Down, or a number key.
pub fn choice_keys(ctx: &egui::Context, enabled: &[bool], cur: usize) -> Option<usize> {
    const NUMS: [Key; 9] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    ctx.input(|i| {
        if i.key_pressed(Key::ArrowDown) {
            return Some(step(enabled, cur, 1));
        }
        if i.key_pressed(Key::ArrowUp) {
            return Some(step(enabled, cur, -1));
        }
        NUMS.iter().position(|k| i.key_pressed(*k)).and_then(|n| numbered(enabled, n + 1))
    })
}

/// A ticked box, 15 points.
pub fn check_box(p: &egui::Painter, c: Pos2, on: bool) {
    let k = theme::c();
    let r = Rect::from_center_size(c, vec2(15.0, 15.0));
    if on {
        p.rect_filled(r, CornerRadius::same(4), k.accent);
        crate::ui::check_mark(p, c + vec2(0.0, 0.5), k.on_accent);
    } else {
        p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.5_f32, k.faint), StrokeKind::Inside);
    }
}

/// A row with a check box, its text and, on the right, a faint note ("2 files"). Toggles `on`.
pub fn check_row(ui: &mut Ui, on: &mut bool, text: &str, side: Option<&str>) -> Response {
    let k = theme::c();
    let w = ui.available_width();
    let g = ui.painter().layout(text.to_string(), theme::body(), k.text, w - 37.0 - if side.is_some() { 90.0 } else { 14.0 });
    let (rect, resp) = ui.allocate_exact_size(vec2(w, (g.size().y + 18.0).max(36.0)), Sense::click());
    let p = ui.painter();
    let hot = resp.hovered();
    p.rect(rect, CornerRadius::same(10), k.input, Stroke::new(1.0_f32, if hot { k.widget_hover_border } else { k.border }), StrokeKind::Inside);
    check_box(p, pos2(rect.left() + 20.0, rect.center().y), *on);
    p.galley(pos2(rect.left() + 37.0, rect.center().y - g.size().y / 2.0), g, k.text);
    if let Some(s) = side {
        p.text(pos2(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, s, theme::secondary(), k.faint);
    }
    if resp.clicked() {
        *on = !*on;
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A one-line text field, taller than egui's, with rounder corners.
pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str) -> Response {
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            w.corner_radius = CornerRadius::same(8);
        }
        ui.add(egui::TextEdit::singleline(text).hint_text(crate::ui::faint_hint(hint)).font(theme::body()).margin(Margin::symmetric(10, 8)).desired_width(f32::INFINITY))
    })
    .inner
}

/// A tinted box for a warning (`col`: modified or deleted), with a drawn warning sign.
pub fn note<R>(ui: &mut Ui, col: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let k = theme::c();
    egui::Frame::new()
        .fill(theme::tint(col, k.elevated, 0.1))
        .stroke(Stroke::new(1.0_f32, theme::tint(col, k.elevated, 0.3)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin { left: 36, right: 12, top: 10, bottom: 10 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let top = ui.min_rect().left_top();
            warning_sign(ui.painter(), top + vec2(-18.0, 8.0), col);
            add(ui)
        })
        .inner
}

/// A triangle with an exclamation mark, centred on `c`.
pub fn warning_sign(p: &egui::Painter, c: Pos2, col: Color32) {
    crate::ui::glyph(p, c, "warning", 16.0, 1.5, col);
}

/// A section title inside a dialog's body ("Release notes").
pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(theme::FS_SECONDARY).color(theme::c().faint));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_grow_and_stay_fixed() {
        assert_eq!([Width::Small, Width::Medium, Width::Large].map(Width::px), [420.0, 520.0, 760.0]);
    }

    #[test]
    fn appearing_eases_out_and_ends_at_one() {
        assert_eq!(eased(0.0), 0.0);
        assert_eq!(eased(APPEAR_SECS), 1.0);
        assert_eq!(eased(10.0), 1.0);
        assert_eq!(eased(-1.0), 0.0);
        let half = eased(APPEAR_SECS / 2.0);
        assert!(half > 0.5 && half < 1.0, "fast at first: {half}");
    }

    #[test]
    fn arrows_go_round_and_skip_disabled_choices() {
        let all = [true; 5];
        assert_eq!(step(&all, 0, 1), 1);
        assert_eq!(step(&all, 4, 1), 0);
        assert_eq!(step(&all, 0, -1), 4);
        // Fast-forward is greyed out when the branch has diverged.
        let diverged = [false, true, true, true, true];
        assert_eq!(step(&diverged, 4, 1), 1);
        assert_eq!(step(&diverged, 1, -1), 4);
        assert_eq!(step(&diverged, 2, 2), 4);
        assert_eq!(step(&[false, false], 1, 1), 1, "nothing to move to");
        assert_eq!(step(&[], 0, 1), 0);
    }

    #[test]
    fn number_keys_pick_enabled_choices_only() {
        let diverged = [false, true, true, true, true];
        assert_eq!(numbered(&diverged, 1), None);
        assert_eq!(numbered(&diverged, 2), Some(1));
        assert_eq!(numbered(&diverged, 5), Some(4));
        assert_eq!(numbered(&diverged, 6), None);
        assert_eq!(numbered(&diverged, 0), None);
    }
}
