//! The Runner skin's stadium (#206): the board's City view as an athletics track. One lane per
//! zone (`city::zone`, at most `MAX_LANES`), one athlete per ticket, placed on the lap by its
//! status (`city::Stage`): Backlog warms up on the infield, Ready waits in the start blocks, In
//! progress runs the first bend, In review the back straight, To test the last bend, and Done
//! stands on the podium. Pure layout in world points around the stadium's centre, y down;
//! `app/track.rs` draws it.

use crate::city::{self, Stage};
use crate::github::{Item, Kind};
use std::collections::BTreeMap;

/// The most lanes a track has; the zones past the seventh share the last one.
pub const MAX_LANES: usize = 8;
/// The inside edge of lane 1: the infield's half width and half height.
pub const INNER: (f32, f32) = (425.0, 195.0);
/// A lane's width.
pub const LANE: f32 = 24.0;
/// Athletes side by side in one lane of one section of the lap, in the warm-up area, on the podium.
pub const PER_SECTION: usize = 3;
pub const WARMUP_SLOTS: usize = 6;
pub const PODIUM: usize = 3;
/// Where the start line and the finish line cross the track, in degrees (y down, so the bottom
/// straight is near 90; the lap runs with the angle going down).
pub const START_DEG: f32 = 116.0;
pub const FINISH_DEG: f32 = 125.0;

/// Where an athlete stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Spot {
    /// Backlog: the warm-up area on the infield.
    Warmup,
    /// Ready: the start blocks.
    Blocks,
    /// In progress: the first bend.
    Bend,
    /// In review: the back straight.
    Back,
    /// To test: the last bend.
    LastBend,
    /// Done: the podium.
    Podium,
}

impl Spot {
    pub const ALL: [Spot; 6] = [Spot::Warmup, Spot::Blocks, Spot::Bend, Spot::Back, Spot::LastBend, Spot::Podium];

    pub fn of(stage: Stage) -> Spot {
        match stage {
            Stage::Lot => Spot::Warmup,
            Stage::Staked => Spot::Blocks,
            Stage::Foundations => Spot::Bend,
            Stage::Frame => Spot::Back,
            Stage::Testing => Spot::LastBend,
            Stage::Built => Spot::Podium,
        }
    }

    /// On the lap, running: the legs move.
    pub fn running(self) -> bool {
        matches!(self, Spot::Bend | Spot::Back | Spot::LastBend)
    }

    /// The angle of the section's first place, and how far apart the next ones stand.
    fn arc(self) -> Option<(f32, f32)> {
        match self {
            Spot::Blocks => Some((110.0, 10.0)),
            Spot::Bend => Some((44.0, 15.0)),
            Spot::Back => Some((-72.0, 19.0)),
            Spot::LastBend => Some((-156.0, 15.0)),
            Spot::Warmup | Spot::Podium => None,
        }
    }

    /// The words painted beside the track by the section, with their angle.
    pub fn mark(self) -> Option<(&'static str, f32)> {
        match self {
            Spot::Blocks => Some(("START · READY", START_DEG - 6.0)),
            Spot::Bend => Some(("IN PROGRESS", 28.0)),
            Spot::Back => Some(("IN REVIEW", -95.0)),
            Spot::LastBend => Some(("TO TEST", -140.0)),
            Spot::Warmup | Spot::Podium => None,
        }
    }
}

/// Each lane after the first starts a little further on, like a staggered start, so neighbours
/// don't stand on each other's labels.
const STAGGER: f32 = 4.0;

/// The middle of `lane` at `deg` degrees.
pub fn at(lane: usize, deg: f32) -> (f32, f32) {
    let r = lane as f32 * LANE + LANE / 2.0;
    let a = deg.to_radians();
    ((INNER.0 + r) * a.cos(), (INNER.1 + r) * a.sin())
}

/// A point `r` points out from the infield's edge at `deg` degrees.
pub fn ring(r: f32, deg: f32) -> (f32, f32) {
    let a = deg.to_radians();
    ((INNER.0 + r) * a.cos(), (INNER.1 + r) * a.sin())
}

/// The warm-up area, the scoreboard and the podium's three steps on the infield (min and max corners).
pub const WARMUP: ((f32, f32), (f32, f32)) = ((-350.0, -90.0), (-160.0, 60.0));
pub const SCOREBOARD: ((f32, f32), (f32, f32)) = ((-115.0, -90.0), (115.0, 6.0));
/// Second, first and third, left to right.
pub const STEPS: [((f32, f32), (f32, f32)); 3] = [((170.0, 10.0), (214.0, 36.0)), ((214.0, -6.0), (258.0, 36.0)), ((258.0, 20.0), (302.0, 36.0))];

/// Where the n-th athlete of the warm-up area stands: two rows of three.
fn warmup_at(n: usize) -> (f32, f32) {
    let ((x0, _), _) = WARMUP;
    let (col, row) = (n % 3, n / 3);
    (x0 + 40.0 + col as f32 * 62.0 + row as f32 * 18.0, -8.0 + row as f32 * 56.0)
}

/// Where the gold, silver and bronze stand: on top of their step.
fn podium_at(n: usize) -> (f32, f32) {
    let step = [1, 0, 2][n];
    let ((x0, y0), (x1, _)) = STEPS[step];
    ((x0 + x1) / 2.0, y0)
}

/// One ticket on the track.
#[derive(Clone, Debug, PartialEq)]
pub struct Athlete {
    pub id: String,
    pub lane: usize,
    pub spot: Spot,
    /// Its place in its section (0 leads; on the podium, 0 is the gold).
    pub slot: usize,
    /// "#41", or "#41 · 2/4" for a feature with sub-issues.
    pub label: String,
    /// A bug: a hurdle stands just ahead of it.
    pub hurdle: bool,
    /// Its feet, in world points.
    pub pos: (f32, f32),
    /// Where it runs on the lap, in degrees (None off the lap).
    pub deg: Option<f32>,
    /// 1 when it faces right, -1 left.
    pub facing: f32,
    pub seed: u32,
}

/// A section too full for all its athletes: how many more, and where the `+k` pill goes.
#[derive(Clone, Debug, PartialEq)]
pub struct More {
    pub spot: Spot,
    pub lane: usize,
    pub more: usize,
    pub pos: (f32, f32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stadium {
    /// The lanes' names, inside out.
    pub lanes: Vec<String>,
    /// The athletes shown, back to front.
    pub athletes: Vec<Athlete>,
    pub more: Vec<More>,
    /// How many tickets each spot has, in `Spot::ALL` order, shown or not.
    pub counts: [usize; 6],
}

impl Stadium {
    pub fn total(&self) -> usize {
        self.counts.iter().sum()
    }

    pub fn count(&self, s: Spot) -> usize {
        self.counts[Spot::ALL.iter().position(|x| *x == s).unwrap_or(0)]
    }

    /// The outside edge of the last lane.
    pub fn outer(&self) -> (f32, f32) {
        let w = self.lanes.len().max(1) as f32 * LANE;
        (INNER.0 + w, INNER.1 + w)
    }
}

/// The athlete's label: its number, with the sub-issues done of a feature.
pub fn label(it: &Item) -> String {
    let n = it.number.map_or_else(|| "draft".to_string(), |n| format!("#{n}"));
    let feature = it.issue_type.as_ref().is_some_and(|t| t.0.eq_ignore_ascii_case("feature"));
    match it.sub_progress {
        (done, total) if feature && total > 0 => format!("{n} · {done}/{total}"),
        _ => n,
    }
}

fn is_bug(it: &Item) -> bool {
    it.kind == Kind::Issue && it.issue_type.as_ref().is_some_and(|t| t.0.eq_ignore_ascii_case("bug"))
}

/// Lays the tickets out: lanes by zone, the biggest zone inside, then by name.
pub fn layout(items: &[&Item]) -> Stadium {
    let mut by_zone: BTreeMap<String, usize> = BTreeMap::new();
    for it in items {
        *by_zone.entry(city::zone(it)).or_default() += 1;
    }
    let mut zones: Vec<(String, usize)> = by_zone.into_iter().collect();
    zones.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut lanes: Vec<String> = zones.iter().map(|z| z.0.clone()).collect();
    if lanes.len() > MAX_LANES {
        lanes.truncate(MAX_LANES - 1);
        lanes.push("Other zones".into());
    }
    let lane_of = |it: &Item| {
        let z = city::zone(it);
        lanes.iter().position(|l| *l == z).unwrap_or(lanes.len().saturating_sub(1))
    };

    // Each section's tickets: on the lap by lane, by number; the podium the latest finished first.
    let mut sections: BTreeMap<(Spot, usize), Vec<&Item>> = BTreeMap::new();
    let mut counts = [0; 6];
    for it in items {
        let spot = Spot::of(Stage::of(&it.status));
        counts[Spot::ALL.iter().position(|s| *s == spot).unwrap_or(0)] += 1;
        let lane = if spot.arc().is_some() { lane_of(it) } else { 0 };
        sections.entry((spot, lane)).or_default().push(it);
    }
    let mut st = Stadium { lanes: lanes.clone(), counts, ..Stadium::default() };
    for ((spot, lane), mut list) in sections {
        if spot == Spot::Podium {
            list.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.number.cmp(&b.number)));
        } else {
            list.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.id.cmp(&b.id)));
        }
        let room = match spot {
            Spot::Warmup => WARMUP_SLOTS,
            Spot::Podium => PODIUM,
            _ => PER_SECTION,
        };
        let place = |slot: usize| -> ((f32, f32), Option<f32>) {
            match spot.arc() {
                Some((start, step)) => {
                    let deg = start - slot as f32 * step - lane as f32 * STAGGER;
                    (at(lane, deg), Some(deg))
                }
                None if spot == Spot::Warmup => (warmup_at(slot), None),
                None => (podium_at(slot), None),
            }
        };
        for (slot, it) in list.iter().take(room).enumerate() {
            let (pos, deg) = place(slot);
            let facing = match deg {
                Some(d) if d.to_radians().sin() < 0.0 => -1.0,
                _ => 1.0,
            };
            let lane = if spot.arc().is_some() { lane } else { lane_of(it) };
            st.athletes.push(Athlete {
                id: it.id.clone(),
                lane,
                spot,
                slot,
                label: label(it),
                hurdle: is_bug(it) && spot.arc().is_some(),
                pos,
                deg,
                facing,
                seed: city::stable_hash(&it.id),
            });
        }
        // The podium's others are counted on the scoreboard only.
        if list.len() > room && spot != Spot::Podium {
            let pos = match spot {
                Spot::Warmup => (WARMUP.1 .0 - 24.0, WARMUP.1 .1 - 12.0),
                _ => place(room).0,
            };
            st.more.push(More { spot, lane, more: list.len() - room, pos });
        }
    }
    st.athletes.sort_by(|a, b| a.pos.1.total_cmp(&b.pos.1));
    st
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Label;

    fn ticket(n: u64, zone: &str, status: &str, ty: &str) -> Item {
        Item {
            id: format!("i{n}"),
            number: Some(n),
            status: status.into(),
            kind: Kind::Issue,
            issue_type: Some((ty.into(), String::new())),
            labels: vec![Label { name: format!("zone:{zone}"), color: String::new() }],
            updated: n as i64,
            ..Default::default()
        }
    }

    fn of(items: &[Item]) -> Stadium {
        layout(&items.iter().collect::<Vec<_>>())
    }

    #[test]
    fn one_lane_per_zone_the_biggest_inside() {
        let s = of(&[ticket(1, "graphe", "Ready", "Task"), ticket(2, "github", "Ready", "Task"), ticket(3, "github", "Backlog", "Task")]);
        assert_eq!(s.lanes, ["github", "graphe"]);
        let a = s.athletes.iter().find(|a| a.id == "i1").unwrap();
        assert_eq!((a.lane, a.spot), (1, Spot::Blocks));
        assert_eq!(s.outer(), (INNER.0 + 48.0, INNER.1 + 48.0));
    }

    #[test]
    fn zones_past_the_seventh_share_the_last_lane() {
        let items: Vec<Item> = (0..11).map(|i| ticket(i, &format!("z{i:02}"), "In progress", "Task")).collect();
        let s = of(&items);
        assert_eq!(s.lanes.len(), MAX_LANES);
        assert_eq!(s.lanes.last().unwrap(), "Other zones");
        let last: Vec<_> = s.athletes.iter().filter(|a| a.lane == MAX_LANES - 1).collect();
        assert_eq!(last.len(), PER_SECTION, "4 zones in the last lane, 3 shown");
        assert_eq!(s.more, vec![More { spot: Spot::Bend, lane: MAX_LANES - 1, more: 1, pos: s.more[0].pos }]);
    }

    #[test]
    fn each_status_has_its_place_on_the_lap() {
        let s = of(&[
            ticket(1, "a", "Backlog", "Task"),
            ticket(2, "a", "Ready", "Task"),
            ticket(3, "a", "In progress", "Task"),
            ticket(4, "a", "In review", "Task"),
            ticket(5, "a", "To test", "Task"),
            ticket(6, "a", "Done", "Task"),
        ]);
        let spot = |id: &str| s.athletes.iter().find(|a| a.id == id).unwrap().spot;
        assert_eq!([spot("i1"), spot("i2"), spot("i3"), spot("i4"), spot("i5"), spot("i6")], Spot::ALL);
        assert_eq!(s.counts, [1; 6]);
        // In progress on the bottom right, in review along the top, to test on the left.
        let pos = |id: &str| s.athletes.iter().find(|a| a.id == id).unwrap().pos;
        assert!(pos("i3").0 > 0.0 && pos("i3").1 > 0.0);
        assert!(pos("i4").1 < -INNER.1);
        let (x, y) = pos("i5");
        assert!(x < -0.8 * INNER.0 && (x / INNER.0).powi(2) + (y / INNER.1).powi(2) > 1.0, "on the left bend, on the track");
        // The warm-up and the podium are on the infield.
        for id in ["i1", "i6"] {
            let (x, y) = pos(id);
            assert!((x / INNER.0).powi(2) + (y / INNER.1).powi(2) < 1.0, "{id} at {x},{y}");
        }
        // Running on the top straight faces left, on the bottom one right.
        assert_eq!(s.athletes.iter().find(|a| a.id == "i4").unwrap().facing, -1.0);
        assert_eq!(s.athletes.iter().find(|a| a.id == "i2").unwrap().facing, 1.0);
    }

    #[test]
    fn a_full_section_shows_what_fits_and_counts_the_rest() {
        let items: Vec<Item> = (1..=5).map(|i| ticket(i, "a", "In review", "Task")).chain((10..20).map(|i| ticket(i, "a", "Backlog", "Task"))).collect();
        let s = of(&items);
        assert_eq!(s.athletes.iter().filter(|a| a.spot == Spot::Back).count(), PER_SECTION);
        assert_eq!(s.athletes.iter().filter(|a| a.spot == Spot::Warmup).count(), WARMUP_SLOTS);
        let more = |spot: Spot| s.more.iter().find(|m| m.spot == spot).map(|m| m.more);
        assert_eq!(more(Spot::Back), Some(2));
        assert_eq!(more(Spot::Warmup), Some(4));
        assert_eq!(s.count(Spot::Back), 5);
        // The places of a section are apart, the first leading.
        let xs: Vec<f32> = s.athletes.iter().filter(|a| a.spot == Spot::Back).map(|a| a.pos.0).collect();
        assert!(xs.windows(2).all(|w| (w[0] - w[1]).abs() > 40.0));
    }

    #[test]
    fn the_podium_holds_the_three_latest_finished() {
        let s = of(&(1..=5).map(|i| ticket(i, "a", "Done", "Task")).collect::<Vec<_>>());
        let podium: Vec<_> = s.athletes.iter().filter(|a| a.spot == Spot::Podium).collect();
        assert_eq!(podium.len(), PODIUM);
        let gold = podium.iter().find(|a| a.slot == 0).unwrap();
        assert_eq!(gold.id, "i5");
        assert!(podium.iter().all(|a| a.pos.1 >= gold.pos.1), "the gold stands highest");
        assert!(s.more.is_empty(), "the others are on the scoreboard");
        assert_eq!((s.count(Spot::Podium), s.total()), (5, 5));
    }

    #[test]
    fn a_bug_has_a_hurdle_and_a_feature_its_relay_legs() {
        let mut f = ticket(41, "a", "In review", "Feature");
        f.sub_progress = (2, 4);
        let s = of(&[f, ticket(44, "a", "In progress", "Bug"), ticket(55, "a", "Backlog", "Bug")]);
        let a = |id: &str| s.athletes.iter().find(|a| a.id == id).unwrap().clone();
        assert_eq!(a("i41").label, "#41 · 2/4");
        assert!(!a("i41").hurdle);
        assert!(a("i44").hurdle);
        assert!(!a("i55").hurdle, "no hurdle in the warm-up area");
        assert_eq!(a("i44").label, "#44");
    }
}
