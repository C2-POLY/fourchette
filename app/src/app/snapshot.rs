//! The photo of the board's third view (#222) and its postcard with a changelog (#223). The
//! camera in the board's bar frames the whole view for one frame (fitted, without the hover's
//! bubble, its caption in the corner) and asks the renderer for the window's image, cropped to
//! the view; then the shutter clicks, a white flash covers the view and the photo shrinks into a
//! thumbnail in the bottom-right corner, with Share… (the macOS menu), Copy and Save…. The
//! postcard first has Claude write the changelog of the chosen audience, then frames the view
//! beside a card with it. Nothing is written in the repository or on GitHub.

use super::*;
use crate::changelog::{self, Audience};
use crate::snapshot::{self as shot, Phase};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};
use std::sync::Arc;

/// How wide the thumbnail is, and how much of the postcard the view takes.
const THUMB_W: f32 = 260.0;
const VIEW_SHARE: f32 = 0.6;

#[derive(Default)]
pub struct SnapState {
    /// The audience last chosen for a postcard.
    pub audience: Audience,
    /// Claude writing the postcard's changelog for an audience.
    pub writing: Option<(Audience, Receiver<Result<String, String>>)>,
    /// A photo to take: drawn framed, then asked of the renderer.
    pub framing: Option<Framing>,
    /// The renderer's image on its way, with where the view was and the pixels per point.
    waiting: Option<(Receiver<Arc<egui::ColorImage>>, Rect, f32, bool)>,
    /// The photo taken, while its animation and thumbnail show.
    pub photo: Option<Photo>,
    /// The screenshot mode's photo shows as a thumbnail at once.
    pub instant: bool,
}

#[derive(Clone)]
pub struct Framing {
    /// The postcard's changelog and its audience; None for a photo alone.
    pub postcard: Option<(Audience, String)>,
    pub since: Instant,
    /// Frames drawn framed so far, and whether the renderer was asked for the image: the view
    /// stays framed until it arrives, since the renderer takes the frame after the request.
    frames: u32,
    asked: bool,
}

impl Framing {
    pub fn new(postcard: Option<(Audience, String)>) -> Self {
        Framing { postcard, since: Instant::now(), frames: 0, asked: false }
    }
}

pub struct Photo {
    texture: egui::TextureHandle,
    png: Vec<u8>,
    path: PathBuf,
    /// Where the view was in the window, in points, and when the photo was taken.
    from: Rect,
    at: f64,
}

impl SnapState {
    /// Whether the view is being drawn for its photo, so what floats over it (a skin's bar, its
    /// litter) stays out of the picture.
    pub fn framing(&self) -> bool {
        self.framing.is_some()
    }

    /// Whether a photo is being taken or waits as a thumbnail, so Esc closes it first.
    pub fn close(&mut self) -> bool {
        let open = self.photo.is_some() || self.writing.is_some();
        self.photo = None;
        self.writing = None;
        open
    }
}

impl Tab {
    /// The repository's name and the project's title, for the caption and the file name.
    fn snapshot_names(&self) -> (String, String) {
        let repo = self.repo.as_ref().map(|r| r.name()).unwrap_or_else(|| "sample-project".into());
        let project = self.project.board.as_ref().and_then(|b| b.project()).map(|p| p.title.clone()).unwrap_or_default();
        (repo, project)
    }

    /// The camera of the board's bar: takes the photo of the whole view.
    pub(super) fn snapshot_take(&mut self) {
        if self.snap.framing.is_none() && self.snap.waiting.is_none() {
            self.snap.photo = None;
            self.snap.framing = Some(Framing::new(None));
        }
    }

    /// The postcard for `audience`: Claude writes the changelog of what isn't released yet and
    /// of the latest release, then the photo is taken beside it.
    pub(super) fn snapshot_postcard(&mut self, ctx: &egui::Context, audience: Audience) {
        self.snap.audience = audience;
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to write its changelog.", true);
            return;
        };
        let slug = r.default_remote().and_then(|rem| r.git(&["remote", "get-url", &rem]).ok()).and_then(|u| crate::github::slug(&u));
        let versions = shot::postcard_versions(changelog::versions(r));
        let model = self.claude_model;
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Without GitHub the changelog still comes from the commits.
            let github = slug.and_then(|s| changelog::github(&s).ok());
            let prompt = changelog::prompt(&versions, github.as_ref(), audience);
            let text = crate::ai::ask(&prompt, crate::ai::SYSTEM, Some(&changelog::schema()), model).map(|v| shot::postcard_text(&versions, &changelog::read_notes(&v, &versions), github.as_ref()));
            let _ = tx.send(text);
            ctx.request_repaint();
        });
        self.snap.photo = None;
        self.snap.writing = Some((audience, rx));
    }

    /// Takes in Claude's changelog and the renderer's image.
    pub(super) fn snapshot_poll(&mut self, ctx: &egui::Context) {
        if let Some(res) = self.snap.writing.as_ref().and_then(|(_, rx)| rx.try_recv().ok()) {
            let audience = self.snap.writing.take().map(|(a, _)| a).unwrap_or_default();
            match res {
                Ok(text) if text.trim().is_empty() => self.notify(format!("Nothing to tell {} yet: no change since the last release", audience.label().to_lowercase()), true),
                Ok(text) => {
                    // The postcard needs the view on screen.
                    self.board.open = true;
                    self.board.city_on = true;
                    self.snap.framing = Some(Framing::new(Some((audience, text))));
                }
                Err(e) => self.notify(format!("Claude couldn't write the changelog: {e}"), true),
            }
        }
        if self.snap.framing.as_ref().is_some_and(|f| f.since.elapsed().as_secs_f32() > 3.0) {
            self.snap.framing = None;
            self.snap.waiting = None;
            self.notify("Couldn't take the photo: show the City view and try again", true);
        }
        let Some((rx, area, ppp, postcard)) = &self.snap.waiting else { return };
        let Ok(img) = rx.try_recv() else { return };
        let (area, ppp, postcard) = (*area, *ppp, *postcard);
        self.snap.waiting = None;
        self.snap.framing = None;
        let Some(img) = shot::crop(&img, area, ppp) else {
            self.notify("Couldn't take the photo", true);
            return;
        };
        let (repo, _) = self.snapshot_names();
        let (date, time) = shot::now();
        let path = std::env::temp_dir().join("Fourchette").join(shot::file_name(&repo, postcard, date, time));
        let mut png = Vec::new();
        let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        let encoded = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, rgba).map(|buf| buf.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png));
        if !matches!(encoded, Some(Ok(()))) {
            self.notify("Couldn't encode the photo", true);
            return;
        }
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
        if let Err(e) = std::fs::write(&path, &png) {
            self.notify(format!("Couldn't keep the photo: {e}"), true);
            return;
        }
        let texture = ctx.load_texture("city_snapshot", img, egui::TextureOptions::LINEAR);
        let now = ctx.input(|i| i.time);
        if !self.snap.instant {
            crate::share::shutter();
        }
        let at = if self.snap.instant { now - 10.0 } else { now };
        self.snap.photo = Some(Photo { texture, png, path, from: area, at });
        ctx.request_repaint();
    }

    /// Draws the view framed for its photo, in `ui`'s whole room: the view fitted (beside the
    /// changelog's card for a postcard) and the caption; from the second frame, asks the renderer
    /// for the window's image, and stays so until it arrives. Returns the ticket clicked, as the view does.
    pub(super) fn snapshot_framed(&mut self, ui: &mut Ui, shown: &[&crate::github::Item], detail: Option<&str>) -> Option<String> {
        let framing = self.snap.framing.clone()?;
        let area = ui.available_rect_before_wrap();
        let view = match framing.postcard {
            Some(_) => Rect::from_min_max(area.min, pos2(area.left() + area.width() * VIEW_SHARE, area.bottom())),
            None => area,
        };
        // Fitted, whatever the person did to the view, and without the hover's bubble.
        let kept = self.board.city.take();
        self.board.shooting = true;
        let clicked = ui.scope_builder(UiBuilder::new().max_rect(view), |ui| self.third_view(ui, shown, detail)).inner;
        self.board.shooting = false;
        self.board.city = kept;
        if let Some((audience, text)) = &framing.postcard {
            let card = Rect::from_min_max(pos2(view.right() + 8.0, area.top() + 24.0), pos2(area.right() - 24.0, area.bottom() - 24.0));
            postcard_card(ui, card, *audience, text);
        }
        let (repo, project) = self.snapshot_names();
        caption(ui.painter(), view, &shot::caption(&repo, &project, shot::now().0));

        let f = self.snap.framing.as_mut().expect("framing");
        f.frames += 1;
        // The first frame lays the view out fitted; the next ones are photographed.
        if f.frames >= 2 && !f.asked {
            f.asked = true;
            let postcard = framing.postcard.is_some();
            let (tx, rx) = channel();
            let ctx = ui.ctx().clone();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let _ = tx.send(img);
                ctx.request_repaint();
            })));
            self.snap.waiting = Some((rx, area, ui.ctx().pixels_per_point(), postcard));
        }
        ui.ctx().request_repaint();
        clicked
    }

    /// The flash, the photo shrinking into its thumbnail and the thumbnail with its buttons; and
    /// Claude's wait for a postcard. `lift`: how much higher than usual it stands (above a skin's bar).
    pub(super) fn snapshot_ui(&mut self, ctx: &egui::Context, lift: f32) {
        let k = theme::c();
        let screen = ctx.content_rect();
        if let Some((audience, _)) = &self.snap.writing {
            let mut cancel = false;
            let audience = *audience;
            egui::Area::new(egui::Id::unique("snapshot_writing")).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, vec2(-16.0, -16.0 - lift)).show(ctx, |ui| {
                crate::modal::card(ui, 12, Margin::same(14), |ui| {
                    ui.set_width(THUMB_W);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Claude is writing the postcard…").color(k.strong));
                    });
                    ui.label(RichText::new(format!("The changelog for {}", audience.label())).size(theme::FS_SMALL).color(k.dim));
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        cancel = crate::modal::button(ui, "Cancel", crate::modal::Kind::Plain, true).clicked();
                    });
                });
            });
            if cancel {
                self.snap.writing = None;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let Some(photo) = &self.snap.photo else { return };
        let now = ctx.input(|i| i.time);
        let size = photo.texture.size_vec2();
        let thumb_h = (THUMB_W * size.y / size.x.max(1.0)).min(220.0);
        let thumb_w = thumb_h * size.x / size.y.max(1.0);
        // The card: the photo, then its buttons; bottom-right.
        let card_size = vec2(thumb_w.max(THUMB_W) + 24.0, thumb_h + 24.0 + 40.0);
        let card = Rect::from_min_size(pos2(screen.right() - 16.0 - card_size.x, screen.bottom() - 16.0 - lift - card_size.y), card_size);
        let thumb = Rect::from_center_size(pos2(card.center().x, card.top() + 12.0 + thumb_h / 2.0), vec2(thumb_w, thumb_h));
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("snapshot_photo"));
        let p = ctx.layer_painter(layer);
        match shot::phase(now - photo.at) {
            Phase::Flash(a) => {
                p.image(photo.texture.id(), photo.from, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
                p.rect_filled(photo.from, CornerRadius::ZERO, Color32::from_white_alpha((a * 235.0) as u8));
                ctx.request_repaint();
            }
            Phase::Shrink(t) => {
                let r = Rect::from_min_max(photo.from.min.lerp(thumb.min, t), photo.from.max.lerp(thumb.max, t));
                p.add(egui::epaint::Shadow { offset: [0, 10], blur: 30, spread: 0, color: theme::shadow(1.2 * t) }.as_shape(r.expand(5.0), CornerRadius::same(6)));
                p.rect_filled(r.expand(5.0 * t), CornerRadius::same(6), Color32::WHITE);
                p.image(photo.texture.id(), r, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
                ctx.request_repaint();
            }
            Phase::Thumb => self.snapshot_thumb(ctx, card, thumb),
        }
    }

    /// The thumbnail's card: the photo in its white border, Share…, Copy, Save… and the cross.
    fn snapshot_thumb(&mut self, ctx: &egui::Context, card: Rect, thumb: Rect) {
        let Some(photo) = &self.snap.photo else { return };
        let k = theme::c();
        let (mut share, mut copy, mut save, mut close) = (None, false, false, false);
        egui::Area::new(egui::Id::unique("snapshot_thumb")).order(egui::Order::Foreground).fixed_pos(card.min).show(ctx, |ui| {
            let p = ui.painter();
            p.add(crate::modal::card_back(card, CornerRadius::same(12)));
            p.rect_stroke(card, CornerRadius::same(12), Stroke::new(1.0_f32, k.border), StrokeKind::Inside);
            p.rect_filled(thumb.expand(4.0), CornerRadius::same(4), Color32::WHITE);
            p.image(photo.texture.id(), thumb, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
            let open = ui.interact(thumb, ui.scope_id().with("open"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Open the image");
            if open.clicked() {
                let _ = std::process::Command::new("open").arg(&photo.path).spawn();
            }
            let x = Rect::from_center_size(pos2(card.right() - 6.0, card.top() + 6.0), vec2(22.0, 22.0));
            ui.painter().circle(x.center(), 11.0, k.elevated, Stroke::new(1.0_f32, k.border));
            close = crate::modal::close_cross(ui, x).clicked();
            let row = Rect::from_min_max(pos2(card.left() + 12.0, thumb.bottom() + 10.0), pos2(card.right() - 12.0, card.bottom() - 6.0));
            ui.scope_builder(UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
                let s = crate::modal::button(ui, "Share…", crate::modal::Kind::Primary, true).on_hover_text("Mail, Messages, AirDrop, Slack…");
                if s.clicked() {
                    share = Some(s.rect);
                }
                copy = crate::modal::button(ui, "Copy", crate::modal::Kind::Plain, true).clicked();
                save = crate::modal::button(ui, "Save…", crate::modal::Kind::Plain, true).clicked();
            });
        });
        let (png, path) = (photo.png.clone(), photo.path.clone());
        if let Some(at) = share {
            if let Err(e) = crate::share::share(&path, at) {
                self.notify(e, true);
            }
        }
        if copy {
            match crate::share::copy_png(&png) {
                Ok(()) => self.notify("Copied the photo: paste it in a message", false),
                Err(e) => self.notify(e, true),
            }
        }
        if save {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if let Some(to) = rfd::FileDialog::new().set_title("Save the photo").set_file_name(&name).add_filter("PNG image", &["png"]).save_file() {
                match std::fs::write(&to, &png) {
                    Ok(()) => self.notify(format!("Saved {}", to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(name)), false),
                    Err(e) => self.notify(format!("Couldn't save the photo: {e}"), true),
                }
            }
        }
        if close {
            self.snap.photo = None;
        }
    }

    /// FOURCHETTE_CITY_SHOT=1|postcard: the photo, or the postcard with the sample changelog,
    /// taken at once and shown as a thumbnail.
    pub(super) fn snapshot_sample(&mut self, postcard: bool) {
        self.snap.instant = true;
        let text = postcard.then(|| {
            let (versions, notes, gh) = changelog::sample();
            let versions = shot::postcard_versions(versions);
            (Audience::Leadership, shot::postcard_text(&versions, &notes, Some(&gh)))
        });
        self.snap.framing = Some(Framing::new(text));
    }
}

/// The caption in the photo's bottom-right corner (the city's hint is on the left): Fourchette's fork dot, then the repository,
/// the project and the date.
fn caption(p: &egui::Painter, view: Rect, text: &str) {
    let k = theme::c();
    let g = p.layout_no_wrap(text.to_string(), FontId::proportional(theme::FS_SECONDARY), k.strong);
    let w = g.size().x + 36.0;
    let pill = Rect::from_min_size(pos2(view.right() - 16.0 - w, view.bottom() - 16.0 - 28.0), vec2(w, 28.0));
    p.add(egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: theme::shadow(0.6) }.as_shape(pill, CornerRadius::same(14)));
    p.rect(pill, CornerRadius::same(14), k.elevated, Stroke::new(1.0_f32, k.border), StrokeKind::Inside);
    p.circle_filled(pos2(pill.left() + 15.0, pill.center().y), 4.0, k.accent);
    p.galley(pos2(pill.left() + 26.0, pill.center().y - g.size().y / 2.0), g, k.strong);
}

/// The postcard's card: "What's new", for whom, then the changelog; what doesn't fit ends in a
/// fade and a line saying there is more.
fn postcard_card(ui: &mut Ui, card: Rect, audience: Audience, text: &str) {
    let k = theme::c();
    let rr = CornerRadius::same(14);
    ui.painter().add(crate::modal::card_back(card, rr));
    ui.painter().rect_stroke(card, rr, Stroke::new(1.0_f32, k.border), StrokeKind::Inside);
    let inner = card.shrink2(vec2(22.0, 20.0));
    let foot = 26.0;
    let body = Rect::from_min_max(inner.min, pos2(inner.right(), inner.bottom() - foot));
    let mut child = ui.new_child(UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(body);
    child.label(RichText::new("What's new").size(theme::FS_TITLE).strong().color(k.strong));
    child.label(RichText::new(format!("For {}", audience.label().to_lowercase())).size(theme::FS_SMALL).color(k.dim));
    child.add_space(4.0);
    super::board::markdown(&mut child, text);
    let used = child.min_rect().bottom();
    let p = ui.painter();
    if used > body.bottom() {
        // A fade over the last lines, then the note.
        let fade = Rect::from_min_max(pos2(card.left() + 1.0, body.bottom() - 36.0), pos2(card.right() - 1.0, body.bottom()));
        let steps = 12;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let band = Rect::from_min_max(pos2(fade.left(), fade.top() + fade.height() * t), pos2(fade.right(), fade.top() + fade.height() * (t + 1.0 / steps as f32) + 0.5));
            p.rect_filled(band, CornerRadius::ZERO, k.elevated.gamma_multiply(t));
        }
        p.text(pos2(inner.left(), inner.bottom() - foot / 2.0), Align2::LEFT_CENTER, "More in the full changelog", FontId::proportional(theme::FS_SMALL), k.faint);
    }
}

/// The camera of the board's bar.
pub(super) fn camera_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    let body = Rect::from_center_size(c + vec2(0.0, 1.0), vec2(16.0, 11.0));
    let s = Stroke::new(1.5_f32, col);
    p.rect_stroke(body, CornerRadius::same(3), s, StrokeKind::Middle);
    p.line_segment([pos2(c.x - 3.5, body.top()), pos2(c.x - 2.0, body.top() - 2.5)], s);
    p.line_segment([pos2(c.x - 2.0, body.top() - 2.5), pos2(c.x + 2.0, body.top() - 2.5)], s);
    p.line_segment([pos2(c.x + 2.0, body.top() - 2.5), pos2(c.x + 3.5, body.top())], s);
    p.circle_stroke(body.center(), 3.0, s);
}

/// The postcard of the board's bar: a card with a stamp and two lines.
pub(super) fn postcard_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    let card = Rect::from_center_size(c, vec2(17.0, 12.0));
    let s = Stroke::new(1.5_f32, col);
    p.rect_stroke(card, CornerRadius::same(2), s, StrokeKind::Middle);
    p.rect_stroke(Rect::from_min_size(pos2(card.right() - 6.5, card.top() + 2.5), vec2(4.0, 4.0)), CornerRadius::ZERO, Stroke::new(1.2_f32, col), StrokeKind::Middle);
    p.line_segment([pos2(card.left() + 3.0, c.y), pos2(c.x + 1.0, c.y)], Stroke::new(1.2_f32, col));
    p.line_segment([pos2(card.left() + 3.0, c.y + 3.0), pos2(c.x - 1.0, c.y + 3.0)], Stroke::new(1.2_f32, col));
}

/// A square button of the board's bar with an icon drawn in it.
pub(super) fn icon_button(ui: &mut Ui, icon: fn(&egui::Painter, Pos2, Color32), hint: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    icon(p, rect.center(), if resp.hovered() { theme::c().strong } else { theme::c().dim });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(hint)
}
