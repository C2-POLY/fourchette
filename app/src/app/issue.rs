//! The issue of the current branch (#35): its chip in the toolbar, next to the branch, and the
//! offer to move it on the project as the work goes, In progress at checkout and In review once
//! its pull request exists. The move goes through the board's (`Tab::project_move`), and the
//! confirmation offers to take it back: that Undo is the project's, not Git's (Cmd+Z).
use super::*;
use crate::github::{self, Board, Item, Kind, Move};
use egui::{pos2, vec2, Align2, CornerRadius, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use std::collections::HashSet;
use std::sync::mpsc::Receiver;

/// How long the confirmation of a move stays, with its Undo.
pub const MOVED_SECS: f32 = 10.0;
/// How often the branch's pull request is looked for again, while none is found.
const PR_EVERY_SECS: u64 = 120;
/// The chip's widest, so the toolbar's buttons keep their room.
const CHIP_MAX_W: f32 = 240.0;

/// The issue a branch is for: the first number of its name (`feat/36-chip`, `fix_36`) that is an
/// issue of the open repository on the project. A number inside a word (`v2`, `64089a`) is not one.
pub fn linked_issue<'a>(board: &'a Board, branch: &str) -> Option<&'a Item> {
    branch
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|t| t.parse::<u64>().ok().filter(|n| n.to_string() == t))
        .find_map(|n| board.items.iter().find(|i| i.kind == Kind::Issue && i.number == Some(n) && board.in_repo(i)))
}

/// The project's status whose name holds `key` ("progress", "review"), when the issue is before it
/// in the project's order (or has no status). Never back, and never out of a finished status.
fn forward(statuses: &[String], current: &str, key: &str) -> Option<String> {
    if github::is_done(current) {
        return None;
    }
    let target = statuses.iter().position(|s| s.to_lowercase().contains(key))?;
    match statuses.iter().position(|s| s == current) {
        Some(at) if at >= target => None,
        _ => Some(statuses[target].clone()),
    }
}

/// The status to offer for the branch's issue: In review when a pull request exists, else In
/// progress just after a checkout. None when there is nothing to move.
pub fn offer(statuses: &[String], current: &str, pr: bool, checkout: bool) -> Option<String> {
    pr.then(|| forward(statuses, current, "review")).flatten().or_else(|| checkout.then(|| forward(statuses, current, "progress")).flatten())
}

/// A move of the branch's issue, offered or done.
#[derive(Clone, Debug)]
pub struct Offer {
    pub item: String,
    pub number: u64,
    pub title: String,
    /// The status it goes to.
    pub to: String,
    /// The status it was in, which Undo puts back.
    pub from: String,
    /// Moved at that time: the card says so and offers Undo for `MOVED_SECS`.
    pub moved: Option<Instant>,
}

/// What the tab knows about its branch's issue between frames.
#[derive(Default)]
pub struct IssueFlow {
    /// The branch last seen ("" when detached); None before the first look.
    branch: Option<String>,
    /// Checked out since the offer was last worked out: In progress may be offered.
    checkout: bool,
    /// Whether the branch has an open pull request, None until `gh` answered.
    pr: Option<bool>,
    /// The lookup running for that branch, and when the last one started.
    pr_job: Option<(String, Receiver<bool>)>,
    pr_at: Option<Instant>,
    /// Moves said no to (item, status), not offered again while the app runs.
    declined: HashSet<(String, String)>,
    pub offer: Option<Offer>,
}

/// Whether `branch` of the repository `slug` has an open pull request. Runs in a background
/// thread; an error counts as none.
fn has_pull_request(slug: &str, branch: &str) -> bool {
    let Some(exe) = github::find_gh() else { return false };
    github::gh(&exe, &["pr", "list", "--repo", slug, "--head", branch, "--state", "open", "--json", "number", "--limit", "1"])
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_array().map(|a| !a.is_empty()))
        .unwrap_or(false)
}

impl Tab {
    /// The issue of the current branch on the loaded project.
    pub(super) fn branch_issue(&self) -> Option<&Item> {
        linked_issue(self.project.board.as_deref()?, self.branch.as_deref()?)
    }

    /// Follows the branch: a checkout makes it look for the pull request, then work out what to
    /// offer once the project is there. With `auto` (Settings › Git) the move is done at once.
    pub(super) fn issue_poll(&mut self, auto: bool) {
        if self.repo.is_none() {
            return;
        }
        let branch = self.branch.clone().unwrap_or_default();
        match &self.issue.branch {
            None => self.issue.branch = Some(branch.clone()),
            Some(b) if *b != branch => {
                self.issue.branch = Some(branch.clone());
                self.issue.checkout = true;
                self.issue.pr = None;
                self.issue.pr_job = None;
                // An offer for the branch left behind goes; a done move keeps its Undo.
                if self.issue.offer.as_ref().is_some_and(|o| o.moved.is_none()) {
                    self.issue.offer = None;
                }
            }
            _ => {}
        }
        if let Some((b, rx)) = &self.issue.pr_job {
            if let Ok(found) = rx.try_recv() {
                if *b == branch {
                    self.issue.pr = Some(found);
                }
                self.issue.pr_job = None;
            }
        }
        if self.issue.offer.as_ref().and_then(|o| o.moved).is_some_and(|at| at.elapsed().as_secs_f32() > MOVED_SECS) {
            self.issue.offer = None;
        }
        let Some(board) = self.project.board.clone() else { return };
        let Some(it) = linked_issue(&board, &branch) else {
            self.issue.checkout = false;
            return;
        };
        // Look for the pull request: after a checkout, then now and then until one is found.
        let due = match self.issue.pr {
            None => true,
            Some(false) => self.issue.pr_at.is_none_or(|t| t.elapsed().as_secs() >= PR_EVERY_SECS),
            Some(true) => false,
        };
        if due && self.issue.pr_job.is_none() {
            let (tx, rx) = channel();
            let (slug, b) = (board.slug.clone(), branch.clone());
            std::thread::spawn(move || {
                let _ = tx.send(has_pull_request(&slug, &b));
            });
            self.issue.pr_job = Some((branch.clone(), rx));
            self.issue.pr_at = Some(Instant::now());
        }
        let Some(pr) = self.issue.pr else { return };
        if self.issue.offer.is_some() {
            return;
        }
        // A pull request linked to the issue on GitHub counts too.
        let pr = pr || it.pull_requests.iter().any(|p| p.state == "OPEN");
        let to = offer(&board.order(), &it.status, pr, self.issue.checkout);
        self.issue.checkout = false;
        let Some(to) = to.filter(|to| !self.issue.declined.contains(&(it.id.clone(), to.clone()))) else { return };
        let o = Offer { item: it.id.clone(), number: it.number.unwrap_or_default(), title: it.title.clone(), to, from: it.status.clone(), moved: None };
        self.issue.offer = Some(o);
        if auto {
            self.issue_accept();
        }
    }

    /// The offer's Move: the issue goes to the top of its new column, like the board's "Move to".
    pub(super) fn issue_accept(&mut self) {
        let Some(o) = self.issue.offer.as_mut().filter(|o| o.moved.is_none()) else { return };
        o.moved = Some(Instant::now());
        let m = Move { item: o.item.clone(), status: Some(o.to.clone()), place: !github::is_done(&o.to), after: None };
        self.project_move(m);
    }

    /// "Not now": the same move is not offered again while the app runs.
    pub(super) fn issue_decline(&mut self) {
        if let Some(o) = self.issue.offer.take() {
            self.issue.declined.insert((o.item, o.to));
        }
    }

    /// The confirmation's Undo: the issue goes back to its status, and that move is not offered again.
    pub(super) fn issue_undo(&mut self) {
        let Some(o) = self.issue.offer.take().filter(|o| o.moved.is_some()) else { return };
        self.issue.declined.insert((o.item.clone(), o.to.clone()));
        self.project_move(Move { item: o.item, status: Some(o.from.clone()), place: false, after: None });
        self.notify(format!("#{} is back in {}", o.number, o.from), false);
    }

    /// The toolbar's chip: a dot and `#n title` in the colour of its status, cut with "…" when long.
    pub(super) fn issue_chip(&self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let Some(board) = self.project.board.as_deref() else { return };
        let Some(it) = self.branch.as_deref().and_then(|b| linked_issue(board, b)) else { return };
        let k = theme::c();
        let col = super::project::status_color(board, &it.status);
        let num = format!("#{}", it.number.unwrap_or_default());
        let p = ui.painter();
        let ng = p.layout_no_wrap(num.clone(), theme::secondary(), k.strong);
        let tg = p.layout_no_wrap(it.title.clone(), theme::secondary(), k.text);
        let w = (24.0 + ng.size().x + 6.0 + tg.size().x + 12.0).min(CHIP_MAX_W);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
        let p = ui.painter();
        let fill = theme::tint(col, k.sidebar, if resp.hovered() { 0.26 } else { 0.16 });
        p.rect(rect, CornerRadius::same(13), fill, Stroke::new(1.0_f32, theme::tint(col, k.sidebar, 0.45)), StrokeKind::Inside);
        p.circle_filled(pos2(rect.left() + 13.0, rect.center().y), 4.0, col);
        let x = rect.left() + 24.0;
        p.galley(pos2(x, rect.center().y - ng.size().y / 2.0), ng.clone(), k.strong);
        let tx = x + ng.size().x + 6.0;
        crate::ui::fit_text(p, pos2(tx, rect.center().y), Align2::LEFT_CENTER, &it.title, theme::secondary(), k.text, rect.right() - 12.0 - tx);
        let tip = format!("{num} {}\n{} · click to see its details", it.title, it.status);
        if resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            cmds.push(Cmd::IssueDetail(it.id.clone()));
        }
    }

    /// The offer, at the bottom right of the window above the status bar: "Move #n to In progress?" with Move and Not
    /// now, then once moved a line that says so with Undo, and a bar for the time left.
    pub(super) fn issue_offer_ui(&self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let Some(o) = &self.issue.offer else { return };
        let k = theme::c();
        let col = self.project.board.as_deref().map(|b| super::project::status_color(b, &o.to)).unwrap_or(k.accent);
        let left = o.moved.map(|at| 1.0 - at.elapsed().as_secs_f32() / MOVED_SECS);
        if left.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        egui::Area::new(egui::Id::new(("issue_offer", self.key))).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, vec2(-16.0, -40.0)).show(ctx, |ui| {
            let bottom = if left.is_some() { 13 } else { 11 };
            let r = crate::modal::card(ui, 10, Margin { left: 12, right: 14, top: 11, bottom }, |ui| {
                ui.set_max_width(340.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                    let p = ui.painter();
                    p.circle_filled(r.center(), 9.0, theme::tint(col, k.elevated, 0.2));
                    if o.moved.is_some() {
                        crate::ui::check_mark(p, r.center() + vec2(0.0, 0.5), col);
                    } else {
                        // An arrow to the right: the issue goes on to its next column.
                        let (c, s) = (r.center(), Stroke::new(1.6_f32, col));
                        p.line_segment([c + vec2(-4.0, 0.0), c + vec2(4.0, 0.0)], s);
                        p.add(egui::Shape::line(vec![c + vec2(0.5, -3.5), c + vec2(4.0, 0.0), c + vec2(0.5, 3.5)], s));
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let head = if o.moved.is_some() { format!("Moved #{} to {}", o.number, o.to) } else { format!("Move #{} to {}?", o.number, o.to) };
                        ui.label(RichText::new(head).color(k.strong));
                        let (rect, _) = ui.allocate_exact_size(vec2(300.0, 16.0), Sense::hover());
                        crate::ui::fit_text(ui.painter(), rect.left_center(), Align2::LEFT_CENTER, &o.title, theme::secondary(), k.dim, rect.width());
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if o.moved.is_some() {
                                if crate::modal::button(ui, "Undo", crate::modal::Kind::Plain, true).clicked() {
                                    cmds.push(Cmd::IssueUndo);
                                }
                            } else {
                                if crate::modal::button(ui, &format!("Move to {}", o.to), crate::modal::Kind::Primary, true).clicked() {
                                    cmds.push(Cmd::IssueMove);
                                }
                                if crate::modal::button(ui, "Not now", crate::modal::Kind::Plain, true).clicked() {
                                    cmds.push(Cmd::IssueDecline);
                                }
                            }
                        });
                    });
                });
            });
            if let Some(left) = left {
                let rect = r.response.rect;
                let track = Rect::from_min_max(pos2(rect.left() + 12.0, rect.bottom() - 6.0), pos2(rect.right() - 12.0, rect.bottom() - 4.0));
                let p = ui.painter();
                p.rect_filled(track, CornerRadius::same(1), k.border);
                p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * left.clamp(0.0, 1.0), track.height())), CornerRadius::same(1), k.accent);
            }
        });
    }

    /// FOURCHETTE_ISSUE=<n>[,progress|review|moved]: the sample project on a branch of issue #n,
    /// with the offer to move it, or the confirmation of the move.
    pub(super) fn issue_sample(&mut self, v: &str) {
        let (n, step) = v.split_once(',').unwrap_or((v, ""));
        let Ok(n) = n.trim().trim_start_matches('#').parse::<u64>() else { return };
        self.branch = Some(format!("feat/{n}-linked-issue"));
        let Some(board) = self.project.board.as_deref() else { return };
        let Some(it) = board.items.iter().find(|i| i.number == Some(n)) else { return };
        let order = board.order();
        let to = match step.trim() {
            "review" => offer(&order, &it.status, true, false),
            "progress" | "moved" => offer(&order, &it.status, false, true),
            _ => None,
        };
        self.issue.offer = to.map(|to| Offer { item: it.id.clone(), number: n, title: it.title.clone(), to, from: it.status.clone(), moved: (step.trim() == "moved").then(Instant::now) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Vec<String> {
        ["Backlog", "Ready", "In progress", "In review", "To test", "Done"].map(String::from).to_vec()
    }

    #[test]
    fn the_branch_names_its_issue_by_its_first_number() {
        let b = github::sample();
        let n = |branch: &str| linked_issue(&b, branch).and_then(|i| i.number);
        assert_eq!(n("feat/52-linked-issue"), Some(52));
        assert_eq!(n("fix_46"), Some(46));
        assert_eq!(n("52"), Some(52));
        // The first number that is an issue: 999 is not one, 48 is.
        assert_eq!(n("feat/999-48-curves"), Some(48));
        // Inside a word, padded, or not on the project: nothing.
        assert_eq!(n("v52"), None);
        assert_eq!(n("feat/052-x"), None);
        assert_eq!(n("claude/35-branch-issue-64089a"), None);
        assert_eq!(n("develop"), None);
        assert_eq!(n(""), None);
    }

    #[test]
    fn only_issues_of_the_open_repository_count() {
        let mut b = github::sample();
        b.items.retain(|i| i.number == Some(52));
        assert!(linked_issue(&b, "feat/52-x").is_some());
        b.items[0].kind = Kind::PullRequest;
        assert!(linked_issue(&b, "feat/52-x").is_none(), "a pull request is not the branch's issue");
        b.items[0].kind = Kind::Issue;
        b.items[0].repo = "someone/else".into();
        assert!(linked_issue(&b, "feat/52-x").is_none(), "an issue of another repository");
    }

    #[test]
    fn a_checkout_offers_in_progress_only_from_before_it() {
        let o = order();
        for from in ["Backlog", "Ready", "No status", ""] {
            assert_eq!(offer(&o, from, false, true).as_deref(), Some("In progress"), "{from}");
        }
        for from in ["In progress", "In review", "To test", "Done"] {
            assert_eq!(offer(&o, from, false, true), None, "never back from {from}");
        }
        // Without a checkout (the app just opened on the branch), nothing.
        assert_eq!(offer(&o, "Ready", false, false), None);
    }

    #[test]
    fn a_pull_request_offers_in_review_with_or_without_a_checkout() {
        let o = order();
        for from in ["Backlog", "Ready", "In progress"] {
            assert_eq!(offer(&o, from, true, false).as_deref(), Some("In review"), "{from}");
            assert_eq!(offer(&o, from, true, true).as_deref(), Some("In review"), "{from}");
        }
        for from in ["In review", "To test", "Done"] {
            assert_eq!(offer(&o, from, true, true), None, "{from}");
        }
    }

    #[test]
    fn a_project_without_those_statuses_offers_nothing_it_lacks() {
        let o: Vec<String> = ["Todo", "Doing", "Done"].map(String::from).to_vec();
        assert_eq!(offer(&o, "Todo", true, true), None);
        let o: Vec<String> = ["Todo", "In Progress", "Done"].map(String::from).to_vec();
        // No review column: a checkout still offers In Progress, matched in any case.
        assert_eq!(offer(&o, "Todo", true, true).as_deref(), Some("In Progress"));
        // A finished issue never moves, even when its status is not in the list.
        assert_eq!(offer(&o, "Closed", false, true), None);
    }
}
