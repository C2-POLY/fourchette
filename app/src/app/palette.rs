//! Command palette, modelled on the original's Quick Launch window
//! (QuickLaunchWindowController, its command providers and its layout — see MECHANICS.md).
use super::*;
use crate::ui::checkout_remote;
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Key, Margin, RichText, Sense, Stroke};

/// Rows shown before the list scrolls (estimated: the original's table size is set in its nib).
const VISIBLE_ROWS: f32 = 12.0;
const PALETTE_W: f32 = 560.0;

/// Every query word must appear in the text (case-insensitive).
fn matches(query: &str, text: &str) -> bool {
    let t = text.to_lowercase();
    query.split_whitespace().all(|w| t.contains(&w.to_lowercase()))
}

/// The original's hidden command: exactly three words, "settings", "maxCommitCount" and a
/// whole number, optionally signed (handleSettingsCommand, sub_100114e44).
fn settings_command(query: &str) -> Option<Result<usize, String>> {
    let w: Vec<&str> = query.split(' ').collect();
    if w.len() != 3 || w[0] != "settings" {
        return None;
    }
    if w[1] != "maxCommitCount" {
        return Some(Err(format!("Unknown setting “{}”", w[1])));
    }
    match w[2].parse::<i64>() {
        Ok(n) if n > 0 => Some(Ok(n as usize)),
        _ => Some(Err("maxCommitCount needs a positive whole number".into())),
    }
}

pub struct Item {
    pub group: &'static str,
    pub title: String,
    pub hint: String,
    pub cmd: Option<Cmd>,
}

fn item(group: &'static str, title: impl Into<String>, hint: impl Into<String>, cmd: Cmd) -> Item {
    Item { group, title: title.into(), hint: hint.into(), cmd: Some(cmd) }
}

impl App {
    fn palette_items(&self, query: &str, checkout_only: bool) -> Vec<Item> {
        let t = self.tab();
        let mut v: Vec<Item> = Vec::new();
        if let Some(res) = settings_command(query) {
            v.push(match res {
                Ok(n) => item("Settings", format!("Set maxCommitCount to {n}"), "press Enter", Cmd::SetCommitLimit(n)),
                Err(e) => Item { group: "Settings", title: e, hint: String::new(), cmd: None },
            });
            return v;
        }
        let cur = t.branch.clone().unwrap_or_default();
        if !checkout_only {
            // DefaultCommandProvider
            if let Some(l) = t.undo_label() {
                v.push(item("Commands", format!("Undo “{l}”"), "⌘Z", Cmd::Undo));
            }
            v.push(item("Commands", "Fetch", "get the latest from the remote", Cmd::Fetch { all: false, quiet: false }));
            v.push(item("Commands", "Fetch all remotes", "", Cmd::Fetch { all: true, quiet: false }));
            v.push(item("Commands", "Pull", "merge the upstream branch", Cmd::Pull));
            v.push(item("Commands", "Push", "send your commits", Cmd::Push));
            v.push(item(
                "Commands",
                "New branch…",
                "at the current commit",
                Cmd::new_ref_at_head(false),
            ));
            v.push(item("Commands", "Stash changes", "", Cmd::stash()));
            v.push(item("Commands", "Pop stash", "", Cmd::pop_stash()));
            v.push(item("Commands", "Show local changes", "", Cmd::Select(Sel::Wip)));
            v.push(item("Commands", "Stage all changes", "", Cmd::Git { label: "All changes staged".into(), args: vec!["add".into(), "-A".into()], network: false }));
            v.push(item("Assistant", "Clean up branches with Claude", "AI · your subscription", Cmd::AiCleanup));
            if !t.status.staged.is_empty() {
                v.push(item("Assistant", "Suggest a commit message with Claude", "AI · from staged changes", Cmd::AiCommitMessage));
            }
            if let Sel::Commit(i) = t.sel {
                if let Some(c) = t.commits.get(i) {
                    v.push(item("Assistant", "Explain this commit with Claude", "AI · plain words", Cmd::AiExplain(c.hash.clone())));
                }
            }
            v.push(item("Commands", "Open repository…", "", Cmd::OpenDialog));
            v.push(item("Commands", "Refresh", "⌘R", Cmd::Reload));
            v.push(item("Commands", "Settings…", "⌘, · appearance, variant, automatic fetch", Cmd::Settings));
            let now = self.library.appearance;
            for a in theme::Appearance::ALL {
                let hint = match (a == now, a) {
                    (true, _) => "current",
                    (_, theme::Appearance::System) => "follow macOS",
                    _ => "",
                };
                v.push(item("Appearance", format!("Appearance: {}", a.label()), hint, Cmd::SetAppearance(a)));
            }
            let d = t.details_layout;
            for place in crate::library::DetailsPlace::ALL {
                let hint = if !d.hidden && d.place == place { "current" } else { "" };
                v.push(item("View", format!("Details panel: {}", place.label()), hint, Cmd::SetDetails(crate::library::DetailsLayout::shown(place))));
            }
            v.push(item("View", "Details panel: Hidden", if d.hidden { "current" } else { "⌘J" }, Cmd::SetDetails(d.hide())));
            // "theme" in the hint so that typing it finds the variants too. "current" is the one on
            // screen, which FOURCHETTE_VARIANT may set apart from the saved one.
            let now = theme::variant();
            for t in theme::Variant::ALL {
                let hint = if t == now { "theme · current".to_string() } else { format!("theme · {}", t.blurb()) };
                v.push(item("Appearance", format!("Variant: {}", t.label()), hint, Cmd::SetVariant(t)));
            }
        }
        // ReferenceCommandProvider
        for r in &t.refs {
            match r.kind {
                RefKind::Local if !r.is_head => {
                    v.push(item("Branches", format!("Check out {}", r.name), "branch", Cmd::Git { label: format!("Switched to {}", r.name), args: vec!["checkout".into(), r.name.clone()], network: false }));
                    if !checkout_only && !cur.is_empty() {
                        v.push(item("Branches", format!("Merge {} into {cur}", r.name), "branch", Cmd::Git { label: format!("Merged {}", r.name), args: vec!["merge".into(), "--no-edit".into(), r.name.clone()], network: false }));
                    }
                }
                RefKind::Remote => v.push(item("Branches", format!("Check out {}", r.name), "remote branch", checkout_remote(r, &t.refs))),
                RefKind::Tag if !checkout_only => v.push(item("Tags", format!("Go to tag {}", r.name), "tag", Cmd::SelectHash(r.target.clone()))),
                _ => {}
            }
        }
        if !checkout_only {
            // WorkspaceCommandProvider: repositories ("Open Recent Repository" in the nib), with
            // the whole repository list rather than only the recent ones.
            v.push(item("Repositories", "Switch repository…", "⌘O", Cmd::Switcher));
            let mut repos: Vec<&crate::library::Entry> = self.library.entries.iter().collect();
            repos.sort_by_key(|e| (std::cmp::Reverse(e.favorite), std::cmp::Reverse(e.last_opened)));
            for e in repos {
                v.push(item("Repositories", format!("Open {}", e.name()), e.group_name(), Cmd::Open(e.path.clone())));
            }
        }
        let mut out: Vec<Item> = v.into_iter().filter(|i| query.trim().is_empty() || matches(query, &format!("{} {}", i.title, i.hint))).collect();
        if !checkout_only && query.trim().len() >= 2 {
            for c in t.commits.iter().filter(|c| matches(query, &format!("{} {} {}", c.subject, c.author, short(&c.hash)))).take(8) {
                out.push(item("Commits", c.subject.clone(), format!("{} · {}", short(&c.hash), c.author), Cmd::SelectHash(c.hash.clone())));
            }
        }
        out
    }

    pub(super) fn palette_ui(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let Some(pal) = self.palette.as_ref() else { return };
        let (query, checkout_only) = (pal.query.clone(), pal.checkout_only);
        let mut items = self.palette_items(&query, checkout_only);
        let n = items.len();
        let (up, down, enter, esc) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let Some(pal) = self.palette.as_mut() else { return };
        if down && n > 0 {
            pal.sel = (pal.sel + 1).min(n - 1);
        }
        if up {
            pal.sel = pal.sel.saturating_sub(1);
        }
        pal.sel = pal.sel.min(n.saturating_sub(1));
        let mut chosen: Option<usize> = if enter && n > 0 { Some(pal.sel) } else { None };
        let mut close = esc;

        // Placement from the original (sub_1001f8330): centred horizontally on the main window,
        // and its frame sits a third of its own height above the window's middle.
        let screen = ctx.screen_rect();
        let list_h = VISIBLE_ROWS * PALETTE_ROW_H;
        let total_h = 20.0 + 44.0 + 6.0 + list_h + 24.0;
        let left = screen.center().x - PALETTE_W * 0.5 - 10.0;
        let top = (screen.center().y - total_h * 2.0 / 3.0).max(screen.top() + 12.0);

        egui::Area::new(egui::Id::new("palette_backdrop")).fixed_pos(screen.min).order(egui::Order::Foreground).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::ZERO, theme::shadow(0.8));
            if resp.clicked() {
                close = true;
            }
        });
        egui::Area::new(egui::Id::new("palette")).order(egui::Order::Tooltip).fixed_pos(pos2(left, top)).show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::c().elevated)
                .stroke(Stroke::new(1.0_f32, theme::c().border_strong))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(10))
                .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: theme::shadow(1.0) })
                .show(ui, |ui| {
                    ui.set_width(PALETTE_W);
                    let hint = if checkout_only { "Check out a branch…" } else { "Type a command, a branch or a commit…" };
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut pal.query)
                            .hint_text(crate::ui::faint_hint(hint))
                            .font(FontId::proportional(theme::FS_TITLE))
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(10, 8)),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        pal.sel = 0;
                    }
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(list_h).min_scrolled_height(list_h.min(n as f32 * PALETTE_ROW_H + 40.0)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let mut last_group = "";
                        for (k, it) in items.iter().enumerate() {
                            // Group row (HeaderCommandProviderItem), 17 pt high.
                            if it.group != last_group {
                                last_group = it.group;
                                let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), PALETTE_HEADER_H), Sense::hover());
                                ui.painter().text(pos2(hr.left() + 10.0, hr.center().y), Align2::LEFT_CENTER, it.group, theme::small(), theme::c().faint);
                            }
                            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), PALETTE_ROW_H), Sense::click());
                            let selected = k == pal.sel;
                            if selected || resp.hovered() {
                                ui.painter().rect_filled(rect, CornerRadius::same(6), if selected { theme::c().select } else { theme::c().hover });
                            }
                            if selected && (up || down) {
                                ui.scroll_to_rect(rect, None);
                            }
                            let p = ui.painter().with_clip_rect(rect);
                            let tc = if it.cmd.is_none() { theme::c().deleted } else if selected { theme::c().strong } else { theme::c().text };
                            let tr = p.text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, &it.title, theme::body(), tc);
                            // Secondary text, like the original's secondaryTextField.
                            p.text(pos2((tr.right() + 12.0).max(rect.right() - 220.0), rect.center().y), Align2::LEFT_CENTER, &it.hint, theme::secondary(), theme::c().faint);
                            if resp.clicked() {
                                chosen = Some(k);
                            }
                        }
                        if items.is_empty() {
                            ui.add_space(6.0);
                            ui.label(RichText::new("Nothing matches").color(theme::c().faint));
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("Up / Down to move · Enter to run · Esc to close").size(theme::FS_SMALL).color(theme::c().faint));
                });
        });
        if let Some(k) = chosen {
            if k < items.len() {
                match items.swap_remove(k).cmd {
                    Some(c) => {
                        cmds.push(c);
                        close = true;
                    }
                    None => {}
                }
            }
        }
        if close {
            self.palette = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::settings_command;

    #[test]
    fn settings_command_rules() {
        assert_eq!(settings_command("settings maxCommitCount 5000"), Some(Ok(5000)));
        assert_eq!(settings_command("settings maxCommitCount +200"), Some(Ok(200)));
        assert!(matches!(settings_command("settings maxCommitCount abc"), Some(Err(_))));
        assert!(matches!(settings_command("settings colour 3"), Some(Err(_))));
        assert_eq!(settings_command("settings maxCommitCount"), None);
        assert_eq!(settings_command("fetch"), None);
    }
}
