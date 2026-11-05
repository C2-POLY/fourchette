//! The changelog window: Claude writes the repository's changelog for the audience chosen, in
//! the background (see crate::changelog). The text can be copied, written to CHANGELOG.md (one
//! Undo step) or put in the notes of a version's GitHub release, after a confirmation.
use super::*;
use crate::changelog::{self, Audience, GitHub, Version};
use crate::modal::{self, pill, text, Modal, Width};
use egui::RichText;

pub enum ChangelogMsg {
    /// Claude's notes for `audience`, and what GitHub gave (or why it couldn't).
    Written { audience: Audience, github: Option<Result<GitHub, String>>, notes: Result<Vec<String>, String> },
    /// The release notes of `tag` were replaced on GitHub.
    Published { tag: String, res: Result<(), String> },
}

#[derive(Default)]
pub struct ChangelogView {
    pub audience: Audience,
    pub versions: Vec<Version>,
    /// None while GitHub hasn't answered; Err when it couldn't (Claude then reads the commits only).
    pub github: Option<Result<GitHub, String>>,
    /// Claude's notes per audience, one per version.
    pub notes: HashMap<Audience, Vec<String>>,
    pub error: Option<String>,
    /// The version whose GitHub release notes would be replaced.
    pub release: String,
    /// Asking before replacing the release notes.
    pub confirm: bool,
    pub job: Option<Receiver<ChangelogMsg>>,
}

impl ChangelogView {
    fn gh(&self) -> Option<&GitHub> {
        self.github.as_ref().and_then(|g| g.as_ref().ok())
    }

    pub fn loading(&self) -> bool {
        self.job.is_some()
    }

    /// The changelog for the chosen audience, once Claude has written it.
    pub fn markdown(&self) -> Option<String> {
        self.notes.get(&self.audience).map(|n| changelog::markdown(&self.versions, n, self.gh()))
    }

    /// The versions that have a release on GitHub, newest first.
    fn releases(&self) -> Vec<&Version> {
        let Some(g) = self.gh() else { return Vec::new() };
        self.versions.iter().filter(|v| g.releases.contains(&v.name)).collect()
    }

    /// The notes of `release` for the chosen audience.
    fn release_notes(&self) -> Option<String> {
        let i = self.versions.iter().position(|v| v.name == self.release)?;
        self.notes.get(&self.audience)?.get(i).cloned()
    }
}

/// "1 tag", "3 tags".
fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

impl Tab {
    /// Opens the window and has Claude write the changelog for the audience last chosen.
    pub(super) fn changelog_open(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to write its changelog.", true);
            return;
        };
        let versions = changelog::versions(r);
        let audience = self.changelog.as_ref().map(|c| c.audience).unwrap_or_default();
        self.changelog = Some(ChangelogView { audience, versions, ..Default::default() });
        self.changelog_write(ctx);
    }

    /// Asks Claude for the chosen audience; the first time, also reads GitHub.
    fn changelog_write(&mut self, ctx: &egui::Context) {
        let slug = self.repo.as_ref().and_then(|r| r.default_remote().and_then(|rem| r.git(&["remote", "get-url", &rem]).ok())).and_then(|u| crate::github::slug(&u));
        let model = self.claude_model;
        let Some(view) = self.changelog.as_mut() else { return };
        if view.loading() {
            return;
        }
        view.error = None;
        let audience = view.audience;
        let versions = view.versions.clone();
        let known = view.github.clone();
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let github = known.or_else(|| Some(slug.ok_or_else(|| "This repository isn't on GitHub.".to_string()).and_then(|s| changelog::github(&s))));
            let gh = github.as_ref().and_then(|g| g.as_ref().ok());
            let prompt = changelog::prompt(&versions, gh, audience);
            let notes = crate::ai::ask(&prompt, crate::ai::SYSTEM, Some(&changelog::schema()), model).map(|v| changelog::read_notes(&v, &versions));
            let _ = tx.send(ChangelogMsg::Written { audience, github, notes });
            ctx.request_repaint();
        });
        view.job = Some(rx);
    }

    pub(super) fn changelog_poll(&mut self) {
        let Some(view) = self.changelog.as_mut() else { return };
        let Some(msg) = view.job.as_ref().and_then(|rx| rx.try_recv().ok()) else { return };
        view.job = None;
        match msg {
            ChangelogMsg::Written { audience, github, notes } => {
                if view.github.is_none() {
                    view.github = github;
                    view.release = view.releases().first().map(|v| v.name.clone()).unwrap_or_default();
                }
                match notes {
                    Ok(n) => {
                        view.notes.insert(audience, n);
                    }
                    Err(e) => view.error = Some(e),
                }
            }
            ChangelogMsg::Published { tag, res } => match res {
                Ok(()) => self.notify(format!("Replaced the notes of release {tag} on GitHub"), false),
                Err(e) => self.notify(e, true),
            },
        }
    }

    /// Writes CHANGELOG.md at the root of the repository; Undo takes it back.
    fn changelog_save(&mut self) {
        let (Some(r), Some(md)) = (&self.repo, self.changelog.as_ref().and_then(|c| c.markdown())) else { return };
        match changelog::write(r, &md) {
            Ok(snap) => {
                self.push_undo(snap);
                self.reload();
                self.notify(format!("Wrote {} — Undo (⌘Z) takes it back", changelog::FILE), false);
            }
            Err(e) => self.notify(e, true),
        }
    }

    /// Replaces the notes of the chosen release on GitHub, in the background.
    fn changelog_publish(&mut self, ctx: &egui::Context) {
        let Some(view) = self.changelog.as_mut() else { return };
        view.confirm = false;
        let (Some(slug), Some(notes)) = (view.gh().map(|g| g.slug.clone()), view.release_notes()) else { return };
        let tag = view.release.clone();
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = changelog::publish(&slug, &tag, &notes);
            let _ = tx.send(ChangelogMsg::Published { tag, res });
            ctx.request_repaint();
        });
        view.job = Some(rx);
    }

    /// The changelog window.
    pub(super) fn changelog_window(&mut self, ctx: &egui::Context) {
        let Some(view) = self.changelog.as_mut() else { return };
        let k = theme::c();
        let loading = view.loading();
        let md = view.markdown();
        let mut frame = Modal::new("changelog", Width::Large, "ai", vec![text("Changelog")]).status(vec![text("for"), pill(view.audience.label())]).cancel(Some("Close"));
        if view.confirm {
            frame = frame.primary(format!("Replace the notes of {}", view.release), true, !loading);
        } else {
            frame = frame.extra(format!("Write {}", changelog::FILE)).hint(modal::undo_hint()).primary("Copy", false, md.is_some());
        }
        let mut pick = None;
        let mut ask_publish = false;
        let mut cancel_confirm = false;
        let out = frame.show(ctx, |ui| {
            if view.confirm {
                modal::note(ui, k.modified, |ui| {
                    ui.label(RichText::new(format!("The notes of release {} on GitHub will be replaced by the “{}” text below. GitHub keeps no copy of the old notes, and Undo can't take this back.", view.release, view.audience.label())).color(k.text));
                });
                ui.add_space(10.0);
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    ui.label(RichText::new(view.release_notes().unwrap_or_default()).font(theme::mono()).color(k.text));
                });
                ui.add_space(8.0);
                if ui.link("Back to the changelog").clicked() {
                    cancel_confirm = true;
                }
                return;
            }
            // The four audiences, two by two; 1 to 4 pick one.
            let i = Audience::ALL.iter().position(|a| *a == view.audience).unwrap_or(0);
            if !loading {
                pick = modal::choice_keys(ui.ctx(), &[true; 4], i);
            }
            for row in Audience::ALL.chunks(2).enumerate() {
                ui.columns(2, |cols| {
                    for (j, a) in row.1.iter().enumerate() {
                        let n = row.0 * 2 + j;
                        let disabled = (loading && *a != view.audience).then_some("Wait for Claude to finish");
                        if modal::choice(&mut cols[j], a.label(), a.line(), Some(n + 1), *a == view.audience, false, disabled).clicked() {
                            pick = Some(n);
                        }
                    }
                });
                ui.add_space(6.0);
            }
            ui.add_space(4.0);
            let commits = plural(view.versions.iter().map(|v| v.commits.len() + v.more).sum(), "commit");
            let tags = plural(view.versions.iter().filter(|v| v.name != changelog::UNRELEASED).count(), "tag");
            let source = match &view.github {
                None => format!("{commits} across {tags}"),
                Some(Ok(g)) => format!("{commits} across {tags}, {} and {} from GitHub", plural(g.open_prs.len(), "open pull request"), plural(g.titles.len(), "issue and pull request title")),
                Some(Err(e)) => format!("{commits} across {tags} — without GitHub: {e}"),
            };
            modal::caption(ui, &source);
            ui.add_space(6.0);
            if let Some(e) = &view.error {
                modal::note(ui, k.deleted, |ui| {
                    ui.label(RichText::new(e).color(k.text));
                });
            } else if let Some(md) = &md {
                egui::Frame::new().fill(k.input).stroke(egui::Stroke::new(1.0_f32, k.border)).corner_radius(egui::CornerRadius::same(10)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(md).font(theme::mono()).color(k.text)).wrap().selectable(true));
                    });
                });
            } else {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label(RichText::new(format!("Claude is writing the changelog for {}…", view.audience.label().to_lowercase())).color(k.dim));
                    ui.label(RichText::new("It only reads the commits and titles — nothing is written until you choose.").size(theme::FS_SECONDARY).color(k.faint));
                });
                ui.add_space(30.0);
            }
            // A version's GitHub release notes, replaced after a confirmation.
            let releases: Vec<String> = view.releases().iter().map(|v| v.name.clone()).collect();
            if md.is_some() && !releases.is_empty() {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    modal::caption(ui, "GitHub release notes of");
                    egui::ComboBox::from_id_salt("changelog_release").selected_text(view.release.as_str()).show_ui(ui, |ui| {
                        for r in &releases {
                            ui.selectable_value(&mut view.release, r.clone(), r.as_str());
                        }
                    });
                    if ui.add_enabled(!loading, egui::Button::new("Replace on GitHub…")).clicked() {
                        ask_publish = true;
                    }
                });
            }
        });
        if cancel_confirm || (out.close && view.confirm) {
            view.confirm = false;
            return;
        }
        if ask_publish {
            view.confirm = true;
        }
        if out.close {
            self.changelog = None;
            return;
        }
        if out.primary {
            if view.confirm {
                self.changelog_publish(ctx);
            } else if let Some(md) = md {
                ctx.copy_text(md);
                self.notify("Copied the changelog", false);
            }
            return;
        }
        if out.extra == Some(0) {
            self.changelog_save();
            return;
        }
        if let Some(n) = pick.filter(|n| Audience::ALL[*n] != view.audience) {
            view.audience = Audience::ALL[n];
            if !view.notes.contains_key(&view.audience) {
                self.changelog_write(ctx);
            }
        }
    }

    /// The screenshot mode's window, with the sample project's changelog.
    pub(super) fn changelog_sample(&mut self, audience: Audience) {
        let (versions, notes, gh) = changelog::sample();
        let release = gh.releases.first().cloned().unwrap_or_default();
        self.changelog = Some(ChangelogView { audience, versions, github: Some(Ok(gh)), notes: [(audience, notes)].into(), release, ..Default::default() });
    }
}
