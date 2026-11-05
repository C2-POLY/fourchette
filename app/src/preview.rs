//! A small animated graph that shows what an action does to a branch (#92): a few commits of the
//! branch and of its upstream, with their labels, drawn like the history graph. The model (`Scene`,
//! `Preview`) is plain data built from the branch's counts and an `Outcome`; `show` paints a scene
//! and slides from the one on screen to the one asked for. Its rows are `ROW` apart, so a list of
//! choices beside it can sit on the same lines. The Check out dialog uses it, and the Pull, Merge
//! and Push dialogs can as well: build a `Preview` and call `show`.
use std::collections::HashMap;

use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

use crate::checkout::{MergeStyle, Sync, SyncAction};
use crate::theme;

/// The drawing is a schematic, whatever the real counts: this many commits on a side that has any.
pub const SIDE: usize = 2;
/// Rows of the tallest scene (a rebase with both sides: replayed, upstream, ghosts, ancestor).
pub const MAX_ROWS: usize = 2 * SIDE + SIDE + 1;
/// The height of a row: a list of choices beside the graph uses it too, so both share their lines.
pub const ROW: f32 = 34.0;
const LANE: f32 = 24.0;
const X0: f32 = 14.0;
/// The first row's centre: half a row down, like the centre of a choice beside it.
const PAD_Y: f32 = ROW / 2.0;
/// Where the labels and messages start, from the left of the drawing.
const TEXT_X: f32 = X0 + 2.0 * LANE + 30.0;
/// The disc of a commit, the checked-out one, its centre dot, and the cut-out stroke around a disc.
const NODE_R: f32 = 7.0;
const HEAD_R: f32 = 9.5;
const DOT_R: f32 = 3.0;
const CUT: f32 = 3.0;
const LINE_W: f32 = 3.0;
const TEXT_SIZE: f32 = 13.0;
const CHIP_SIZE: f32 = 12.0;
const CHIP_H: f32 = 20.0;
const CHIP_GAP: f32 = 5.0;
/// How long the slide takes, and the wait before the first one.
const SLIDE: f64 = 0.36;
const FIRST_WAIT: f64 = 0.25;

/// What happens to the branch. `Stay`: nothing moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    FastForward,
    Merge,
    /// One new single-parent commit with the upstream's changes.
    Squash,
    Rebase,
    Reset,
    Stay,
}

impl Outcome {
    pub const ALL: [Outcome; 6] = [Outcome::FastForward, Outcome::Merge, Outcome::Squash, Outcome::Rebase, Outcome::Reset, Outcome::Stay];

    pub fn of(a: SyncAction, m: MergeStyle) -> Self {
        match a {
            SyncAction::FastForward => Outcome::FastForward,
            SyncAction::Merge => match m {
                MergeStyle::Commit => Outcome::Merge,
                MergeStyle::Squash => Outcome::Squash,
            },
            SyncAction::Rebase => Outcome::Rebase,
            SyncAction::Reset => Outcome::Reset,
            SyncAction::JustCheckout => Outcome::Stay,
        }
    }
}

/// A commit's colour, as a lane of the history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Local,
    Upstream,
    Base,
}

impl Tone {
    fn color(self) -> Color32 {
        theme::lane_color(match self {
            Tone::Local => 1,
            Tone::Upstream => 2,
            Tone::Base => 8,
        })
    }
}

/// A commit's name inside a preview, stable between the scenes so it can slide from one to the other.
pub type Id = u16;
const BASE: Id = 1;
const MERGE: Id = 2;
const SQUASH: Id = 3;
const fn up(i: usize) -> Id {
    100 + i as Id
}
const fn mine(i: usize) -> Id {
    200 + i as Id
}
const fn replay(i: usize) -> Id {
    300 + i as Id
}

#[derive(Clone, Debug, PartialEq)]
pub struct Commit {
    pub id: Id,
    pub message: String,
    pub lane: usize,
    pub row: usize,
    pub tone: Tone,
    pub parents: Vec<Id>,
    /// A ghost: the commit the branch no longer has, drawn dashed.
    pub ghost: bool,
    pub merge: bool,
    /// When the commit is new, the one it starts from while it appears.
    pub from: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    pub remote: bool,
    pub at: Id,
}

/// One state of the graph: commits from the newest at row 0, labels and the checked-out commit.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub commits: Vec<Commit>,
    pub labels: Vec<Label>,
    pub head: Option<Id>,
}

impl Scene {
    pub fn rows(&self) -> usize {
        self.commits.iter().map(|c| c.row + 1).max().unwrap_or(0)
    }

    pub fn commit(&self, id: Id) -> Option<&Commit> {
        self.commits.iter().find(|c| c.id == id)
    }
}

/// The scene now and the scene after the action.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub before: Scene,
    pub after: Scene,
    /// Rows of the tallest scene this branch can show, whatever the action: the drawing keeps
    /// that height, so the dialog does not change size when the choice changes.
    pub rows: usize,
}

impl Preview {
    pub fn from_sync(sync: &Sync, action: SyncAction, merge: MergeStyle) -> Self {
        Self::new(&sync.branch, &sync.upstream, sync.ahead, sync.behind, Outcome::of(action, merge))
    }

    /// `ahead` commits only the branch has, `behind` only its upstream has.
    pub fn new(branch: &str, upstream: &str, ahead: usize, behind: usize, outcome: Outcome) -> Self {
        let (before, after) = Self::scenes(branch, upstream, ahead, behind, outcome);
        let rows = Outcome::ALL.iter().map(|o| Self::scenes(branch, upstream, ahead, behind, *o).1.rows()).chain([before.rows()]).max().unwrap_or(MAX_ROWS);
        Preview { before, after, rows }
    }

    fn scenes(branch: &str, upstream: &str, ahead: usize, behind: usize, outcome: Outcome) -> (Scene, Scene) {
        let (nu, nm) = (if behind > 0 { SIDE } else { 0 }, if ahead > 0 { SIDE } else { 0 });
        // The upstream's commits come first (rows 0..nu), then the branch's own, then the common one.
        let commit = |id: Id, message: &str, lane: usize, row: usize, tone: Tone, parents: Vec<Id>| Commit { id, message: message.into(), lane, row, tone, parents, ghost: false, merge: false, from: None };
        let upstream_chain = |first_row: usize| -> Vec<Commit> {
            (0..nu)
                .map(|i| {
                    let parent = if i + 1 < nu { up(i + 1) } else { BASE };
                    commit(up(i), "Commit on the upstream", 0, first_row + i, Tone::Upstream, vec![parent])
                })
                .collect()
        };
        let own_chain = |first_row: usize, lane: usize, id: fn(usize) -> Id, ghost: bool, end: Id| -> Vec<Commit> {
            (0..nm)
                .map(|i| {
                    let parent = if i + 1 < nm { id(i + 1) } else { end };
                    let mut c = commit(id(i), "Commit of the branch", lane, first_row + i, Tone::Local, vec![parent]);
                    c.ghost = ghost;
                    c
                })
                .collect()
        };
        let label = |name: &str, remote: bool, at: Id| Label { name: name.into(), remote, at };
        let base_at = |row: usize| commit(BASE, "Common ancestor", 0, row, Tone::Base, vec![]);
        let own_tip = if nm > 0 { mine(0) } else { BASE };
        let up_tip = if nu > 0 { up(0) } else { BASE };

        let mut before_commits = upstream_chain(0);
        before_commits.extend(own_chain(nu, 1, mine, false, BASE));
        before_commits.push(base_at(nu + nm));
        let before = Scene { commits: before_commits.clone(), labels: vec![label(branch, false, own_tip), label(upstream, true, up_tip)], head: None };

        let mut after = Scene { commits: before_commits, labels: vec![label(branch, false, own_tip), label(upstream, true, up_tip)], head: Some(own_tip) };
        // With no commit of its own, a merge, a rebase and a reset are the same fast-forward.
        let outcome = if nm == 0 && matches!(outcome, Outcome::Merge | Outcome::Rebase | Outcome::Reset) { Outcome::FastForward } else { outcome };
        match outcome {
            Outcome::Stay => {}
            // Only possible with nothing of its own; with something it is a reset without the loss.
            Outcome::FastForward | Outcome::Reset => {
                for c in after.commits.iter_mut().filter(|c| (200..300).contains(&c.id)) {
                    c.ghost = true;
                }
                after.labels[0].at = up_tip;
                after.head = Some(up_tip);
            }
            Outcome::Squash => {
                // One new commit on the branch, with a single parent: no line from the upstream.
                for c in after.commits.iter_mut() {
                    c.row += 1;
                }
                after.commits.insert(0, commit(SQUASH, &format!("Squash {upstream} into {branch}"), 1, 0, Tone::Local, vec![own_tip]));
                after.labels[0].at = SQUASH;
                after.head = Some(SQUASH);
            }
            Outcome::Merge => {
                // A merge commit joins both at the top; everything else moves one row down.
                for c in after.commits.iter_mut() {
                    c.row += 1;
                }
                let mut m = commit(MERGE, &format!("Merge {upstream} into {branch}"), 1, 0, Tone::Local, vec![own_tip, up_tip]);
                m.merge = true;
                after.commits.insert(0, m);
                after.labels[0].at = MERGE;
                after.head = Some(MERGE);
            }
            Outcome::Rebase => {
                // The branch's commits are replayed on top of the upstream's, the old ones stay dashed.
                for c in after.commits.iter_mut() {
                    c.row += nm;
                }
                for c in after.commits.iter_mut().filter(|c| (200..300).contains(&c.id)) {
                    c.ghost = true;
                    c.lane = 2;
                }
                let mut copies = own_chain(0, 1, replay, false, up_tip);
                for (i, c) in copies.iter_mut().enumerate() {
                    c.from = Some(mine(i));
                }
                after.commits.splice(0..0, copies);
                after.labels[0].at = replay(0);
                after.head = Some(replay(0));
            }
        }
        (before, after)
    }

    pub fn scene(&self, after: bool) -> &Scene {
        if after {
            &self.after
        } else {
            &self.before
        }
    }
}

// ------------------------------------------------------------------ animation

/// Where a commit is drawn: in canvas points, how visible, and whether it is a ghost.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Node {
    x: f32,
    y: f32,
    o: f32,
    ghost: bool,
}

type Pose = HashMap<Id, Node>;

fn rest(c: &Commit) -> Node {
    Node { x: X0 + c.lane as f32 * LANE, y: PAD_Y + c.row as f32 * ROW, o: 1.0, ghost: c.ghost }
}

/// What the preview remembers between frames.
#[derive(Clone)]
struct Slide {
    /// The scene being slid to, the one it left, and the pose it left from.
    to: Scene,
    left: Scene,
    from: Pose,
    /// What was drawn last frame.
    last: Pose,
    last_labels: HashMap<String, Pos2>,
    from_labels: HashMap<String, Pos2>,
    /// The head marker as it was drawn last frame, and as it was when the slide began.
    last_head: Option<Marker>,
    from_head: Option<Marker>,
    t0: f64,
}

/// The checked-out commit's marker: it glides with the slide, growing in (`vis` 0 to 1) or out.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Marker {
    pos: Pos2,
    vis: f32,
    color: Color32,
}

fn marker_of(s: &Scene) -> Option<Marker> {
    let c = s.commit(s.head?)?;
    let r = rest(c);
    Some(Marker { pos: pos2(r.x, r.y), vis: 1.0, color: c.tone.color() })
}

/// The marker `e` of the way from `from` to the one the scene `to` asks for.
fn blend_head(from: Option<Marker>, to: &Scene, e: f32) -> Option<Marker> {
    match (from, marker_of(to)) {
        (Some(f), Some(t)) => Some(Marker { pos: f.pos + (t.pos - f.pos) * e, vis: f.vis + (1.0 - f.vis) * e, color: t.color }),
        (None, Some(t)) => Some(Marker { vis: e, ..t }),
        (Some(f), None) => (f.vis * (1.0 - e) > 0.001).then_some(Marker { vis: f.vis * (1.0 - e), ..f }),
        (None, None) => None,
    }
}

fn pose_of(s: &Scene) -> Pose {
    s.commits.iter().map(|c| (c.id, rest(c))).collect()
}

fn label_spots(s: &Scene, pose: &Pose) -> HashMap<String, Pos2> {
    s.labels.iter().filter_map(|l| pose.get(&l.at).map(|n| (l.name.clone(), pos2(n.x, n.y)))).collect()
}

fn ease(t: f64) -> f32 {
    (if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }) as f32
}

/// The pose at `k` (0 to 1) of the slide from `from` to the scene `to`. A commit that was not there
/// appears at the one it comes from (or drops in a little); one that is gone fades out in place.
fn blend(from: &Pose, to: &Scene, left: &Scene, e: f32) -> Pose {
    let mut out = Pose::new();
    for c in &to.commits {
        let b = rest(c);
        let a = match (from.get(&c.id), c.from.and_then(|f| from.get(&f))) {
            (Some(a), _) => *a,
            (None, Some(f)) => Node { o: 0.0, ghost: false, ..*f },
            (None, None) => Node { o: 0.0, y: b.y - 10.0, ..b },
        };
        out.insert(c.id, Node { x: a.x + (b.x - a.x) * e, y: a.y + (b.y - a.y) * e, o: a.o + (b.o - a.o) * e, ghost: b.ghost });
    }
    for c in &left.commits {
        if to.commit(c.id).is_none() {
            if let Some(a) = from.get(&c.id) {
                out.insert(c.id, Node { o: a.o * (1.0 - e), ..*a });
            }
        }
    }
    out
}

// ------------------------------------------------------------------ painting

/// The preview's height for `rows` rows: one `ROW` each.
pub fn height(rows: usize) -> f32 {
    rows as f32 * ROW
}

/// Paints the preview of `p` in the width of `ui`, from its top left: its scene after the action
/// (`after`) or now, sliding there from what was on screen. `id` tells this preview from another
/// in the same window.
pub fn show(ui: &mut Ui, id: egui::Id, p: &Preview, after: bool) {
    let target = p.scene(after);
    // As tall as the tallest scene this branch can show, so it keeps its size for every action.
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height(p.rows)), Sense::hover());
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let mut s = ctx.data_mut(|d| d.get_temp::<Slide>(id)).unwrap_or_else(|| {
        // First frame: start from the scene now and slide to the one asked for.
        let pose = pose_of(&p.before);
        let spots = label_spots(&p.before, &pose);
        Slide { to: p.before.clone(), left: p.before.clone(), from: pose.clone(), last: pose, last_labels: spots.clone(), from_labels: spots, last_head: None, from_head: None, t0: now + FIRST_WAIT }
    });
    if s.to != *target {
        s.left = s.to.clone();
        s.to = target.clone();
        s.from = s.last.clone();
        s.from_labels = s.last_labels.clone();
        s.from_head = s.last_head;
        s.t0 = if s.t0 > now { s.t0 } else { now };
    }
    let k = ((now - s.t0) / SLIDE).clamp(0.0, 1.0);
    let e = ease(k);
    let pose = blend(&s.from, &s.to, &s.left, e);
    let mut spots = HashMap::new();
    let rest_spots = label_spots(&s.to, &pose_of(&s.to));
    for (name, to) in &rest_spots {
        let a = s.from_labels.get(name).copied().unwrap_or(*to);
        spots.insert(name.clone(), a + (*to - a) * e);
    }
    let head = blend_head(s.from_head, &s.to, e);
    s.last_head = head;
    s.last = pose.clone();
    s.last_labels = spots.clone();
    let moving = now < s.t0 + SLIDE;
    let (scene, left) = (s.to.clone(), s.left.clone());
    ctx.data_mut(|d| d.insert_temp(id, s));
    if moving {
        ctx.request_repaint();
    }
    if ui.is_rect_visible(rect) {
        paint(ui, rect, &scene, &left, &pose, &spots, &rest_spots, head, after);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint(ui: &Ui, rect: Rect, scene: &Scene, left: &Scene, pose: &Pose, spots: &HashMap<String, Pos2>, rest_spots: &HashMap<String, Pos2>, head: Option<Marker>, after: bool) {
    let c = theme::c();
    let painter = ui.painter_at(rect);
    let at = |n: &Node| rect.min + vec2(n.x, n.y);
    let mut commits: Vec<&Commit> = scene.commits.iter().collect();
    commits.extend(left.commits.iter().filter(|l| scene.commit(l.id).is_none() && pose.contains_key(&l.id)));

    // Edges and nodes, in layers: a line that joins a node from another lane is cut out by a ring
    // in the panel's colour around the node; a line along the node's own lane meets it flush, so a
    // short run of commits reads as one solid line, never as dashes. Only ghosts are dashed.
    let mut straight = Vec::new();
    for cm in &commits {
        let Some(a) = pose.get(&cm.id) else { continue };
        for pid in &cm.parents {
            let Some(b) = pose.get(pid) else { continue };
            let ghost = a.ghost || b.ghost;
            let col = if ghost { c.faint } else { cm.tone.color() };
            let col = col.gamma_multiply((a.o.min(b.o)) * if ghost { 0.7 } else { 1.0 });
            let pts = edge(at(a), at(b));
            let shape = if ghost { Shape::Vec(Shape::dashed_line(&pts, Stroke::new(LINE_W, col), 4.0, 5.0)) } else { Shape::line(pts, Stroke::new(LINE_W, col)) };
            if (a.x - b.x).abs() < 0.5 {
                straight.push(shape);
            } else {
                painter.add(shape);
            }
        }
    }
    let head_r = head.map(|m| NODE_R + (HEAD_R - NODE_R) * m.vis);
    let head_o = scene.head.and_then(|id| pose.get(&id)).map_or(1.0, |n| n.o);
    for cm in &commits {
        let Some(n) = pose.get(&cm.id) else { continue };
        painter.circle_filled(at(n), NODE_R + CUT / 2.0, c.bg.gamma_multiply(n.o));
    }
    if let (Some(m), Some(r)) = (head, head_r) {
        painter.circle_filled(rect.min + m.pos.to_vec2(), r + CUT / 2.0, c.bg.gamma_multiply(head_o));
    }
    painter.extend(straight);
    // Nodes: filled discs, a ghost hollow and dashed. The checked-out commit gets its bigger disc
    // with a dot, gliding with the slide.
    for cm in &commits {
        let Some(n) = pose.get(&cm.id) else { continue };
        let ctr = at(n);
        if n.ghost {
            painter.circle_filled(ctr, NODE_R, c.bg.gamma_multiply(n.o));
            painter.extend(Shape::dashed_line(&circle_points(ctr, NODE_R), Stroke::new(1.8_f32, c.faint.gamma_multiply(n.o)), 2.5, 2.5));
        } else {
            painter.circle_filled(ctr, NODE_R, cm.tone.color().gamma_multiply(n.o));
        }
    }
    if let (Some(m), Some(r)) = (head, head_r) {
        let ctr = rect.min + m.pos.to_vec2();
        painter.circle_filled(ctr, r, m.color.gamma_multiply(head_o));
        painter.circle_filled(ctr, DOT_R * m.vis, c.bg.gamma_multiply(head_o));
    }
    // Labels, then each commit's message after the labels of its row.
    let ends = row_ends(&painter, scene);
    let chip_font = FontId::monospace(CHIP_SIZE);
    let mut row_x: HashMap<i32, f32> = HashMap::new();
    for l in &scene.labels {
        let (Some(spot), Some(rest_spot)) = (spots.get(&l.name), rest_spots.get(&l.name)) else { continue };
        let w = chip_w(&painter, &l.name);
        // Labels of one row sit side by side, in the row they end up in, so a sliding label does not
        // push its neighbour around.
        let key = rest_spot.y.round() as i32;
        let x = *row_x.get(&key).unwrap_or(&TEXT_X);
        row_x.insert(key, x + w + CHIP_GAP);
        let fill = (if l.remote { Tone::Upstream } else { Tone::Local }).color();
        let r = Rect::from_min_size(rect.min + vec2(x, spot.y - CHIP_H / 2.0), vec2(w, CHIP_H));
        painter.rect_filled(r, CornerRadius::same(5), fill);
        painter.text(r.center(), Align2::CENTER_CENTER, &l.name, chip_font.clone(), on(fill));
    }
    let font = FontId::proportional(TEXT_SIZE);
    for cm in &commits {
        let (Some(n), Some(target)) = (pose.get(&cm.id), if scene.commit(cm.id).is_some() { Some(rest(cm)) } else { pose.get(&cm.id).copied() }) else { continue };
        let x = rect.min.x + ends.get(&(target.y.round() as i32)).copied().unwrap_or(TEXT_X);
        let col = if n.ghost { c.faint } else if after && scene.head == Some(cm.id) { c.strong } else { c.text };
        let col = col.gamma_multiply(n.o);
        let pos = pos2(x, rect.min.y + n.y);
        let r = painter.text(pos, Align2::LEFT_CENTER, &cm.message, font.clone(), col);
        if n.ghost {
            painter.line_segment([pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], Stroke::new(1.0_f32, col));
        }
    }
}

fn chip_w(painter: &egui::Painter, name: &str) -> f32 {
    painter.layout_no_wrap(name.to_string(), FontId::monospace(CHIP_SIZE), Color32::WHITE).size().x + 14.0
}

/// Where each row's message starts, after the labels that end up in it (keyed by the row's y).
fn row_ends(painter: &egui::Painter, scene: &Scene) -> HashMap<i32, f32> {
    let mut ends = HashMap::new();
    for l in &scene.labels {
        let Some(c) = scene.commit(l.at) else { continue };
        let x = ends.entry(rest(c).y.round() as i32).or_insert(TEXT_X);
        *x += chip_w(painter, &l.name) + CHIP_GAP;
    }
    ends
}

/// Text colour for a chip filled with `fill`: dark on a light colour, white on a dark one.
fn on(fill: Color32) -> Color32 {
    let l = (0.299 * fill.r() as f32 + 0.587 * fill.g() as f32 + 0.114 * fill.b() as f32) / 255.0;
    if l > 0.5 {
        Color32::from_rgb(0x14, 0x15, 0x18)
    } else {
        Color32::WHITE
    }
}

fn circle_points(c: Pos2, r: f32) -> Vec<Pos2> {
    (0..=24).map(|i| c + Vec2::angled(i as f32 / 24.0 * std::f32::consts::TAU) * r).collect()
}

/// A child's line to its parent: straight down, then a soft curve into the parent's lane.
fn edge(a: Pos2, b: Pos2) -> Vec<Pos2> {
    if (a.x - b.x).abs() < 0.5 {
        return vec![a, b];
    }
    let my = (b.y - ROW * 0.75).max(a.y);
    let (c1, c2, end) = (pos2(a.x, my + 4.0), pos2(b.x, my + 2.0), pos2(b.x, (b.y - ROW * 0.3).max(my + 2.0)));
    let mut pts = vec![a, pos2(a.x, my - 8.0_f32.min(my - a.y))];
    let start = *pts.last().unwrap();
    for i in 1..=12 {
        let t = i as f32 / 12.0;
        let u = 1.0 - t;
        pts.push(pos2(
            u * u * u * start.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * end.x,
            u * u * u * start.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * end.y,
        ));
    }
    pts.push(b);
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(s: &Scene) -> Vec<Id> {
        s.commits.iter().map(|c| c.id).collect()
    }

    fn names(s: &Scene) -> Vec<(&str, Id)> {
        s.labels.iter().map(|l| (l.name.as_str(), l.at)).collect()
    }

    fn diverged(o: Outcome) -> Preview {
        Preview::new("main", "origin/main", 2, 3, o)
    }

    #[test]
    fn before_has_both_sides_and_their_labels() {
        let p = diverged(Outcome::Merge);
        assert_eq!(ids(&p.before), vec![up(0), up(1), mine(0), mine(1), BASE]);
        assert_eq!(names(&p.before), vec![("main", mine(0)), ("origin/main", up(0))]);
        assert_eq!(p.before.head, None);
        assert_eq!(p.before.rows(), 5);
    }

    #[test]
    fn behind_only_has_the_branch_at_the_common_commit() {
        let p = Preview::new("main", "origin/main", 0, 2, Outcome::FastForward);
        assert_eq!(ids(&p.before), vec![up(0), up(1), BASE]);
        assert_eq!(names(&p.before), vec![("main", BASE), ("origin/main", up(0))]);
        assert_eq!(names(&p.after), vec![("main", up(0)), ("origin/main", up(0))]);
        assert_eq!(p.after.head, Some(up(0)));
        assert!(p.after.commits.iter().all(|c| !c.ghost));
    }

    #[test]
    fn without_commits_of_its_own_merge_rebase_and_reset_are_a_fast_forward() {
        let ff = Preview::new("main", "origin/main", 0, 3, Outcome::FastForward).after;
        for o in [Outcome::Merge, Outcome::Rebase, Outcome::Reset] {
            assert_eq!(Preview::new("main", "origin/main", 0, 3, o).after, ff);
        }
    }

    #[test]
    fn merge_adds_one_commit_joining_both() {
        let p = diverged(Outcome::Merge);
        assert_eq!(p.after.commits.len(), p.before.commits.len() + 1);
        let m = p.after.commit(MERGE).unwrap();
        assert!(m.merge && m.row == 0);
        assert_eq!(m.parents, vec![mine(0), up(0)]);
        assert_eq!(names(&p.after), vec![("main", MERGE), ("origin/main", up(0))]);
        assert_eq!(p.after.head, Some(MERGE));
        assert!(p.after.commits.iter().all(|c| !c.ghost));
    }

    #[test]
    fn squash_adds_one_single_parent_commit_on_the_branch() {
        for (ahead, tip) in [(2, mine(0)), (0, BASE)] {
            let p = Preview::new("main", "origin/main", ahead, 3, Outcome::Squash);
            assert_eq!(p.after.commits.len(), p.before.commits.len() + 1);
            let sq = p.after.commit(SQUASH).unwrap();
            assert_eq!((sq.row, sq.lane, sq.merge), (0, 1, false));
            assert_eq!(sq.parents, vec![tip], "no line from the upstream");
            assert_eq!(sq.message, "Squash origin/main into main");
            assert_eq!(names(&p.after), vec![("main", SQUASH), ("origin/main", up(0))]);
            assert_eq!(p.after.head, Some(SQUASH));
        }
    }

    #[test]
    fn every_action_of_a_branch_gets_the_height_of_its_tallest_scene() {
        // Diverged: a rebase is the tallest, whatever the action shown.
        for o in Outcome::ALL {
            assert_eq!(diverged(o).rows, MAX_ROWS, "{o:?}");
        }
        // Only behind: the upstream's commits, the ancestor and a squashed commit.
        for o in Outcome::ALL {
            assert_eq!(Preview::new("main", "origin/main", 0, 3, o).rows, SIDE + 2, "{o:?}");
        }
        assert_eq!(height(MAX_ROWS), MAX_ROWS as f32 * ROW);
    }

    #[test]
    fn a_row_of_the_graph_lines_up_with_a_row_beside_it() {
        // The centre of row i is where a list row of height ROW has its centre.
        let p = diverged(Outcome::Merge);
        for c in &p.after.commits {
            assert_eq!(rest(c).y, (c.row as f32 + 0.5) * ROW);
        }
    }

    #[test]
    fn only_commits_that_leave_the_history_are_ghosts() {
        for (ahead, behind) in [(0, 3), (2, 3)] {
            for o in Outcome::ALL {
                let p = Preview::new("main", "origin/main", ahead, behind, o);
                let ghosts: Vec<Id> = p.after.commits.iter().filter(|c| c.ghost).map(|c| c.id).collect();
                // (A fast-forward is not offered to a branch with commits of its own; drawn, it is a reset.)
                let gone: Vec<Id> = if ahead > 0 && matches!(o, Outcome::Rebase | Outcome::Reset | Outcome::FastForward) { vec![mine(0), mine(1)] } else { vec![] };
                assert_eq!(ghosts, gone, "{o:?} with {ahead} ahead");
                // The upstream's commits are never ghosts: after a merge they are the merge
                // commit's second parent, after a squash they are still the upstream's line.
                assert!(p.after.commits.iter().filter(|c| (100..200).contains(&c.id)).all(|c| !c.ghost));
            }
        }
        // A squash has no line to the upstream's commits.
        let sq = Preview::new("main", "origin/main", 0, 3, Outcome::Squash);
        assert!(sq.after.commits.iter().filter(|c| c.id == SQUASH).all(|c| c.parents.iter().all(|p| !(100..200).contains(p))));
    }

    #[test]
    fn rebase_replays_the_commits_and_dashes_the_old_ones() {
        let p = diverged(Outcome::Rebase);
        let copies: Vec<_> = p.after.commits.iter().filter(|c| (300..400).contains(&c.id)).collect();
        assert_eq!(copies.len(), 2);
        assert_eq!(copies[0].from, Some(mine(0)));
        assert_eq!(copies[1].parents, vec![up(0)]);
        assert!(p.after.commits.iter().filter(|c| (200..300).contains(&c.id)).all(|c| c.ghost && c.lane == 2));
        assert!(copies.iter().all(|c| !c.ghost));
        assert_eq!(names(&p.after), vec![("main", replay(0)), ("origin/main", up(0))]);
        // The replayed commits come first, then the upstream's.
        assert!(copies.iter().all(|c| c.row < p.after.commit(up(0)).unwrap().row));
    }

    #[test]
    fn reset_turns_the_branch_commits_into_ghosts() {
        let p = diverged(Outcome::Reset);
        assert_eq!(ids(&p.after), ids(&p.before));
        assert!(p.after.commits.iter().filter(|c| (200..300).contains(&c.id)).all(|c| c.ghost));
        assert_eq!(names(&p.after), vec![("main", up(0)), ("origin/main", up(0))]);
        assert_eq!(p.after.head, Some(up(0)));
    }

    #[test]
    fn staying_moves_nothing() {
        let p = diverged(Outcome::Stay);
        assert_eq!(p.after.commits, p.before.commits);
        assert_eq!(names(&p.after), names(&p.before));
        assert_eq!(p.after.head, Some(mine(0)));
    }

    #[test]
    fn the_drawing_ignores_the_real_counts() {
        for o in Outcome::ALL {
            assert_eq!(Preview::new("main", "origin/main", 2, 3, o), Preview::new("main", "origin/main", 9, 40, o));
        }
        let p = Preview::new("main", "origin/main", 7, 12, Outcome::Merge);
        assert_eq!(p.before.commits.iter().filter(|c| (100..200).contains(&c.id)).count(), SIDE);
        assert_eq!(p.before.commits.iter().filter(|c| (200..300).contains(&c.id)).count(), SIDE);
        // A side with nothing is not drawn.
        assert_eq!(Preview::new("main", "origin/main", 0, 12, Outcome::Merge).before.commits.len(), SIDE + 1);
    }

    #[test]
    fn no_scene_is_taller_than_the_fixed_height() {
        let mut tallest = 0;
        for (a, b) in [(0, 3), (2, 3), (5, 1)] {
            for o in Outcome::ALL {
                tallest = tallest.max(Preview::new("main", "origin/main", a, b, o).rows);
            }
        }
        assert_eq!(tallest, MAX_ROWS);
    }

    #[test]
    fn the_head_marker_glides_with_its_commit() {
        let p = diverged(Outcome::Merge);
        assert_eq!(marker_of(&p.before), None);
        let end = marker_of(&p.after).unwrap();
        // It grows in on the new tip, then moves to the next tip when the action changes.
        let grown = blend_head(None, &p.after, 0.5).unwrap();
        assert_eq!((grown.pos, grown.vis), (end.pos, 0.5));
        let rebased = diverged(Outcome::Rebase);
        let mid = blend_head(Some(end), &rebased.after, 0.5).unwrap();
        assert_eq!(mid.vis, 1.0);
        assert_eq!(blend_head(Some(end), &p.before, 1.0), None);
    }

    #[test]
    fn it_builds_from_a_sync() {
        let sync = Sync { branch: "main".into(), upstream: "origin/main".into(), upstream_ref: "refs/remotes/origin/main".into(), ahead: 1, behind: 1 };
        let p = Preview::from_sync(&sync, SyncAction::Merge, MergeStyle::Commit);
        assert!(p.after.commit(MERGE).is_some());
        assert_eq!(p.after.labels.iter().find(|l| l.name == "origin/main").map(|l| l.remote), Some(true));
    }

    #[test]
    fn a_slide_starts_where_it_left_and_ends_on_the_scene() {
        let p = diverged(Outcome::Rebase);
        let from = pose_of(&p.before);
        let start = blend(&from, &p.after, &p.before, 0.0);
        // A replayed commit starts invisible on the commit it copies.
        let r = start[&replay(0)];
        assert_eq!((r.x, r.y, r.o), (from[&mine(0)].x, from[&mine(0)].y, 0.0));
        let end = blend(&from, &p.after, &p.before, 1.0);
        assert_eq!(end, pose_of(&p.after));
    }
}
