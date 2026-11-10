//! The Gamer skin's session (#218): each Git action earns XP (a commit 100, a push 300, a merge
//! 500) and Undo takes 100 back, never below 0. Actions that follow each other within a minute
//! make a streak: the third counts double, the fourth and the next ones triple. An operation
//! stopped on conflicts opens a boss fight: nothing counts until the next action, which beats the
//! boss (+200) and counts as usual. Five levels, each announced once, the last, Legend, opening
//! the GG card once. The room gets messier as the session goes: a can per push, a pizza box per
//! merge, a crisp bag every 5 commits, a spill when a boss fight starts; Undo takes the last piece
//! back. Kept in memory only: it starts again at 0, in a tidy room, at each launch. `app/xp.rs`
//! draws the bar, the toolbar pill, the toasts and the GG card; `app/room.rs` the room.

use crate::course::Stride;

/// The levels, from the XP each starts at; the last is Legend.
pub const LEVELS: [(u32, &str); 5] = [(0, "Noob"), (1000, "Rookie"), (2500, "Pro"), (4000, "Elite"), (LEGEND, "Legend")];
/// Legend: the GG card.
pub const LEGEND: u32 = 6000;
/// An action within this many seconds of the one before keeps the streak going.
pub const STREAK_SECS: u64 = 60;
/// What beating a boss earns, on top of the action that beats it.
pub const BOSS_XP: u32 = 200;
/// A crisp bag lands every this many commits.
pub const COMMITS_PER_BAG: u32 = 5;

/// A piece of the room's mess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    /// An energy drink can: a push.
    Can,
    /// A pizza box: a merge.
    Pizza,
    /// A crisp bag: every 5 commits.
    Bag,
    /// A lime spill: a boss fight started.
    Spill,
}

/// The operation stopped on conflicts that the boss stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boss {
    pub op: String,
    pub conflicts: usize,
}

/// What an action did to the session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Play {
    /// The XP actually gained or lost (Undo at 0 loses none, a boss fight none).
    pub xp: i32,
    /// The streak it counted with: 1, 2 or 3.
    pub streak: u32,
    /// The level reached for the first time this session, if any (the furthest one; never
    /// Legend, which is `legend`), as an index of `LEVELS`.
    pub level: Option<usize>,
    /// Legend has just been reached: the GG card.
    pub legend: bool,
    /// A boss fight has just started.
    pub boss: bool,
    /// This action beat the boss.
    pub beaten: bool,
    /// The first commit, push or merge of the session.
    pub first: bool,
    /// The piece of mess it left in the room.
    pub piece: Option<Piece>,
    /// The piece Undo took back.
    pub taken: Option<Piece>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    pub xp: u32,
    pub commits: u32,
    pub pushes: u32,
    pub merges: u32,
    /// The longest chain of the session: actions in a row, each within a minute of the one before.
    pub best: u32,
    /// Bosses beaten.
    pub bosses: u32,
    /// The boss fight going on.
    pub boss: Option<Boss>,
    /// The mess in the room, oldest first.
    pub mess: Vec<Piece>,
    /// When the last action was made, in seconds since launch.
    last: Option<u64>,
    /// How many actions in a row, each within a minute of the one before.
    chain: u32,
    /// The furthest level already announced.
    level_seen: usize,
    /// Legend is reached: the GG card shows once.
    pub legend: bool,
}

/// The streak a chain of `n` actions makes.
fn multiplier(n: u32) -> u32 {
    match n {
        0..=2 => 1,
        3 => 2,
        _ => 3,
    }
}

/// The level of `xp`, as an index of `LEVELS`.
pub fn level_of(xp: u32) -> usize {
    LEVELS.iter().take_while(|(k, _)| *k <= xp).count().saturating_sub(1)
}

impl Session {
    /// Counts one action made `secs` seconds after launch.
    pub fn play(&mut self, s: &Stride, secs: u64) -> Play {
        let mut out = Play { streak: 1, ..Play::default() };
        let base: u32 = match s {
            Stride::Commit => 100,
            Stride::Push => 300,
            Stride::Merge => 500,
            Stride::Undo => {
                let lost = self.xp.min(100);
                self.xp -= lost;
                self.chain = 0;
                out.xp = -(lost as i32);
                out.taken = self.mess.pop();
                return out;
            }
            Stride::Cramp { op, conflicts } => {
                // A second stop during the fight only changes the boss.
                out.boss = self.boss.is_none();
                if out.boss {
                    self.mess.push(Piece::Spill);
                    out.piece = Some(Piece::Spill);
                }
                self.boss = Some(Boss { op: op.clone(), conflicts: *conflicts });
                self.chain = 0;
                return out;
            }
        };
        let count = match s {
            Stride::Commit => &mut self.commits,
            Stride::Push => &mut self.pushes,
            _ => &mut self.merges,
        };
        *count += 1;
        out.first = *count == 1;
        out.piece = match s {
            Stride::Commit => self.commits.is_multiple_of(COMMITS_PER_BAG).then_some(Piece::Bag),
            Stride::Push => Some(Piece::Can),
            _ => Some(Piece::Pizza),
        };
        self.mess.extend(out.piece);
        let close = self.last.is_some_and(|t| secs.saturating_sub(t) < STREAK_SECS);
        self.chain = if close && self.chain > 0 { self.chain + 1 } else { 1 };
        self.last = Some(secs);
        out.streak = multiplier(self.chain);
        self.best = self.best.max(self.chain);
        let mut gain = base * out.streak;
        if self.boss.take().is_some() {
            out.beaten = true;
            self.bosses += 1;
            gain += BOSS_XP;
        }
        self.xp += gain;
        out.xp = gain as i32;
        let reached = level_of(self.xp);
        if reached > self.level_seen {
            self.level_seen = reached;
            if reached < LEVELS.len() - 1 {
                out.level = Some(reached);
            }
        }
        if self.xp >= LEGEND && !self.legend {
            self.legend = true;
            out.legend = true;
        }
        out
    }

    /// The level reached, as an index of `LEVELS`.
    pub fn level(&self) -> usize {
        level_of(self.xp)
    }

    /// The next level ahead, or None at Legend.
    pub fn next_level(&self) -> Option<(u32, &'static str)> {
        LEVELS.iter().copied().find(|(k, _)| *k > self.xp)
    }

    /// The streak running at `secs` (×2, ×3), the one the last action made: 1 without one.
    pub fn streak(&self, secs: u64) -> u32 {
        if self.streak_left(secs).is_some() { multiplier(self.chain) } else { 1 }
    }

    /// The streak the next action counts with if made at `secs`: 1 when the chain is broken.
    pub fn next_streak(&self, secs: u64) -> u32 {
        if self.streak_left(secs).is_some() { multiplier(self.chain + 1) } else { 1 }
    }

    /// Seconds left to keep the chain going, while one runs.
    pub fn streak_left(&self, secs: u64) -> Option<u64> {
        let last = self.last?;
        let gone = secs.saturating_sub(last);
        (self.chain > 0 && self.boss.is_none() && gone < STREAK_SECS).then(|| STREAK_SECS - gone)
    }

    /// Touch grass: everything into the bin bag, the XP kept.
    pub fn tidy(&mut self) {
        self.mess.clear();
    }

    /// New game+: back to 0, in a tidy room.
    pub fn new_game(&mut self) {
        *self = Session::default();
    }

    /// "17 commits · 2 pushes · 1 merge".
    pub fn tally(&self) -> String {
        let n = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        format!("{} · {} · {}", n(self.commits, "commit", "commits"), n(self.pushes, "push", "pushes"), n(self.merges, "merge", "merges"))
    }

    /// A session for the screenshots, at `secs`: `xp` with the streak ×3 running since an action
    /// 18 seconds before (Legend: finished a while ago), the counts and the mess that go with it,
    /// `boss` with a rebase stopped on 2 conflicts.
    pub fn sample(xp: u32, secs: u64, boss: bool) -> Session {
        let legend = xp >= LEGEND;
        let (commits, pushes, merges) = match xp {
            0..=3000 => (17, 2, 1),
            3001..=3999 => (21, 3, 1),
            4000..=5999 => (31, 5, 3),
            _ => (34, 6, 3),
        };
        // The mess an honest session leaves, in the order it was left.
        let mut mess = Vec::new();
        let (mut c, mut p, mut m) = (0, 0, 0);
        for k in 0..(commits + pushes + merges) {
            if k % 7 == 3 && p < pushes {
                p += 1;
                mess.push(Piece::Can);
            } else if k % 11 == 5 && m < merges {
                m += 1;
                mess.push(Piece::Pizza);
            } else if c < commits {
                c += 1;
                if c % COMMITS_PER_BAG == 0 {
                    mess.push(Piece::Bag);
                }
            }
        }
        mess.extend(std::iter::repeat_n(Piece::Can, (pushes - p) as usize));
        mess.extend(std::iter::repeat_n(Piece::Pizza, (merges - m) as usize));
        if boss {
            mess.push(Piece::Spill);
        }
        let last = secs.saturating_sub(if legend { 600 } else { 18 });
        Session {
            xp,
            commits,
            pushes,
            merges,
            best: if legend { 7 } else { 4 },
            bosses: if legend { 2 } else { 1 },
            boss: boss.then(|| Boss { op: "rebase".into(), conflicts: 2 }),
            mess,
            last: Some(last),
            chain: if boss { 0 } else { 4 },
            level_seen: level_of(xp),
            legend,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cramp() -> Stride {
        Stride::Cramp { op: "rebase".into(), conflicts: 2 }
    }

    #[test]
    fn each_action_earns_its_xp() {
        let mut s = Session::default();
        // Far apart: no streak.
        let c = s.play(&Stride::Commit, 0);
        assert_eq!((c.xp, c.first), (100, true));
        assert_eq!(s.play(&Stride::Push, 100).xp, 300);
        assert_eq!(s.play(&Stride::Merge, 200).xp, 500);
        assert!(!s.play(&Stride::Commit, 300).first);
        assert_eq!(s.xp, 1000);
        assert_eq!((s.commits, s.pushes, s.merges), (2, 1, 1));
        assert_eq!(s.tally(), "2 commits · 1 push · 1 merge");
    }

    #[test]
    fn actions_within_a_minute_make_a_streak() {
        let mut s = Session::default();
        assert_eq!(s.play(&Stride::Commit, 0).streak, 1);
        assert_eq!(s.play(&Stride::Commit, 30).streak, 1);
        assert_eq!(s.next_streak(40), 2, "the third action would count double");
        let third = s.play(&Stride::Commit, 80);
        assert_eq!((third.streak, third.xp), (2, 200));
        let fourth = s.play(&Stride::Push, 139);
        assert_eq!((fourth.streak, fourth.xp), (3, 900));
        assert_eq!(s.play(&Stride::Commit, 150).streak, 3);
        assert_eq!((s.streak(160), s.streak_left(160)), (3, Some(50)));
        // A minute without an action breaks it.
        assert_eq!((s.streak(210), s.next_streak(210)), (1, 1));
        assert_eq!(s.play(&Stride::Commit, 210).streak, 1);
        assert_eq!(s.best, 5, "the longest chain: five actions in a row");
    }

    #[test]
    fn undo_takes_100_back_never_below_0_and_breaks_the_streak() {
        let mut s = Session::default();
        s.play(&Stride::Commit, 0);
        s.play(&Stride::Commit, 10);
        assert_eq!(s.play(&Stride::Undo, 20).xp, -100);
        assert_eq!(s.xp, 100);
        s.play(&Stride::Undo, 30);
        assert_eq!(s.play(&Stride::Undo, 40), Play { streak: 1, ..Play::default() });
        assert_eq!(s.xp, 0);
        s.play(&Stride::Commit, 50);
        s.play(&Stride::Commit, 55);
        s.play(&Stride::Undo, 56);
        assert_eq!(s.play(&Stride::Commit, 57).streak, 1, "the rewind broke the streak");
    }

    #[test]
    fn a_boss_fight_freezes_the_xp_until_the_next_action_beats_it() {
        let mut s = Session::default();
        s.play(&Stride::Commit, 0);
        s.play(&Stride::Commit, 10);
        let fight = s.play(&cramp(), 20);
        assert!(fight.boss && fight.xp == 0);
        assert_eq!(s.boss, Some(Boss { op: "rebase".into(), conflicts: 2 }));
        assert_eq!(s.streak_left(25), None, "no streak during a fight");
        // A second stop is the same fight.
        let again = s.play(&Stride::Cramp { op: "rebase".into(), conflicts: 1 }, 22);
        assert!(!again.boss && again.piece.is_none());
        assert_eq!(s.boss.as_ref().map(|b| b.conflicts), Some(1));
        let next = s.play(&Stride::Commit, 30);
        assert!(next.beaten && s.boss.is_none());
        assert_eq!((next.streak, next.xp), (1, 100 + BOSS_XP as i32), "the streak starts again, the boss is worth 200");
        assert_eq!(s.bosses, 1);
        assert!(!s.play(&Stride::Commit, 35).beaten);
    }

    #[test]
    fn each_level_is_announced_once() {
        let mut s = Session { xp: 950, ..Session::default() };
        assert_eq!(s.play(&Stride::Commit, 0).level, Some(1));
        assert_eq!(s.play(&Stride::Undo, 100).level, None);
        assert_eq!(s.play(&Stride::Commit, 200).level, None, "back at Rookie a second time");
        assert_eq!(s.next_level(), Some((2500, "Pro")));
        // A big action past two levels announces the furthest.
        let mut s = Session { xp: 2600, level_seen: 1, chain: 3, last: Some(0), ..Session::default() };
        assert_eq!(s.play(&Stride::Merge, 10).level, Some(3), "2,600 + 1,500: Pro and Elite, Elite said");
        assert_eq!(s.level(), 3);
    }

    #[test]
    fn legend_opens_the_gg_card_once() {
        let mut s = Session { xp: 5700, level_seen: 3, ..Session::default() };
        let p = s.play(&Stride::Merge, 0);
        assert!(p.legend);
        assert_eq!(p.level, None, "Legend is the card, not a level toast");
        assert_eq!((s.level(), s.next_level()), (4, None));
        assert!(!s.play(&Stride::Merge, 100).legend);
        assert_eq!(s.xp, 6700);
    }

    #[test]
    fn each_action_leaves_its_piece_and_undo_takes_the_last_back() {
        let mut s = Session::default();
        for t in 0..4 {
            assert_eq!(s.play(&Stride::Commit, t * 100).piece, None);
        }
        assert_eq!(s.play(&Stride::Commit, 500).piece, Some(Piece::Bag), "a bag every 5 commits");
        assert_eq!(s.play(&Stride::Push, 600).piece, Some(Piece::Can));
        assert_eq!(s.play(&Stride::Merge, 700).piece, Some(Piece::Pizza));
        assert_eq!(s.play(&cramp(), 800).piece, Some(Piece::Spill));
        assert_eq!(s.mess, [Piece::Bag, Piece::Can, Piece::Pizza, Piece::Spill]);
        assert_eq!(s.play(&Stride::Undo, 900).taken, Some(Piece::Spill));
        assert_eq!(s.mess.len(), 3);
    }

    #[test]
    fn tidy_keeps_the_xp_and_a_new_game_starts_again() {
        let mut s = Session::sample(2840, 600, false);
        assert!(!s.mess.is_empty());
        s.tidy();
        assert!(s.mess.is_empty());
        assert_eq!(s.xp, 2840);
        s.new_game();
        assert_eq!(s, Session::default());
    }

    #[test]
    fn the_samples_show_what_the_screenshots_need() {
        let s = Session::sample(2840, 600, false);
        assert_eq!((s.level(), s.streak(600), s.streak_left(600)), (2, 3, Some(42)));
        assert_eq!(s.mess.iter().filter(|p| **p == Piece::Can).count(), 2);
        let b = Session::sample(3300, 600, true);
        assert!(b.boss.is_some() && b.mess.last() == Some(&Piece::Spill));
        let gg = Session::sample(LEGEND, 900, false);
        assert!(gg.legend && gg.streak(900) == 1 && gg.level() == 4);
    }
}
