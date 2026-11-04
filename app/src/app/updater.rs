//! Updates (issue #80): the check in the background, the toolbar's pill and the update dialog.
//! The work itself is in `crate::update`; this side only polls its channels each frame.
use super::*;
use crate::update::{self, Progress, Release, Version};
use egui::{vec2, Align, CornerRadius, Key, Layout, Margin, RichText, Sense, Stroke};
use std::sync::mpsc::TryRecvError;

type CheckResult = Result<Option<Release>, String>;

#[derive(Default)]
pub struct UpdateState {
    /// The .app to replace. None under `cargo run` and in screenshots: then nothing is checked.
    bundle: Option<PathBuf>,
    /// The newer release found, shown as the toolbar's pill.
    pub available: Option<Release>,
    checking: Option<Receiver<CheckResult>>,
    next_check: Option<Instant>,
    /// The update dialog is open.
    pub dialog: bool,
    installing: Option<Receiver<Progress>>,
    /// The install's latest step; `Failed` stays shown in the dialog until it is closed.
    progress: Option<Progress>,
}

impl UpdateState {
    pub fn new(screenshot: bool) -> Self {
        let bundle = if screenshot { None } else { update::running_bundle() };
        let next_check = bundle.as_ref().map(|_| Instant::now());
        UpdateState { bundle, next_check, ..Default::default() }
    }

    /// `FOURCHETTE_UPDATE=1` shows the pill with a sample release, `=dialog` also opens the
    /// dialog, `=downloading`, `=installing` and `=error` show the dialog at that step.
    pub fn apply_env(&mut self) {
        let Ok(v) = std::env::var("FOURCHETTE_UPDATE") else { return };
        self.available = Some(update::sample());
        self.dialog = v != "1";
        self.progress = match v.as_str() {
            "downloading" => Some(Progress::Downloading(0.42)),
            "installing" => Some(Progress::Installing),
            "error" => Some(Progress::Failed("The download is damaged: its SHA-256 doesn't match the release's.".into())),
            _ => None,
        };
    }

    fn busy(&self) -> bool {
        self.installing.is_some()
    }
}

impl App {
    /// Each frame: starts the check when it is due, and takes what the check and the install sent.
    pub(super) fn update_poll(&mut self, ctx: &egui::Context) {
        let u = &mut self.update;
        if let Some(rx) = &u.checking {
            match rx.try_recv() {
                Ok(res) => {
                    u.checking = None;
                    // No network, a rate limit…: nothing to show, the next check will tell.
                    if let (Ok(found), false) = (res, u.busy()) {
                        u.dialog &= found.is_some();
                        u.available = found;
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
                    let _ = tx.send(update::check());
                    ctx.request_repaint();
                });
                u.checking = Some(rx);
            }
            // A frame when the next check is due, even if nothing else happens on screen.
            if let Some(next) = u.next_check {
                ctx.request_repaint_after(next.saturating_duration_since(now));
            }
        }

        let Some(rx) = &u.installing else { return };
        loop {
            match rx.try_recv() {
                Ok(p) => u.progress = Some(p),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        u.installing = None;
        match (&u.progress, &u.bundle) {
            (Some(Progress::Done), Some(bundle)) => {
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

    /// "Update and Restart": the download and the swap run on a thread; the dialog shows their steps.
    pub(super) fn update_install(&mut self, ctx: &egui::Context) {
        let u = &mut self.update;
        if u.busy() {
            return;
        }
        let Some(release) = u.available.clone() else { return };
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
            if let Err(e) = update::install(&release, &bundle, &send) {
                send(Progress::Failed(e));
            }
        });
        u.installing = Some(rx);
        u.progress = Some(Progress::Downloading(0.0));
    }

    /// The update dialog: the new version, its notes, then "Update and Restart" and "Later", or
    /// the install's progress, or its error.
    pub(super) fn update_dialog(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let u = &self.update;
        if !u.dialog {
            return;
        }
        let Some(r) = &u.available else { return };
        let c = theme::c();
        let current = Version::current();
        let same = r.version == current;
        egui::Modal::new(egui::Id::new("update")).show(ctx, |ui| {
            ui.set_width(460.0);
            let title = if same { format!("A new build of Fourchette {}", r.version) } else { format!("Fourchette {} is available", r.version) };
            ui.label(RichText::new(title).size(theme::FS_TITLE).color(c.strong));
            let have = if update::COMMIT.is_empty() { current.to_string() } else { format!("{current} ({})", short(update::COMMIT)) };
            let line = if same { format!("Built from {}. You have {have}.", short(&r.commit)) } else { format!("You have {have}.") };
            ui.label(RichText::new(line).color(c.dim));
            ui.add_space(10.0);
            ui.label(RichText::new("Release notes").size(theme::FS_SMALL).color(c.faint));
            ui.add_space(2.0);
            egui::Frame::new().fill(c.bg).stroke(Stroke::new(1.0_f32, c.border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(12)).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                    let notes = update::plain_notes(&r.notes);
                    let notes = if notes.is_empty() { "No notes for this release.".to_string() } else { notes };
                    ui.label(RichText::new(notes).color(c.text));
                });
            });
            ui.add_space(12.0);

            let escape = ui.input(|i| i.key_pressed(Key::Escape));
            match &u.progress {
                Some(Progress::Downloading(f)) => {
                    progress_row(ui, &format!("Downloading… {:.0}%", f * 100.0), Some(*f));
                }
                Some(Progress::Installing) => progress_row(ui, "Installing…", None),
                Some(Progress::Done) => progress_row(ui, "Restarting…", None),
                Some(Progress::Failed(e)) => {
                    egui::Frame::new().fill(theme::tint(c.deleted, c.elevated, 0.12)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("The update didn't install. Fourchette is unchanged.").color(c.strong));
                        ui.label(RichText::new(e.as_str()).size(theme::FS_SECONDARY).color(c.dim));
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if primary_button(ui, "Try Again").clicked() {
                            cmds.push(Cmd::InstallUpdate);
                        }
                        if !r.page.is_empty() && ui.button("Open Release Page").clicked() {
                            cmds.push(Cmd::OpenUrl(r.page.clone()));
                        }
                        if ui.button("Later").clicked() || escape {
                            cmds.push(Cmd::UpdateDialog(false));
                        }
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        if primary_button(ui, "Update and Restart").clicked() {
                            cmds.push(Cmd::InstallUpdate);
                        }
                        if ui.button("Later").clicked() || escape {
                            cmds.push(Cmd::UpdateDialog(false));
                        }
                    });
                }
            }
        });
    }
}

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let c = theme::c();
    ui.add(egui::Button::new(RichText::new(text).color(c.on_accent)).fill(c.accent).stroke(Stroke::new(1.0_f32, c.accent)))
}

/// A step of the install: its text, and a bar for the download (a spinner otherwise).
fn progress_row(ui: &mut egui::Ui, text: &str, part: Option<f32>) {
    let c = theme::c();
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
    ui.add_space(4.0);
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.label(RichText::new("Fourchette restarts once the new version is in place.").size(theme::FS_SMALL).color(c.faint));
    });
}
