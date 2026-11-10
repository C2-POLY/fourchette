//! The photo of the board's third view (#222) and its postcard with a changelog (#223). The
//! camera in the board's bar frames the whole view for one frame (fitted, without the hover's
//! bubble, its caption in the corner) and asks the renderer for the window's image, cropped to
//! the view; then the shutter clicks, a white flash covers the view and the photo shrinks into a
//! thumbnail in the bottom-right corner, with Share… (the macOS menu), Copy and Save…. The
//! postcard first has Claude write the changelog of the chosen audience, takes the view's photo
//! exactly as the camera does (without the caption), then composes a vintage postcard around it
//! (#232), drawn over the window for a couple of frames and photographed: "Greetings from" in the
//! paper's margin above the photo, never over it, the changelog as the message, a stamp, a
//! postmark and the address. The photo is then pasted back at its own resolution, and the card
//! laid on the skin's table when asked (`snapshot::on_table`). Nothing is written in the
//! repository or on GitHub.

use super::*;
use crate::changelog::{self, Audience};
use crate::snapshot::{self as shot, Phase, Surface};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};
use egui::emath::GuiRounding;
use std::sync::Arc;

/// How wide the thumbnail is.
const THUMB_W: f32 = 260.0;
/// The postcard's margins, its title band above the photo, the caption under it, and the
/// message side's width against the photo's.
const PAD: f32 = 28.0;
const TITLE_H: f32 = 108.0;
const CAPTION_H: f32 = 28.0;
const MESSAGE_SHARE: f32 = 0.62;

/// The postcard is paper in light and dark mode alike: its colours don't follow the theme.
const PAPER: Color32 = Color32::from_rgb(246, 240, 226);
const PAPER_EDGE: Color32 = Color32::from_rgb(222, 211, 190);
const INK: Color32 = Color32::from_rgb(40, 40, 54);
const INK_TEXT: Color32 = Color32::from_rgb(62, 59, 70);
const INK_DIM: Color32 = Color32::from_rgb(124, 116, 104);
const RULE: Color32 = Color32::from_rgb(206, 194, 172);
const LINK: Color32 = Color32::from_rgb(31, 95, 168);
const POSTMARK: Color32 = Color32::from_rgba_premultiplied(30, 44, 84, 175);
const STAMP: Color32 = Color32::from_rgb(38, 150, 140);
const LETTERS: Color32 = Color32::from_rgb(244, 190, 72);
const LETTERS_SHADE: Color32 = Color32::from_rgb(150, 52, 36);

/// The renderer's image of the window, on its way.
type Shooting = Receiver<Arc<egui::ColorImage>>;

#[derive(Default)]
pub struct SnapState {
    /// The audience last chosen for a postcard.
    pub audience: Audience,
    /// Claude writing the postcard's changelog for an audience.
    pub writing: Option<(Audience, Receiver<Result<String, String>>)>,
    /// A photo to take: drawn framed, then asked of the renderer.
    pub framing: Option<Framing>,
    /// The renderer's image of the framed view on its way, with where the view was, the pixels
    /// per point and the postcard it is for.
    waiting: Option<(Shooting, Rect, f32, Option<Card>)>,
    /// The postcard being composed around its photo.
    composing: Option<Compose>,
    /// The postcard being laid on its table in a thread, with where it was on screen.
    developing: Option<(Receiver<egui::ColorImage>, Rect)>,
    /// The photo taken, while its animation and thumbnail show.
    pub photo: Option<Photo>,
    /// The screenshot mode's photo shows as a thumbnail at once.
    pub instant: bool,
}

/// What a postcard says, and whether it lies on the skin's table or stands alone.
#[derive(Clone)]
pub struct Card {
    audience: Audience,
    text: String,
    table: bool,
}

#[derive(Clone)]
pub struct Framing {
    /// The postcard the view's photo is for; None for a photo alone.
    card: Option<Card>,
    pub since: Instant,
    /// Frames drawn framed so far, and whether the renderer was asked for the image: the view
    /// stays framed until it arrives, since the renderer takes the frame after the request.
    frames: u32,
    asked: bool,
}

impl Framing {
    fn new(card: Option<Card>) -> Self {
        Framing { card, since: Instant::now(), frames: 0, asked: false }
    }
}

/// A postcard composed around the view's photo: drawn over the window, then photographed.
struct Compose {
    card: Card,
    photo: egui::ColorImage,
    texture: egui::TextureHandle,
    names: (String, String),
    date: (i32, u32, u32),
    since: Instant,
    frames: u32,
    /// Where the card and its photo were drawn, and the renderer's image on its way.
    drawn: Option<(Rect, Rect)>,
    waiting: Option<(Shooting, f32)>,
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

    /// Whether a photo is on its way, from the framing to the thumbnail.
    fn busy(&self) -> bool {
        self.framing.is_some() || self.waiting.is_some() || self.composing.is_some() || self.developing.is_some()
    }

    /// Whether a photo is being taken or waits as a thumbnail, so Esc closes it first.
    pub fn close(&mut self) -> bool {
        let open = self.photo.is_some() || self.writing.is_some();
        self.photo = None;
        self.writing = None;
        open
    }
}

/// The table under the postcard for a skin: its city's world, as a surface.
fn surface(skin: crate::settings::Skin) -> Surface {
    use crate::settings::Skin;
    match skin {
        Skin::Normal => Surface::Wood,
        Skin::Runner => Surface::Tartan,
        Skin::Trail => Surface::Trail,
        Skin::Eater => Surface::Gingham,
        Skin::Gamer | Skin::Player => Surface::Desk,
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
        if !self.snap.busy() {
            self.snap.photo = None;
            self.snap.framing = Some(Framing::new(None));
        }
    }

    /// The postcard for `audience`: Claude writes the changelog of what isn't released yet and
    /// of the latest release, then the photo is taken and the card composed around it.
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

    /// Takes in Claude's changelog and the renderer's images.
    pub(super) fn snapshot_poll(&mut self, ctx: &egui::Context) {
        if let Some(res) = self.snap.writing.as_ref().and_then(|(_, rx)| rx.try_recv().ok()) {
            let audience = self.snap.writing.take().map(|(a, _)| a).unwrap_or_default();
            match res {
                Ok(text) if text.trim().is_empty() => self.notify(format!("Nothing to tell {} yet: no change since the last release", audience.label().to_lowercase()), true),
                Ok(text) => {
                    // The postcard needs the view on screen.
                    self.board.open = true;
                    self.board.city_on = true;
                    self.snap.framing = Some(Framing::new(Some(Card { audience, text, table: self.postcard_table })));
                }
                Err(e) => self.notify(format!("Claude couldn't write the changelog: {e}"), true),
            }
        }
        if self.snap.framing.as_ref().is_some_and(|f| f.since.elapsed().as_secs_f32() > 3.0) || self.snap.composing.as_ref().is_some_and(|c| c.since.elapsed().as_secs_f32() > 3.0) {
            self.snap.framing = None;
            self.snap.waiting = None;
            self.snap.composing = None;
            self.notify("Couldn't take the photo: show the City view and try again", true);
        }
        if let Some((rx, from)) = &self.snap.developing {
            if let Ok(img) = rx.try_recv() {
                let from = *from;
                self.snap.developing = None;
                self.snapshot_keep(ctx, img, from, true);
            }
            return;
        }
        self.snapshot_composed(ctx);
        let Some((rx, area, ppp, _)) = &self.snap.waiting else { return };
        let Ok(img) = rx.try_recv() else { return };
        let (area, ppp) = (*area, *ppp);
        let card = self.snap.waiting.take().and_then(|w| w.3);
        self.snap.framing = None;
        let Some(img) = shot::crop(&img, area, ppp) else {
            self.notify("Couldn't take the photo", true);
            return;
        };
        match card {
            // The view's photo, untouched: the card is composed around it.
            Some(card) => {
                let texture = ctx.load_texture("postcard_photo", img.clone(), egui::TextureOptions::LINEAR);
                self.snap.composing = Some(Compose { card, photo: img, texture, names: self.snapshot_names(), date: shot::now().0, since: Instant::now(), frames: 0, drawn: None, waiting: None });
                ctx.request_repaint();
            }
            None => self.snapshot_keep(ctx, img, area, false),
        }
    }

    /// Takes in the composed postcard: cropped to the card, the photo pasted back at its own
    /// resolution, then laid on its table in a thread or kept.
    fn snapshot_composed(&mut self, ctx: &egui::Context) {
        let Some(c) = &self.snap.composing else { return };
        let (Some((card, photo)), Some((rx, ppp))) = (c.drawn, &c.waiting) else { return };
        let Ok(img) = rx.try_recv() else { return };
        let ppp = *ppp;
        let Some(c) = self.snap.composing.take() else { return };
        let Some(mut img) = shot::crop(&img, card, ppp) else {
            self.notify("Couldn't take the photo", true);
            return;
        };
        let at = Rect::from_min_max((photo.min - card.min.to_vec2()) * ppp, (photo.max - card.min.to_vec2()) * ppp);
        shot::paste(&mut img, &c.photo, at);
        match c.card.table.then(|| surface(self.skin)) {
            Some(surface) => {
                // The table takes a moment at Retina size: in a thread, the window keeps going.
                let (tx, rx) = channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(shot::on_table(&img, surface));
                    ctx.request_repaint();
                });
                self.snap.developing = Some((rx, card));
            }
            None => self.snapshot_keep(ctx, img, card, true),
        }
    }

    /// Keeps the photo taken: encoded, written in the temporary folder, and on its way to the thumbnail.
    fn snapshot_keep(&mut self, ctx: &egui::Context, img: egui::ColorImage, area: Rect, postcard: bool) {
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

    /// Draws the view framed for its photo, in `ui`'s whole room: the view fitted and, for a
    /// photo alone, the caption; from the second frame, asks the renderer for the window's image,
    /// and stays so until it arrives. Returns the ticket clicked, as the view does.
    pub(super) fn snapshot_framed(&mut self, ui: &mut Ui, shown: &[&crate::github::Item], detail: Option<&str>) -> Option<String> {
        let framing = self.snap.framing.clone()?;
        let area = ui.available_rect_before_wrap();
        // Fitted, whatever the person did to the view, and without the hover's bubble.
        let kept = self.board.city.take();
        self.board.shooting = true;
        let clicked = ui.scope_builder(UiBuilder::new().max_rect(area), |ui| self.third_view(ui, shown, detail)).inner;
        self.board.shooting = false;
        self.board.city = kept;
        // A postcard says it all on its paper; the photo alone carries the caption.
        if framing.card.is_none() {
            let (repo, project) = self.snapshot_names();
            caption(ui.painter(), area, &shot::caption(&repo, &project, shot::now().0));
        }

        let f = self.snap.framing.as_mut().expect("framing");
        f.frames += 1;
        // The first frame lays the view out fitted; the next ones are photographed.
        if f.frames >= 2 && !f.asked {
            f.asked = true;
            let (tx, rx) = channel();
            let ctx = ui.ctx().clone();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let _ = tx.send(img);
                ctx.request_repaint();
            })));
            self.snap.waiting = Some((rx, area, ui.ctx().pixels_per_point(), framing.card));
        }
        ui.ctx().request_repaint();
        clicked
    }

    /// Draws the postcard being composed over the whole window, centred; from the second frame,
    /// asks the renderer for the window's image.
    fn snapshot_compose(&mut self, ctx: &egui::Context) {
        let Some(c) = self.snap.composing.as_mut() else { return };
        let size = c.texture.size_vec2();
        let l = Postcard::new(ctx.content_rect(), size.x / size.y.max(1.0));
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("postcard_compose"));
        let mut ui = Ui::new(ctx.clone(), egui::Id::unique("postcard_compose"), UiBuilder::new().layer_id(layer).max_rect(l.card));
        let (repo, project) = &c.names;
        postcard(&mut ui, &l, &c.card, &c.texture, repo, project, c.date);
        c.drawn = Some((l.card, l.photo));
        c.frames += 1;
        if c.frames >= 2 && c.waiting.is_none() {
            let (tx, rx) = channel();
            let cx = ctx.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let _ = tx.send(img);
                cx.request_repaint();
            })));
            c.waiting = Some((rx, ctx.pixels_per_point()));
        }
        ctx.request_repaint();
    }

    /// The flash, the photo shrinking into its thumbnail and the thumbnail with its buttons; and
    /// Claude's wait for a postcard. `lift`: how much higher than usual it stands (above a skin's bar).
    pub(super) fn snapshot_ui(&mut self, ctx: &egui::Context, lift: f32) {
        self.snapshot_compose(ctx);
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

    /// FOURCHETTE_CITY_SHOT=1|postcard[,table]: the photo, or the postcard with the sample
    /// changelog (on the skin's table), taken at once and shown as a thumbnail.
    pub(super) fn snapshot_sample(&mut self, postcard: bool, table: bool) {
        self.snap.instant = true;
        let card = postcard.then(|| {
            let (versions, notes, gh) = changelog::sample();
            let versions = shot::postcard_versions(versions);
            let text = shot::postcard_text(&versions, &notes, Some(&gh));
            Card { audience: Audience::Leadership, text, table }
        });
        self.snap.framing = Some(Framing::new(card));
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

/// Where each part of the postcard sits, around a photo of the view at its own proportions.
struct Postcard {
    card: Rect,
    /// "Greetings from" and the repository's name, in the paper above the photo.
    title: Rect,
    /// The view's photo, untouched, with its printed caption under it.
    photo: Rect,
    /// The line between the photo and the message.
    divider: f32,
    /// The message side: the stamp in its top-right corner, the address at its bottom.
    message: Rect,
    stamp: Rect,
}

impl Postcard {
    /// The postcard as large as fits in `screen`, around a photo `aspect` wide for one tall.
    fn new(screen: Rect, aspect: f32) -> Self {
        let room = screen.shrink(32.0);
        let w = ((room.width() - 4.0 * PAD) / (1.0 + MESSAGE_SHARE)).min((room.height() - 2.0 * PAD - TITLE_H - CAPTION_H) * aspect).max(120.0);
        let h = w / aspect.max(0.1);
        let size = vec2(w * (1.0 + MESSAGE_SHARE) + 4.0 * PAD, 2.0 * PAD + TITLE_H + h + CAPTION_H);
        let card = Rect::from_center_size(room.center(), size).round_ui();
        let title = Rect::from_min_size(card.min + vec2(PAD, PAD), vec2(w, TITLE_H));
        let photo = Rect::from_min_size(pos2(card.left() + PAD, title.bottom()), vec2(w, h)).round_ui();
        let divider = photo.right() + PAD;
        let message = Rect::from_min_max(pos2(divider + PAD, card.top() + PAD), card.max - vec2(PAD, PAD));
        let stamp = Rect::from_min_size(pos2(message.right() - 72.0, message.top()), vec2(72.0, 86.0));
        Postcard { card, title, photo, divider, message, stamp }
    }
}

fn medium(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name(theme::MEDIUM.into()))
}

/// The whole postcard: the paper, the titles above the photo, the photo, its caption, the
/// divider, the stamp, the postmark, the message and the address.
fn postcard(ui: &mut Ui, l: &Postcard, card: &Card, photo: &egui::TextureHandle, repo: &str, project: &str, date: (i32, u32, u32)) {
    let p = ui.painter().clone();
    p.add(egui::epaint::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(110) }.as_shape(l.card, CornerRadius::ZERO));
    p.rect(l.card, CornerRadius::ZERO, PAPER, Stroke::new(1.0_f32, PAPER_EDGE), StrokeKind::Inside);
    greetings(&p, l.title, repo);
    p.image(photo.id(), l.photo, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
    p.rect_stroke(l.photo, CornerRadius::ZERO, Stroke::new(1.0_f32, INK.gamma_multiply(0.35)), StrokeKind::Outside);
    let printed = if project.is_empty() { "Printed by Fourchette".to_string() } else { format!("{project} · printed by Fourchette") };
    p.text(pos2(l.photo.left(), l.photo.bottom() + 10.0), Align2::LEFT_TOP, printed, FontId::proportional(theme::FS_SMALL), INK_DIM);

    // The line down the middle, and the card's name across the top of the message side.
    p.line_segment([pos2(l.divider, l.card.top() + PAD * 1.6), pos2(l.divider, l.card.bottom() - PAD * 1.6)], Stroke::new(1.0_f32, RULE));
    p.text(pos2(l.message.left(), l.message.top() + 2.0), Align2::LEFT_TOP, "P O S T   C A R D", medium(13.0), INK_DIM);

    stamp(&p, l.stamp);
    // Over the stamp's lower-left corner, its date clear of it.
    postmark(&p, pos2(l.stamp.left() - 24.0, l.stamp.bottom() - 10.0), l.message.left(), date);

    // The address: who it is to and from, on ruled lines.
    let line = 30.0;
    let address = Rect::from_min_max(pos2(l.message.left(), l.message.bottom() - 2.0 * line), l.message.max);
    for (i, (label, value)) in [("To", card.audience.label().to_string()), ("From", repo.to_string())].into_iter().enumerate() {
        let y = address.top() + line * (i as f32 + 1.0);
        p.hline(address.x_range(), y, Stroke::new(1.0_f32, RULE));
        p.text(pos2(address.left(), y - 5.0), Align2::LEFT_BOTTOM, label, FontId::proportional(theme::FS_SMALL), INK_DIM);
        p.text(pos2(address.left() + 44.0, y - 4.0), Align2::LEFT_BOTTOM, value, medium(15.0), INK);
    }

    // The message: the changelog in ink, cut with a fade into the paper when it is too long.
    let body = Rect::from_min_max(pos2(l.message.left(), l.stamp.bottom() + 18.0), pos2(l.message.right(), address.top() - 16.0));
    let mut child = ui.new_child(UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(body);
    child.label(RichText::new("What's new").font(medium(21.0)).color(INK));
    child.label(RichText::new(format!("Dear {},", card.audience.label().to_lowercase())).italics().size(theme::FS_SECONDARY).color(INK_DIM));
    child.add_space(2.0);
    let ink = super::board::Ink { strong: INK, text: INK_TEXT, dim: INK_DIM, rule: RULE, link: LINK };
    super::board::markdown_ink(&mut child, &card.text, &ink);
    if child.min_rect().bottom() > body.bottom() {
        let foot = 22.0;
        let fade = Rect::from_min_max(pos2(body.left(), body.bottom() - foot - 40.0), pos2(body.right(), body.bottom() - foot));
        let steps = 14;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let band = Rect::from_min_max(pos2(fade.left(), fade.top() + fade.height() * t), pos2(fade.right(), fade.top() + fade.height() * (t + 1.0 / steps as f32) + 0.5));
            p.rect_filled(band, CornerRadius::ZERO, PAPER.gamma_multiply(t));
        }
        p.rect_filled(Rect::from_min_max(pos2(body.left(), body.bottom() - foot), body.max), CornerRadius::ZERO, PAPER);
        p.text(pos2(body.left(), body.bottom()), Align2::LEFT_BOTTOM, "More in the full changelog…", FontId::proportional(theme::FS_SMALL), INK_DIM);
    }
}

/// "Greetings from" on a paper ribbon, then the repository's name in big retro letters (gold,
/// outlined in ink, with a red block shadow), both in the title band above the photo.
fn greetings(p: &egui::Painter, band: Rect, repo: &str) {
    let job = {
        let mut j = egui::text::LayoutJob::default();
        j.append("Greetings from", 0.0, egui::TextFormat { font_id: FontId::proportional(17.0), color: INK, italics: true, ..Default::default() });
        j
    };
    let g = p.layout_job(job);
    let ribbon = Rect::from_min_size(band.min, vec2(g.size().x + 24.0, 30.0));
    p.add(egui::epaint::Shadow { offset: [0, 2], blur: 6, spread: 0, color: Color32::from_black_alpha(45) }.as_shape(ribbon, CornerRadius::same(3)));
    p.rect(ribbon, CornerRadius::same(3), Color32::WHITE, Stroke::new(1.0_f32, PAPER_EDGE), StrokeKind::Inside);
    p.galley(ribbon.center() - g.size() / 2.0, g, INK);

    let name = repo.to_uppercase();
    let room = band.width() - 8.0;
    let mut size = 52.0;
    let width = |s: f32| p.layout_no_wrap(name.clone(), medium(s), INK).size().x + s * 0.16;
    if width(size) > room {
        size *= room / width(size);
    }
    let at = pos2(band.left(), ribbon.bottom() + 6.0);
    let depth = (size * 0.08).max(2.0);
    let outline = (size * 0.035).max(1.2);
    // The block shadow, step by step down to the right.
    let mut d = depth;
    while d > 0.0 {
        p.text(at + vec2(d, d), Align2::LEFT_TOP, &name, medium(size), LETTERS_SHADE);
        d -= 1.0;
    }
    for i in 0..12 {
        let a = i as f32 / 12.0 * std::f32::consts::TAU;
        p.text(at + vec2(a.cos(), a.sin()) * outline, Align2::LEFT_TOP, &name, medium(size), INK);
    }
    p.text(at, Align2::LEFT_TOP, &name, medium(size), LETTERS);
}

/// The stamp: white with a perforated edge, Fourchette's fork in gold on teal, its value and name.
fn stamp(p: &egui::Painter, r: Rect) {
    p.add(egui::epaint::Shadow { offset: [1, 2], blur: 4, spread: 0, color: Color32::from_black_alpha(50) }.as_shape(r, CornerRadius::ZERO));
    p.rect_filled(r, CornerRadius::ZERO, Color32::WHITE);
    // The perforations: paper-coloured bites all round the edge.
    let hole = 2.6;
    let step = 8.0;
    let (nx, ny) = ((r.width() / step).round() as i32, (r.height() / step).round() as i32);
    for i in 0..=nx {
        let x = r.left() + r.width() * i as f32 / nx as f32;
        p.circle_filled(pos2(x, r.top()), hole, PAPER);
        p.circle_filled(pos2(x, r.bottom()), hole, PAPER);
    }
    for i in 0..=ny {
        let y = r.top() + r.height() * i as f32 / ny as f32;
        p.circle_filled(pos2(r.left(), y), hole, PAPER);
        p.circle_filled(pos2(r.right(), y), hole, PAPER);
    }
    let inner = r.shrink(7.0);
    p.rect_filled(inner, CornerRadius::ZERO, STAMP);
    super::art::fork(p, inner.center() + vec2(0.0, -2.0), inner.height() * 0.62, super::art::GOLD);
    p.text(inner.left_top() + vec2(4.0, 3.0), Align2::LEFT_TOP, "50", medium(10.0), Color32::WHITE);
    p.text(pos2(inner.center().x, inner.bottom() - 3.0), Align2::CENTER_BOTTOM, "FOURCHETTE", medium(7.0), Color32::WHITE);
}

/// The postmark over the stamp's corner: two rings with the date, and the wavy cancellation lines
/// running left as far as `left`.
fn postmark(p: &egui::Painter, c: Pos2, left: f32, date: (i32, u32, u32)) {
    let s = Stroke::new(1.6_f32, POSTMARK);
    p.circle_stroke(c, 34.0, s);
    p.circle_stroke(c, 27.0, Stroke::new(1.0_f32, POSTMARK));
    let day = shot::day(date).to_uppercase();
    let (d, rest) = day.split_once(' ').unwrap_or((&day, ""));
    let (month, year) = rest.split_once(' ').unwrap_or((rest, ""));
    p.text(c + vec2(0.0, -12.0), Align2::CENTER_CENTER, "FOURCHETTE", medium(7.5), POSTMARK);
    p.text(c + vec2(0.0, 1.0), Align2::CENTER_CENTER, format!("{d} {month}"), medium(13.0), POSTMARK);
    p.text(c + vec2(0.0, 13.0), Align2::CENTER_CENTER, year, medium(9.0), POSTMARK);
    let end = (c.x - 40.0).max(left + 40.0);
    let start = (end - 150.0).max(left + 70.0);
    for k in -1..=1 {
        let y = c.y + k as f32 * 9.0;
        let pts: Vec<Pos2> = (0..=40).map(|i| {
            let x = start + (end - start) * i as f32 / 40.0;
            pos2(x, y + ((x - start) / 9.0).sin() * 2.6)
        }).collect();
        p.add(egui::Shape::line(pts, s));
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
