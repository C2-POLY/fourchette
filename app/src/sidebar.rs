//! The sidebar's Local and Remote sections (#120): the pure parts (short remote addresses,
//! branch folders, the remote a branch tracks) and the painted rows and icons.

use crate::git::{GitRef, RefKind};
use crate::theme;
use crate::pulls::Pull;
use crate::ui::{check_mark, chevron, fit_galley, fit_text, pr_badge, pr_badge_width};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui};

/// `owner/repo` for a GitHub or GitLab-style URL (https, ssh or scp form), else the host, else
/// the text as it is (a local path).
pub fn short_url(url: &str) -> String {
    let u = url.trim();
    let rest = match u.split_once("://") {
        Some((_, r)) => r.to_string(),
        None => match u.split_once('@') {
            // scp form: git@github.com:owner/repo.git
            Some((_, r)) if r.contains(':') => r.replacen(':', "/", 1),
            _ => u.to_string(),
        },
    };
    let rest = rest.rsplit_once('@').map(|x| x.1.to_string()).unwrap_or(rest);
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let host = parts.next().unwrap_or("");
    let path: Vec<&str> = parts.filter(|s| !s.is_empty()).collect();
    if !u.contains("://") && !u.contains('@') {
        return u.to_string();
    }
    let host_name = host.split(':').next().unwrap_or(host);
    match path.len() {
        0 => host_name.to_string(),
        _ if host_name.contains("github") || host_name.contains("gitlab") => path.join("/"),
        _ => host_name.to_string(),
    }
}

/// One entry of a branch list: a branch shown by its (rest of the) name, or a folder.
#[derive(Debug, PartialEq)]
pub enum Entry<'a, T> {
    Item(&'a T),
    Folder(String, Vec<&'a T>),
}

/// Groups `items` by the part of their name before the first "/": the ones without a prefix
/// first, then the folders, each in the order they come in.
pub fn group_folders<'a, T>(items: &'a [T], name: impl Fn(&T) -> &str) -> Vec<Entry<'a, T>> {
    let mut plain = Vec::new();
    let mut folders: Vec<(String, Vec<&T>)> = Vec::new();
    for it in items {
        match name(it).split_once('/') {
            Some((dir, _)) => match folders.iter_mut().find(|f| f.0 == dir) {
                Some(f) => f.1.push(it),
                None => folders.push((dir.to_string(), vec![it])),
            },
            None => plain.push(Entry::Item(it)),
        }
    }
    plain.extend(folders.into_iter().map(|(d, v)| Entry::Folder(d, v)));
    plain
}

/// Is a local branch tracked by a remote branch? (`origin/x` is tracked when a local branch's upstream is it.)
pub fn is_tracked(remote_ref: &str, locals: &[GitRef]) -> bool {
    locals.iter().any(|l| l.kind == RefKind::Local && l.upstream == remote_ref)
}

// ------------------------------------------------------------------ icons

fn stroke(col: Color32) -> Stroke {
    Stroke::new(1.3_f32, col)
}

/// A laptop: the screen over a base.
pub fn laptop_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    p.rect_stroke(Rect::from_center_size(c + vec2(0.0, -1.0), vec2(10.0, 7.0)), CornerRadius::same(1), s, StrokeKind::Middle);
    p.line_segment([c + vec2(-7.0, 5.0), c + vec2(7.0, 5.0)], s);
}

/// A cloud: three bumps on a flat base, outlined without the arcs that fall inside the others.
pub fn cloud_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    let bumps = [(vec2(-3.2, 0.8), 3.0_f32), (vec2(0.6, -1.6), 3.8), (vec2(4.0, 1.2), 2.6)];
    for (i, (o, r)) in bumps.iter().enumerate() {
        let mut run: Vec<Pos2> = Vec::new();
        for k in 0..=24 {
            let a = std::f32::consts::TAU * k as f32 / 24.0;
            let pt = *o + vec2(a.cos(), a.sin()) * *r;
            let inside = bumps.iter().enumerate().any(|(j, (o2, r2))| j != i && (pt - *o2).length() < r2 - 0.3);
            if pt.y > 3.9 || inside {
                if run.len() > 1 {
                    p.add(Shape::line(std::mem::take(&mut run).into_iter().map(|q| c + q.to_vec2()).collect(), s));
                }
                run.clear();
            } else {
                run.push(pt.to_pos2());
            }
        }
        if run.len() > 1 {
            p.add(Shape::line(run.into_iter().map(|q| c + q.to_vec2()).collect(), s));
        }
    }
    p.line_segment([c + vec2(-3.2, 3.8), c + vec2(4.0, 3.8)], s);
}

/// A folder: a body with a tab.
pub fn folder_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    p.add(Shape::closed_line(vec![c + vec2(-6.0, -4.0), c + vec2(-2.0, -4.0), c + vec2(-0.5, -2.5), c + vec2(6.0, -2.5), c + vec2(6.0, 4.5), c + vec2(-6.0, 4.5)], s));
}

/// A repository: a tall card with a bookmark line.
pub fn repo_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 12.0)), CornerRadius::same(2), s, StrokeKind::Middle);
    p.line_segment([c + vec2(-5.0, 2.5), c + vec2(5.0, 2.5)], s);
    p.line_segment([c + vec2(-2.0, -2.0), c + vec2(2.0, -2.0)], s);
}

/// A git branch: two dots on a line and a third one that merges into it.
pub fn branch_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = Stroke::new(1.2_f32, col);
    p.line_segment([c + vec2(-2.5, -2.4), c + vec2(-2.5, 2.4)], s);
    for o in [vec2(-2.5, -4.2), vec2(-2.5, 4.2), vec2(3.0, -2.6)] {
        p.circle_stroke(c + o, 1.7, s);
    }
    p.add(Shape::line(vec![c + vec2(3.0, -0.9), c + vec2(3.0, 0.2), c + vec2(1.8, 1.5), c + vec2(-2.5, 2.4)], s));
}

/// A price tag outline: the shape of the graph's tag pills (`ui::tag_icon`), not filled.
pub fn tag_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    p.add(Shape::closed_line(vec![c + vec2(-5.3, -4.0), c + vec2(1.6, -4.0), c + vec2(5.6, 0.0), c + vec2(1.6, 4.0), c + vec2(-5.3, 4.0)], s));
    p.circle_stroke(c + vec2(-2.4, 0.0), 1.1, Stroke::new(1.1_f32, col));
}

/// A worktree: two stacked windows, one behind the other.
pub fn worktree_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    p.add(Shape::line(vec![c + vec2(-3.0, -4.5), c + vec2(5.5, -4.5), c + vec2(5.5, 2.5)], s));
    p.rect_stroke(Rect::from_min_max(c + vec2(-5.5, -2.5), c + vec2(3.0, 5.0)), CornerRadius::same(1), s, StrokeKind::Middle);
}

/// A stash: an open tray with a thing set down into it.
pub fn stash_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    p.add(Shape::line(vec![c + vec2(-6.0, 0.0), c + vec2(-6.0, 5.0), c + vec2(6.0, 5.0), c + vec2(6.0, 0.0)], s));
    p.line_segment([c + vec2(0.0, -5.0), c + vec2(0.0, 2.0)], s);
    p.add(Shape::line(vec![c + vec2(-3.0, -1.0), c + vec2(0.0, 2.0), c + vec2(3.0, -1.0)], s));
}

/// A warning triangle with its exclamation mark.
pub fn warning_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = Stroke::new(1.2_f32, col);
    p.add(Shape::closed_line(vec![c + vec2(0.0, -5.0), c + vec2(5.5, 4.0), c + vec2(-5.5, 4.0)], s));
    p.line_segment([c + vec2(0.0, -1.5), c + vec2(0.0, 1.0)], s);
    p.circle_filled(c + vec2(0.0, 2.6), 0.7, col);
}

/// A small arrow, up or down, with its stem.
pub fn arrow(p: &Painter, c: Pos2, up: bool, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    let d = if up { -1.0 } else { 1.0 };
    p.line_segment([c + vec2(0.0, -3.5 * d), c + vec2(0.0, 3.5 * d)], s);
    p.add(Shape::line(vec![c + vec2(-2.5, 1.0 * d), c + vec2(0.0, 3.5 * d), c + vec2(2.5, 1.0 * d)], s));
}

// ------------------------------------------------------------------ rows

const ROW_H: f32 = 26.0;

fn row_rect(ui: &mut Ui, indent: f32) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    (Rect::from_min_max(rect.min + vec2(indent, 0.0), rect.max), resp)
}

/// A folder of a branch list: chevron, folder icon, name, count.
pub fn folder_row(ui: &mut Ui, name: &str, count: usize, open: bool, indent: f32) -> egui::Response {
    let (rect, resp) = row_rect(ui, indent);
    let p = ui.painter();
    let y = rect.center().y;
    chevron(p, pos2(rect.left() + 14.0, y), open, theme::c().faint);
    folder_icon(p, pos2(rect.left() + 31.0, y), theme::c().dim);
    fit_text(p, pos2(rect.left() + 44.0, y), Align2::LEFT_CENTER, name, theme::body(), theme::c().text, rect.width() - 80.0);
    p.text(pos2(rect.right() - 10.0, y), Align2::RIGHT_CENTER, count.to_string(), theme::small(), theme::c().faint);
    resp
}

/// Where a local branch's upstream stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upstream {
    /// No upstream: the branch exists only on this Mac.
    None,
    /// It tracks a remote branch that exists.
    Tracked,
    /// It tracks a remote branch that no longer exists.
    Gone,
}

/// The small sign a row shows at its right edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Cloud,
    Warning,
}

impl Upstream {
    /// "Local only" shows nothing: the others get a cloud or a warning triangle.
    pub fn sign(self) -> Option<Sign> {
        match self {
            Upstream::None => None,
            Upstream::Tracked => Some(Sign::Cloud),
            Upstream::Gone => Some(Sign::Warning),
        }
    }
}

/// The state of a branch's `upstream` (empty for none), given whether that remote branch exists.
pub fn upstream_state(upstream: &str, exists: bool) -> Upstream {
    match (upstream.is_empty(), exists) {
        (true, _) => Upstream::None,
        (false, true) => Upstream::Tracked,
        (false, false) => Upstream::Gone,
    }
}

/// Whether a section is worth showing: not when there is nothing to list in it.
pub fn section_shown(total: usize) -> bool {
    total > 0
}

/// The sections folded when a repository opens: the tags, which are many and rarely needed.
pub fn initial_collapsed() -> std::collections::HashSet<String> {
    ["Tags".to_string()].into_iter().collect()
}

/// What a local branch row shows on its right.
pub struct Tracking {
    pub upstream: Upstream,
    pub ahead: usize,
    pub behind: usize,
}

/// Where the rows of a section start, from the row's left edge: under the header's title.
pub const ITEM_INDENT: f32 = 16.0;

/// Draws the signs from the right edge `x`; returns the left edge they reach. The ↑↓ counts
/// stay; a cloud says the upstream exists, a triangle that it is gone; "local only" shows nothing.
fn tracking_badges(p: &Painter, t: &Tracking, right: f32, y: f32) -> f32 {
    let c = theme::c();
    let mut x = right;
    match t.upstream.sign() {
        Some(Sign::Cloud) => {
            cloud_icon(p, pos2(x - 7.0, y), c.dim);
            x -= 18.0;
        }
        Some(Sign::Warning) => {
            warning_icon(p, pos2(x - 6.0, y), c.modified);
            x -= 18.0;
        }
        None => {}
    }
    if t.upstream != Upstream::Gone {
        for (n, up, col) in [(t.behind, false, c.modified), (t.ahead, true, c.added)] {
            if n > 0 {
                let g = p.layout_no_wrap(n.to_string(), theme::small(), col);
                x -= g.size().x;
                p.galley(pos2(x, y - g.size().y / 2.0), g, col);
                x -= 6.0;
                arrow(p, pos2(x, y), up, col);
                x -= 8.0;
            }
        }
    }
    x
}

/// A local branch: a grey branch icon (a green check mark for the current one, whose name is
/// stronger), name, its pull request badge, then ahead/behind and the upstream's sign on the right.
pub fn local_branch_row(ui: &mut Ui, name: &str, current: bool, indent: f32, t: &Tracking, pr: Option<&Pull>) -> (egui::Response, Option<Rect>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let p = ui.painter();
    let c = theme::c();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), c.hover);
    }
    let y = rect.center().y;
    let x = rect.left() + 10.0 + indent;
    if current {
        check_mark(p, pos2(x + 5.0, y), c.added);
    } else {
        branch_icon(p, pos2(x + 5.0, y), c.faint);
    }
    let name_left = x + 17.0;
    let left = pr_after(p, pr, tracking_badges(p, t, rect.right() - 8.0, y), y);
    let (tc, font) = if current { (c.strong, theme::body_medium()) } else { (c.text, theme::body()) };
    fit_text(p, pos2(name_left, y), Align2::LEFT_CENTER, name, font, tc, left - 8.0 - name_left);
    (resp, pr.map(|q| badge_rect(rect, left, q)))
}

/// The hit area of a row's pull request badge drawn at `left`: a little wider than the badge.
fn badge_rect(row: Rect, left: f32, pr: &Pull) -> Rect {
    Rect::from_min_max(pos2(left - 3.0, row.top()), pos2(left + pr_badge_width(pr) + 3.0, row.bottom()))
}

/// Draws a pull request badge ending before `right` and returns where the name must stop.
fn pr_after(p: &Painter, pr: Option<&Pull>, right: f32, y: f32) -> f32 {
    let Some(pr) = pr else { return right };
    let left = right - 8.0 - pr_badge_width(pr);
    pr_badge(p, left, y, pr);
    left
}

/// A remote: chevron, repo icon, name in medium weight, short address on the right.
pub fn remote_row(ui: &mut Ui, name: &str, short: &str, open: bool, indent: f32) -> egui::Response {
    let (rect, resp) = row_rect(ui, indent);
    let p = ui.painter();
    let c = theme::c();
    let y = rect.center().y;
    chevron(p, pos2(rect.left() + 14.0, y), open, c.faint);
    repo_icon(p, pos2(rect.left() + 31.0, y), c.dim);
    let right = rect.right() - 10.0;
    let mut left = right;
    if !short.is_empty() {
        let g = fit_galley(p, short, theme::small(), c.faint, (rect.width() * 0.5).max(40.0));
        left = right - g.size().x;
        p.galley(pos2(left, y - g.size().y / 2.0), g, c.faint);
    }
    fit_text(p, pos2(rect.left() + 44.0, y), Align2::LEFT_CENTER, name, theme::body(), c.strong, left - 10.0 - (rect.left() + 44.0));
    resp
}

/// A remote branch: grey branch icon, name, its pull request badge.
pub fn remote_branch_row(ui: &mut Ui, name: &str, indent: f32, pr: Option<&Pull>) -> (egui::Response, Option<Rect>) {
    let (rect, resp) = row_rect(ui, 0.0);
    let p = ui.painter();
    let c = theme::c();
    let y = rect.center().y;
    let x = rect.left() + 10.0 + indent;
    let right = pr_after(p, pr, rect.right() - 10.0, y);
    branch_icon(p, pos2(x + 5.0, y), c.faint);
    fit_text(p, pos2(x + 17.0, y), Align2::LEFT_CENTER, name, theme::body(), c.text, right - 8.0 - (x + 17.0));
    (resp, pr.map(|q| badge_rect(rect, right, q)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_urls() {
        assert_eq!(short_url("https://github.com/C2-POLY/fourchette.git"), "C2-POLY/fourchette");
        assert_eq!(short_url("https://github.com/C2-POLY/fourchette"), "C2-POLY/fourchette");
        assert_eq!(short_url("git@github.com:C2-POLY/fourchette.git"), "C2-POLY/fourchette");
        assert_eq!(short_url("ssh://git@gitlab.com/group/sub/proj.git"), "group/sub/proj");
        assert_eq!(short_url("https://user:tok@github.com/a/b.git"), "a/b");
        assert_eq!(short_url("https://git.example.com/team/app.git"), "git.example.com");
        assert_eq!(short_url("git@example.org:team/app.git"), "example.org");
        assert_eq!(short_url("/Users/me/repo.git"), "/Users/me/repo.git");
        assert_eq!(short_url(""), "");
    }

    #[test]
    fn folders() {
        let names = ["main", "feature/a", "develop", "hotfix/x", "feature/b"];
        let g = group_folders(&names, |s| s);
        assert_eq!(g, vec![Entry::Item(&names[0]), Entry::Item(&names[2]), Entry::Folder("feature".into(), vec![&names[1], &names[4]]), Entry::Folder("hotfix".into(), vec![&names[3]])]);
    }

    #[test]
    fn upstream_states() {
        assert_eq!(upstream_state("", false), Upstream::None);
        assert_eq!(upstream_state("", true), Upstream::None);
        assert_eq!(upstream_state("origin/a", true), Upstream::Tracked);
        assert_eq!(upstream_state("origin/a", false), Upstream::Gone);
    }

    #[test]
    fn signs_by_upstream() {
        assert_eq!(Upstream::None.sign(), None);
        assert_eq!(Upstream::Tracked.sign(), Some(Sign::Cloud));
        assert_eq!(Upstream::Gone.sign(), Some(Sign::Warning));
    }

    #[test]
    fn sections_shown_only_with_something_to_list() {
        assert!(section_shown(1));
        assert!(!section_shown(0));
    }

    #[test]
    fn tags_start_folded() {
        assert!(initial_collapsed().contains("Tags"));
        assert!(!initial_collapsed().contains("Local"));
    }

    #[test]
    fn tracked() {
        let mk = |name: &str, kind, up: &str| GitRef { name: name.into(), full: name.into(), kind, target: String::new(), is_head: false, upstream: up.into(), ahead: 0, behind: 0 };
        let refs = vec![mk("a", RefKind::Local, "origin/a"), mk("origin/a", RefKind::Remote, ""), mk("origin/b", RefKind::Remote, "")];
        assert!(is_tracked("origin/a", &refs));
        assert!(!is_tracked("origin/b", &refs));
    }
}
