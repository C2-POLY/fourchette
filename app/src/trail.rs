//! The Ultra-trail skin's mountain (#207): the board's City view as one long climb of 160 km,
//! from the registration tent in the valley to the summit. One runner per ticket on the trail,
//! placed by its status (`city::Stage`): Backlog at the tent before the start, Ready under the
//! start arch, In progress on the climb, In review at the aid station of km 92, To test on the
//! ridge, and Done under the summit's arch. Pure layout in world points, y down: x is
//! `KM` points per km from the start line, y the ridge's height above the valley (`HEIGHT` at the
//! summit, negative). `app/trail.rs` draws it.

use crate::city::{self, Stage};
use crate::github::Item;
use crate::track;
use std::collections::BTreeMap;

/// The course's length in km.
pub const LENGTH_KM: f32 = 160.0;
/// World points per km along the course, and from the valley to the summit.
pub const KM: f32 = 5.6;
pub const HEIGHT: f32 = 430.0;
/// The valley's and the summit's altitude, in metres.
pub const VALLEY_M: f32 = 1040.0;
pub const RISE_M: f32 = 2080.0;
/// The aid station's km.
pub const AID_KM: f32 = 92.0;

/// The mountain's ridge, km → 0 (valley) … 1 (summit): the validated mockup's "one long climb".
const PROFILE: [(f32, f32); 22] = [
    (0.0, 0.04),
    (6.0, 0.05),
    (12.0, 0.13),
    (20.0, 0.22),
    (28.0, 0.30),
    (36.0, 0.33),
    (44.0, 0.40),
    (52.0, 0.47),
    (60.0, 0.45),
    (68.0, 0.52),
    (76.0, 0.58),
    (84.0, 0.60),
    (92.0, 0.60),
    (100.0, 0.61),
    (108.0, 0.70),
    (116.0, 0.78),
    (124.0, 0.76),
    (132.0, 0.80),
    (140.0, 0.78),
    (148.0, 0.84),
    (154.0, 0.90),
    (160.0, 1.0),
];

/// The ridge's height at `km`, from 0 (valley) to 1 (summit), eased between the profile's points.
/// Flat before the start, the summit after the finish.
pub fn profile(km: f32) -> f32 {
    if km <= 0.0 {
        return PROFILE[0].1;
    }
    if km >= LENGTH_KM {
        return 1.0;
    }
    for w in PROFILE.windows(2) {
        let ((k0, h0), (k1, h1)) = (w[0], w[1]);
        if km <= k1 {
            let u = (km - k0) / (k1 - k0);
            let s = (1.0 - (std::f32::consts::PI * u).cos()) / 2.0;
            return h0 + (h1 - h0) * s;
        }
    }
    1.0
}

/// The altitude at `km`, in metres.
pub fn altitude(km: f32) -> f32 {
    VALLEY_M + profile(km) * RISE_M
}

/// The climbing from the start to `km`, in metres (D+), by steps of half a km.
pub fn climb(km: f32) -> f32 {
    let mut d = 0.0;
    let mut k = 0.5;
    while k <= km + 1e-3 {
        d += (altitude(k) - altitude(k - 0.5)).max(0.0);
        k += 0.5;
    }
    d
}

/// The trail at `km`, in world points.
pub fn ground(km: f32) -> (f32, f32) {
    (km * KM, -profile(km) * HEIGHT)
}

/// Where a runner stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Spot {
    /// Backlog: the registration tent in the valley, before the start.
    Tent,
    /// Ready: under the start arch.
    Start,
    /// In progress: the long climb.
    Climb,
    /// In review: the aid station of km 92.
    Aid,
    /// To test: the ridge.
    Ridge,
    /// Done: under the summit's arch.
    Summit,
}

impl Spot {
    pub const ALL: [Spot; 6] = [Spot::Tent, Spot::Start, Spot::Climb, Spot::Aid, Spot::Ridge, Spot::Summit];

    pub fn of(stage: Stage) -> Spot {
        match stage {
            Stage::Lot => Spot::Tent,
            Stage::Staked => Spot::Start,
            Stage::Foundations => Spot::Climb,
            Stage::Frame => Spot::Aid,
            Stage::Testing => Spot::Ridge,
            Stage::Built => Spot::Summit,
        }
    }

    /// Its stretch of the course, in km (the tent's is before the start).
    pub fn range(self) -> (f32, f32) {
        match self {
            Spot::Tent => (-15.0, -6.0),
            Spot::Start => (0.0, 6.0),
            Spot::Climb => (6.0, 84.0),
            Spot::Aid => (84.0, 102.0),
            Spot::Ridge => (102.0, 154.0),
            Spot::Summit => (154.0, LENGTH_KM),
        }
    }

    /// How many runners it shows; the others are a `+k` pill.
    pub fn room(self) -> usize {
        match self {
            Spot::Tent | Spot::Aid | Spot::Summit => 3,
            Spot::Start => 2,
            Spot::Climb => 7,
            Spot::Ridge => 5,
        }
    }

    /// On the trail, running: the legs move, each has its own label, a bug a rock ahead.
    pub fn running(self) -> bool {
        matches!(self, Spot::Climb | Spot::Aid | Spot::Ridge)
    }

    /// The place's name and the status it stands for, as the mountain's signs say.
    pub fn sign(self) -> (&'static str, &'static str) {
        match self {
            Spot::Tent => ("Registration", "Backlog"),
            Spot::Start => ("Start", "Ready"),
            Spot::Climb => ("The climb", "In progress"),
            Spot::Aid => ("Aid station", "In review"),
            Spot::Ridge => ("Ridge", "To test"),
            Spot::Summit => ("Summit", "Done"),
        }
    }
}

/// One ticket on the mountain.
#[derive(Clone, Debug, PartialEq)]
pub struct Runner {
    pub id: String,
    /// Its zone's index in `Mountain::zones`, for its singlet's colour.
    pub zone: usize,
    pub spot: Spot,
    /// Its place in its spot (0 leads; at the summit, 0 is the latest finished).
    pub slot: usize,
    /// Its km on the course (before 0 at the tent).
    pub km: f32,
    /// Its feet, in world points.
    pub pos: (f32, f32),
    /// "#41", or "#41 · 2/4" for a feature with sub-issues.
    pub label: String,
    /// Its label stands higher, so it clears its neighbour's.
    pub high: bool,
    /// A bug on the trail: a rock lies just ahead of it.
    pub rock: bool,
    pub seed: u32,
}

/// A spot too full for all its runners: how many more, and where the `+k` pill goes.
#[derive(Clone, Debug, PartialEq)]
pub struct More {
    pub spot: Spot,
    pub more: usize,
    pub pos: (f32, f32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mountain {
    /// The zones, the biggest first: a runner's singlet is its zone's colour.
    pub zones: Vec<String>,
    /// The runners shown, back to front.
    pub runners: Vec<Runner>,
    pub more: Vec<More>,
    /// How many tickets each spot has, in `Spot::ALL` order, shown or not.
    pub counts: [usize; 6],
}

impl Mountain {
    pub fn total(&self) -> usize {
        self.counts.iter().sum()
    }

    pub fn count(&self, s: Spot) -> usize {
        self.counts[Spot::ALL.iter().position(|x| *x == s).unwrap_or(0)]
    }

    /// The runners of a spot, leader first.
    pub fn at(&self, s: Spot) -> Vec<&Runner> {
        let mut v: Vec<&Runner> = self.runners.iter().filter(|r| r.spot == s).collect();
        v.sort_by_key(|r| r.slot);
        v
    }
}

/// The most zones with a colour of their own; the others share the last one.
pub const MAX_ZONES: usize = 8;

/// The km of the `slot`-th of `n` runners in `spot` (0 leads), and how far its feet stand below
/// the trail.
fn place(spot: Spot, slot: usize, n: usize) -> (f32, f32) {
    let (a, b) = spot.range();
    match spot {
        // Before the tent, on the valley floor, the first nearest the start.
        Spot::Tent => (-7.5 - slot as f32 * 3.6, 17.0 + (slot % 2) as f32 * 6.0),
        // Around the aid station, 6 km apart, the leader furthest on.
        Spot::Aid => {
            let k = AID_KM + ((n as f32 - 1.0) / 2.0 - slot as f32) * 6.0;
            (k.clamp(a + 1.0, b - 1.0), 2.0)
        }
        // Under the summit's arch: the latest in the middle, the others on each side, lower.
        Spot::Summit => match slot {
            0 => (LENGTH_KM, 2.0),
            1 => (LENGTH_KM - 3.2, 2.0 + 29.0 - (profile(LENGTH_KM) - profile(LENGTH_KM - 3.2)) * HEIGHT),
            _ => (LENGTH_KM + 3.2, 31.0),
        },
        // Spread along the stretch, the leader furthest on.
        _ => (a + (b - a) * (n - slot) as f32 / (n as f32 + 0.5) - (b - a) * 0.25 / (n as f32 + 0.5), 2.0),
    }
}

fn is_bug(it: &Item) -> bool {
    it.kind == crate::github::Kind::Issue && it.issue_type.as_ref().is_some_and(|t| t.0.eq_ignore_ascii_case("bug"))
}

/// Lays the tickets out on the mountain, each spot by number (the lowest leads), the summit the
/// latest finished first.
pub fn layout(items: &[&Item]) -> Mountain {
    let mut by_zone: BTreeMap<String, usize> = BTreeMap::new();
    for it in items {
        *by_zone.entry(city::zone(it)).or_default() += 1;
    }
    let mut zones: Vec<(String, usize)> = by_zone.into_iter().collect();
    zones.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut zones: Vec<String> = zones.into_iter().map(|z| z.0).collect();
    if zones.len() > MAX_ZONES {
        zones.truncate(MAX_ZONES - 1);
        zones.push("Other zones".into());
    }
    let zone_of = |it: &Item| {
        let z = city::zone(it);
        zones.iter().position(|l| *l == z).unwrap_or(zones.len().saturating_sub(1))
    };

    let mut spots: BTreeMap<Spot, Vec<&Item>> = BTreeMap::new();
    let mut counts = [0; 6];
    for it in items {
        let spot = Spot::of(Stage::of(&it.status));
        counts[Spot::ALL.iter().position(|s| *s == spot).unwrap_or(0)] += 1;
        spots.entry(spot).or_default().push(it);
    }
    let mut m = Mountain { zones: zones.clone(), counts, ..Mountain::default() };
    for (spot, mut list) in spots {
        if spot == Spot::Summit {
            list.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.number.cmp(&b.number)));
        } else {
            list.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.id.cmp(&b.id)));
        }
        let n = list.len().min(spot.room());
        for (slot, it) in list.iter().take(n).enumerate() {
            let (km, below) = place(spot, slot, n);
            let (x, y) = ground(km.max(0.0));
            let x = if km < 0.0 { km * KM } else { x };
            m.runners.push(Runner {
                id: it.id.clone(),
                zone: zone_of(it),
                spot,
                slot,
                km,
                pos: (x, y + below),
                label: track::label(it),
                high: spot.running() && slot % 2 == 1,
                rock: spot.running() && is_bug(it),
                seed: city::stable_hash(&it.id),
            });
        }
        if list.len() > n {
            // Behind the last one shown, on the trail; at the tent and the summit, beside the group.
            let pos = match spot {
                Spot::Tent => ((-15.0 - 3.6) * KM, -profile(0.0) * HEIGHT + 17.0),
                Spot::Summit => (ground(LENGTH_KM).0 + 6.0 * KM, -HEIGHT + 34.0),
                _ => {
                    let k = (place(spot, n - 1, n).0 - 4.0).max(spot.range().0);
                    let (x, y) = ground(k);
                    (x, y + 2.0)
                }
            };
            m.more.push(More { spot, more: list.len() - n, pos });
        }
    }
    m.runners.sort_by(|a, b| a.pos.1.total_cmp(&b.pos.1));
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Kind, Label};

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

    fn of(items: &[Item]) -> Mountain {
        layout(&items.iter().collect::<Vec<_>>())
    }

    #[test]
    fn the_profile_starts_low_and_climbs_to_the_summit() {
        assert!(profile(0.0) < 0.1);
        assert_eq!(profile(-10.0), profile(0.0), "flat in the valley");
        assert_eq!(profile(LENGTH_KM), 1.0);
        assert_eq!(profile(200.0), 1.0);
        // It passes through the mockup's points.
        assert!((profile(92.0) - 0.60).abs() < 1e-4);
        assert!((profile(154.0) - 0.90).abs() < 1e-4);
        // The summit is the highest point, the start the lowest.
        let heights: Vec<f32> = (0..=320).map(|i| profile(i as f32 / 2.0)).collect();
        assert!(heights.iter().all(|h| *h >= profile(0.0) && *h <= 1.0));
        // Monotone between the profile's points: the cosine easing never overshoots them.
        for w in PROFILE.windows(2) {
            let (lo, hi) = (w[0].1.min(w[1].1), w[0].1.max(w[1].1));
            for i in 0..=20 {
                let h = profile(w[0].0 + (w[1].0 - w[0].0) * i as f32 / 20.0);
                assert!(h >= lo - 1e-5 && h <= hi + 1e-5);
            }
        }
        // Rising overall: each 20 km higher than the 20 before, but for the flat aid station.
        assert!(profile(40.0) > profile(20.0) && profile(130.0) > profile(100.0));
        assert_eq!(altitude(LENGTH_KM), VALLEY_M + RISE_M);
        assert!(climb(LENGTH_KM) >= RISE_M * 0.95 && climb(46.0) > 0.0 && climb(46.0) < climb(LENGTH_KM));
    }

    #[test]
    fn each_status_has_its_stretch_of_the_course() {
        let s = of(&[
            ticket(1, "a", "Backlog", "Task"),
            ticket(2, "a", "Ready", "Task"),
            ticket(3, "a", "In progress", "Task"),
            ticket(4, "a", "In review", "Task"),
            ticket(5, "a", "To test", "Task"),
            ticket(6, "a", "Done", "Task"),
        ]);
        let r = |id: &str| s.runners.iter().find(|a| a.id == id).unwrap().clone();
        assert_eq!([r("i1").spot, r("i2").spot, r("i3").spot, r("i4").spot, r("i5").spot, r("i6").spot], Spot::ALL);
        assert_eq!(s.counts, [1; 6]);
        assert!(r("i1").km < 0.0, "the tent is before the start");
        for (id, spot) in [("i2", Spot::Start), ("i3", Spot::Climb), ("i4", Spot::Aid), ("i5", Spot::Ridge), ("i6", Spot::Summit)] {
            let (a, b) = spot.range();
            assert!(r(id).km >= a && r(id).km <= b, "{id} at km {}", r(id).km);
        }
        assert!((r("i4").km - AID_KM).abs() < 1e-4, "alone at the aid station");
        assert_eq!(r("i6").km, LENGTH_KM);
        // Higher up the further on.
        assert!(r("i5").pos.1 < r("i3").pos.1 && r("i6").pos.1 < r("i5").pos.1);
    }

    #[test]
    fn a_spot_spreads_its_runners_the_lowest_number_leading() {
        let items: Vec<Item> = (1..=4).map(|i| ticket(i, "a", "In progress", "Task")).collect();
        let s = of(&items);
        let climb = s.at(Spot::Climb);
        assert_eq!(climb.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["i1", "i2", "i3", "i4"]);
        let kms: Vec<f32> = climb.iter().map(|r| r.km).collect();
        assert!(kms.windows(2).all(|w| w[0] - w[1] > 10.0), "{kms:?}");
        assert!(kms.iter().all(|k| (6.0..=84.0).contains(k)));
        // Neighbours' labels at two heights.
        assert_eq!(climb.iter().map(|r| r.high).collect::<Vec<_>>(), [false, true, false, true]);
    }

    #[test]
    fn a_full_spot_shows_what_fits_and_counts_the_rest() {
        let items: Vec<Item> = (1..=5).map(|i| ticket(i, "a", "In review", "Task")).chain((10..20).map(|i| ticket(i, "a", "Backlog", "Task"))).collect();
        let s = of(&items);
        assert_eq!(s.at(Spot::Aid).len(), Spot::Aid.room());
        assert_eq!(s.at(Spot::Tent).len(), Spot::Tent.room());
        let more = |spot: Spot| s.more.iter().find(|m| m.spot == spot).map(|m| m.more);
        assert_eq!(more(Spot::Aid), Some(2));
        assert_eq!(more(Spot::Tent), Some(7));
        assert_eq!((s.count(Spot::Aid), s.total()), (5, 15));
        // The aid station's three stand around km 92, inside their stretch.
        let kms: Vec<f32> = s.at(Spot::Aid).iter().map(|r| r.km).collect();
        assert_eq!(kms, [98.0, 92.0, 86.0]);
    }

    #[test]
    fn the_summit_holds_the_three_latest_finished() {
        let s = of(&(1..=5).map(|i| ticket(i, "a", "Done", "Task")).collect::<Vec<_>>());
        let top = s.at(Spot::Summit);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].id, "i5");
        assert!(top.iter().all(|r| r.pos.1 >= top[0].pos.1), "the latest stands highest");
        assert_eq!(s.more[0].more, 2);
    }

    #[test]
    fn a_bug_has_a_rock_and_a_feature_its_checkpoints() {
        let mut f = ticket(41, "a", "In review", "Feature");
        f.sub_progress = (2, 4);
        let s = of(&[f, ticket(44, "b", "In progress", "Bug"), ticket(55, "a", "Backlog", "Bug")]);
        let r = |id: &str| s.runners.iter().find(|a| a.id == id).unwrap().clone();
        assert_eq!(r("i41").label, "#41 · 2/4");
        assert!(!r("i41").rock);
        assert!(r("i44").rock);
        assert!(!r("i55").rock, "no rock at the tent");
        assert_eq!(s.zones, ["a", "b"]);
        assert_eq!((r("i41").zone, r("i44").zone), (0, 1));
    }
}
