//! The tabs above the history: Changes, History and Project (#123). They replace the three
//! entries that used to head the sidebar. The active one comes from the tab's state, so ⌘B, the
//! palette and the "Uncommitted changes" row keep it in step without telling it anything.
use super::*;
use crate::ui::board_icon;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui};

/// Height of the strip, in points.
pub(super) const STRIP_H: f32 = 42.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum View {
    Changes,
    History,
    Project,
}

/// The view on screen: the board whenever it is open, else the local changes when they are
/// selected, else the history.
pub(super) fn active_view(board_open: bool, wip_selected: bool) -> View {
    if board_open {
        View::Project
    } else if wip_selected {
        View::Changes
    } else {
        View::History
    }
}

/// Why the Project tab can't be used, or `None` when it can. It stays usable while the project
/// loads and after a failed read (the board offers to try again); it is greyed out when the
/// repository has no GitHub remote or no project holds its issues.
pub(super) fn project_unavailable(loading: bool, failed: bool, board_has_projects: Option<bool>) -> Option<&'static str> {
    if loading || failed {
        return None;
    }
    match board_has_projects {
        Some(true) => None,
        Some(false) => Some("No GitHub project holds this repository's issues. Link one from the repository's Projects tab on GitHub."),
        None => Some("This repository has no GitHub remote, so there is no project to show."),
    }
}

/// A pencil, for the local changes.
fn pencil_icon(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    p.line_segment([pos2(c.x - 5.0, c.y + 5.0), pos2(c.x + 4.5, c.y - 4.5)], Stroke::new(3.2_f32, col));
    p.line_segment([pos2(c.x - 6.0, c.y + 6.0), pos2(c.x - 3.0, c.y + 6.0)], s);
}

/// Three lines, for the list of commits.
fn lines_icon(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    for (i, w) in [8.0, 6.0, 8.0].iter().enumerate() {
        let y = c.y - 4.5 + i as f32 * 4.5;
        p.line_segment([pos2(c.x - w, y), pos2(c.x + w, y)], Stroke::new(1.5_f32, col));
    }
}

impl Tab {
    pub(super) fn view_tabs(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let active = active_view(self.board.open, self.sel == Sel::Wip);
        let reason = project_unavailable(self.project.loading.is_some(), self.project.error.is_some(), self.project.board.as_ref().map(|b| !b.projects.is_empty()));
        let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), STRIP_H), Sense::hover());
        let p = ui.painter().clone();
        p.line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0_f32, theme::c().border));

        let items = [
            (View::Changes, "Changes", self.status.staged.len() + self.status.unstaged.len(), None, "Local changes (staged and unstaged files)"),
            (View::History, "History", self.commits.len(), None, "The history of the repository, on HEAD"),
            (View::Project, "Project", self.project_open_count(), reason, "The GitHub project of this repository (⌘B)"),
        ];
        let font = theme::body();
        let small = theme::small();
        // Measure first: the track is as wide as its three segments.
        let widths: Vec<(f32, f32, f32)> = items
            .iter()
            .map(|(_, label, n, _, _)| {
                let lw = p.layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE).size().x;
                let nw = if *n > 0 { p.layout_no_wrap(n.to_string(), small.clone(), Color32::WHITE).size().x } else { 0.0 };
                (lw, nw, 12.0 + 14.0 + 6.0 + lw + if nw > 0.0 { 6.0 + nw } else { 0.0 } + 12.0)
            })
            .collect();
        let total: f32 = widths.iter().map(|w| w.2).sum::<f32>() + 2.0 * 3.0 + 2.0 * 2.0;
        let track = Rect::from_min_size(pos2(strip.left() + 16.0, strip.center().y - 15.0), vec2(total, 30.0));
        p.rect_filled(track, CornerRadius::same(9), theme::c().hover);

        let mut x = track.left() + 3.0;
        for ((view, label, n, why, tip), (lw, nw, w)) in items.iter().zip(&widths) {
            let seg = Rect::from_min_size(pos2(x, track.top() + 3.0), vec2(*w, 24.0));
            x += w + 2.0;
            let enabled = why.is_none() || *view == active;
            let resp = ui.interact(seg, ui.id().with(("view_tab", *view as u8)), if enabled { Sense::click() } else { Sense::hover() });
            let on = *view == active;
            if on {
                p.rect_filled(seg.translate(vec2(0.0, 1.0)), CornerRadius::same(7), theme::c().border);
                p.rect_filled(seg, CornerRadius::same(7), theme::c().elevated);
                p.rect_stroke(seg, CornerRadius::same(7), Stroke::new(1.0_f32, theme::c().border), StrokeKind::Inside);
            } else if resp.hovered() && enabled {
                p.rect_filled(seg, CornerRadius::same(7), theme::c().widget_hover);
            }
            let (ink, faint) = if !enabled {
                (theme::c().faint, theme::c().faint)
            } else if on {
                (theme::c().strong, theme::c().dim)
            } else if resp.hovered() {
                (theme::c().text, theme::c().dim)
            } else {
                (theme::c().dim, theme::c().faint)
            };
            let ic = pos2(seg.left() + 12.0 + 7.0, seg.center().y);
            match view {
                View::Changes => pencil_icon(&p, ic, ink),
                View::History => lines_icon(&p, ic, ink),
                View::Project => board_icon(&p, ic, ink),
            }
            let tx = seg.left() + 12.0 + 14.0 + 6.0;
            p.text(pos2(tx, seg.center().y), Align2::LEFT_CENTER, label, font.clone(), ink);
            if *nw > 0.0 {
                p.text(pos2(tx + lw + 6.0, seg.center().y), Align2::LEFT_CENTER, n, small.clone(), faint);
            }
            let resp = if let Some(r) = why { resp.on_hover_text(*r) } else { resp.on_hover_text(*tip) };
            if enabled {
                resp.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && enabled {
                match view {
                    View::Changes => cmds.push(Cmd::Select(Sel::Wip)),
                    View::History if !self.commits.is_empty() => cmds.push(Cmd::SelectHash(self.head.clone())),
                    View::History if self.board.open => cmds.push(Cmd::Board),
                    View::History => {}
                    View::Project if !self.board.open => cmds.push(Cmd::Board),
                    View::Project => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_wins_over_the_selection() {
        assert_eq!(active_view(true, true), View::Project);
        assert_eq!(active_view(true, false), View::Project);
        assert_eq!(active_view(false, true), View::Changes);
        assert_eq!(active_view(false, false), View::History);
    }

    #[test]
    fn project_is_greyed_only_when_there_is_none() {
        assert!(project_unavailable(true, false, None).is_none(), "loading");
        assert!(project_unavailable(false, true, None).is_none(), "failed read: the board offers to retry");
        assert!(project_unavailable(false, false, Some(true)).is_none());
        assert!(project_unavailable(false, false, Some(false)).unwrap().contains("No GitHub project"));
        assert!(project_unavailable(false, false, None).unwrap().contains("no GitHub remote"));
    }
}
