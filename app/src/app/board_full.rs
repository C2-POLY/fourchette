//! The full board (⌃⌘B, issue #153): the project's board alone in the window, for the most
//! columns. The toolbar, the tabs, the sidebar, the details panel and the status bar give way to
//! one thin bar that keeps what is needed to switch quickly: the repository (its tabs and ⌘O),
//! the branch, the Changes · History · Project views and the way out. A ticket's detail opens in
//! a drawer over the columns. It is the window's mode, not macOS full screen (⌃⌘F).
use super::board::segmented;
use super::*;
use crate::modal::{self, Cap};
use egui::{pos2, vec2, Align, Align2, CornerRadius, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui};

/// Height of the thin bar that replaces the toolbar.
pub(super) const BAR_H: f32 = 44.0;
/// Width of the ticket's drawer.
const DRAWER_W: f32 = 420.0;

/// Whether the window shows the full board, and the tab it was last shown for.
#[derive(Default)]
pub(super) struct FullBoard {
    pub(super) on: bool,
    tab: u64,
}

/// What the full board does on a frame, from the active tab.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// The window as usual: the mode is off, or the tab shows the home page.
    Normal,
    /// The board alone.
    Show,
    /// Another tab came up with its board closed: open it, the mode goes on.
    OpenBoard,
    /// The board of the same tab was closed (Changes, History, ⌘B): the mode ends.
    Leave,
}

/// The rule behind `Step`: switching tab or repository keeps the mode, closing the board ends it.
pub(super) fn step(on: bool, shown_for: u64, tab: u64, home: bool, board_open: bool) -> Step {
    match (on, home, board_open) {
        (false, ..) | (true, true, _) => Step::Normal,
        (true, false, true) => Step::Show,
        (true, false, false) if tab != shown_for => Step::OpenBoard,
        _ => Step::Leave,
    }
}

impl FullBoard {
    pub(super) fn enter(&mut self, tab: u64) {
        self.on = true;
        self.tab = tab;
    }
}

/// Four corners pointing inwards: leave the full board.
fn exit_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = c + vec2(dx * 2.0, dy * 2.0);
        p.line_segment([corner, corner + vec2(dx * 4.0, 0.0)], s);
        p.line_segment([corner, corner + vec2(0.0, dy * 4.0)], s);
    }
}

/// Four corners pointing outwards: show the board alone.
pub(super) fn enter_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = c + vec2(dx * 6.0, dy * 6.0);
        p.line_segment([corner, corner - vec2(dx * 4.0, 0.0)], s);
        p.line_segment([corner, corner - vec2(0.0, dy * 4.0)], s);
    }
}

/// A folder, for the repository.
fn repo_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    let r = Rect::from_center_size(c + vec2(0.0, 1.0), vec2(14.0, 10.0));
    p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.3_f32, col), StrokeKind::Inside);
    p.line_segment([r.left_top() + vec2(1.0, 0.0), r.left_top() + vec2(5.0, -2.5)], Stroke::new(1.3_f32, col));
    p.line_segment([r.left_top() + vec2(5.0, -2.5), r.left_top() + vec2(7.0, 0.0)], Stroke::new(1.3_f32, col));
}

/// A small downward chevron.
fn chevron(p: &egui::Painter, c: Pos2, col: Color32) {
    let s = Stroke::new(1.3_f32, col);
    p.line_segment([c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0)], s);
    p.line_segment([c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)], s);
}

/// A flat button of the thin bar: an icon drawn by `icon` (if any), its text, and room for
/// `extra` points on the right. Returns the response and the rect left for `extra`.
fn bar_button(ui: &mut Ui, icon: Option<fn(&egui::Painter, Pos2, Color32)>, text: &str, strong: bool, extra: f32) -> (egui::Response, Rect) {
    let k = theme::c();
    let font = theme::body();
    let g = ui.painter().layout_no_wrap(text.to_string(), font, k.text);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(10.0 + icon_w + g.size().x + extra + 10.0, 28.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), k.widget_hover);
    }
    let col = if strong || resp.hovered() { k.strong } else { k.dim };
    if let Some(f) = icon {
        f(p, pos2(rect.left() + 10.0 + 8.0, rect.center().y), col);
    }
    let tx = rect.left() + 10.0 + icon_w;
    p.text(pos2(tx, rect.center().y), Align2::LEFT_CENTER, text, theme::body(), col);
    let rest = Rect::from_min_max(pos2(tx + g.size().x, rect.top()), pos2(rect.right() - 10.0, rect.bottom()));
    (resp, rest)
}

impl App {
    /// ⌃⌘B: shows the active tab's board alone, opening the board if needed, or leaves.
    pub(super) fn board_full_toggle(&mut self) {
        if self.full.on {
            self.full.on = false;
            return;
        }
        if !self.tab().board.open {
            self.board_toggle();
        }
        let key = self.tab().key;
        self.full.enter(key);
    }

    /// Whether this frame shows the full board, after following the active tab.
    pub(super) fn board_full_shown(&mut self) -> bool {
        let t = self.tab();
        match step(self.full.on, self.full.tab, t.key, t.home, t.board.open) {
            Step::Normal => false,
            Step::Show => {
                self.full.tab = self.tab().key;
                true
            }
            Step::OpenBoard => {
                self.board_toggle();
                self.full.tab = self.tab().key;
                true
            }
            Step::Leave => {
                self.full.on = false;
                false
            }
        }
    }

    /// Esc with nothing else to close leaves the full board. Returns whether it did.
    pub(super) fn board_full_escape(&mut self) -> bool {
        std::mem::take(&mut self.full.on)
    }

    /// The thin bar at the window's top edge, in place of the toolbar.
    pub(super) fn board_full_bar(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        crate::titlebar::drag_area(ui);
        let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let k = theme::c();
        ui.horizontal_centered(|ui| {
            ui.add_space(crate::titlebar::inset(fullscreen).max(10.0));
            ui.spacing_mut().item_spacing.x = 4.0;

            // The repository: its tabs, another repository (⌘O), a new tab (⌘T).
            let name = self.tab().name();
            let (repo, rest) = bar_button(ui, Some(repo_icon), &name, true, 14.0);
            chevron(ui.painter(), pos2(rest.right() - 4.0, rest.center().y), k.dim);
            let repo = repo.on_hover_text("Tabs and repositories");
            egui::Popup::menu(&repo).show(|ui| {
                crate::ui::menu_rows(ui);
                ui.set_min_width(240.0);
                if self.tabs.len() > 1 {
                    ui.label(RichText::new("Tabs").size(theme::FS_SMALL).color(k.faint));
                    for (i, t) in self.tabs.iter().enumerate() {
                        if ui.add(egui::Button::selectable(i == self.active, t.name())).clicked() {
                            cmds.push(Cmd::SelectTab(i));
                        }
                    }
                    ui.separator();
                }
                if ui.add(egui::Button::new("Switch repository…").right_text(RichText::new("⌘O").color(k.faint))).clicked() {
                    cmds.push(Cmd::Switcher);
                }
                if ui.add(egui::Button::new("New tab").right_text(RichText::new("⌘T").color(k.faint))).clicked() {
                    cmds.push(Cmd::NewTab);
                }
            });

            // The branch: a click checks out another one, as the toolbar's.
            let t = self.tab();
            let branch = t.branch.clone().unwrap_or_else(|| format!("detached at {}", short(&t.head)));
            if bar_button(ui, None, &branch, false, 0.0).0.on_hover_text("Check out another branch").clicked() {
                cmds.push(Cmd::PaletteCheckout);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(8.0);
                // The way out, with its key.
                let (exit, rest) = bar_button(ui, Some(exit_icon), "Exit full board", true, 46.0);
                let mut cap = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(rest.right() - 38.0, rest.center().y - 9.0), vec2(38.0, 18.0))));
                modal::keycap(&mut cap, Cap::Text("⌃⌘B"));
                if exit.on_hover_text("Bring back the toolbar, the sidebar and the details (⌃⌘B, Esc)").clicked() {
                    cmds.push(Cmd::BoardFull);
                }

                // The views, in the middle of what is left.
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    // Centred on the window, as long as the branch leaves room.
                    let w = super::board::segmented_width(ui, &["Changes", "History", "Project"]);
                    let left = ui.ctx().content_rect().center().x - w / 2.0;
                    let room = ui.available_width() - w;
                    ui.add_space((left - ui.cursor().left()).clamp(8.0, room.max(8.0)));
                    let t = self.tab();
                    if let Some(i) = segmented(ui, &["Changes", "History", "Project"], 2) {
                        match i {
                            0 => cmds.push(Cmd::Select(Sel::Wip)),
                            _ if !t.commits.is_empty() => cmds.push(Cmd::SelectHash(t.head.clone())),
                            _ => cmds.push(Cmd::Board),
                        }
                    }
                });
            });
        });
    }
}

impl Tab {
    /// The ticket's detail in a drawer at the right, over the columns, below `top`.
    pub(super) fn board_drawer(&mut self, ctx: &egui::Context, top: f32, cmds: &mut Vec<Cmd>) {
        if self.board.detail.is_none() {
            return;
        }
        let screen = ctx.content_rect();
        let rect = Rect::from_min_max(pos2(screen.right() - DRAWER_W, top), screen.max);
        let k = theme::c();
        egui::Area::new(egui::Id::unique(("board_drawer", self.key)))
            .order(egui::Order::Middle)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                ui.set_clip_rect(rect.expand2(vec2(24.0, 0.0)));
                // A soft shadow on the columns, then the drawer.
                ui.painter().add(egui::epaint::Shadow { offset: [-6, 0], blur: 20, spread: 0, color: Color32::from_black_alpha(if theme::is_light() { 30 } else { 90 }) }.as_shape(rect, CornerRadius::ZERO));
                ui.painter().rect_filled(rect, CornerRadius::ZERO, k.bg);
                ui.painter().line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0_f32, k.border));
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                egui::Frame::new().inner_margin(Margin::symmetric(16, 0)).show(ui, |ui| {
                    ui.set_width(rect.width() - 32.0);
                    ui.set_height(rect.height());
                    self.board_detail_panel(ui, cmds);
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_follows_the_tab() {
        // Off, or a tab on the home page: the window as usual.
        assert_eq!(step(false, 1, 1, false, true), Step::Normal);
        assert_eq!(step(true, 1, 2, true, false), Step::Normal);
        // On, with the board open: the board alone.
        assert_eq!(step(true, 1, 1, false, true), Step::Show);
        // Another tab (⌃Tab, ⌘O) with its board closed: open it, keep the mode.
        assert_eq!(step(true, 1, 2, false, false), Step::OpenBoard);
        // The same tab's board closed by Changes, History or ⌘B: the mode ends.
        assert_eq!(step(true, 1, 1, false, false), Step::Leave);
    }
}
