//! The Runner skin's race of the day (#206): 8,000 m on the track, 20 laps of 400 m, where each
//! Git action moves the runner on (a commit 100 m, a push 300, a merge 500) and Undo takes 100 m
//! back. A split every 1,000 m, the finish at 8,000. Kept in memory only: it starts again at 0
//! at each launch. `app/course.rs` draws the course bar, the toolbar pill, the coach's toasts and
//! the medal. The Ultra-trail skin (#207) runs the same race on a mountain: 160 km, each action
//! twenty times further, a checkpoint every 20 km (`Race::Ultra`). Metres stay the unit.

/// The track race's length and a lap, in metres.
pub const LENGTH: u32 = 8000;
pub const LAP: u32 = 400;
/// A split every this many metres (the last one before the finish at 7,000).
pub const SPLIT_EVERY: u32 = 1000;

/// Which race the runner runs: the Runner's 8,000 m on the track or the Ultra-trail's 160 km.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Race {
    #[default]
    Track,
    Ultra,
}

impl Race {
    /// The finish, in metres.
    pub fn length(self) -> u32 {
        match self {
            Race::Track => LENGTH,
            Race::Ultra => 160_000,
        }
    }

    /// A split (a checkpoint on the ultra) every this many metres.
    pub fn split_every(self) -> u32 {
        match self {
            Race::Track => SPLIT_EVERY,
            Race::Ultra => 20_000,
        }
    }

    /// How many times further than on the track an action takes the runner.
    pub fn scale(self) -> i32 {
        match self {
            Race::Track => 1,
            Race::Ultra => 20,
        }
    }

    /// "2,300 m" on the track, "46 km" on the ultra.
    pub fn distance(self, m: u32) -> String {
        match self {
            Race::Track => metres(m),
            Race::Ultra => km(m),
        }
    }
}

/// What moved the runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stride {
    Commit,
    Push,
    Merge,
    /// Cmd+Z: 100 m back.
    Undo,
    /// An operation stopped on conflicts (`op`: merge, rebase…, with `conflicts` files): no metre.
    Cramp { op: String, conflicts: usize },
}

impl Stride {
    /// How far it moves the runner, in metres.
    pub fn metres(&self) -> i32 {
        match self {
            Stride::Commit => 100,
            Stride::Push => 300,
            Stride::Merge => 500,
            Stride::Undo => -100,
            Stride::Cramp { .. } => 0,
        }
    }
}

/// What a stride did to the race.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Moved {
    /// The metres actually gained or lost (Undo at the start loses none).
    pub metres: i32,
    /// The split reached for the first time this race, if any (the furthest one).
    pub split: Option<u32>,
    /// The runner has just crossed the finish line.
    pub finished: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Course {
    /// The race run: the track's by default.
    pub race: Race,
    pub metres: u32,
    pub commits: u32,
    pub pushes: u32,
    pub merges: u32,
    /// The furthest split already announced, so going back and over it again stays quiet.
    pub split_seen: u32,
    /// The finish is reached: the medal shows once.
    pub finished: bool,
}

impl Course {
    /// Counts one stride: never below 0 nor past the finish.
    pub fn stride(&mut self, s: &Stride) -> Moved {
        match s {
            Stride::Commit => self.commits += 1,
            Stride::Push => self.pushes += 1,
            Stride::Merge => self.merges += 1,
            Stride::Undo | Stride::Cramp { .. } => {}
        }
        let (length, every) = (self.race.length(), self.race.split_every());
        let before = self.metres;
        self.metres = (self.metres as i32 + s.metres() * self.race.scale()).clamp(0, length as i32) as u32;
        let mut moved = Moved { metres: self.metres as i32 - before as i32, ..Moved::default() };
        let mark = self.metres / every * every;
        if mark > self.split_seen && mark < length {
            self.split_seen = mark;
            moved.split = Some(mark);
        }
        if self.metres == length && !self.finished {
            self.finished = true;
            moved.finished = true;
        }
        moved
    }

    /// "Run it again": back to the start of the same race, the counts to 0.
    pub fn restart(&mut self) {
        *self = Course { race: self.race, ..Course::default() };
    }

    /// The next split, or the finish.
    pub fn next_split(&self) -> u32 {
        let every = self.race.split_every();
        ((self.metres / every + 1) * every).min(self.race.length())
    }

    /// The race's finish, in metres.
    pub fn length(&self) -> u32 {
        self.race.length()
    }

    /// The lap being run, from 1 to 20.
    pub fn lap(&self) -> u32 {
        (self.metres / LAP + 1).min(LENGTH / LAP)
    }

    /// "9 commits · 3 pushes · 1 merge".
    pub fn tally(&self) -> String {
        let n = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        format!("{} · {} · {}", n(self.commits, "commit", "commits"), n(self.pushes, "push", "pushes"), n(self.merges, "merge", "merges"))
    }

    /// A race at `metres` with the counts that make it, for the screenshots: 2,300, 3,000 or
    /// 8,000 m on the track, twenty times that on the ultra (46, 60 or 160 km).
    pub fn sample(race: Race, metres: u32) -> Course {
        let (commits, pushes, merges) = match metres / race.scale() as u32 {
            3000 => (11, 3, 2),
            8000 => (41, 8, 3),
            _ => (9, 3, 1),
        };
        let metres = (100 * commits + 300 * pushes + 500 * merges) * race.scale() as u32;
        let every = race.split_every();
        Course { race, metres, commits, pushes, merges, split_seen: metres / every * every, finished: metres >= race.length() }
    }
}

/// 8000 → "8,000".
pub fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "2,300 m".
pub fn metres(m: u32) -> String {
    format!("{} m", thousands(m))
}

/// "46 km", "12.5 km" (the ultra's distances, from metres).
pub fn km(m: u32) -> String {
    if m.is_multiple_of(1000) {
        format!("{} km", thousands(m / 1000))
    } else {
        format!("{:.1} km", m as f32 / 1000.0)
    }
}

/// Time at the keyboard: "42 min", "2 h", "9 h 12".
pub fn keyboard_time(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs / 60 % 60);
    match (h, m) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_action_moves_the_runner_its_distance() {
        let mut c = Course::default();
        assert_eq!(c.stride(&Stride::Commit).metres, 100);
        assert_eq!(c.stride(&Stride::Push).metres, 300);
        assert_eq!(c.stride(&Stride::Merge).metres, 500);
        assert_eq!(c.metres, 900);
        assert_eq!((c.commits, c.pushes, c.merges), (1, 1, 1));
        assert_eq!(c.tally(), "1 commit · 1 push · 1 merge");
        assert_eq!(c.lap(), 3);
    }

    #[test]
    fn undo_takes_100_m_back_and_never_below_the_start() {
        let mut c = Course::default();
        c.stride(&Stride::Push);
        assert_eq!(c.stride(&Stride::Undo).metres, -100);
        assert_eq!(c.metres, 200);
        c.stride(&Stride::Undo);
        c.stride(&Stride::Undo);
        assert_eq!(c.stride(&Stride::Undo), Moved::default());
        assert_eq!(c.metres, 0);
    }

    #[test]
    fn a_cramp_keeps_the_runner_where_it_is() {
        let mut c = Course::default();
        c.stride(&Stride::Merge);
        assert_eq!(c.stride(&Stride::Cramp { op: "rebase".into(), conflicts: 2 }), Moved::default());
        assert_eq!(c.metres, 500);
    }

    #[test]
    fn each_split_is_announced_once() {
        let mut c = Course { metres: 800, ..Course::default() };
        assert_eq!(c.stride(&Stride::Push).split, Some(1000));
        assert_eq!(c.stride(&Stride::Undo).split, None);
        assert_eq!(c.metres, 1000);
        c.stride(&Stride::Undo);
        assert_eq!(c.stride(&Stride::Commit).split, None, "back at 1,000 m a second time");
        assert_eq!(c.next_split(), 2000);
    }

    #[test]
    fn the_finish_shows_once_and_run_it_again_starts_over() {
        let mut c = Course { metres: 7700, ..Course::default() };
        let m = c.stride(&Stride::Merge);
        assert!(m.finished);
        assert_eq!((c.metres, m.metres, m.split), (8000, 300, None), "the finish is no split");
        assert!(!c.stride(&Stride::Commit).finished);
        assert_eq!((c.metres, c.next_split(), c.lap()), (8000, 8000, 20));
        c.restart();
        assert_eq!(c, Course::default());
    }

    #[test]
    fn the_samples_add_up() {
        for m in [2300, 3000, 8000] {
            assert_eq!(Course::sample(Race::Track, m).metres, m);
            assert_eq!(Course::sample(Race::Ultra, m * 20).metres, m * 20);
        }
        assert!(Course::sample(Race::Ultra, 160_000).finished);
        assert_eq!(Course::sample(Race::Ultra, 60_000).split_seen, 60_000);
    }

    #[test]
    fn the_ultra_goes_twenty_times_further() {
        let mut c = Course { race: Race::Ultra, ..Course::default() };
        assert_eq!(c.stride(&Stride::Commit).metres, 2000);
        assert_eq!(c.stride(&Stride::Push).metres, 6000);
        assert_eq!(c.stride(&Stride::Merge).metres, 10_000);
        assert_eq!(c.stride(&Stride::Undo).metres, -2000);
        assert_eq!(c.metres, 16_000);
        assert_eq!(c.next_split(), 20_000);
        assert_eq!(Race::Ultra.distance(c.metres), "16 km");
        assert_eq!(Race::Track.distance(2300), "2,300 m");
    }

    #[test]
    fn the_ultra_has_a_checkpoint_every_20_km_and_finishes_at_160() {
        let mut c = Course { race: Race::Ultra, metres: 16_000, ..Course::default() };
        assert_eq!(c.stride(&Stride::Push).split, Some(20_000));
        assert_eq!(c.stride(&Stride::Commit).split, None);
        c.metres = 155_000;
        let m = c.stride(&Stride::Merge);
        assert!(m.finished);
        assert_eq!((c.metres, m.metres, m.split), (160_000, 5000, None), "the finish is no checkpoint");
        c.restart();
        assert_eq!(c, Course { race: Race::Ultra, ..Course::default() }, "run it again stays on the ultra");
    }

    #[test]
    fn the_numbers_read_well() {
        assert_eq!(thousands(8000), "8,000");
        assert_eq!(thousands(900), "900");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(metres(2300), "2,300 m");
        assert_eq!(km(46_000), "46 km");
        assert_eq!(km(12_500), "12.5 km");
        assert_eq!(keyboard_time(42 * 60), "42 min");
        assert_eq!(keyboard_time(2 * 3600), "2 h");
        assert_eq!(keyboard_time(9 * 3600 + 12 * 60), "9 h 12");
    }
}
