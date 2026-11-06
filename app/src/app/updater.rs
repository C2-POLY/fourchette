//! Updates (issues #80 and #97): the check in the background, on the channel chosen in Settings,
//! the toolbar's pill and the update dialog. The work itself is in `crate::update`; this side only
//! polls its channels each frame and keeps the last check's outcome for the Settings section.
//! Also "What's new" (#137): the items the release brings in the update dialog, and once after an
//! update the "What's new in Fourchette" dialog.
use super::*;
use crate::settings::UpdateChannel;
use crate::update::{self, NewsItem, Progress, Release, Section, Shown, Version};
use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, CornerRadius, Margin, RichText, Sense, Stroke};
use std::sync::mpsc::TryRecvError;

type CheckResult = Result<Option<Release>, String>;

/// Where the checks stand, as the Settings section shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum CheckStatus {
    /// Under `cargo run` and in screenshots: the app never checks.
    Off,
    Never,
    Checking,
    UpToDate,
    /// A newer release, the one the pill offers.
    Available(Version),
    /// The last check failed (no network, a rate limit…), with its error.
    Failed(String),
}

#[derive(Default)]
pub struct UpdateState {
    /// The .app to replace. None under `cargo run` and in screenshots: then nothing is checked.
    bundle: Option<PathBuf>,
    /// The newer release found, shown as the toolbar's pill.
    pub available: Option<Release>,
    /// The check running, and the channel it was started on.
    checking: Option<(UpdateChannel, Receiver<CheckResult>)>,
    next_check: Option<Instant>,
    /// The last finished check: Ok when it got an answer, the error otherwise. None before the first.
    last: Option<Result<(), String>>,
    /// When the last check finished, in seconds since 1970.
    pub checked_at: Option<i64>,
    /// The update dialog is open.
    pub dialog: bool,
    installing: Option<Receiver<Progress>>,
    /// The install's latest step; `Failed` stays shown in the dialog until it is closed.
    progress: Option<Progress>,
    /// What the update just installed brought, shown once at launch until OK.
    pub whats_new: Option<Shown>,
}

impl UpdateState {
    pub fn new(screenshot: bool) -> Self {
        let bundle = if screenshot { None } else { update::running_bundle() };
        let next_check = bundle.as_ref().map(|_| Instant::now());
        // Left by the install that relaunched this app; read once, then gone.
        let whats_new = if screenshot { None } else { update::shown_file().and_then(|f| update::take_shown(&f, Version::current())) };
        UpdateState { bundle, next_check, whats_new, ..Default::default() }
    }

    /// `FOURCHETTE_UPDATE=1` shows the pill with a sample release, `=dialog` also opens the
    /// dialog, `=checking`, `=downloading`, `=installing` and `=error` show the dialog at that step.
    /// `FOURCHETTE_WHATS_NEW=1` shows the "What's new in Fourchette" dialog with sample items.
    pub fn apply_env(&mut self) {
        if std::env::var("FOURCHETTE_WHATS_NEW").is_ok_and(|v| v == "1") {
            self.whats_new = Some(update::sample_shown());
        }
        let Ok(v) = std::env::var("FOURCHETTE_UPDATE") else { return };
        self.available = Some(update::sample());
        self.dialog = v != "1";
        self.progress = match v.as_str() {
            "checking" => Some(Progress::Checking),
            "downloading" => Some(Progress::Downloading(0.42)),
            "installing" => Some(Progress::Installing),
            "error" => Some(Progress::Failed("The download is damaged: its SHA-256 doesn't match the release's.".into())),
            _ => None,
        };
    }

    fn busy(&self) -> bool {
        self.installing.is_some()
    }

    /// Checks run: the app runs from its .app bundle.
    pub fn can_check(&self) -> bool {
        self.bundle.is_some()
    }

    pub fn status(&self) -> CheckStatus {
        if self.checking.is_some() {
            return CheckStatus::Checking;
        }
        if let Some(Err(e)) = &self.last {
            return CheckStatus::Failed(e.clone());
        }
        match (&self.available, &self.last) {
            (Some(r), _) => CheckStatus::Available(r.version),
            (None, _) if !self.can_check() => CheckStatus::Off,
            (None, Some(_)) => CheckStatus::UpToDate,
            (None, None) => CheckStatus::Never,
        }
    }

    /// Settings' "Check now", or a new channel: the next frame starts a check on `channel`,
    /// unless one is already running on it. One running on another channel is dropped when it
    /// answers, and this one starts then.
    pub fn check_now(&mut self, channel: UpdateChannel) {
        let running = self.checking.as_ref().is_some_and(|(on, _)| *on == channel);
        if self.can_check() && !running {
            self.next_check = Some(Instant::now());
        }
    }
}

pub(super) fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl App {
    /// Each frame: starts the check when it is due, and takes what the check and the install sent.
    pub(super) fn update_poll(&mut self, ctx: &egui::Context) {
        let chosen = self.library.settings.update_channel;
        let u = &mut self.update;
        if let Some((on, rx)) = &u.checking {
            let on = *on;
            match rx.try_recv() {
                // An answer for the channel left in the meantime: the check on the new one starts below.
                Ok(_) if on != chosen => u.checking = None,
                Ok(res) => {
                    u.checking = None;
                    u.checked_at = Some(now_secs());
                    match res {
                        // The pill keeps what it offered; the next check will tell.
                        Err(e) => u.last = Some(Err(e)),
                        Ok(found) => {
                            u.last = Some(Ok(()));
                            if !u.busy() {
                                u.dialog &= found.is_some();
                                u.available = found;
                            }
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => u.checking = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(next) = u.next_check {
            let now = Instant::now();
            if now >= next && u.checking.is_none() && !u.busy() {
                u.next_check = Some(now + update::CHECK_EVERY);
                let (tx, rx) = channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(update::check(chosen));
                    ctx.request_repaint();
                });
                u.checking = Some((chosen, rx));
            }
            // A frame when the next check is due, even if nothing else happens on screen.
            if let Some(next) = u.next_check {
                ctx.request_repaint_after(next.saturating_duration_since(now));
            }
        }

        let Some(rx) = &u.installing else { return };
        loop {
            match rx.try_recv() {
                // The release read again on GitHub replaces the one the check found.
                Ok(Progress::Found(r)) => u.available = Some(*r),
                Ok(p) => u.progress = Some(p),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        u.installing = None;
        match (&u.progress, &u.bundle) {
            (Some(Progress::UpToDate), _) => {
                u.available = None;
                u.dialog = false;
                u.progress = None;
                self.notify("No update to install: GitHub has nothing newer than this Fourchette any more", false);
            }
            (Some(Progress::Done), Some(bundle)) => {
                // What the update brings, for the relaunched app to show once.
                if let (Some(r), Some(file)) = (&u.available, update::shown_file()) {
                    let news = r.news.as_deref().unwrap_or_default();
                    if !news.is_empty() {
                        let _ = update::save_shown(&file, r.version, news);
                    }
                }
                if let Err(e) = update::relaunch_after_exit(bundle) {
                    u.progress = Some(Progress::Failed(format!("Fourchette is up to date but couldn't restart ({e}). Quit and open it again.")));
                    return;
                }
                // Quit the usual way: the tabs are kept for the next launch.
                self.save_tabs();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (Some(Progress::Failed(_)), _) => {}
            _ => u.progress = Some(Progress::Failed("The update stopped unexpectedly.".into())),
        }
    }

    pub(super) fn update_dialog_open(&mut self, open: bool) {
        let u = &mut self.update;
        if open {
            u.dialog = u.available.is_some();
            if u.dialog {
                self.palette = None;
                self.switcher = None;
                self.settings_open = false;
            }
        } else if !u.busy() {
            u.dialog = false;
            u.progress = None;
        }
    }

    /// "Update and Restart": reading the release again, the download and the swap run on a thread;
    /// the dialog shows their steps.
    pub(super) fn update_install(&mut self, ctx: &egui::Context) {
        let on = self.library.settings.update_channel;
        let u = &mut self.update;
        if u.busy() {
            return;
        }
        if u.available.is_none() {
            return;
        }
        let Some(bundle) = u.bundle.clone() else {
            u.progress = Some(Progress::Failed("Fourchette isn't running from an app bundle, so it can't replace itself.".into()));
            return;
        };
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let send = |p: Progress| {
                let _ = tx.send(p);
                ctx.request_repaint();
            };
            if let Err(e) = update::install_latest(on, &bundle, &send) {
                send(Progress::Failed(e));
            }
        });
        u.installing = Some(rx);
        u.progress = Some(Progress::Checking);
    }

    /// The update dialog: the new version, its notes, then "Update and Restart" and "Later", or
    /// the install's progress, or its error.
    pub(super) fn update_dialog(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        use crate::modal::{self, pill, text, Modal, Width};
        let u = &self.update;
        if !u.dialog {
            return;
        }
        let Some(r) = &u.available else { return };
        let c = theme::c();
        let current = Version::current();
        let same = r.version == current;
        let title = if same { vec![text("A new build of Fourchette"), pill(r.version.to_string())] } else { vec![text("Fourchette"), pill(r.version.to_string()), text("is available")] };
        let have = match update::commit() { Some(c) => format!("{current} ({})", short(c)), None => current.to_string() };
        let line = if same { format!("Built from {}. You have {have}.", short(&r.commit)) } else { format!("You have {have}.") };
        let mut frame = Modal::new("update", Width::Medium, "pull", title).status(vec![text(line)]).hint(vec![modal::Hint::Text("Fourchette restarts once it is installed".into())]).cancel(Some("Later"));
        frame = match &u.progress {
            None => frame.primary("Update and Restart", false, true),
            Some(Progress::Failed(_)) => {
                let f = frame.hint(Vec::new()).primary("Try Again", false, true);
                if r.page.is_empty() {
                    f
                } else {
                    f.extra("Open Release Page")
                }
            }
            // Downloading or installing: nothing to press, and it can't be closed.
            Some(_) => frame.cancel(None).closable(false),
        };
        let out = frame.show(ctx, |ui| {
            match &r.news {
                // The items this build lacks; the plain notes for a release without the section.
                Some(news) => {
                    modal::caption(ui, "What's new");
                    news_box(ui, news, 220.0);
                }
                None => {
                    modal::caption(ui, "Release notes");
                    notes_box(ui, |ui| {
                        let notes = update::plain_notes(&r.notes);
                        let notes = if notes.is_empty() { "No notes for this release.".to_string() } else { notes };
                        ui.label(RichText::new(notes).color(c.text));
                    }, 220.0);
                }
            }
            match &u.progress {
                Some(Progress::Checking) => progress_row(ui, "Checking GitHub…", None),
                Some(Progress::Downloading(f)) => progress_row(ui, &format!("Downloading… {:.0}%", f * 100.0), Some(*f)),
                Some(Progress::Installing) => progress_row(ui, "Installing…", None),
                Some(Progress::Done) => progress_row(ui, "Restarting…", None),
                Some(Progress::Failed(e)) => {
                    ui.add_space(8.0);
                    modal::note(ui, c.deleted, |ui| {
                        ui.label(RichText::new("The update didn't install. Fourchette is unchanged.").color(c.strong));
                        ui.label(RichText::new(e.as_str()).size(theme::FS_SECONDARY).color(c.dim));
                    });
                }
                Some(Progress::Found(_) | Progress::UpToDate) | None => {}
            }
        });
        if out.primary {
            cmds.push(Cmd::InstallUpdate);
        }
        if out.extra == Some(0) {
            cmds.push(Cmd::OpenUrl(r.page.clone()));
        }
        if out.close {
            cmds.push(Cmd::UpdateDialog(false));
        }
    }

    /// Once after an update: what it brought, then OK.
    pub(super) fn whats_new_dialog(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        use crate::modal::{pill, text, Hint, Modal, Width};
        let Some(w) = &self.update.whats_new else { return };
        let out = Modal::new("whats-new", Width::Medium, "tag", vec![text("What's new in Fourchette"), pill(w.version.to_string())])
            .status(vec![text("Fourchette is up to date.")])
            .hint(vec![Hint::Text("Every release's notes: Help, Release Notes".into())])
            .cancel(None)
            .primary("OK", false, true)
            .show(ctx, |ui| news_box(ui, &w.items, 320.0));
        if out.primary || out.close {
            cmds.push(Cmd::CloseWhatsNew);
        }
    }
}

/// The framed, scrolling box the notes sit in.
fn notes_box(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui), max_height: f32) {
    let c = theme::c();
    egui::Frame::new().fill(c.bg).stroke(Stroke::new(1.0_f32, c.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(12)).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, add);
    });
}

/// "What's new": each section's label, then its items; a line when there is none.
fn news_box(ui: &mut egui::Ui, news: &[NewsItem], max_height: f32) {
    notes_box(
        ui,
        |ui| {
            if news.is_empty() {
                ui.label(RichText::new("Small fixes and internal changes.").color(theme::c().text));
                return;
            }
            let mut first = true;
            for s in Section::ALL {
                let mut items = update::in_section(news, s).peekable();
                if items.peek().is_none() {
                    continue;
                }
                if !first {
                    ui.add_space(10.0);
                }
                first = false;
                section_label(ui, s);
                ui.add_space(4.0);
                for it in items {
                    news_row(ui, it);
                }
            }
        },
        max_height,
    );
}

/// A section's name as a small tinted pill: New in the accent, Fixed in green, Improved in amber.
fn section_label(ui: &mut egui::Ui, s: Section) {
    let c = theme::c();
    let col = match s {
        Section::New => c.accent_text,
        Section::Fixed => c.added,
        Section::Improved => c.modified,
    };
    let font = egui::FontId::proportional(theme::FS_SMALL);
    let galley = ui.painter().layout_no_wrap(s.label().to_uppercase(), font, col);
    let size = galley.size() + vec2(14.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same((size.y / 2.0) as u8), theme::tint(col, c.bg, 0.16));
    p.galley(rect.center() - galley.size() / 2.0, galley, col);
}

/// An item: a drawn bullet, its text wrapped to the box, and its pull request's number dimmed.
fn news_row(ui: &mut egui::Ui, it: &NewsItem) {
    const INDENT: f32 = 16.0;
    let c = theme::c();
    let font = theme::body();
    let w = ui.available_width();
    let mut job = LayoutJob::default();
    job.wrap.max_width = (w - INDENT).max(40.0);
    job.append(&it.text, 0.0, TextFormat { font_id: font.clone(), color: c.text, ..Default::default() });
    if let Some(n) = it.pr {
        job.append(&format!("  #{n}"), 0.0, TextFormat { font_id: font.clone(), color: c.faint, ..Default::default() });
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let line = ui.fonts_mut(|f| f.row_height(&font));
    let (rect, _) = ui.allocate_exact_size(vec2(w, galley.size().y + 2.0), Sense::hover());
    let p = ui.painter();
    p.circle_filled(pos2(rect.left() + 6.0, rect.top() + line / 2.0), 2.0, c.dim);
    p.galley(rect.min + vec2(INDENT, 0.0), galley, c.text);
}

/// A step of the install: its text, and a bar for the download (a spinner otherwise).
fn progress_row(ui: &mut egui::Ui, text: &str, part: Option<f32>) {
    let c = theme::c();
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if part.is_none() {
            ui.spinner();
        }
        ui.label(RichText::new(text).color(c.text));
    });
    if let Some(f) = part {
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(3), c.border_strong);
        let mut done = rect;
        done.set_width(rect.width() * f.clamp(0.0, 1.0));
        p.rect_filled(done, CornerRadius::same(3), c.accent);
    }
}
