//! Repository switcher (⌘O): favourites, recent ones and every repository under its group.
use super::*;
use crate::library::Entry;
use crate::repo_state::States;
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Key, Margin, Rect, RichText, Sense, Stroke};

const SWITCHER_W: f32 = 620.0;
const ROW_H: f32 = 30.0;
const HEADER_H: f32 = 22.0;
const LIST_H: f32 = 440.0;
/// Rows shown under "Recent" when nothing is typed.
const RECENT_SHOWN: usize = 5;

#[derive(Default)]
pub struct Switcher {
    pub query: String,
    pub sel: usize,
    /// Opened by ⌘T or the tab strip's "+": the pick goes to a new tab.
    pub new_tab: bool,
}

struct Row {
    section: String,
    entry: Entry,
    /// ⌘1…⌘9 for the first nine favourites.
    shortcut: Option<usize>,
}

fn matches(query: &str, e: &Entry) -> bool {
    let t = format!("{} {} {}", e.name(), e.group_name(), e.path.display()).to_lowercase();
    query.split_whitespace().all(|w| t.contains(&w.to_lowercase()))
}

/// "~/Documents/code" rather than "/Users/me/Documents/code".
fn tilde(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    match std::env::var("HOME") {
        Ok(h) if s.starts_with(&h) => format!("~{}", &s[h.len()..]),
        _ => s,
    }
}

/// "1 repository", "3 repositories".
fn repos_count(n: usize) -> String {
    format!("{n} {}", if n == 1 { "repository" } else { "repositories" })
}

/// The first line of Git's message, for a list where the whole of it is in the tooltip.
fn first_line(e: &str) -> &str {
    e.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(e)
}

/// A rounded badge from its right edge `right`: `icon` draws in its first `icon_w` points, then
/// the text. Returns its rectangle.
fn badge(p: &egui::Painter, right: f32, y: f32, text: &str, col: egui::Color32, icon_w: f32, icon: impl FnOnce(&egui::Painter, egui::Pos2)) -> Rect {
    let g = p.layout_no_wrap(text.to_string(), theme::small(), col);
    let w = icon_w + g.size().x + 14.0;
    let r = Rect::from_min_size(pos2(right - w, y - 9.0), vec2(w, 18.0));
    p.rect(r, CornerRadius::same(9), theme::tint(col, theme::c().bg, 0.12), Stroke::NONE, egui::StrokeKind::Inside);
    icon(p, pos2(r.left() + 7.0, y));
    p.galley(pos2(r.left() + 7.0 + icon_w, y - g.size().y / 2.0), g, col);
    r
}

/// The state of a repository on its row, from the right edge `right`, right to left: a failed
/// fetch, the reviews waiting for me, the open pull requests, behind, ahead, the changes.
/// Nothing when it is clean, level with its upstream and has no pull request. Returns the left
/// edge reached and the link of the badge clicked, if any.
fn state_badges(ui: &egui::Ui, p: &egui::Painter, states: &States, path: &std::path::Path, right: f32, y: f32, id: egui::Id) -> (f32, Option<String>) {
    let c = theme::c();
    let mut x = right;
    let mut link = None;
    let mut hover = |r: Rect, k: &str, tip: String, url: Option<String>| {
        let resp = ui.interact(r, id.with(k), if url.is_some() { Sense::click() } else { Sense::hover() });
        let resp = if url.is_some() { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
        if resp.on_hover_text(tip).clicked() {
            link = url;
        }
    };
    if let Some(e) = states.fetch_error(path) {
        let r = badge(p, x, y, "fetch failed", c.deleted, 0.0, |_, _| {});
        hover(r, "fetch", format!("The last fetch failed:\n{e}"), None);
        x = r.left() - 6.0;
    }
    if let Some(pr) = states.pulls(path) {
        if pr.review > 0 {
            let r = badge(p, x, y, &format!("{} to review", pr.review), c.accent, 0.0, |_, _| {});
            hover(r, "review", format!("Your review is requested on {} — open them on GitHub", if pr.review == 1 { "1 pull request".into() } else { format!("{} pull requests", pr.review) }), Some(pr.review_url()));
            x = r.left() - 6.0;
        }
        if pr.open > 0 {
            let r = badge(p, x, y, &pr.open.to_string(), c.added, 15.0, |p, at| crate::ui::pr_icon(p, at.x - 1.0, at.y, c.added, false));
            hover(r, "pulls", format!("{} — open them on GitHub", if pr.open == 1 { "1 open pull request".into() } else { format!("{} open pull requests", pr.open) }), Some(pr.url()));
            x = r.left() - 6.0;
        }
    }
    if let Some(l) = states.local(path) {
        let start = x;
        for (n, up, col) in [(l.behind, false, c.modified), (l.ahead, true, c.added)] {
            if n > 0 {
                let g = p.layout_no_wrap(n.to_string(), theme::small(), col);
                x -= g.size().x;
                p.galley(pos2(x, y - g.size().y / 2.0), g, col);
                x -= 6.0;
                crate::sidebar::arrow(p, pos2(x, y), up, col);
                x -= 10.0;
            }
        }
        if x < start {
            let r = Rect::from_x_y_ranges(x..=start, y - 9.0..=y + 9.0);
            hover(r, "track", format!("{} ahead, {} behind its upstream", l.ahead, l.behind), None);
            x -= 2.0;
        }
        if l.changes > 0 {
            let r = badge(p, x, y, &l.changes.to_string(), c.modified, 9.0, |p, at| {
                p.circle_filled(at + vec2(1.5, 0.0), 3.0, c.modified);
            });
            hover(r, "changes", if l.changes == 1 { "1 changed file".into() } else { format!("{} changed files", l.changes) }, None);
            x = r.left() - 6.0;
        }
    }
    (x, link)
}

impl App {
    /// Takes the background states and says when "Fetch all" has finished.
    pub(super) fn states_poll(&mut self) {
        let Some(f) = self.states.poll() else { return };
        if f.failures.is_empty() {
            self.notify(format!("Fetched {}", repos_count(f.total)), false);
        } else {
            let names: Vec<String> = f.failures.iter().map(|(p, _)| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
            self.notify(format!("Fetch all: {} of {} failed ({}) — ⌘O shows why", f.failures.len(), repos_count(f.total), names.join(", ")), true);
        }
    }

    /// Fetches every repository of the list, from the switcher or the palette.
    pub(super) fn fetch_all_repos(&mut self, ctx: &egui::Context) {
        let paths: Vec<PathBuf> = self.library.entries.iter().map(|e| e.path.clone()).collect();
        match self.states.fetch_all(&paths, ctx) {
            Ok(()) if self.switcher.is_none() => {
                let n = self.states.fetch.as_ref().map_or(0, |f| f.total);
                self.notify(format!("Fetching {}…", repos_count(n)), false);
            }
            Ok(()) => {}
            Err(e) => self.notify(e, true),
        }
    }

    /// FOURCHETTE_SWITCHER in a screenshot: a sample list with sample states instead of the
    /// person's repositories; `fetch` is "fetching" or "fetched" to show Fetch all.
    pub(super) fn switcher_sample(&mut self, fetch: &str) {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/me".into());
        let t = crate::library::now();
        let e = |path: &str, group: Option<&str>, favorite: bool, ago: i64| Entry {
            path: PathBuf::from(format!("{home}/{path}")),
            group: group.map(String::from),
            favorite,
            last_opened: t - ago,
            project: None,
            details: None,
        };
        let entries = vec![
            e("code/acme/fourchette", None, true, 60),
            e("code/acme/fourchette-site", None, true, 3_600),
            e("code/clients/api", Some("Clients"), false, 7_200),
            e("code/clients/shop", Some("Clients"), false, 86_400),
            e("code/acme/design-system", None, false, 90_000),
            e("code/personal/dotfiles", None, false, 200_000),
            e("code/clients/mobile-app", Some("Clients"), false, 300_000),
            e("code/personal/legacy-api", None, false, 9_000_000),
        ];
        let paths: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();
        let missing = vec![paths[7].clone()];
        self.library = self.library.sample(entries);
        self.states = States::sample(&paths, missing, fetch);
    }

    /// The repositories "Remove N missing" takes off: those whose folder is gone, except on an
    /// external disk not plugged in (`Library::missing`); in a screenshot, the sample's.
    pub(super) fn missing_repos(&self) -> Vec<PathBuf> {
        if self.home.sample {
            return self.home.sample_missing.clone();
        }
        if let Some(m) = self.states.sample_missing() {
            return m.to_vec();
        }
        self.library.missing()
    }

    fn switcher_rows(&self, query: &str) -> Vec<Row> {
        let row = |section: &str, e: &Entry, shortcut: Option<usize>| Row { section: section.into(), entry: e.clone(), shortcut };
        if !query.trim().is_empty() {
            let mut v: Vec<&Entry> = self.library.entries.iter().filter(|e| matches(query, e)).collect();
            v.sort_by_key(|e| (std::cmp::Reverse(e.favorite), std::cmp::Reverse(e.last_opened), e.name().to_lowercase()));
            return v.into_iter().map(|e| row("Matches", e, None)).collect();
        }
        let mut out = Vec::new();
        for (k, e) in self.library.favorites().into_iter().enumerate() {
            out.push(row("★ Favorites", e, (k < 9).then_some(k + 1)));
        }
        for e in self.library.recent().into_iter().filter(|e| !e.favorite).take(RECENT_SHOWN) {
            out.push(row("Recent", e, None));
        }
        for (g, entries) in self.library.groups() {
            for e in entries {
                out.push(row(&g, e, None));
            }
        }
        out
    }

    /// ⌘1…⌘9 open the favourites, from anywhere.
    pub(super) fn favorite_keys(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        const KEYS: [Key; 9] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        let hit = ctx.input(|i| if i.modifiers.command { KEYS.iter().position(|k| i.key_pressed(*k)) } else { None });
        if let Some(e) = hit.and_then(|k| self.library.favorites().get(k).map(|e| e.path.clone())) {
            self.switcher = None;
            cmds.push(Cmd::Open(e));
        }
    }

    pub(super) fn switcher_ui(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let Some(sw) = self.switcher.as_ref() else { return };
        let query = sw.query.clone();
        let rows = self.switcher_rows(&query);
        let n = rows.len();
        let current = self.tab().repo.as_ref().map(|r| r.root.clone());
        // Repositories open in a tab: picking one goes to its tab.
        let open: Vec<PathBuf> = self.tabs.iter().filter_map(|t| t.root().map(|p| p.to_path_buf())).collect();
        let groups = self.library.custom_groups();
        let all: Vec<PathBuf> = self.library.entries.iter().map(|e| e.path.clone()).collect();
        self.states.ensure(&all, &open, ctx);
        let missing = self.missing_repos();
        let states = &self.states;
        let (up, down, enter, esc) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let Some(sw) = self.switcher.as_mut() else { return };
        let new_tab = sw.new_tab;
        if down && n > 0 {
            sw.sel = (sw.sel + 1).min(n - 1);
        }
        if up {
            sw.sel = sw.sel.saturating_sub(1);
        }
        sw.sel = sw.sel.min(n.saturating_sub(1));
        let mut chosen: Option<usize> = if enter && n > 0 { Some(sw.sel) } else { None };
        let mut close = esc;

        let screen = ctx.content_rect();
        let left = screen.center().x - SWITCHER_W * 0.5 - 12.0;
        let top = (screen.top() + 70.0).max(screen.center().y - 330.0);
        let t = crate::modal::appear(ctx, egui::Id::unique("switcher"));
        egui::Area::new(egui::Id::unique("switcher_backdrop")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            if resp.clicked() {
                close = true;
            }
        });
        let area = egui::Area::new(egui::Id::unique("switcher")).order(egui::Order::Foreground).fixed_pos(pos2(left, top)).show(ctx, |ui| {
            ui.multiply_opacity(t);
            crate::modal::card(ui, crate::modal::RADIUS, Margin::same(12), |ui| {
                    ui.set_width(SWITCHER_W);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut sw.query)
                            .hint_text(crate::ui::faint_hint(if new_tab { "Open in a new tab — type a name, a group or a folder…" } else { "Switch repository — type a name, a group or a folder…" }))
                            .font(FontId::proportional(theme::FS_TITLE))
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(10, 8)),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        sw.sel = 0;
                    }
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(LIST_H).min_scrolled_height(LIST_H.min(n as f32 * ROW_H + 60.0)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let mut last = "";
                        for (k, row) in rows.iter().enumerate() {
                            if row.section != last {
                                last = &row.section;
                                let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
                                ui.painter().text(pos2(hr.left() + 10.0, hr.bottom() - 6.0), Align2::LEFT_BOTTOM, &row.section, theme::small(), theme::c().faint);
                            }
                            let e = &row.entry;
                            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                            let selected = k == sw.sel;
                            if selected || resp.hovered() {
                                ui.painter().rect_filled(rect, CornerRadius::same(6), if selected { theme::c().select } else { theme::c().hover });
                            }
                            if selected && (up || down) {
                                ui.scroll_to_rect(rect, None);
                            }
                            // The star toggles the favourite without opening the repository.
                            let star = Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(22.0, 22.0));
                            let star_resp = ui.interact(star, ui.scope_id().with(("star", k)), Sense::click());
                            let star_col = if e.favorite { theme::c().modified } else if star_resp.hovered() { theme::c().dim } else { theme::c().border };
                            let p = ui.painter().with_clip_rect(rect);
                            p.text(star.center(), Align2::CENTER_CENTER, if e.favorite { "★" } else { "☆" }, FontId::proportional(13.0), star_col);
                            if star_resp.clicked() {
                                cmds.push(Cmd::RepoFavorite(e.path.clone()));
                            }
                            let missing = states.missing(&e.path);
                            let is_current = current.as_deref() == Some(e.path.as_path());
                            let is_open = open.iter().any(|o| o == &e.path);
                            let tc = if missing { theme::c().faint } else if selected || is_current { theme::c().strong } else { theme::c().text };
                            let name = p.text(pos2(rect.left() + 34.0, rect.center().y), Align2::LEFT_CENTER, e.name(), theme::body(), tc);
                            let mut x = name.right() + 10.0;
                            if is_open {
                                let r = p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, "✔ open", theme::small(), theme::c().added);
                                x = r.right() + 10.0;
                            }
                            // A folder that is gone keeps its message, without state.
                            let mut badges_left = rect.right() - 44.0;
                            let mut badge_clicked = false;
                            if !missing {
                                let (left, link) = state_badges(ui, &p, states, &e.path, rect.right() - 44.0, rect.center().y, ui.scope_id().with(("state", k)));
                                badges_left = left;
                                if let Some(u) = link {
                                    badge_clicked = true;
                                    cmds.push(Cmd::OpenUrl(u));
                                }
                            }
                            let where_ = if missing { format!("{} — folder missing", tilde(&e.path)) } else { tilde(e.path.parent().unwrap_or(&e.path)) };
                            p.with_clip_rect(Rect::from_min_max(pos2(x, rect.top()), pos2(badges_left - 8.0, rect.bottom()))).text(
                                pos2(x.max(rect.left() + 230.0), rect.center().y),
                                Align2::LEFT_CENTER,
                                where_,
                                theme::secondary(),
                                if missing { theme::c().deleted.gamma_multiply(0.8) } else { theme::c().faint },
                            );
                            if let Some(s) = row.shortcut {
                                p.text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, format!("⌘{s}"), theme::small(), theme::c().dim);
                            }
                            if resp.clicked() && !star_resp.clicked() && !badge_clicked {
                                chosen = Some(k);
                            }
                            resp.on_hover_text(e.path.display().to_string()).context_menu(|ui| {
                                crate::ui::menu_rows(ui);
                                ui.set_min_width(220.0);
                                if ui.button(if e.favorite { "Remove from favorites" } else { "Add to favorites" }).clicked() {
                                    cmds.push(Cmd::RepoFavorite(e.path.clone()));
                                }
                                ui.menu_button("Move to group", |ui| {
                                    crate::ui::menu_rows(ui);
                                    for g in &groups {
                                        if ui.add_enabled(e.group.as_deref() != Some(g.as_str()), egui::Button::new(g)).clicked() {
                                            cmds.push(Cmd::RepoGroup(e.path.clone(), Some(g.clone())));
                                        }
                                    }
                                    if !groups.is_empty() {
                                        ui.separator();
                                    }
                                    if ui.button("New group…").clicked() {
                                        cmds.push(Cmd::Dialog(Dialog::NewGroup { path: e.path.clone(), name: String::new() }));
                                    }
                                    if e.group.is_some() {
                                        let folder = e.path.parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                        if ui.button(format!("Back to its folder “{folder}”")).clicked() {
                                            cmds.push(Cmd::RepoGroup(e.path.clone(), None));
                                        }
                                    }
                                });
                                ui.separator();
                                if !missing && ui.button("Show in Finder").clicked() {
                                    cmds.push(Cmd::Reveal(e.path.clone()));
                                }
                                if ui.button("Copy path").clicked() {
                                    cmds.push(Cmd::Copy(e.path.display().to_string()));
                                }
                                ui.separator();
                                if ui.button(RichText::new("Remove from the list").color(theme::c().deleted)).on_hover_text("The folder itself is not touched").clicked() {
                                    cmds.push(Cmd::RepoForget(e.path.clone()));
                                }
                            });
                        }
                        if rows.is_empty() {
                            ui.add_space(10.0);
                            let msg = if query.trim().is_empty() { "No repositories yet — add one below." } else { "Nothing matches" };
                            ui.label(RichText::new(msg).color(theme::c().faint));
                        }
                    });
                    ui.add_space(10.0);
                    let line = ui.cursor().top() - 0.5;
                    ui.painter().hline(ui.max_rect().x_range(), line, Stroke::new(1.0_f32, theme::c().border));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        use crate::modal::{button, Cap, Kind};
                        if button(ui, "Add a repository…", Kind::Plain, true).clicked() {
                            cmds.push(Cmd::OpenDialog);
                        }
                        if button(ui, "Add every repository in a folder…", Kind::Plain, true).clicked() {
                            cmds.push(Cmd::ScanFolder);
                        }
                        let running = states.fetch.as_ref().is_some_and(|f| f.running());
                        if button(ui, "Fetch all", Kind::Plain, !running).on_hover_text("git fetch --all --prune in every repository of the list, a few at a time").clicked() {
                            cmds.push(Cmd::FetchAllRepos);
                        }
                        if remove_missing_button(ui, &missing) {
                            cmds.push(Cmd::RepoRemoveMissing(missing.clone()));
                        }
                        // No room left for the keyboard hints beside "Remove N missing".
                        if missing.is_empty() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // Right to left: the last hint first.
                                ui.spacing_mut().item_spacing.x = 5.0;
                                let faint = |t: &str| RichText::new(t).size(theme::FS_SECONDARY).color(theme::c().faint);
                                ui.label(faint("close"));
                                crate::modal::keycap(ui, Cap::Text("Esc"));
                                ui.add_space(6.0);
                                ui.label(faint("open"));
                                crate::modal::keycap(ui, Cap::Text("Enter"));
                            });
                        }
                    });
                    match &states.fetch {
                        Some(f) if f.running() => {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add(egui::ProgressBar::new(f.done as f32 / f.total.max(1) as f32).desired_width(160.0).desired_height(6.0));
                                ui.label(RichText::new(format!("Fetching {} of {}…", f.done + 1, repos_count(f.total))).size(theme::FS_SECONDARY).color(theme::c().dim));
                            });
                        }
                        Some(f) if f.failures.is_empty() => {
                            ui.label(RichText::new(format!("Fetched {}.", repos_count(f.total))).size(theme::FS_SECONDARY).color(theme::c().added));
                        }
                        _ => {
                            ui.label(RichText::new("Right-click a repository to organise it.").size(theme::FS_SMALL).color(theme::c().faint));
                        }
                    }
                    // Failures stay listed, with Git's reason, until the next Fetch all.
                    if let Some(f) = states.fetch.as_ref().filter(|f| !f.failures.is_empty()) {
                        ui.add_space(4.0);
                        let head = if f.running() { format!("{} failed so far:", f.failures.len()) } else { format!("{} of {} could not be fetched:", f.failures.len(), repos_count(f.total)) };
                        ui.label(RichText::new(head).size(theme::FS_SECONDARY).color(theme::c().deleted));
                        for (path, err) in &f.failures {
                            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            ui.add(egui::Label::new(RichText::new(format!("{name} — {}", first_line(err))).size(theme::FS_SMALL).color(theme::c().dim)).truncate()).on_hover_text(err);
                        }
                    }
                });
        });
        crate::modal::animate_layer(ctx, area.response.layer_id, area.response.rect.center(), t);
        if let Some(k) = chosen {
            if let Some(row) = rows.get(k) {
                if row.entry.path.exists() {
                    let p = row.entry.path.clone();
                    cmds.push(if new_tab { Cmd::OpenTab(p) } else { Cmd::Open(p) });
                    close = true;
                } else {
                    self.notify(format!("{} isn't there any more. Right-click it to remove it from the list.", tilde(&row.entry.path)), true);
                }
            }
        }
        if close {
            self.switcher = None;
            // A finished Fetch all's summary goes with the switcher; the rows keep their failures.
            if self.states.fetch.as_ref().is_some_and(|f| !f.running() && self.states.sample.is_none()) {
                self.states.fetch = None;
            }
        }
    }
}

/// "Remove N missing", shown only when some repositories of the list lost their folder (the home
/// page and the switcher). Returns true when clicked.
pub(super) fn remove_missing_button(ui: &mut egui::Ui, missing: &[PathBuf]) -> bool {
    if missing.is_empty() {
        return false;
    }
    crate::modal::button(ui, &format!("Remove {} missing", missing.len()), crate::modal::Kind::Plain, true)
        .on_hover_text("Takes the repositories whose folder is gone off the list. The folders on disk are not touched, and those on an external disk that is not plugged in stay.")
        .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn matches_name_group_and_folder() {
        let e = Entry { path: PathBuf::from("/code/nova/site"), group: Some("Clients".into()), favorite: false, last_opened: 0, project: None, details: None };
        assert!(matches("site", &e));
        assert!(matches("cli SITE", &e));
        assert!(matches("nova", &e));
        assert!(!matches("api", &e));
    }
}
