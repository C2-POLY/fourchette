//! Small drawing helpers: graph lines, nodes, sidebar rows, toolbar buttons.
use crate::app::{short, Cmd, Dialog};
use crate::git::{self, FileChange, GitRef, RefKind};
use crate::graph::Row;
use crate::pulls::{Ci as PrCi, Pull, State as PrState};
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui};

pub fn lane_x(gx: f32, lane: usize) -> f32 {
    gx + theme::GRAPH_INSET + lane as f32 * theme::LANE_W
}

fn segment(p: &egui::Painter, a: Pos2, b: Pos2, col: Color32) {
    let stroke = Stroke::new(theme::LINE_W, col);
    if (a.x - b.x).abs() < 0.5 {
        p.line_segment([a, b], stroke);
    } else {
        // Smooth S-curve between lanes.
        let dy = (b.y - a.y) * 0.65;
        p.add(egui::epaint::CubicBezierShape::from_points_stroke(
            [a, pos2(a.x, a.y + dy), pos2(b.x, b.y - dy), b],
            false,
            Color32::TRANSPARENT,
            stroke,
        ));
    }
}

pub fn draw_row_graph(p: &egui::Painter, row: &Row, gx: f32, top: f32, bottom: f32, alpha: f32) {
    let mid = (top + bottom) * 0.5; // lines meet at the row middle, like the original
    for e in &row.top {
        segment(p, pos2(lane_x(gx, e.from), top), pos2(lane_x(gx, e.to), mid), theme::lane_color(e.color).gamma_multiply(alpha));
    }
    for e in &row.bottom {
        segment(p, pos2(lane_x(gx, e.from), mid), pos2(lane_x(gx, e.to), bottom), theme::lane_color(e.color).gamma_multiply(alpha));
    }
}

/// A commit in the graph: a small dot in its lane colour, a hollow ring for a merge,
/// and a ring around the checked-out commit. `bg` is the row's background.
pub fn draw_node(p: &egui::Painter, c: Pos2, row: &Row, is_head: bool, matched: bool, bg: Color32) {
    let col = theme::lane_color(row.color);
    let col = if matched { col } else { theme::tint(col, bg, 0.4) };
    if is_head {
        p.circle_filled(c, theme::NODE_R + 4.0, bg);
        p.circle_stroke(c, theme::NODE_R + 3.0, Stroke::new(1.5_f32, col));
    }
    if row.is_merge {
        p.circle_filled(c, theme::MERGE_R + 2.0, bg);
        p.circle_stroke(c, theme::MERGE_R, Stroke::new(1.75_f32, col));
        return;
    }
    p.circle_filled(c, theme::NODE_R + 2.0, bg);
    p.circle_filled(c, theme::NODE_R, col);
}

/// Fill, border and text colours of a branch or tag label drawn in its lane colour.
pub fn ref_colors(r: &GitRef, lane: Color32, bg: Color32) -> (Color32, Option<Color32>, Color32) {
    let text = theme::readable(lane, 0.35);
    match r.kind {
        // Discreet: an outline only, on the row's background.
        RefKind::Tag => (Color32::TRANSPARENT, Some(theme::c().border_strong), theme::c().dim),
        RefKind::Remote => (bg, Some(theme::tint(lane, bg, 0.4)), text),
        _ if r.is_head => (theme::tint(lane, bg, 0.3), Some(theme::tint(lane, bg, 0.6)), theme::readable(lane, 0.55)),
        _ => (theme::tint(lane, bg, 0.18), None, text),
    }
}

/// The width of a pull request badge: its icon, and its CI dot when it has checks.
pub fn pr_badge_width(pr: &Pull) -> f32 {
    if pr.ci.is_some() {
        21.0
    } else {
        11.0
    }
}

/// A pull request badge whose left edge is at `left` and centre line at `y`: the pull request
/// icon (two branches joined by an arrow) in the colour of its state, then a dot in the colour
/// of its checks, a ring while they run.
pub fn pr_badge(p: &egui::Painter, left: f32, y: f32, pr: &Pull) {
    let t = theme::c();
    let col = match pr.state {
        PrState::Draft => t.dim,
        PrState::Open => t.added,
        // The project's "blue": the accent is too close to the merged purple in some variants.
        PrState::Approved => theme::lane_color(0),
        PrState::ChangesRequested => theme::lane_color(3),
        PrState::Merged => t.renamed,
        PrState::Closed => t.deleted,
    };
    pr_icon(p, left, y, col, pr.state == PrState::Merged);
    let c = pos2(left + 17.5, y);
    match pr.ci {
        Some(PrCi::Success) => {
            p.circle_filled(c, 3.2, t.added);
        }
        Some(PrCi::Failure) => {
            p.circle_filled(c, 3.2, t.deleted);
        }
        Some(PrCi::Pending) => {
            p.circle_stroke(c, 2.7, Stroke::new(1.3_f32, t.modified));
            p.circle_filled(c, 1.1, t.modified);
        }
        None => {}
    }
}

/// The pull request icon, 11 pt wide from `left`: two branches joined by an arrow, or merged.
pub fn pr_icon(p: &egui::Painter, left: f32, y: f32, col: Color32, merged: bool) {
    let s = Stroke::new(1.3_f32, col);
    let (x0, x1, r) = (left + 1.8, left + 9.0, 1.6);
    let (top, bottom) = (y - 3.8, y + 3.8);
    p.circle_stroke(pos2(x0, top), r, s);
    p.circle_stroke(pos2(x0, bottom), r, s);
    p.line_segment([pos2(x0, top + r), pos2(x0, bottom - r)], s);
    p.circle_stroke(pos2(x1, bottom), r, s);
    if merged {
        // Merged: the right branch joins the left one.
        p.line_segment([pos2(x1, bottom - r), pos2(x1, y - 0.6)], s);
        p.line_segment([pos2(x1, y - 0.6), pos2(x0 + 0.6, top + r)], s);
    } else {
        // The arrow from the right branch back to the left one.
        let (ax, ay) = (x0 + 3.6, top);
        p.add(Shape::line(vec![pos2(x1, bottom - r), pos2(x1, ay), pos2(ax, ay)], s));
        p.add(Shape::line(vec![pos2(ax + 1.5, ay - 1.5), pos2(ax, ay), pos2(ax + 1.5, ay + 1.5)], s));
    }
}

/// Text on one line that ends with "…" when it is wider than `max_w`. Returns where it was drawn.
pub fn fit_text(p: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32, max_w: f32) -> Rect {
    let galley = fit_galley(p, text, font, color, max_w);
    let rect = align.anchor_size(pos, galley.size());
    p.galley(rect.min, galley, color);
    rect
}

/// The galley of one line of `text`, cut with "…" at its end when it is wider than `max_w`.
pub fn fit_galley(p: &egui::Painter, text: &str, font: FontId, color: Color32, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: max_w.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    p.layout_job(job)
}

/// One label of the history's branch column: a local branch with its remote branch when both
/// point at the commit, a branch only local or only remote, a tag, or a detached HEAD.
pub struct RefPill<'a> {
    /// The ref the pill is named after and acts on: the local branch when there is one.
    pub main: &'a GitRef,
    /// The remote branch merged into a local branch's pill.
    pub remote: Option<&'a GitRef>,
}

impl RefPill<'_> {
    pub fn text(&self) -> &str {
        if self.main.kind == RefKind::Head {
            "HEAD"
        } else {
            &self.main.name
        }
    }

    /// The width of the icons before the name: a check for the checked-out branch, a computer
    /// for a local branch, a cloud for a remote one, a tag for a tag.
    fn icons_w(&self) -> f32 {
        let n = match self.main.kind {
            RefKind::Local => 1 + self.main.is_head as usize + self.remote.is_some() as usize,
            RefKind::Remote | RefKind::Tag => 1,
            RefKind::Head | RefKind::Stash => 0,
        };
        // A little room between the last icon and the name.
        if n == 0 {
            0.0
        } else {
            n as f32 * PILL_ICON_W + 2.0
        }
    }

    /// The width of everything but the name: paddings, icons and `extra` (a badge).
    pub fn chrome_w(&self, extra: f32) -> f32 {
        PILL_PAD_L + self.icons_w() + PILL_PAD_R + extra
    }

    /// The width of the whole pill, its name in full.
    pub fn width(&self, p: &egui::Painter, extra: f32) -> f32 {
        p.layout_no_wrap(self.text().to_owned(), theme::pill(), Color32::WHITE).size().x + self.chrome_w(extra)
    }
}

const PILL_ICON_W: f32 = 12.0;
const PILL_PAD_L: f32 = 5.0;
const PILL_PAD_R: f32 = 7.0;

/// The remote branch of `local` among `refs`: its upstream, or without one the only
/// `<remote>/<name>` of the same name.
pub fn remote_partner(local: &GitRef, refs: &[GitRef]) -> Option<usize> {
    if !local.upstream.is_empty() {
        return refs.iter().position(|q| q.kind == RefKind::Remote && q.name == local.upstream);
    }
    let same_name = |q: &GitRef| q.kind == RefKind::Remote && q.full.strip_prefix("refs/remotes/").and_then(|s| s.split_once('/')).is_some_and(|(_, b)| b == local.name);
    let mut found = refs.iter().enumerate().filter(|(_, q)| same_name(q));
    match (found.next(), found.next()) {
        (Some((j, _)), None) => Some(j),
        _ => None,
    }
}

/// The pills of one commit's refs, in the order `git::refs_by_target` gives (checked-out branch
/// first, tags last): each local branch takes its remote branch on the same commit, and the other
/// remote branches keep a pill of their own with their remote's name.
pub fn ref_pills(refs: &[GitRef]) -> Vec<RefPill<'_>> {
    let mut partner = vec![None; refs.len()];
    let mut taken = vec![false; refs.len()];
    for (i, r) in refs.iter().enumerate() {
        if r.kind != RefKind::Local {
            continue;
        }
        if let Some(j) = remote_partner(r, refs).filter(|&j| !taken[j]) {
            partner[i] = Some(j);
            taken[j] = true;
        }
    }
    refs.iter().enumerate().filter(|(i, r)| !taken[*i] && r.kind != RefKind::Stash).map(|(i, r)| RefPill { main: r, remote: partner[i].map(|j| &refs[j]) }).collect()
}

/// A pill in `rect`: its icons, its name (ending with "…" when it does not fit) and the pull
/// request badge `pull` on its right. `lane` is its commit's lane colour, `bg` the colour behind it.
pub fn draw_ref_pill(p: &egui::Painter, rect: Rect, pill: &RefPill, lane: Color32, bg: Color32, pull: Option<&Pull>) {
    let (fill, stroke, text) = ref_colors(pill.main, lane, bg);
    p.rect_filled(rect, CornerRadius::same(4), fill);
    if let Some(s) = stroke {
        p.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0_f32, s), StrokeKind::Inside);
    }
    let y = rect.center().y;
    let behind = if fill == Color32::TRANSPARENT { bg } else { fill };
    let mut x = rect.left() + PILL_PAD_L;
    let mut icon = |draw: &dyn Fn(&egui::Painter, Pos2)| {
        draw(p, pos2(x + PILL_ICON_W / 2.0 - 0.5, y));
        x += PILL_ICON_W;
    };
    match pill.main.kind {
        RefKind::Local => {
            if pill.main.is_head {
                icon(&|p, c| head_check(p, c, text));
            }
            icon(&|p, c| computer_icon(p, c, text));
            if pill.remote.is_some() {
                icon(&|p, c| cloud_icon(p, c, text));
            }
        }
        RefKind::Remote => icon(&|p, c| cloud_icon(p, c, text)),
        RefKind::Tag => icon(&|p, c| tag_icon(p, c, text, behind)),
        RefKind::Head | RefKind::Stash => {}
    }
    if x > rect.left() + PILL_PAD_L {
        x += 2.0;
    }
    let bw = pull.map(|q| pr_badge_width(q) + 5.0).unwrap_or(0.0);
    let clip = p.with_clip_rect(rect.shrink(1.0));
    let galley = fit_galley(p, pill.text(), theme::pill(), text, rect.right() - PILL_PAD_R - bw - x);
    clip.galley(pos2(x, y - galley.size().y / 2.0), galley, text);
    if let Some(q) = pull {
        pr_badge(&clip, pill_badge_left(rect, q), y, q);
    }
}

/// Where the pull request badge of a pill in `rect` starts.
pub fn pill_badge_left(rect: Rect, pull: &Pull) -> f32 {
    rect.right() - PILL_PAD_R - pr_badge_width(pull)
}

/// The checked-out branch's check mark, smaller than `check_mark`.
fn head_check(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::line(vec![c + vec2(-3.2, 0.2), c + vec2(-1.0, 2.4), c + vec2(3.2, -2.4)], Stroke::new(1.5_f32, col)));
}

/// A computer (a screen on its stand), about 9 pt wide: a local branch.
pub fn computer_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    p.rect_filled(Rect::from_center_size(c + vec2(0.0, -1.3), vec2(9.0, 6.4)), CornerRadius::same(1), col);
    p.line_segment([c + vec2(0.0, 1.8), c + vec2(0.0, 3.6)], Stroke::new(1.3_f32, col));
    p.line_segment([c + vec2(-2.6, 4.0), c + vec2(2.6, 4.0)], Stroke::new(1.3_f32, col));
}

/// A cloud, about 10 pt wide: a remote branch.
pub fn cloud_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    p.circle_filled(c + vec2(-2.5, 1.0), 2.4, col);
    p.circle_filled(c + vec2(0.4, -0.9), 3.2, col);
    p.circle_filled(c + vec2(3.0, 1.2), 2.2, col);
    p.rect_filled(Rect::from_min_max(c + vec2(-2.5, 1.0), c + vec2(3.0, 3.4)), CornerRadius::ZERO, col);
}

/// A price tag with its hole, about 9 pt wide; `hole` is the colour behind it.
pub fn tag_icon(p: &egui::Painter, c: Pos2, col: Color32, hole: Color32) {
    let pts = [vec2(-4.3, -3.2), vec2(1.3, -3.2), vec2(4.5, 0.0), vec2(1.3, 3.2), vec2(-4.3, 3.2)];
    p.add(Shape::convex_polygon(pts.iter().map(|&v| c + v).collect(), col, Stroke::NONE));
    p.circle_filled(c + vec2(-2.0, 0.0), 1.1, hole);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gref(name: &str, kind: RefKind, is_head: bool) -> GitRef {
        GitRef { name: name.into(), full: name.into(), kind, target: "abc".into(), is_head, upstream: String::new(), ahead: 0, behind: 0 }
    }

    fn args(c: Option<Cmd>) -> Option<Vec<String>> {
        match c {
            Some(Cmd::Git { args, .. }) => Some(args),
            Some(_) => panic!("not a Git command"),
            None => None,
        }
    }

    /// The checkout a label's double-click asks for, as `checkout::plan` resolves it.
    fn checkout_args(c: Option<Cmd>, refs: &[GitRef]) -> Option<Vec<String>> {
        match c {
            Some(Cmd::Checkout { name, remote }) => match crate::checkout::plan(&name, remote, refs, &[]) {
                crate::checkout::Plan::Now { args, .. } => Some(args),
                crate::checkout::Plan::Ask(_) | crate::checkout::Plan::FastForward(_) => panic!("syncs"),
            },
            Some(_) => panic!("not a checkout"),
            None => None,
        }
    }

    #[test]
    fn double_click_on_a_label_checks_out_only_what_can_be_checked_out() {
        let refs = vec![gref("main", RefKind::Local, true), gref("dev", RefKind::Local, false), gref("origin/dev", RefKind::Remote, false), gref("origin/new", RefKind::Remote, false)];
        let args = |r: &GitRef| checkout_args(label_checkout(r), &refs);
        assert_eq!(args(&refs[1]), Some(vec!["checkout".into(), "dev".into()]));
        assert_eq!(args(&refs[0]), None);
        assert_eq!(args(&gref("v1", RefKind::Tag, false)), None);
        assert_eq!(args(&gref("HEAD", RefKind::Head, false)), None);
        assert_eq!(args(&gref("stash@{0}", RefKind::Stash, false)), None);
        assert_eq!(args(&refs[2]), Some(vec!["checkout".into(), "dev".into()]));
        assert_eq!(args(&refs[3]), Some(vec!["checkout".into(), "--track".into(), "origin/new".into()]));
    }

    fn tracking(name: &str, up: &str, ahead: usize, behind: usize, is_head: bool) -> GitRef {
        GitRef { upstream: up.into(), ahead, behind, full: format!("refs/heads/{name}"), ..gref(name, RefKind::Local, is_head) }
    }

    fn remote(name: &str) -> GitRef {
        GitRef { full: format!("refs/remotes/{name}"), ..gref(name, RefKind::Remote, false) }
    }

    fn wt(branch: &str, missing: bool) -> crate::worktree::Worktree {
        crate::worktree::Worktree { path: format!("/tmp/wt/{branch}").into(), branch: Some(branch.into()), head: String::new(), is_main: false, missing, locked: false }
    }

    /// Each pill as "<name>" with "+<remote>" when a remote branch is merged into it.
    fn pills(refs: &[GitRef]) -> Vec<String> {
        ref_pills(refs).iter().map(|p| p.remote.map_or(p.text().to_string(), |r| format!("{}+{}", p.text(), r.name))).collect()
    }

    #[test]
    fn a_local_branch_and_its_remote_branch_make_one_pill() {
        // Its upstream on the same commit joins it; another remote's branch keeps its own pill.
        let refs = git::refs_by_target(&[tracking("main", "origin/main", 0, 0, true), remote("origin/main"), remote("upstream/main"), gref("v1", RefKind::Tag, false)]);
        assert_eq!(pills(&refs["abc"]), ["main+origin/main", "upstream/main", "v1"]);
        // Without an upstream: the only remote branch of the same name.
        let refs = [tracking("dev", "", 0, 0, false), remote("origin/dev"), remote("origin/feature/dev")];
        assert_eq!(pills(&refs), ["dev+origin/dev", "origin/feature/dev"]);
        // Two of the same name, without an upstream: neither is chosen.
        let refs = [tracking("dev", "", 0, 0, false), remote("origin/dev"), remote("upstream/dev")];
        assert_eq!(pills(&refs), ["dev", "origin/dev", "upstream/dev"]);
        // Its upstream elsewhere: the remote branch of the same name stays apart.
        let refs = [tracking("dev", "origin/other", 0, 0, false), remote("origin/dev")];
        assert_eq!(pills(&refs), ["dev", "origin/dev"]);
        // A remote branch joins one local branch only.
        let refs = [tracking("a", "origin/main", 0, 0, false), tracking("b", "origin/main", 0, 0, false), remote("origin/main")];
        assert_eq!(pills(&refs), ["a+origin/main", "b"]);
    }

    #[test]
    fn double_click_on_a_merged_pill_checks_out_the_local_branch() {
        let refs = [tracking("dev", "origin/dev", 0, 0, false), remote("origin/dev"), remote("origin/new")];
        let ps = ref_pills(&refs);
        assert_eq!(checkout_args(label_checkout(ps[0].main), &refs), Some(vec!["checkout".into(), "dev".into()]));
        assert_eq!(checkout_args(label_checkout(ps[1].main), &refs), Some(vec!["checkout".into(), "--track".into(), "origin/new".into()]));
    }

    #[test]
    fn fast_forward_shows_only_when_strictly_behind() {
        let refs = vec![remote("origin/dev"), remote("origin/main")];
        // Behind: shown, with the count, moving the branch without checkout and offline.
        let ff = fast_forward(&tracking("dev", "origin/dev", 0, 3, false), &refs, &[]).expect("shown");
        assert_eq!((ff.text.as_str(), ff.count.as_str()), ("Fast-forward to 'origin/dev'", "3 commits"));
        let Ok(Cmd::Git { label, args: a, network }) = ff.action else { panic!("enabled") };
        assert_eq!(label, "Fast-forwarded dev");
        assert_eq!(a, ["fetch", "--no-write-fetch-head", "--no-tags", "--no-recurse-submodules", ".", "refs/remotes/origin/dev:refs/heads/dev"]);
        assert!(!network);
        assert_eq!(fast_forward(&tracking("dev", "origin/dev", 0, 1, false), &refs, &[]).unwrap().count, "1 commit");
        // The checked-out branch: merge --ff-only, which also updates the files.
        let ff = fast_forward(&tracking("main", "origin/main", 0, 2, true), &refs, &[wt("main", false)]).expect("shown");
        assert_eq!(args(ff.action.ok()), Some(vec!["merge".into(), "--ff-only".into(), "refs/remotes/origin/main".into()]));
        // Up to date, ahead, diverged, no upstream, upstream gone or unknown: hidden.
        for b in [
            tracking("dev", "origin/dev", 0, 0, false),
            tracking("dev", "origin/dev", 2, 0, false),
            tracking("dev", "origin/dev", 1, 2, false),
            tracking("dev", "", 0, 3, false),
            tracking("dev", "origin/gone", 0, 3, false),
            remote("origin/dev"),
        ] {
            assert!(fast_forward(&b, &refs, &[]).is_none(), "{} {}/{}", b.name, b.ahead, b.behind);
        }
    }

    #[test]
    fn fast_forward_is_greyed_out_for_a_branch_checked_out_in_another_worktree() {
        let refs = vec![remote("origin/dev")];
        let b = tracking("dev", "origin/dev", 0, 3, false);
        let ff = fast_forward(&b, &refs, &[wt("other", false), wt("dev", false)]).expect("shown");
        let why = ff.action.err().expect("greyed out");
        assert!(why.contains("checked out in the worktree “dev”"), "{why}");
        let why = fast_forward(&b, &refs, &[wt("dev", true)]).unwrap().action.err().expect("greyed out");
        assert!(why.contains("Clean up missing worktrees"), "{why}");
        // Checked out here: not "elsewhere", it fast-forwards with merge.
        let here = tracking("dev", "origin/dev", 0, 3, true);
        assert!(fast_forward(&here, &refs, &[wt("dev", false)]).unwrap().action.is_ok());
    }

    #[test]
    fn long_label_is_cut_with_an_ellipsis_within_its_width() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_pass(Default::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            let long = "feature/remember-the-last-opened-repository-per-window";
            let g = fit_galley(&p, long, theme::small(), Color32::WHITE, 80.0);
            assert!(g.size().x <= 80.0 + 0.5, "too wide: {}", g.size().x);
            let last = g.rows[0].row.glyphs.last().map(|gl| gl.chr);
            assert_eq!(last, Some('…'), "no ellipsis at the end");
            assert!(g.text().starts_with("feature/"));
            let short = fit_galley(&p, "main", theme::small(), Color32::WHITE, 80.0);
            assert_eq!(short.text(), "main");
        });
        // No renderer takes the font atlas: dropped on purpose.
        out.textures_delta.clear();
    }
}

/// A small chevron pointing right (closed) or down (open), drawn rather than typed.
pub fn chevron(p: &egui::Painter, c: Pos2, open: bool, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    let pts = if open { vec![c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)] } else { vec![c + vec2(-1.5, -3.5), c + vec2(2.0, 0.0), c + vec2(-1.5, 3.5)] };
    p.add(Shape::line(pts, s));
}

/// A drawn check mark (the font's own is uneven).
pub fn check_mark(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::line(vec![c + vec2(-4.0, 0.0), c + vec2(-1.2, 3.0), c + vec2(4.0, -3.0)], Stroke::new(1.6_f32, col)));
}

/// The status bar button that shows the appearance and switches to the next one.
pub fn appearance_button(ui: &mut Ui, a: theme::Appearance) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::c().hover);
    }
    let col = if resp.hovered() { theme::c().text } else { theme::c().dim };
    let bg = if resp.hovered() { theme::c().hover } else { theme::c().sidebar };
    appearance_icon(p, rect.center(), a, col, bg);
    resp
}

/// The icon of an appearance, about 14 pt wide; `bg` is the colour behind it (the moon is cut from it).
pub fn appearance_icon(p: &egui::Painter, c: Pos2, a: theme::Appearance, col: Color32, bg: Color32) {
    match a {
        // Half dark, half light: follows macOS.
        theme::Appearance::System => {
            p.circle_stroke(c, 5.5, Stroke::new(1.3_f32, col));
            let half: Vec<Pos2> = (0..=16).map(|k| c + egui::Vec2::angled(std::f32::consts::FRAC_PI_2 + k as f32 / 16.0 * std::f32::consts::PI) * 5.5).collect();
            p.add(Shape::convex_polygon(half, col, Stroke::NONE));
        }
        // A crescent moon.
        theme::Appearance::Dark => {
            p.circle_filled(c, 5.5, col);
            p.circle_filled(c + vec2(2.8, -2.3), 4.6, bg);
        }
        // A sun: a disc and eight rays.
        theme::Appearance::Light => {
            p.circle_filled(c, 2.8, col);
            for k in 0..8 {
                let d = egui::Vec2::angled(k as f32 * std::f32::consts::FRAC_PI_4);
                p.line_segment([c + d * 4.6, c + d * 6.6], Stroke::new(1.3_f32, col));
            }
        }
    }
}

/// The status bar button for the details panel: a window outline with its right or bottom
/// part filled where the panel sits, or empty when the panel is hidden.
pub fn details_button(ui: &mut Ui, l: crate::library::DetailsLayout) -> egui::Response {
    use crate::library::DetailsPlace;
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), theme::c().hover);
    }
    let col = if resp.hovered() { theme::c().text } else { theme::c().dim };
    let win = Rect::from_center_size(rect.center(), vec2(15.0, 12.0));
    if !l.hidden {
        let part = match l.place {
            DetailsPlace::Right => Rect::from_min_max(pos2(win.right() - 5.5, win.top()), win.max),
            DetailsPlace::Bottom => Rect::from_min_max(pos2(win.left(), win.bottom() - 4.5), win.max),
        };
        let r = 2;
        let corners = match l.place {
            DetailsPlace::Right => CornerRadius { nw: 0, ne: r, sw: 0, se: r },
            DetailsPlace::Bottom => CornerRadius { nw: 0, ne: 0, sw: r, se: r },
        };
        p.rect_filled(part, corners, col);
    }
    p.rect_stroke(win, CornerRadius::same(2), Stroke::new(1.3_f32, col), StrokeKind::Inside);
    resp
}

/// A round avatar with the author's initials.
pub fn avatar(p: &egui::Painter, c: Pos2, r: f32, name: &str) {
    let col = theme::author_color(name);
    let k = if theme::is_light() { 0.9 } else { 0.75 };
    p.circle_filled(c, r, theme::tint(col, theme::c().bg, k));
    p.text(c, Align2::CENTER_CENTER, theme::initials(name), FontId::proportional((r * 0.95).max(8.0)), theme::c().on_accent);
}

pub fn ref_label(r: &GitRef) -> String {
    match r.kind {
        RefKind::Head => "HEAD".into(),
        RefKind::Local if r.is_head => format!("✔ {}", r.name),
        RefKind::Local => r.name.clone(),
        RefKind::Remote => format!("☁ {}", r.name),
        RefKind::Tag => format!("🏷 {}", r.name),
        RefKind::Stash => r.name.clone(),
    }
}

/// A sidebar row of the Tags, Worktrees or Stashes sections (`kind` "tag", "worktree" or
/// "stash"): a drawn icon, then the text, lined up with the branch rows.
pub fn side_item(ui: &mut Ui, kind: &str, text: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    let c = theme::c();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), c.hover);
    }
    let y = rect.center().y;
    let x = rect.left() + 10.0 + crate::sidebar::ITEM_INDENT;
    let at = pos2(x + 5.0, y);
    match kind {
        "tag" => tag_icon(p, at, c.faint, if resp.hovered() { c.hover } else { c.sidebar }),
        "worktree" => crate::sidebar::worktree_icon(p, at, c.faint),
        _ => crate::sidebar::stash_icon(p, at, c.faint),
    }
    fit_text(p, pos2(x + 17.0, y), Align2::LEFT_CENTER, text, theme::body(), c.text, rect.right() - 8.0 - (x + 17.0));
    resp
}

/// A sidebar row whose text stops `reserve` points earlier on the right, for a badge drawn
/// there (`side_trailing` gives where).
#[allow(clippy::too_many_arguments)]
/// Placeholder text of a field, in the tertiary colour.
pub fn faint_hint(text: impl Into<String>) -> RichText {
    RichText::new(text).color(theme::c().faint)
}

/// A section's header: its chevron (always shown), its drawn icon and its title.
/// The title starts where `sidebar::ITEM_INDENT` puts the rows' icons.
pub fn section_header(ui: &mut Ui, title: &str, open: bool, icon: fn(&egui::Painter, Pos2, Color32)) -> egui::Response {
    ui.add_space(6.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter();
    let col = if resp.hovered() { theme::c().text } else { theme::c().dim };
    let y = rect.center().y;
    chevron(p, pos2(rect.left() + 14.0, y), open, theme::c().faint);
    icon(p, pos2(rect.left() + 31.0, y), col);
    p.text(pos2(rect.left() + 44.0, y), Align2::LEFT_CENTER, title, theme::secondary(), col);
    resp
}

pub fn toolbar_pill(ui: &mut Ui, caption: &str, value: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(value.to_string(), theme::body(), theme::c().strong);
    let w = (galley.size().x + 40.0).clamp(110.0, 260.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    p.text(pos2(rect.left() + 10.0, rect.top() + 11.0), Align2::LEFT_CENTER, caption, theme::small(), theme::c().faint);
    fit_text(p, pos2(rect.left() + 10.0, rect.top() + 27.0), Align2::LEFT_CENTER, value, theme::body(), theme::c().strong, rect.width() - 34.0);
    chevron(p, pos2(rect.right() - 13.0, rect.top() + 27.0), true, theme::c().dim);
    resp
}

/// The toolbar's "Update available" capsule: an arrow into a tray and the text, in the accent.
pub fn update_pill(ui: &mut Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), theme::secondary(), theme::c().accent_text);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 40.0, 26.0), Sense::click());
    let p = ui.painter();
    let c = theme::c();
    let fill = if resp.hovered() { theme::tint(c.accent, c.sidebar, 0.32) } else { theme::tint(c.accent, c.sidebar, 0.18) };
    p.rect(rect, CornerRadius::same(13), fill, Stroke::new(1.0_f32, theme::tint(c.accent, c.sidebar, 0.55)), StrokeKind::Inside);
    // An arrow down onto a tray.
    let ic = pos2(rect.left() + 17.0, rect.center().y);
    let s = Stroke::new(1.5_f32, c.accent_text);
    p.line_segment([ic + vec2(0.0, -5.5), ic + vec2(0.0, 2.0)], s);
    p.add(Shape::line(vec![ic + vec2(-3.5, -1.5), ic + vec2(0.0, 2.0), ic + vec2(3.5, -1.5)], s));
    p.add(Shape::line(vec![ic + vec2(-5.5, 2.5), ic + vec2(-5.5, 5.5), ic + vec2(5.5, 5.5), ic + vec2(5.5, 2.5)], s));
    p.galley(pos2(rect.left() + 29.0, rect.center().y - galley.size().y / 2.0), galley, c.accent_text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Thin outline icons on a 24-unit grid (Tabler style), scaled to `size` points (20 in the toolbar).
/// egui strokes have flat caps, so open ends get a small dot to look round.
struct Ink<'a> {
    p: &'a egui::Painter,
    c: Pos2,
    s: Stroke,
    scale: f32,
}

impl Ink<'_> {
    const WIDTH: f32 = 1.5;

    /// A point of the 24-unit grid, relative to the icon centre.
    fn t(&self, x: f32, y: f32) -> Pos2 {
        self.c + vec2(x - 12.0, y - 12.0) * self.scale
    }
    fn dot(&self, at: Pos2) {
        self.p.circle_filled(at, self.s.width / 2.0, self.s.color);
    }
    /// Open polyline of grid points, round at both ends.
    fn line(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.t(x, y)).collect();
        self.dot(v[0]);
        self.dot(*v.last().unwrap());
        self.p.add(Shape::line(v, self.s));
    }
    fn closed(&self, pts: Vec<Pos2>) {
        self.p.add(Shape::closed_line(pts, self.s));
    }
    /// Points of an arc (grid units, angles in degrees, screen orientation).
    fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
        let n = ((to - from).abs() / 8.0).ceil().max(2.0) as usize;
        (0..=n)
            .map(|k| {
                let a = (from + (to - from) * k as f32 / n as f32).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    }
    fn ring(&self, x: f32, y: f32, r: f32) {
        self.p.circle_stroke(self.t(x, y), r * self.scale, self.s);
    }
    /// Four-pointed star with concave sides.
    fn star(&self, x: f32, y: f32, r: f32) {
        let k = r * 0.22;
        let tips = [(0.0, -r), (r, 0.0), (0.0, r), (-r, 0.0)];
        let mut v = Vec::new();
        for i in 0..4 {
            let (ax, ay) = tips[i];
            let (bx, by) = tips[(i + 1) % 4];
            let ctl = ((ax + bx) / 2.0 * k / r, (ay + by) / 2.0 * k / r);
            for j in 0..8 {
                let u = j as f32 / 8.0;
                let w = 1.0 - u;
                v.push(self.t(
                    x + w * w * ax + 2.0 * w * u * ctl.0 + u * u * bx,
                    y + w * w * ay + 2.0 * w * u * ctl.1 + u * u * by,
                ));
            }
        }
        self.closed(v);
    }
}

/// Toolbar button with a drawn icon above its label.
pub fn tool_button(ui: &mut Ui, icon: &str, label: &str, badge: usize, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(58.0, 48.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() || active {
        p.rect_filled(rect, CornerRadius::same(7), theme::c().hover);
    }
    let col = if resp.hovered() { theme::c().strong } else { theme::c().text };
    let col = if icon == "ai" && active { theme::c().accent_text } else { col };
    glyph(p, pos2(rect.center().x, rect.top() + 17.0), icon, 20.0, Ink::WIDTH, col);
    p.text(pos2(rect.center().x, rect.bottom() - 8.0), Align2::CENTER_CENTER, label, theme::small(), if resp.hovered() { theme::c().strong } else { theme::c().dim });
    if badge > 0 {
        let s = badge.min(99).to_string();
        let b = Rect::from_center_size(pos2(rect.center().x + 13.0, rect.top() + 7.0), vec2(10.0 + s.len() as f32 * 6.0, 16.0));
        p.rect_filled(b, CornerRadius::same(8), theme::c().accent);
        p.text(b.center(), Align2::CENTER_CENTER, s, theme::small(), theme::c().on_accent);
    }
    resp
}

/// A small square button with one of `glyph`'s icons and a tooltip: the row actions of the
/// sidebar and the cockpit, where a text button would be too heavy.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
    icon_button_at(ui, rect, resp, icon, tip)
}

fn icon_button_at(ui: &Ui, rect: Rect, resp: egui::Response, icon: &str, tip: &str) -> egui::Response {
    let p = ui.painter();
    let c = theme::c();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), c.hover);
    }
    glyph(p, rect.center(), icon, 15.0, 1.4, if resp.hovered() { c.strong } else { c.dim });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Icon buttons on the right of a `section_header`: the index of the one
/// clicked. Drawn after the header, so a click on one doesn't fold the section.
pub fn header_actions(ui: &mut Ui, header: Rect, actions: &[(&str, &str)]) -> Option<usize> {
    let mut right = header.right() - 6.0;
    let mut clicked = None;
    for (i, (icon, tip)) in actions.iter().enumerate().rev() {
        let rect = Rect::from_center_size(pos2(right - 12.0, header.center().y), vec2(24.0, 22.0));
        let resp = ui.interact(rect, ui.scope_id().with(("header-action", header.min.y as i32, i)), Sense::click());
        if icon_button_at(ui, rect, resp, icon, tip).clicked() {
            clicked = Some(i);
        }
        right -= 26.0;
    }
    clicked
}

/// One of the toolbar's outline icons ("fetch", "ai", "undo", "pull", "push", "branch", "board",
/// "stash", "pop"), the dialogs' ("tag", "trash", "folder", "warning"), or the row actions'
/// ("copy", "terminal", "open", "cockpit"), centred on `c`, `size`
/// points wide, drawn with lines `width` thick.
pub fn glyph(p: &egui::Painter, c: Pos2, icon: &str, size: f32, width: f32, col: Color32) {
    let k = Ink { p, c, s: Stroke::new(width, col), scale: size / 24.0 };
    match icon {
        // two arcs with a chevron each
        "fetch" => {
            k.line(&Ink::arc(12.0, 12.0, 8.0, -7.0, -160.0));
            k.line(&[(4.0, 5.0), (4.0, 9.3), (8.0, 9.3)]);
            k.line(&Ink::arc(12.0, 12.0, 8.0, 173.0, 20.0));
            k.line(&[(20.0, 19.0), (20.0, 14.7), (16.0, 14.7)]);
        }
        // a page with a plus above a minus: what a diff adds and removes
        "diff" => {
            let page = Rect::from_min_max(k.t(4.0, 3.0), k.t(20.0, 21.0));
            k.p.rect_stroke(page, CornerRadius::same(3), k.s, StrokeKind::Middle);
            k.line(&[(12.0, 6.0), (12.0, 11.0)]);
            k.line(&[(9.5, 8.5), (14.5, 8.5)]);
            k.line(&[(9.5, 16.0), (14.5, 16.0)]);
        }
        "ai" => {
            k.star(9.0, 14.5, 8.5);
            k.star(19.5, 5.0, 3.2);
        }
        // arrow pointing left, coming back from a turn on the right
        "undo" => {
            let mut path = vec![(14.0, 19.0)];
            path.extend(Ink::arc(16.0, 14.5, 4.5, 90.0, -90.0));
            path.push((4.0, 10.0));
            k.line(&path);
            k.line(&[(8.5, 14.5), (4.0, 10.0), (8.5, 5.5)]);
        }
        "pull" => {
            k.line(&[(12.0, 4.0), (12.0, 16.0)]);
            k.line(&[(7.5, 11.5), (12.0, 16.0), (16.5, 11.5)]);
            k.line(&[(5.0, 20.0), (19.0, 20.0)]);
        }
        "push" => {
            k.line(&[(12.0, 4.0), (12.0, 16.0)]);
            k.line(&[(7.5, 8.5), (12.0, 4.0), (16.5, 8.5)]);
            k.line(&[(5.0, 20.0), (19.0, 20.0)]);
        }
        "branch" => {
            let r = 2.4;
            k.ring(6.0, 18.0, r);
            k.ring(6.0, 6.0, r);
            k.ring(18.0, 6.0, r);
            k.line(&[(6.0, 6.0 + r), (6.0, 18.0 - r)]);
            let mut curve = Ink::arc(15.6, 15.6, 9.6, -90.0, -180.0);
            curve.push((6.0, 15.6));
            k.line(&curve);
        }
        // a medal on its ribbon: the Runner skin's finish (#206)
        "medal" => {
            k.line(&[(7.0, 3.0), (10.5, 10.5)]);
            k.line(&[(17.0, 3.0), (13.5, 10.5)]);
            k.ring(12.0, 15.5, 5.5);
            k.ring(12.0, 15.5, 2.0);
        }
        // a frame holding three columns of different heights
        "board" => {
            let frame = Rect::from_min_max(k.t(3.0, 3.0), k.t(21.0, 21.0));
            k.p.rect_stroke(frame, CornerRadius::same(3), k.s, StrokeKind::Middle);
            k.line(&[(8.0, 7.5), (8.0, 15.5)]);
            k.line(&[(12.0, 7.5), (12.0, 11.5)]);
            k.line(&[(16.0, 7.5), (16.0, 13.5)]);
        }
        // lid above an open body, with a handle
        "stash" => {
            let lid = Rect::from_min_max(k.t(3.0, 4.0), k.t(21.0, 8.0));
            k.p.rect_stroke(lid, CornerRadius::same(2), k.s, StrokeKind::Middle);
            let mut body = vec![(5.0, 8.5)];
            body.extend(Ink::arc(7.0, 18.0, 2.0, 180.0, 90.0));
            body.extend(Ink::arc(17.0, 18.0, 2.0, 90.0, 0.0));
            body.push((19.0, 8.5));
            k.line(&body);
            k.line(&[(10.0, 12.5), (14.0, 12.5)]);
        }
        // open body with an arrow leaving it upwards
        "pop" => {
            let mut body = vec![(4.0, 11.0)];
            body.extend(Ink::arc(6.0, 18.0, 2.0, 180.0, 90.0));
            body.extend(Ink::arc(18.0, 18.0, 2.0, 90.0, 0.0));
            body.push((20.0, 11.0));
            k.line(&body);
            k.line(&[(12.0, 15.0), (12.0, 3.0)]);
            k.line(&[(7.5, 7.5), (12.0, 3.0), (16.5, 7.5)]);
        }
        // a label with a hole, pointing up-left
        "tag" => {
            k.closed([(4.0, 4.0), (11.5, 4.0), (20.5, 13.0), (13.0, 20.5), (4.0, 11.5)].iter().map(|&(x, y)| k.t(x, y)).collect());
            k.ring(8.5, 8.5, 1.5);
        }
        // a bin with its lid and handle
        "trash" => {
            k.line(&[(4.0, 7.0), (20.0, 7.0)]);
            k.line(&[(9.0, 7.0), (9.5, 4.0), (14.5, 4.0), (15.0, 7.0)]);
            k.line(&[(6.0, 7.0), (7.0, 20.0), (17.0, 20.0), (18.0, 7.0)]);
            k.line(&[(10.0, 11.0), (10.0, 16.0)]);
            k.line(&[(14.0, 11.0), (14.0, 16.0)]);
        }
        // a folder with its tab
        "folder" => {
            k.closed([(3.0, 6.0), (3.0, 19.0), (21.0, 19.0), (21.0, 8.5), (12.0, 8.5), (10.0, 5.0), (4.0, 5.0)].iter().map(|&(x, y)| k.t(x, y)).collect());
        }
        // two overlapping pages
        "copy" => {
            k.p.rect_stroke(Rect::from_min_max(k.t(8.0, 8.0), k.t(20.0, 20.0)), CornerRadius::same(2), k.s, StrokeKind::Middle);
            k.line(&[(16.0, 8.0), (16.0, 4.0), (4.0, 4.0), (4.0, 16.0), (8.0, 16.0)]);
        }
        // a window with a prompt and a cursor
        "terminal" => {
            k.p.rect_stroke(Rect::from_min_max(k.t(3.0, 4.0), k.t(21.0, 20.0)), CornerRadius::same(3), k.s, StrokeKind::Middle);
            k.line(&[(7.0, 9.0), (10.5, 12.0), (7.0, 15.0)]);
            k.line(&[(12.5, 15.5), (17.0, 15.5)]);
        }
        // an arrow leaving a box by its top right corner
        "open" => {
            k.line(&[(11.0, 4.0), (4.0, 4.0), (4.0, 20.0), (20.0, 20.0), (20.0, 13.0)]);
            k.line(&[(11.0, 13.0), (20.0, 4.0)]);
            k.line(&[(14.0, 4.0), (20.0, 4.0), (20.0, 10.0)]);
        }
        // four panes: every worktree at a glance
        "cockpit" => {
            for (x, y) in [(3.5, 3.5), (13.0, 3.5), (3.5, 13.0), (13.0, 13.0)] {
                k.p.rect_stroke(Rect::from_min_max(k.t(x, y), k.t(x + 7.5, y + 7.5)), CornerRadius::same(2), k.s, StrokeKind::Middle);
            }
        }
        // an arrow coming down into an open tray: a repository brought onto the Mac
        "clone" => {
            k.line(&[(12.0, 3.5), (12.0, 14.0)]);
            k.line(&[(7.5, 9.5), (12.0, 14.0), (16.5, 9.5)]);
            k.line(&[(4.0, 13.0), (4.0, 20.0), (20.0, 20.0), (20.0, 13.0)]);
        }
        // a triangle with an exclamation mark
        "warning" => {
            k.closed([(12.0, 3.5), (21.5, 20.0), (2.5, 20.0)].iter().map(|&(x, y)| k.t(x, y)).collect());
            k.line(&[(12.0, 9.5), (12.0, 14.0)]);
            k.p.circle_filled(k.t(12.0, 17.0), k.s.width * 0.75, k.s.color);
        }
        _ => {}
    }
}

pub fn person_card(ui: &mut Ui, caption: &str, name: &str, email: &str, time: i64) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
        avatar(ui.painter(), rect.center(), 16.0, name);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(name).color(theme::c().strong));
                ui.label(RichText::new(caption).size(theme::FS_SMALL).color(theme::c().faint));
            });
            ui.label(RichText::new(format!("{email} · {}", git::relative_time(time))).size(theme::FS_SECONDARY).color(theme::c().dim));
        });
    });
}

/// A changed file; returns the row response and whether its action button was clicked.
pub fn file_row(ui: &mut Ui, f: &FileChange, selected: bool, action: Option<&str>) -> (egui::Response, bool) {
    file_row_at(ui, f, selected, action, None)
}

/// A changed file in the Tree view: `depth` indents it and only its name shows, the folders
/// above it give the rest.
pub fn file_row_at(ui: &mut Ui, f: &FileChange, selected: bool, action: Option<&str>, depth: Option<usize>) -> (egui::Response, bool) {
    let indent = depth.map_or(0.0, |d| d as f32 * TREE_INDENT + 16.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().select);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let y = rect.center().y;
    let col = theme::status_color(f.code);
    let badge = Rect::from_center_size(pos2(rect.left() + 14.0 + indent, y), vec2(18.0, 17.0));
    p.rect_filled(badge, CornerRadius::same(4), theme::tint(col, theme::c().bg, 0.16));
    let letter = if f.code == '?' { 'A' } else { f.code };
    p.text(badge.center(), Align2::CENTER_CENTER, letter.to_string(), FontId::monospace(theme::FS_SMALL), col);
    let (dir, file) = match f.path.rsplit_once('/') {
        Some((_, n)) if depth.is_some() => (String::new(), n.to_string()),
        Some((d, n)) => (format!("{d}/"), n.to_string()),
        None => (String::new(), f.path.clone()),
    };
    let show_btn = action.is_some() && (resp.hovered() || selected);
    let right = rect.right() - if show_btn { 72.0 } else { 6.0 };
    let x = rect.left() + 30.0 + indent;
    let r = fit_text(p, pos2(x, y), Align2::LEFT_CENTER, &file, theme::body(), if selected { theme::c().strong } else { theme::c().text }, right - x);
    if r.right() + 30.0 < right {
        fit_text(p, pos2(r.right() + 6.0, y), Align2::LEFT_CENTER, &dir, theme::secondary(), theme::c().faint, right - r.right() - 6.0);
    }
    let mut clicked_btn = false;
    if let (Some(a), true) = (action, show_btn) {
        let br = Rect::from_min_max(pos2(rect.right() - 68.0, rect.top() + 3.0), pos2(rect.right() - 4.0, rect.bottom() - 3.0));
        let hovered = ui.rect_contains_pointer(br);
        p.rect_filled(br, CornerRadius::same(5), if hovered { theme::c().accent } else { theme::c().elevated });
        p.rect_stroke(br, CornerRadius::same(5), Stroke::new(1.0_f32, if hovered { theme::c().accent } else { theme::c().border_strong }), StrokeKind::Inside);
        p.text(br.center(), Align2::CENTER_CENTER, a, theme::small(), if hovered { theme::c().on_accent } else { theme::c().strong });
        clicked_btn = resp.clicked() && hovered;
    }
    (resp, clicked_btn)
}

/// How far each level of the Tree view moves its rows to the right.
pub const TREE_INDENT: f32 = 14.0;

/// A folder of the Tree view: chevron, folder icon, name and the number of files under it. The
/// action (Stage or Unstage the folder) shows on hover. Returns the row and whether the action
/// was clicked.
pub fn dir_row(ui: &mut Ui, name: &str, depth: usize, open: bool, count: usize, action: &str) -> (egui::Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), theme::c().hover);
    }
    let y = rect.center().y;
    let x0 = rect.left() + depth as f32 * TREE_INDENT;
    // The chevron, drawn: pointing right when folded, down when open.
    let c = pos2(x0 + 9.0, y);
    let pts = if open { [c + vec2(-3.5, -1.8), c + vec2(0.0, 1.8), c + vec2(3.5, -1.8)] } else { [c + vec2(-1.8, -3.5), c + vec2(1.8, 0.0), c + vec2(-1.8, 3.5)] };
    p.add(Shape::line(pts.to_vec(), Stroke::new(1.4_f32, theme::c().dim)));
    folder_icon(p, pos2(x0 + 26.0, y), theme::c().dim);
    let show_btn = resp.hovered();
    let right = rect.right() - if show_btn { 72.0 } else { 6.0 };
    let x = x0 + 38.0;
    let r = fit_text(p, pos2(x, y), Align2::LEFT_CENTER, name, theme::body(), theme::c().text, right - x);
    if r.right() + 30.0 < right {
        p.text(pos2(r.right() + 8.0, y), Align2::LEFT_CENTER, count.to_string(), theme::secondary(), theme::c().faint);
    }
    let mut clicked_btn = false;
    if show_btn {
        let br = Rect::from_min_max(pos2(rect.right() - 68.0, rect.top() + 3.0), pos2(rect.right() - 4.0, rect.bottom() - 3.0));
        let hovered = ui.rect_contains_pointer(br);
        p.rect_filled(br, CornerRadius::same(5), if hovered { theme::c().accent } else { theme::c().elevated });
        p.rect_stroke(br, CornerRadius::same(5), Stroke::new(1.0_f32, if hovered { theme::c().accent } else { theme::c().border_strong }), StrokeKind::Inside);
        p.text(br.center(), Align2::CENTER_CENTER, action, theme::small(), if hovered { theme::c().on_accent } else { theme::c().strong });
        clicked_btn = resp.clicked() && hovered;
    }
    (resp, clicked_btn)
}

/// A small folder: a body with a tab on its top left.
fn folder_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    let body = Rect::from_center_size(c + vec2(0.0, 1.0), vec2(13.0, 9.0));
    let tab = Rect::from_min_size(body.left_top() - vec2(0.0, 2.0), vec2(5.5, 2.5));
    p.rect_stroke(tab, CornerRadius::same(1), Stroke::new(1.2_f32, col), StrokeKind::Inside);
    p.rect_filled(body, CornerRadius::same(2), theme::tint(col, theme::c().bg, 0.35));
    p.rect_stroke(body, CornerRadius::same(2), Stroke::new(1.2_f32, col), StrokeKind::Inside);
}

/// The List / Tree switch above the local changes: two drawn icons, the chosen one highlighted.
/// Returns the new choice when a half is clicked (`true` for Tree).
pub fn files_view_switch(ui: &mut Ui, tree: bool) -> Option<bool> {
    let (rect, _) = ui.allocate_exact_size(vec2(52.0, 20.0), Sense::hover());
    let k = theme::c();
    ui.painter().rect(rect, CornerRadius::same(6), k.input, Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
    let mut out = None;
    for (i, label) in ["List", "Tree"].into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * 26.0, rect.top()), vec2(26.0, 20.0)).shrink(2.0);
        let on = tree == (i == 1);
        let resp = ui.interact(r, ui.scope_id().with(("files_view", i)), Sense::click()).on_hover_text(format!("{label} view"));
        let p = ui.painter();
        if on {
            p.rect_filled(r, CornerRadius::same(4), k.select);
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(4), k.hover);
        }
        let col = if on { k.strong } else { k.dim };
        let c = r.center();
        let st = Stroke::new(1.3_f32, col);
        if i == 0 {
            for dy in [-3.5_f32, 0.0, 3.5] {
                p.line_segment([c + vec2(-4.5, dy), c + vec2(4.5, dy)], st);
            }
        } else {
            for (dx, dy, len) in [(-4.5_f32, -3.5_f32, 9.0_f32), (-1.5, 0.0, 6.0), (-1.5, 3.5, 6.0)] {
                p.line_segment([c + vec2(dx, dy), c + vec2(dx + len, dy)], st);
            }
            p.line_segment([c + vec2(-4.5, -3.5), c + vec2(-4.5, 3.5)], st);
        }
        if resp.clicked() && !on {
            out = Some(i == 1);
        }
    }
    out
}

/// The head of a diff: a Unified / Side by side switch and a "Whole file" toggle button, drawn
/// right to left (call it inside a right-to-left layout). Returns the new `(split, whole)` when
/// one is clicked.
pub fn diff_view_switch(ui: &mut Ui, split: bool, whole: bool) -> Option<(bool, bool)> {
    let k = theme::c();
    let font = FontId::proportional(theme::FS_SECONDARY);
    let width = |ui: &Ui, t: &str| ui.painter().layout_no_wrap(t.to_string(), font.clone(), k.text).size().x;
    let mut out = None;
    // Whole file: one button that is lit while on (the rightmost, so it is drawn first).
    let ww = width(ui, "Whole file") + 20.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ww, 22.0), Sense::hover());
    let resp = ui.interact(rect, ui.scope_id().with("diff_whole"), Sense::click()).on_hover_text("Show the whole file, with the changes highlighted");
    let p = ui.painter();
    if whole {
        p.rect(rect, CornerRadius::same(6), k.widget_active, Stroke::new(1.0_f32, k.widget_active_border), StrokeKind::Inside);
    } else {
        let bg = if resp.hovered() { k.hover } else { k.input };
        p.rect(rect, CornerRadius::same(6), bg, Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
    }
    p.text(rect.center(), Align2::CENTER_CENTER, "Whole file", font.clone(), if whole { k.strong } else { k.dim });
    if resp.clicked() {
        out = Some((split, !whole));
    }
    ui.add_space(6.0);
    // Unified | Side by side.
    let labels = ["Unified", "Side by side"];
    let widths: Vec<f32> = labels.iter().map(|l| width(ui, l) + 20.0).collect();
    let (rect, _) = ui.allocate_exact_size(vec2(widths.iter().sum::<f32>() + 4.0, 22.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(6), k.input, Stroke::new(1.0_f32, k.border_strong), StrokeKind::Inside);
    let mut x = rect.left() + 2.0;
    for (i, label) in labels.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(widths[i], 18.0));
        x += widths[i];
        let on = split == (i == 1);
        let resp = ui.interact(r, ui.scope_id().with(("diff_mode", i)), Sense::click());
        let p = ui.painter();
        if on {
            p.rect_filled(r, CornerRadius::same(4), k.select);
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(4), k.hover);
        }
        p.text(r.center(), Align2::CENTER_CENTER, label, font.clone(), if on { k.strong } else { k.dim });
        if resp.clicked() && !on {
            out = Some((i == 1, whole));
        }
    }
    out
}

/// Checks out a remote branch: its local branch when there is one, else a new tracking branch
/// (`checkout::plan` decides, against the tab's refs).
pub fn checkout_remote(r: &GitRef) -> Cmd {
    Cmd::Checkout { name: r.name.clone(), remote: true }
}

/// What a double-click on a branch label of the history does: check out a local branch that is not
/// the current one, or a remote branch like the sidebar does. Tags, HEAD, stashes: nothing.
pub fn label_checkout(r: &GitRef) -> Option<Cmd> {
    match r.kind {
        RefKind::Local if !r.is_head => Some(Cmd::Checkout { name: r.name.clone(), remote: false }),
        RefKind::Remote => Some(checkout_remote(r)),
        _ => None,
    }
}

/// The "Fast-forward to 'origin/x'" entry of a local branch's menu.
pub struct FastForward {
    pub text: String,
    /// "3 commits", shown on the right of the entry.
    pub count: String,
    /// What a click does, or why the entry is greyed out.
    pub action: Result<Cmd, String>,
}

/// The fast-forward entry of a local branch, only when it is behind its upstream and not ahead (as
/// the last fetch saw it): none when up to date, ahead, diverged, without upstream or gone. A
/// branch checked out in another worktree gets it greyed out, since Git refuses to move it.
pub fn fast_forward(b: &GitRef, refs: &[GitRef], worktrees: &[crate::worktree::Worktree]) -> Option<FastForward> {
    if b.kind != RefKind::Local || b.upstream.is_empty() || b.behind == 0 || b.ahead > 0 {
        return None;
    }
    let up = refs.iter().filter(|r| r.name == b.upstream).min_by_key(|r| r.kind != RefKind::Remote)?;
    let count = if b.behind == 1 { "1 commit".to_string() } else { format!("{} commits", b.behind) };
    let elsewhere = worktrees.iter().find(|w| !b.is_head && w.branch.as_deref() == Some(b.name.as_str()));
    let action = match elsewhere {
        Some(w) if w.missing => Err(format!("{} is checked out in the worktree “{}”, whose folder is gone. Clean up missing worktrees first.", b.name, w.name())),
        Some(w) => Err(format!("{} is checked out in the worktree “{}”. Fast-forward it from there.", b.name, w.name())),
        None => Ok(Cmd::Git { label: format!("Fast-forwarded {}", b.name), args: git::fast_forward_args(&b.name, &up.full, b.is_head), network: false }),
    };
    Some(FastForward { text: format!("Fast-forward to '{}'", b.upstream), count, action })
}

/// Roomier rows in a context menu than egui's own (2 points either side): call it first.
pub fn menu_rows(ui: &mut Ui) {
    ui.spacing_mut().button_padding = vec2(8.0, 3.0);
    ui.set_min_width(180.0);
}

/// The menu of a local branch. `remote`: the remote branch of the same pill in the graph, which
/// "Delete branch…" ticks too (the checked-out branch then deletes only that one).
pub fn branch_menu(ui: &mut Ui, b: &GitRef, remote: Option<&GitRef>, current: &str, refs: &[GitRef], worktrees: &[crate::worktree::Worktree], cmds: &mut Vec<Cmd>) {
    menu_rows(ui);
    if !b.is_head && ui.button("Check out").clicked() {
        cmds.push(Cmd::Checkout { name: b.name.clone(), remote: false });
    }
    if let Some(ff) = fast_forward(b, refs, worktrees) {
        let button = egui::Button::new(ff.text).shortcut_text(ff.count);
        match ff.action {
            Ok(cmd) => {
                if ui.add(button).clicked() {
                    cmds.push(cmd);
                }
            }
            Err(why) => {
                ui.add_enabled(false, button).on_disabled_hover_text(why);
            }
        }
    }
    if !b.is_head && !current.is_empty() {
        if ui.button(format!("Merge {} into {}…", b.name, current)).clicked() {
            cmds.push(Cmd::AskMerge { name: b.name.clone(), remote: false });
        }
        if ui.button(format!("Rebase {} onto {}", current, b.name)).clicked() {
            cmds.push(Cmd::Dialog(Dialog::Confirm {
                title: "Rebase".into(),
                message: format!("Do you want to rebase “{current}” onto “{}”?", b.name),
                label: "Rebased".into(),
                args: vec!["rebase".into(), b.name.clone()],
                network: false,
            }));
        }
    }
    if ui.button("New branch from here…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::NewRef { at: b.name.clone(), label: b.name.clone(), name: String::new(), checkout: true, tag: false }));
    }
    if ui.button("Copy name").clicked() {
        cmds.push(Cmd::Copy(b.name.clone()));
    }
    if !b.is_head || remote.is_some() {
        ui.separator();
        if ui.button(RichText::new("Delete branch…").color(theme::c().deleted)).clicked() {
            // Opens the delete dialog, which also handles a worktree using the branch and offers
            // its remote branch.
            cmds.push(Cmd::AskDeleteBranch { local: (!b.is_head).then(|| b.name.clone()), remote: remote.map(|r| r.name.clone()) });
        }
    }
}

/// The menu of a remote branch ("origin/feat/x"), in the sidebar and on a pill of the graph.
pub fn remote_branch_menu(ui: &mut Ui, r: &GitRef, cmds: &mut Vec<Cmd>) {
    menu_rows(ui);
    if ui.button("Check out").clicked() {
        cmds.push(checkout_remote(r));
    }
    if ui.button(format!("Merge {} into current branch…", r.name)).clicked() {
        cmds.push(Cmd::AskMerge { name: r.name.clone(), remote: true });
    }
    if ui.button("Copy name").clicked() {
        cmds.push(Cmd::Copy(r.name.clone()));
    }
    if !r.name.ends_with("/HEAD") {
        ui.separator();
        if ui.button(RichText::new("Delete branch…").color(theme::c().deleted)).clicked() {
            cmds.push(Cmd::AskDeleteBranch { local: None, remote: Some(r.name.clone()) });
        }
    }
}

pub fn commit_menu(ui: &mut Ui, hash: &str, subject: &str, branch: Option<&str>, cmds: &mut Vec<Cmd>) {
    menu_rows(ui);
    let s = short(hash).to_string();
    if ui.button("New branch here…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::NewRef { at: hash.into(), label: format!("commit {s}"), name: String::new(), checkout: true, tag: false }));
    }
    if ui.button("New tag here…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::NewRef { at: hash.into(), label: format!("commit {s}"), name: String::new(), checkout: false, tag: true }));
    }
    ui.separator();
    if ui.button("Cherry-pick this commit").clicked() {
        cmds.push(Cmd::Git { label: format!("Cherry-picked {s}"), args: vec!["cherry-pick".into(), hash.into()], network: false });
    }
    if ui.button("Revert this commit").clicked() {
        cmds.push(Cmd::Git { label: format!("Reverted {s}"), args: vec!["revert".into(), "--no-edit".into(), hash.into()], network: false });
    }
    if ui.button("Check out this commit…").clicked() {
        cmds.push(Cmd::Dialog(Dialog::Confirm {
            title: "Checkout".into(),
            message: format!("Do you want to check out commit {s} without a branch (detached HEAD)?"),
            label: format!("Checked out {s}"),
            args: vec!["checkout".into(), hash.into()],
            network: false,
        }));
    }
    if let Some(b) = branch {
        ui.menu_button(format!("Reset {b} to here"), |ui| {
            crate::ui::menu_rows(ui);
            for (mode, explain) in [("soft", "keep all changes staged"), ("mixed", "keep changes, unstaged"), ("hard", "throw away all changes")] {
                if ui.button(format!("{mode} — {explain}")).clicked() {
                    cmds.push(Cmd::Dialog(Dialog::Confirm {
                        title: "Reset branch".into(),
                        message: format!("Do you want to reset “{b}” to commit {s} ({mode}: {explain})?"),
                        label: format!("Reset {b} to {s}"),
                        args: vec!["reset".into(), format!("--{mode}"), hash.into()],
                        network: false,
                    }));
                }
            }
        });
    }
    ui.separator();
    if ui.button("Copy SHA").clicked() {
        cmds.push(Cmd::Copy(hash.into()));
    }
    if ui.button("Copy message").clicked() {
        cmds.push(Cmd::Copy(subject.into()));
    }
}

/// The board symbol: a frame cut in three columns with a card or two in each.
pub fn board_icon(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    for (i, cards) in [2, 1, 3].iter().enumerate() {
        let x = c.x - 9.0 + i as f32 * 6.5;
        for k in 0..*cards {
            let r = egui::Rect::from_min_size(pos2(x, c.y - 7.0 + k as f32 * 5.0), vec2(5.0, 3.6));
            p.rect_filled(r, CornerRadius::same(1), color);
        }
    }
}

/// The room a pull request badge takes inside a pill, none without one.
pub fn badge_w(pull: Option<&Pull>) -> f32 {
    pull.map_or(0.0, |q| pr_badge_width(q) + 5.0)
}
