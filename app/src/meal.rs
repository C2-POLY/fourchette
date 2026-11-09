//! The Big eater skin's daily menu (#212): each Git action is a bite (a commit 50 kcal, a push
//! 250, a merge 400) and Undo spits 50 back out. Bites that follow each other within a minute
//! make a combo: the third counts double, the fourth and the next ones triple. An operation
//! stopped on conflicts is an indigestion that pauses the menu until the next bite. Five stops
//! on the way, each announced once, the last the Golden Fork at 3,000 kcal. Kept in memory only:
//! it starts again at 0 at each launch. `app/meal.rs` draws the bar, the toolbar pill, the
//! announcer's toasts and the Golden Fork card.

use crate::course::Stride;

/// The day's goal: the Golden Fork.
pub const GOAL: u32 = 3000;
/// A bite within this many seconds of the one before keeps the combo going.
pub const COMBO_SECS: u64 = 60;
/// The stops on the way, in kcal, the last one the goal.
pub const STOPS: [(u32, &str); 5] = [(500, "Starter"), (1200, "Main"), (1800, "Cheese"), (2400, "Dessert"), (GOAL, "Golden fork")];

/// What a stride did to the menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bite {
    /// The kcal actually gained or lost (Undo at 0 loses none, an indigestion none).
    pub kcal: i32,
    /// The combo it counted with: 1, 2 or 3.
    pub combo: u32,
    /// The stop reached for the first time this launch, if any (the furthest one; never the
    /// Golden fork, which is `golden`).
    pub stop: Option<usize>,
    /// The Golden Fork has just been reached.
    pub golden: bool,
    /// This bite ended an indigestion.
    pub cured: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meal {
    pub kcal: u32,
    /// Bites by kind: commits are chips, pushes burgers, merges pizzas.
    pub chips: u32,
    pub burgers: u32,
    pub pizzas: u32,
    /// The best combo of the day.
    pub best: u32,
    /// When the last bite was taken, in seconds since launch.
    last: Option<u64>,
    /// How many bites in a row, each within a minute of the one before.
    chain: u32,
    /// Stopped on conflicts: nothing counts until the next bite.
    indigestion: bool,
    /// How many of `STOPS` have been announced.
    stops_seen: usize,
    /// The Golden Fork is reached: the card shows once.
    pub golden: bool,
}

/// The combo a chain of `n` bites makes.
fn multiplier(n: u32) -> u32 {
    match n {
        0..=2 => 1,
        3 => 2,
        _ => 3,
    }
}

impl Meal {
    /// Counts one stride taken `secs` seconds after launch.
    pub fn bite(&mut self, s: &Stride, secs: u64) -> Bite {
        let mut out = Bite { combo: 1, ..Bite::default() };
        let base: u32 = match s {
            Stride::Commit => 50,
            Stride::Push => 250,
            Stride::Merge => 400,
            Stride::Undo => {
                let lost = self.kcal.min(50);
                self.kcal -= lost;
                self.chain = 0;
                out.kcal = -(lost as i32);
                return out;
            }
            Stride::Cramp { .. } => {
                self.indigestion = true;
                self.chain = 0;
                return out;
            }
        };
        match s {
            Stride::Commit => self.chips += 1,
            Stride::Push => self.burgers += 1,
            _ => self.pizzas += 1,
        }
        out.cured = std::mem::take(&mut self.indigestion);
        let close = self.last.is_some_and(|t| secs.saturating_sub(t) < COMBO_SECS);
        self.chain = if close && self.chain > 0 { self.chain + 1 } else { 1 };
        self.last = Some(secs);
        out.combo = multiplier(self.chain);
        self.best = self.best.max(out.combo);
        let gain = base * out.combo;
        self.kcal += gain;
        out.kcal = gain as i32;
        let reached = STOPS.iter().take_while(|(k, _)| *k <= self.kcal).count();
        if reached > self.stops_seen {
            self.stops_seen = reached;
            if reached < STOPS.len() {
                out.stop = Some(reached - 1);
            }
        }
        if self.kcal >= GOAL && !self.golden {
            self.golden = true;
            out.golden = true;
        }
        out
    }

    /// Stopped on conflicts: the bar waits for the next bite.
    pub fn sick(&self) -> bool {
        self.indigestion
    }

    /// The combo the next bite counts with if taken at `secs`: 1 when the chain is broken.
    pub fn streak(&self, secs: u64) -> u32 {
        match self.combo_left(secs) {
            Some(_) => multiplier(self.chain + 1),
            None => 1,
        }
    }

    /// The combo running at `secs` (×2, ×3), the one the last bite made: 1 without one.
    pub fn combo(&self, secs: u64) -> u32 {
        if self.combo_left(secs).is_some() { multiplier(self.chain) } else { 1 }
    }

    /// Seconds left to keep the chain going, while one runs.
    pub fn combo_left(&self, secs: u64) -> Option<u64> {
        let last = self.last?;
        let gone = secs.saturating_sub(last);
        (self.chain > 0 && !self.indigestion && gone < COMBO_SECS).then(|| COMBO_SECS - gone)
    }

    /// The next stop ahead, or None past the Golden Fork.
    pub fn next_stop(&self) -> Option<(u32, &'static str)> {
        STOPS.iter().copied().find(|(k, _)| *k > self.kcal)
    }

    /// "Second helping": back to an empty plate.
    pub fn restart(&mut self) {
        *self = Meal::default();
    }

    /// "11 chips · 1 burger · 1 pizza".
    pub fn tally(&self) -> String {
        let n = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        format!("{} · {} · {}", n(self.chips, "chips", "chips"), n(self.burgers, "burger", "burgers"), n(self.pizzas, "pizza", "pizzas"))
    }

    /// A menu at `kcal` for the screenshots, at `secs`: below the goal with the combo ×3
    /// running since a bite 18 seconds before, at the goal finished a while ago.
    pub fn sample(kcal: u32, secs: u64) -> Meal {
        let golden = kcal >= GOAL;
        let (chips, burgers, pizzas) = if golden { (28, 4, 2) } else { (11, 1, 1) };
        let stops_seen = STOPS.iter().filter(|(k, _)| *k <= kcal).count();
        let last = secs.saturating_sub(if golden { 600 } else { 18 });
        Meal { kcal, chips, burgers, pizzas, best: 3, last: Some(last), chain: 4, indigestion: false, stops_seen, golden }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_action_is_a_bite_of_its_kcal() {
        let mut m = Meal::default();
        // Far apart: no combo.
        assert_eq!(m.bite(&Stride::Commit, 0).kcal, 50);
        assert_eq!(m.bite(&Stride::Push, 100).kcal, 250);
        assert_eq!(m.bite(&Stride::Merge, 200).kcal, 400);
        assert_eq!(m.kcal, 700);
        assert_eq!((m.chips, m.burgers, m.pizzas), (1, 1, 1));
        assert_eq!(m.tally(), "1 chips · 1 burger · 1 pizza");
    }

    #[test]
    fn bites_within_a_minute_make_a_combo() {
        let mut m = Meal::default();
        assert_eq!(m.bite(&Stride::Commit, 0).combo, 1);
        assert_eq!(m.bite(&Stride::Commit, 30).combo, 1);
        assert_eq!(m.streak(40), 2, "the third bite would count double");
        let third = m.bite(&Stride::Commit, 80);
        assert_eq!((third.combo, third.kcal), (2, 100));
        let fourth = m.bite(&Stride::Push, 139);
        assert_eq!((fourth.combo, fourth.kcal), (3, 750));
        assert_eq!(m.bite(&Stride::Commit, 150).combo, 3);
        assert_eq!(m.combo(160), 3);
        assert_eq!(m.combo_left(160), Some(50));
        // A minute without a bite breaks it.
        assert_eq!((m.combo(210), m.streak(210)), (1, 1));
        assert_eq!(m.bite(&Stride::Commit, 210).combo, 1);
        assert_eq!(m.best, 3);
    }

    #[test]
    fn undo_spits_50_kcal_and_never_goes_below_0() {
        let mut m = Meal::default();
        m.bite(&Stride::Commit, 0);
        m.bite(&Stride::Commit, 10);
        assert_eq!(m.bite(&Stride::Undo, 20).kcal, -50);
        assert_eq!(m.kcal, 50);
        m.bite(&Stride::Undo, 30);
        assert_eq!(m.bite(&Stride::Undo, 40), Bite { combo: 1, ..Bite::default() });
        assert_eq!(m.kcal, 0);
        // Spitting breaks the combo.
        m.bite(&Stride::Commit, 50);
        m.bite(&Stride::Commit, 55);
        m.bite(&Stride::Undo, 56);
        assert_eq!(m.bite(&Stride::Commit, 57).combo, 1);
    }

    #[test]
    fn an_indigestion_pauses_until_the_next_bite() {
        let mut m = Meal::default();
        m.bite(&Stride::Commit, 0);
        m.bite(&Stride::Commit, 10);
        let cramp = m.bite(&Stride::Cramp { op: "rebase".into(), conflicts: 2 }, 20);
        assert_eq!(cramp.kcal, 0);
        assert!(m.sick());
        assert_eq!(m.combo_left(25), None, "no combo while sick");
        let next = m.bite(&Stride::Commit, 30);
        assert!(next.cured && !m.sick());
        assert_eq!((next.combo, next.kcal), (1, 50), "the combo starts again");
        assert!(!m.bite(&Stride::Commit, 35).cured);
    }

    #[test]
    fn each_stop_is_announced_once() {
        let mut m = Meal { kcal: 450, ..Meal::default() };
        assert_eq!(m.bite(&Stride::Commit, 0).stop, Some(0));
        assert_eq!(m.bite(&Stride::Undo, 100).stop, None);
        assert_eq!(m.bite(&Stride::Commit, 200).stop, None, "back at the Starter a second time");
        assert_eq!(m.next_stop(), Some((1200, "Main")));
        // A big bite past two stops announces the furthest.
        let mut m = Meal { kcal: 1150, stops_seen: 1, ..Meal::default() };
        m.bite(&Stride::Merge, 0);
        m.bite(&Stride::Merge, 300);
        assert_eq!(m.kcal, 1950);
        let mut m2 = Meal { kcal: 1150, stops_seen: 1, chain: 3, last: Some(0), ..Meal::default() };
        assert_eq!(m2.bite(&Stride::Push, 10).stop, Some(2), "1,150 + 750: Main and Cheese, Cheese said");
    }

    #[test]
    fn the_golden_fork_shows_once() {
        let mut m = Meal { kcal: 2700, stops_seen: 4, ..Meal::default() };
        let b = m.bite(&Stride::Merge, 0);
        assert!(b.golden);
        assert_eq!(b.stop, None, "the Golden Fork is no stop");
        assert_eq!(m.next_stop(), None);
        assert!(!m.bite(&Stride::Merge, 100).golden);
        assert_eq!(m.kcal, 3500);
        m.restart();
        assert_eq!(m, Meal::default());
    }

    #[test]
    fn the_samples_show_what_the_screenshots_need() {
        let m = Meal::sample(1240, 600);
        assert_eq!((m.kcal, m.combo(600), m.combo_left(600)), (1240, 3, Some(42)));
        assert_eq!(m.next_stop(), Some((1800, "Cheese")));
        let done = Meal::sample(3000, 900);
        assert!(done.golden && done.combo(900) == 1);
    }
}
