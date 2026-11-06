//! The home page (issue #109), shown in a tab without a repository: at launch when no tab is
//! left to reopen, in a new tab (⌘T, "+") and with ⇧⌘O. The repository list as cards, in the
//! sections of `crate::home::sections`; a click opens one in this tab.
use super::*;
use crate::home::{Cluster, Kind, RepoInfo, Section};
use crate::library::Entry;
use crate::ui::fit_text;
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Key, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use std::path::Path;
use std::sync::mpsc::TryRecvError;

const PAGE_W: f32 = 1040.0;
const CARD_W: f32 = 236.0;
const CARD_H: f32 = 64.0;
const GAP: f32 = 10.0;
/// Inside a proposal's frame.
const PAD: f32 = 12.0;

/// The search, and what was read from each repository's folder (its GitHub organisation).
#[derive(Default)]
pub struct HomeState {
    pub query: String,
    info: HashMap<PathBuf, RepoInfo>,
    rx: Option<Receiver<(PathBuf, RepoInfo)>>,
    /// `FOURCHETTE_HOME`: invented repositories, whose folders are never read.
    sample: bool,
}

impl HomeState {
    /// For `FOURCHETTE_HOME=1` screenshots: the invented repositories in place of the list.
    pub(super) fn sample(&mut self, library: &mut crate::library::Library) {
        let (entries, info) = crate::home::sample();
        *library = library.sample(entries);
        self.info = info;
        self.sample = true;
    }

    /// Takes what the reading thread found; starts one for the repositories not read yet.
    fn poll(&mut self, entries: &[Entry], ctx: &egui::Context) {
        if let Some(rx) = &self.rx {
            loop {
                match rx.try_recv() {
                    Ok((p, i)) => {
                        self.info.insert(p, i);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.rx = None;
                        break;
                    }
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        if self.sample {
            return;
        }
        let todo: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).filter(|p| !self.info.contains_key(p)).collect();
        if todo.is_empty() {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for p in todo {
                let i = RepoInfo::read(&p);
                if tx.send((p, i)).is_err() {
                    return;
                }
            }
        });
        self.rx = Some(rx);
    }
}

/// "~/dev/acme" rather than "/Users/me/dev/acme".
fn tilde(p: &Path) -> String {
    let s = p.display().to_string();
    match std::env::var("HOME") {
        Ok(h) if s.starts_with(&h) => format!("~{}", &s[h.len()..]),
        _ => s,
    }
}

impl App {
    /// ⇧⌘O: the tab of the home page, else a new one. The sample project gives its tab up.
    pub(super) fn show_home(&mut self) {
        if let Some(i) = self.tabs.iter().position(|t| t.home) {
            return self.activate(i);
        }
        if self.tab().is_demo() && self.tab().pending.is_none() {
            self.tabs[self.active] = Tab::home();
            self.palette = None;
            return;
        }
        self.tabs.push(Tab::home());
        self.activate(self.tabs.len() - 1);
    }

    /// The toolbar of the home page's tab: the window's edge, its title and the update pill.
    pub(super) fn home_toolbar(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        crate::titlebar::drag_area(ui);
        let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ui.horizontal_centered(|ui| {
            ui.add_space(crate::titlebar::inset(fullscreen).max(6.0) + 6.0);
            ui.label(RichText::new("Home").size(theme::FS_TITLE).strong().color(theme::c().strong));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                if self.update.available.is_some() && crate::ui::update_pill(ui, "Update available").on_hover_text("A new version of Fourchette is ready to install").clicked() {
                    cmds.push(Cmd::UpdateDialog(true));
                }
            });
        });
    }

    /// The page, in the centre of the window under the toolbar (and the tabs).
    pub(super) fn home_page(&mut self, root: &mut egui::Ui, cmds: &mut Vec<Cmd>) {
        let ctx = &root.ctx().clone();
        self.home.poll(&self.library.entries, ctx);
        let sections = crate::home::sections(&self.library.entries, &self.home.info, &self.library.dismissed, &self.home.query);
        let open: Vec<PathBuf> = self.tabs.iter().filter_map(|t| t.root().map(Path::to_path_buf)).collect();
        // The sample's folders are invented: none of them is there.
        let missing = if self.home.sample { HashSet::new() } else { self.library.entries.iter().filter(|e| !e.path.exists()).map(|e| e.path.clone()).collect() };
        let marks = Marks { open, missing };
        let overlay = self.palette.is_some() || self.switcher.is_some() || self.settings_open || self.modal();
        let total = self.library.entries.len();
        let home = &mut self.home;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::c().bg)).show(root, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let full = ui.available_width();
                let w = (full - 48.0).min(PAGE_W);
                let side = ((full - w) / 2.0).max(0.0);
                ui.add_space(28.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                    ui.set_width(w);
                    header(ui, home, total, overlay, &sections, cmds);
                    ui.add_space(18.0);
                    if total == 0 {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("No repositories yet").size(theme::FS_TITLE).color(theme::c().strong));
                            ui.add_space(4.0);
                            ui.label(RichText::new("Add one, or every repository in a folder, and they show up here.").color(theme::c().dim));
                        });
                    } else if sections.is_empty() {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| ui.label(RichText::new("Nothing matches").color(theme::c().faint)));
                    }
                    // A repository can show twice (Favorites, then its organisation): each section has its own ids.
                    for (n, s) in sections.iter().enumerate() {
                        ui.push_id(("home_section", n), |ui| section(ui, s, &marks, cmds));
                        ui.add_space(12.0);
                    }
                    });
                });
                ui.add_space(40.0);
            });
        });
    }
}

/// What a card says beside the list: open in a tab, or its folder gone.
struct Marks {
    open: Vec<PathBuf>,
    missing: HashSet<PathBuf>,
}

/// The title, the search and the buttons that add repositories.
fn header(ui: &mut Ui, home: &mut HomeState, total: usize, overlay: bool, sections: &[Section], cmds: &mut Vec<Cmd>) {
    let k = theme::c();
    ui.label(RichText::new("Repositories").size(22.0).strong().color(k.strong));
    let n = if total == 1 { "1 repository".to_string() } else { format!("{total} repositories") };
    ui.label(RichText::new(format!("{n}, by GitHub organisation — names that start alike are proposed as a group")).color(k.faint));
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        let edit = ui.add(
            egui::TextEdit::singleline(&mut home.query)
                .hint_text(crate::ui::faint_hint("Search by name, folder or organisation…"))
                .desired_width(340.0)
                .margin(Margin::symmetric(10, 7)),
        );
        // Typing searches at once, unless something on top has the keyboard.
        if !overlay && ui.memory(|m| m.focused().is_none()) {
            edit.request_focus();
        }
        let (enter, esc) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        if edit.has_focus() || edit.lost_focus() {
            if enter && !home.query.trim().is_empty() {
                if let Some(e) = sections.first().and_then(|s| s.clusters.first()).and_then(|c| c.entries.first()) {
                    cmds.push(Cmd::Open(e.path.clone()));
                }
            }
            if esc {
                home.query.clear();
            }
        }
        ui.add_space(8.0);
        use crate::modal::{button, Kind as Btn};
        if button(ui, "Add a repository…", Btn::Plain, true).clicked() {
            cmds.push(Cmd::OpenDialog);
        }
        if button(ui, "Add every repository in a folder…", Btn::Plain, true).clicked() {
            cmds.push(Cmd::ScanFolder);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let link = ui.add(egui::Label::new(RichText::new("Explore the sample project").color(k.accent_text)).sense(Sense::click()));
            if link.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("A sample history to look around, without opening a repository").clicked() {
                cmds.push(Cmd::HomeSample);
            }
        });
    });
}

/// A section: its title, then its proposals in a frame each, then its other repositories.
fn section(ui: &mut Ui, s: &Section, marks: &Marks, cmds: &mut Vec<Cmd>) {
    let k = theme::c();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let title = match s.kind {
            Kind::Favorites => format!("★ {}", s.title),
            _ => s.title.clone(),
        };
        ui.label(RichText::new(title).size(theme::FS_BODY).strong().color(k.strong));
        ui.label(RichText::new(s.len().to_string()).size(theme::FS_SMALL).color(k.faint));
        let caption = match s.kind {
            Kind::Group => "group",
            Kind::Org => "GitHub",
            _ => "",
        };
        if !caption.is_empty() {
            ui.label(RichText::new(caption).size(theme::FS_SMALL).color(k.dim));
        }
    });
    ui.add_space(8.0);
    for c in &s.clusters {
        match &c.proposal {
            Some(_) => {
                proposal(ui, c, marks, cmds);
                ui.add_space(GAP);
            }
            None => cards(ui, &c.entries, marks, cmds),
        }
    }
}

/// Repositories proposed together: a frame with what they share, Keep as a group and Dismiss.
fn proposal(ui: &mut Ui, c: &Cluster, marks: &Marks, cmds: &mut Vec<Cmd>) {
    let Some(p) = &c.proposal else { return };
    let k = theme::c();
    let frame = egui::Frame::new()
        .fill(theme::tint(k.accent_soft, k.bg, 0.5))
        .stroke(Stroke::new(1.0_f32, k.border_strong))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(PAD as i8));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("“{}”", p.name)).strong().color(k.strong));
            ui.label(RichText::new(format!("{} repositories start with this name — keep them together?", c.entries.len())).color(k.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                use crate::modal::{button, Kind as Btn};
                if button(ui, "Dismiss", Btn::Plain, true).on_hover_text("Don't propose it again").clicked() {
                    cmds.push(Cmd::HomeDismiss(p.key.clone()));
                }
                let tip = format!("Puts them in the group “{}”, as Move to group does in the switcher (⌘O)", p.name);
                if button(ui, "Keep as a group", Btn::Primary, true).on_hover_text(tip).clicked() {
                    cmds.push(Cmd::HomeKeep { name: p.name.clone(), paths: c.entries.iter().map(|e| e.path.clone()).collect() });
                }
            });
        });
        ui.add_space(8.0);
        cards(ui, &c.entries, marks, cmds);
    });
}

/// The cards, as many to a row as the width holds.
fn cards(ui: &mut Ui, entries: &[Entry], marks: &Marks, cmds: &mut Vec<Cmd>) {
    let w = ui.available_width();
    let cols = (((w + GAP) / (CARD_W + GAP)).floor() as usize).max(1);
    let cw = (w - GAP * (cols - 1) as f32) / cols as f32;
    for row in entries.chunks(cols) {
        let (strip, _) = ui.allocate_exact_size(vec2(w, CARD_H), Sense::hover());
        for (i, e) in row.iter().enumerate() {
            let r = Rect::from_min_size(pos2(strip.left() + i as f32 * (cw + GAP), strip.top()), vec2(cw, CARD_H));
            card(ui, r, e, marks, cmds);
        }
        ui.add_space(GAP);
    }
}

/// One repository: its initials, name and folder, when it was last opened, and a star.
fn card(ui: &mut Ui, r: Rect, e: &Entry, marks: &Marks, cmds: &mut Vec<Cmd>) {
    let k = theme::c();
    let id = ui.scope_id().with(("home_card", &e.path));
    let resp = ui.interact(r, id, Sense::click());
    let star = Rect::from_center_size(pos2(r.right() - 18.0, r.top() + 18.0), vec2(24.0, 24.0));
    let star_resp = ui.interact(star, id.with("star"), Sense::click());
    let hot = resp.hovered() || star_resp.hovered();
    let p = ui.painter().with_clip_rect(r);
    let rr = CornerRadius::same(10);
    p.rect_filled(r, rr, if hot { k.widget_hover } else { k.elevated });
    p.rect_stroke(r, rr, Stroke::new(1.0_f32, if hot { k.widget_hover_border } else { k.border }), StrokeKind::Inside);

    let missing = marks.missing.contains(&e.path);
    let is_open = marks.open.contains(&e.path);
    let name = e.name();
    crate::ui::avatar(&p, pos2(r.left() + 26.0, r.center().y), 15.0, &name);
    let x = r.left() + 50.0;
    let room = star.left() - 4.0 - x;
    fit_text(&p, pos2(x, r.top() + 18.0), Align2::LEFT_CENTER, &name, theme::body(), if missing { k.faint } else { k.strong }, room);
    let folder = tilde(e.path.parent().unwrap_or(&e.path));
    fit_text(&p, pos2(x, r.center().y + 1.0), Align2::LEFT_CENTER, &folder, theme::small(), k.faint, r.right() - 12.0 - x);
    let (when, col) = if missing {
        ("Folder missing".to_string(), k.deleted)
    } else if is_open {
        ("Open in a tab".to_string(), k.added)
    } else if e.last_opened > 0 {
        (format!("Opened {}", crate::git::relative_time(e.last_opened)), k.dim)
    } else {
        ("Never opened".to_string(), k.faint)
    };
    fit_text(&p, pos2(x, r.bottom() - 15.0), Align2::LEFT_CENTER, &when, theme::small(), col, r.right() - 12.0 - x);

    // The star toggles the favourite without opening the repository; it shows on a favourite
    // or under the pointer.
    if e.favorite || hot {
        let col = if e.favorite { k.modified } else if star_resp.hovered() { k.dim } else { k.border_strong };
        p.text(star.center(), Align2::CENTER_CENTER, if e.favorite { "★" } else { "☆" }, FontId::proportional(14.0), col);
    }
    if star_resp.clicked() {
        cmds.push(Cmd::RepoFavorite(e.path.clone()));
    } else if resp.clicked() {
        cmds.push(Cmd::Open(e.path.clone()));
    }
    resp.on_hover_text(e.path.display().to_string()).context_menu(|ui| {
        crate::ui::menu_rows(ui);
        ui.set_min_width(220.0);
        if ui.button("Open in a new tab").clicked() {
            cmds.push(Cmd::OpenTab(e.path.clone()));
        }
        if ui.button(if e.favorite { "Remove from favorites" } else { "Add to favorites" }).clicked() {
            cmds.push(Cmd::RepoFavorite(e.path.clone()));
        }
        if e.group.is_some() && ui.button("Back to its folder's group").clicked() {
            cmds.push(Cmd::RepoGroup(e.path.clone(), None));
        }
        ui.separator();
        if !missing && ui.button("Show in Finder").clicked() {
            cmds.push(Cmd::Reveal(e.path.clone()));
        }
        if ui.button("Copy path").clicked() {
            cmds.push(Cmd::Copy(e.path.display().to_string()));
        }
        ui.separator();
        if ui.button(RichText::new("Remove from the list").color(k.deleted)).on_hover_text("The folder itself is not touched").clicked() {
            cmds.push(Cmd::RepoForget(e.path.clone()));
        }
    });
}
