//! The sidebar's Local and Remote sections (#120): the pure parts (short remote addresses,
//! branch folders, the remote a branch tracks) and the painted rows and icons.

use crate::git::{GitRef, RefKind};
use crate::theme;
use crate::pulls::Pull;
use crate::ui::{chevron, fit_galley, fit_text, pr_badge, pr_badge_width};
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

/// The remote an upstream such as "origin/develop" belongs to, among the repository's `remotes`
/// (the longest match wins, since a remote name may contain "/").
pub fn upstream_remote<'a>(upstream: &str, remotes: &'a [String]) -> Option<&'a str> {
    remotes.iter().filter(|r| upstream.strip_prefix(r.as_str()).is_some_and(|rest| rest.starts_with('/'))).max_by_key(|r| r.len()).map(|r| r.as_str())
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

/// Two chain links.
pub fn link_icon(p: &Painter, c: Pos2, col: Color32) {
    let s = stroke(col);
    p.rect_stroke(Rect::from_center_size(c + vec2(-2.5, 0.0), vec2(6.5, 4.5)), CornerRadius::same(2), s, StrokeKind::Middle);
    p.rect_stroke(Rect::from_center_size(c + vec2(2.5, 0.0), vec2(6.5, 4.5)), CornerRadius::same(2), s, StrokeKind::Middle);
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

/// What a local branch row shows on its right.
pub struct Tracking<'a> {
    pub remote: Option<&'a str>,
    pub gone: bool,
    pub ahead: usize,
    pub behind: usize,
}

/// How much of the tracking a local branch row shows, from the fullest to the most compact.
/// A row takes the fullest one that leaves its name readable (#127).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// The pill with the cloud and the remote's name, and `=` when level with it.
    Full,
    /// The pill with the cloud only, and `=`; a laptop for "local only".
    Icon,
    /// The bare cloud only: `=` is left to the tooltip. Ahead and behind counts always stay.
    Tight,
}

/// The width a name needs before the row shrinks its tracking: the whole name when short, else
/// enough for about fifteen letters.
const NAME_MIN: f32 = 120.0;

/// The fullest `Fit` whose tracking width (given by `width`) leaves `need` for the name in
/// `room`, else the most compact.
pub fn choose_fit(room: f32, need: f32, width: impl Fn(Fit) -> f32) -> Fit {
    [Fit::Full, Fit::Icon].into_iter().find(|&f| room - width(f) >= need).unwrap_or(Fit::Tight)
}

/// Draws the badges from the right edge `x`; returns the left edge they reach.
fn tracking_badges(p: &Painter, t: &Tracking, fit: Fit, right: f32, y: f32) -> f32 {
    let c = theme::c();
    let mut x = right;
    match t.remote {
        // Short of room, "local only" becomes the Local section's laptop.
        None if fit != Fit::Full => {
            laptop_icon(p, pos2(x - 7.0, y), c.faint);
            x -= 14.0;
        }
        None => {
            let g = p.layout_no_wrap("local only".into(), egui::FontId::proportional(11.0), c.faint);
            x -= g.size().x;
            // Italic look: the font has no italic, so a slanted underline-free faint text it is.
            p.galley(pos2(x, y - g.size().y / 2.0), g, c.faint);
        }
        Some(name) => {
            let col = if t.gone { c.faint } else { c.dim };
            let g = (fit == Fit::Full).then(|| p.layout_no_wrap(name.to_string(), theme::small(), col));
            // Tight: the bare cloud, without the pill around it.
            let w = g.as_ref().map_or(if fit == Fit::Tight { 14.0 } else { 24.0 }, |g| g.size().x + 32.0);
            let r = Rect::from_min_size(pos2(x - w, y - 9.0), vec2(w, 18.0));
            if fit != Fit::Tight {
                p.rect(r, CornerRadius::same(9), theme::tint(c.accent, c.sidebar, 0.1), Stroke::new(1.0_f32, c.border), StrokeKind::Inside);
            }
            cloud_icon(p, pos2(r.center().x.max(r.left() + 7.0).min(r.left() + 12.0), y), col);
            match g {
                Some(g) => {
                    let ty = y - g.size().y / 2.0;
                    let tx = r.left() + 23.0;
                    let right_edge = tx + g.size().x;
                    p.galley(pos2(tx, ty), g, col);
                    if t.gone {
                        p.line_segment([pos2(tx, y), pos2(right_edge, y)], Stroke::new(1.0_f32, c.faint));
                    }
                }
                // Gone, with the name hidden: the cloud is struck through.
                None if t.gone => {
                    p.line_segment([pos2(r.left() + 2.0, y), pos2(r.right() - 2.0, y)], Stroke::new(1.0_f32, c.faint));
                }
                None => {}
            }
            x = r.left() - 6.0;
            if !t.gone {
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
                if t.ahead == 0 && t.behind == 0 && fit != Fit::Tight {
                    // Level with its upstream: an equals sign.
                    let s = Stroke::new(1.4_f32, c.faint);
                    x -= 4.0;
                    p.line_segment([pos2(x - 3.5, y - 2.0), pos2(x + 3.5, y - 2.0)], s);
                    p.line_segment([pos2(x - 3.5, y + 2.0), pos2(x + 3.5, y + 2.0)], s);
                    x -= 6.0;
                }
            }
        }
    }
    x
}

/// A local branch: colour dot, name, its pull request badge, then ahead/behind and the tracked
/// remote on the right. When the name would be cut short, the tracking shrinks first (`Fit`).
#[allow(clippy::too_many_arguments)]
pub fn local_branch_row(ui: &mut Ui, name: &str, color: Color32, current: bool, indent: f32, t: &Tracking, pr: Option<&Pull>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let p = ui.painter();
    if current {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().select);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let y = rect.center().y;
    let x = rect.left() + 10.0 + indent;
    p.circle_filled(pos2(x + 3.0, y), 3.5, color);
    let (name_left, right) = (x + 15.0, rect.right() - 8.0);
    let room = pr_after(&ghost(p), pr, right, y) - 8.0 - name_left;
    let need = p.layout_no_wrap(name.to_string(), theme::body(), theme::c().text).size().x.min(NAME_MIN);
    let fit = choose_fit(room, need, |f| right - tracking_badges(&ghost(p), t, f, right, y));
    let left = pr_after(p, pr, tracking_badges(p, t, fit, right, y), y);
    let tc = if current { theme::c().strong } else { theme::c().text };
    fit_text(p, pos2(name_left, y), Align2::LEFT_CENTER, name, theme::body(), tc, left - 8.0 - name_left);
    resp
}

/// A copy of `p` that measures without drawing.
fn ghost(p: &Painter) -> Painter {
    let mut g = p.clone();
    g.set_invisible();
    g
}

/// Draws a pull request badge ending before `right` and returns where the name must stop.
fn pr_after(p: &Painter, pr: Option<&Pull>, right: f32, y: f32) -> f32 {
    let Some(pr) = pr else { return right };
    let left = right - 8.0 - pr_badge_width(pr);
    pr_badge(p, left, y, pr);
    left
}

/// A remote: chevron, repo icon, name in medium weight, short address on the right.
pub fn remote_row(ui: &mut Ui, name: &str, short: &str, open: bool) -> egui::Response {
    let (rect, resp) = row_rect(ui, 0.0);
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

/// A remote branch: dot, name, its pull request badge, then a link icon when a local branch
/// tracks it, else "not local".
pub fn remote_branch_row(ui: &mut Ui, name: &str, color: Color32, indent: f32, tracked: bool, pr: Option<&Pull>) -> egui::Response {
    let (rect, resp) = row_rect(ui, 0.0);
    let p = ui.painter();
    let c = theme::c();
    let y = rect.center().y;
    let x = rect.left() + 10.0 + indent;
    let mut right = rect.right() - 10.0;
    if tracked {
        link_icon(p, pos2(right - 6.0, y), c.dim);
        right -= 18.0;
    } else {
        let g = p.layout_no_wrap("not local".into(), egui::FontId::proportional(11.0), c.faint);
        right -= g.size().x;
        p.galley(pos2(right, y - g.size().y / 2.0), g, c.faint);
        right -= 6.0;
    }
    let right = pr_after(p, pr, right, y);
    let (dot, tc) = if tracked { (color, c.text) } else { (theme::tint(color, c.sidebar, 0.4), c.faint) };
    p.circle_filled(pos2(x + 3.0, y), 3.0, dot);
    fit_text(p, pos2(x + 15.0, y), Align2::LEFT_CENTER, name, theme::body(), tc, right - 8.0 - (x + 15.0));
    resp
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
    fn upstream_remotes() {
        let remotes = vec!["origin".to_string(), "up".to_string(), "up/stream".to_string()];
        assert_eq!(upstream_remote("origin/develop", &remotes), Some("origin"));
        assert_eq!(upstream_remote("up/stream/main", &remotes), Some("up/stream"));
        assert_eq!(upstream_remote("up/main", &remotes), Some("up"));
        assert_eq!(upstream_remote("origins/x", &remotes), None);
        assert_eq!(upstream_remote("", &remotes), None);
    }

    #[test]
    fn folders() {
        let names = ["main", "feature/a", "develop", "hotfix/x", "feature/b"];
        let g = group_folders(&names, |s| s);
        assert_eq!(g, vec![Entry::Item(&names[0]), Entry::Item(&names[2]), Entry::Folder("feature".into(), vec![&names[1], &names[4]]), Entry::Folder("hotfix".into(), vec![&names[3]])]);
    }

    #[test]
    fn fits() {
        // Full needs 80, Icon 40, Tight 20.
        let w = |f| match f {
            Fit::Full => 80.0,
            Fit::Icon => 40.0,
            Fit::Tight => 20.0,
        };
        assert_eq!(choose_fit(200.0, 96.0, w), Fit::Full);
        assert_eq!(choose_fit(140.0, 96.0, w), Fit::Icon);
        assert_eq!(choose_fit(120.0, 96.0, w), Fit::Tight);
        // A short name keeps the full tracking where a long one would not.
        assert_eq!(choose_fit(120.0, 30.0, w), Fit::Full);
    }

    #[test]
    fn tracked() {
        let mk = |name: &str, kind, up: &str| GitRef { name: name.into(), full: name.into(), kind, target: String::new(), is_head: false, upstream: up.into(), ahead: 0, behind: 0 };
        let refs = vec![mk("a", RefKind::Local, "origin/a"), mk("origin/a", RefKind::Remote, ""), mk("origin/b", RefKind::Remote, "")];
        assert!(is_tracked("origin/a", &refs));
        assert!(!is_tracked("origin/b", &refs));
    }
}
