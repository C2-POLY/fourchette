//! The About window (#228): the app's icon, version, build and update channel, and the
//! "Check for Updates" button that used to be Settings' "Check now". It reads `UpdateState::status`
//! each frame; a check asked from here opens the update dialog when it finds a release
//! (`updater.rs`).
use super::settings::{checked_ago, ellipsize};
use super::updater::{now_secs, CheckStatus};
use super::*;
use crate::modal::{text, Modal, Width};
use crate::update::{self, Version};
use egui::{vec2, RichText, Sense};

/// The shipped icon as a texture, made once.
fn icon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::NULL.with("about_icon");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let img = image::load_from_memory(crate::icon::png()).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let t = ctx.load_texture("about_icon", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    Some(t)
}

impl App {
    pub(super) fn about_open(&mut self, open: bool) {
        self.about_open = open;
        if open {
            self.palette = None;
            self.switcher = None;
            self.settings_open = false;
        }
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        if !self.about_open {
            return;
        }
        let c = theme::c();
        let u = &self.update;
        let status = u.status();
        let channel = self.library.settings.update_channel.label();
        let current = Version::current();
        let can = status != CheckStatus::Off;
        let (label, enabled) = match &status {
            CheckStatus::Available(_) => ("Show Update", true),
            CheckStatus::Checking => ("Check for Updates", false),
            _ => ("Check for Updates", can),
        };
        let ago = u.checked_at.map(|t| checked_ago(now_secs() - t));
        let texture = icon_texture(ctx);
        let out = Modal::new("about", Width::Small, "branch", vec![text("Fourchette")])
            .status(vec![text("A Git client for macOS")])
            .cancel(Some("Close"))
            .primary(label, false, enabled)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    if let Some(t) = &texture {
                        ui.add(egui::Image::new(t).fit_to_exact_size(vec2(88.0, 88.0)));
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new(format!("Version {current}")).size(15.0).color(c.strong));
                    let built = match update::commit() {
                        Some(h) => format!("Built from {}", short(h)),
                        None => "Not built from a Git checkout".to_string(),
                    };
                    ui.label(RichText::new(built).size(theme::FS_SECONDARY).color(c.dim));
                    ui.label(RichText::new(format!("{channel} channel")).size(theme::FS_SECONDARY).color(c.dim));
                    ui.add_space(12.0);
                    status_line(ui, &status, ago.as_deref(), current);
                });
            });
        if out.primary {
            cmds.push(match status {
                CheckStatus::Available(_) => Cmd::UpdateDialog(true),
                _ => Cmd::UpdateCheckNow,
            });
        }
        if out.close {
            cmds.push(Cmd::About(false));
        }
    }
}

/// What the last check found, in one line under the build: a spinner while it runs.
fn status_line(ui: &mut egui::Ui, status: &CheckStatus, ago: Option<&str>, current: Version) {
    let c = theme::c();
    let line = |ui: &mut egui::Ui, title: &str, help: Option<&str>, col| {
        ui.label(RichText::new(title).color(col));
        if let Some(h) = help {
            ui.label(RichText::new(h).size(theme::FS_SECONDARY).color(c.faint));
        }
    };
    match status {
        CheckStatus::Checking => {
            ui.spinner();
            ui.add_space(2.0);
            line(ui, "Checking…", None, c.text);
        }
        CheckStatus::UpToDate => line(ui, "Fourchette is up to date", ago, c.text),
        CheckStatus::Available(v) if *v == current => line(ui, &format!("A new build of {v} is available"), ago, c.accent_text),
        CheckStatus::Available(v) => line(ui, &format!("{v} is available"), ago, c.accent_text),
        CheckStatus::Failed(e) => {
            ui.label(RichText::new("Couldn't check").color(c.deleted));
            let r = ui.add(egui::Label::new(RichText::new(ellipsize(e, 44)).size(theme::FS_SECONDARY).color(c.faint)).sense(Sense::hover()));
            r.on_hover_text(e.as_str());
        }
        CheckStatus::Off => line(ui, "Checks only run in the installed app", None, c.dim),
        CheckStatus::Never => line(ui, "Not checked yet", Some("The first check runs at launch"), c.dim),
    }
}
