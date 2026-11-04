//! Repository switcher (⌘O): favourites, recent ones and every repository under its group.
use super::*;
use crate::library::Entry;
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

impl App {
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
        let current = self.repo.as_ref().map(|r| r.root.clone());
        let groups = self.library.custom_groups();
        let (up, down, enter, esc) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let Some(sw) = self.switcher.as_mut() else { return };
        if down && n > 0 {
            sw.sel = (sw.sel + 1).min(n - 1);
        }
        if up {
            sw.sel = sw.sel.saturating_sub(1);
        }
        sw.sel = sw.sel.min(n.saturating_sub(1));
        let mut chosen: Option<usize> = if enter && n > 0 { Some(sw.sel) } else { None };
        let mut close = esc;

        let screen = ctx.screen_rect();
        let left = screen.center().x - SWITCHER_W * 0.5 - 12.0;
        let top = (screen.top() + 70.0).max(screen.center().y - 330.0);
        egui::Area::new(egui::Id::new("switcher_backdrop")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_black_alpha(110));
            if resp.clicked() {
                close = true;
            }
        });
        egui::Area::new(egui::Id::new("switcher")).order(egui::Order::Foreground).fixed_pos(pos2(left, top)).show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::PANEL)
                .stroke(Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.6)))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(12))
                .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(140) })
                .show(ui, |ui| {
                    ui.set_width(SWITCHER_W);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut sw.query)
                            .hint_text("Switch repository — type a name, a group or a folder…")
                            .font(FontId::proportional(16.0))
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
                                ui.painter().text(pos2(hr.left() + 10.0, hr.bottom() - 6.0), Align2::LEFT_BOTTOM, row.section.to_uppercase(), FontId::proportional(10.0), theme::FAINT);
                            }
                            let e = &row.entry;
                            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                            let selected = k == sw.sel;
                            if selected || resp.hovered() {
                                ui.painter().rect_filled(rect, CornerRadius::same(6), if selected { theme::SELECT } else { theme::HOVER });
                            }
                            if selected && (up || down) {
                                ui.scroll_to_rect(rect, None);
                            }
                            // The star toggles the favourite without opening the repository.
                            let star = Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(22.0, 22.0));
                            let star_resp = ui.interact(star, ui.id().with(("star", k)), Sense::click());
                            let star_col = if e.favorite { theme::MODIFIED } else if star_resp.hovered() { theme::DIM } else { theme::BORDER };
                            let p = ui.painter().with_clip_rect(rect);
                            p.text(star.center(), Align2::CENTER_CENTER, if e.favorite { "★" } else { "☆" }, FontId::proportional(13.0), star_col);
                            if star_resp.clicked() {
                                cmds.push(Cmd::RepoFavorite(e.path.clone()));
                            }
                            let missing = !e.path.exists();
                            let is_current = current.as_deref() == Some(e.path.as_path());
                            let tc = if missing { theme::FAINT } else if selected || is_current { Color32::WHITE } else { theme::TEXT };
                            let name = p.text(pos2(rect.left() + 34.0, rect.center().y), Align2::LEFT_CENTER, e.name(), FontId::proportional(14.0), tc);
                            let mut x = name.right() + 10.0;
                            if is_current {
                                let r = p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, "✔ open", FontId::proportional(11.0), theme::ADDED);
                                x = r.right() + 10.0;
                            }
                            let where_ = if missing { format!("{} — folder missing", tilde(&e.path)) } else { tilde(e.path.parent().unwrap_or(&e.path)) };
                            p.with_clip_rect(Rect::from_min_max(pos2(x, rect.top()), pos2(rect.right() - 44.0, rect.bottom()))).text(
                                pos2(x.max(rect.left() + 230.0), rect.center().y),
                                Align2::LEFT_CENTER,
                                where_,
                                FontId::proportional(11.5),
                                if missing { theme::DELETED.gamma_multiply(0.8) } else { theme::FAINT },
                            );
                            if let Some(s) = row.shortcut {
                                p.text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, format!("⌘{s}"), FontId::proportional(11.0), theme::DIM);
                            }
                            if resp.clicked() && !star_resp.clicked() {
                                chosen = Some(k);
                            }
                            resp.on_hover_text(e.path.display().to_string()).context_menu(|ui| {
                                ui.set_min_width(220.0);
                                if ui.button(if e.favorite { "☆  Remove from favorites" } else { "★  Add to favorites" }).clicked() {
                                    cmds.push(Cmd::RepoFavorite(e.path.clone()));
                                }
                                ui.menu_button("Move to group", |ui| {
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
                                if ui.button(RichText::new("Remove from the list").color(theme::DELETED)).on_hover_text("The folder itself is not touched").clicked() {
                                    cmds.push(Cmd::RepoForget(e.path.clone()));
                                }
                            });
                        }
                        if rows.is_empty() {
                            ui.add_space(10.0);
                            let msg = if query.trim().is_empty() { "No repositories yet — add one below." } else { "Nothing matches" };
                            ui.label(RichText::new(msg).color(theme::FAINT));
                        }
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("+  Add a repository…").clicked() {
                            cmds.push(Cmd::OpenDialog);
                        }
                        if ui.button("📂  Add every repository in a folder…").clicked() {
                            cmds.push(Cmd::ScanFolder);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new("Enter to open · right-click to organise · Esc").size(10.5).color(theme::FAINT));
                        });
                    });
                });
        });
        if let Some(k) = chosen {
            if let Some(row) = rows.get(k) {
                if row.entry.path.exists() {
                    cmds.push(Cmd::Open(row.entry.path.clone()));
                    close = true;
                } else {
                    self.notify(format!("{} isn't there any more. Right-click it to remove it from the list.", tilde(&row.entry.path)), true);
                }
            }
        }
        if close {
            self.switcher = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn matches_name_group_and_folder() {
        let e = Entry { path: PathBuf::from("/code/nova/site"), group: Some("Clients".into()), favorite: false, last_opened: 0, project: None };
        assert!(matches("site", &e));
        assert!(matches("cli SITE", &e));
        assert!(matches("nova", &e));
        assert!(!matches("api", &e));
    }
}
