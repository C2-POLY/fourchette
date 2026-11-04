//! The person's settings, chosen in the Settings window (Cmd+,) and kept in repos.json under
//! `"settings"`, next to the appearance and the variant (which `Library` keeps apart, since the
//! palette and the status bar change them too). A missing or invalid value falls back to its default.
use serde_json::{json, Value};

/// "fetchRemotesAutomatically" default true (sub_10031b7cc).
pub const DEFAULT_AUTO_FETCH: bool = true;
/// How often the automatic fetch runs. Not found in the original's code (estimated).
pub const DEFAULT_FETCH_MINUTES: u32 = 10;
/// The intervals the Settings window offers, in minutes.
pub const FETCH_MINUTES: [u32; 4] = [5, 10, 30, 60];
/// "pullSheetRebase" default false: Pull merges (sub_10031b7cc).
pub const DEFAULT_PULL_REBASE: bool = false;
/// "pullSheetStashAndReapply" default false (sub_10031b7cc).
pub const DEFAULT_PULL_AUTOSTASH: bool = false;
/// "deleteStashAfterApply" default false: Apply keeps the stash (sub_10031b7cc).
pub const DEFAULT_DELETE_STASH_AFTER_APPLY: bool = false;

/// "contextSize" default (sub_10031b7cc): the lines of context around a change in a diff.
pub const DEFAULT_DIFF_CONTEXT: u32 = 3;
/// The context sizes the Settings window offers.
pub const DIFF_CONTEXTS: [u32; 5] = [1, 3, 5, 10, 25];
/// "commitSubjectLimitLow" / "commitSubjectLimitHigh" defaults (sub_10031b7cc).
pub const DEFAULT_SUBJECT_LOW: usize = 50;
pub const DEFAULT_SUBJECT_HIGH: usize = 70;
/// The subject limits stay inside this range, and low stays under high.
pub const SUBJECT_RANGE: std::ops::RangeInclusive<usize> = 10..=200;
/// "maxCommitCount" default (settings table, sub_10031b7cc).
pub const DEFAULT_MAX_COMMITS: usize = 100_000;
/// The commit counts the Settings window offers.
pub const MAX_COMMITS: [usize; 4] = [1_000, 10_000, 50_000, 100_000];

/// The interface font: Inter is built into the app, the others are read from the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UiFont {
    #[default]
    Inter,
    SanFrancisco,
    HelveticaNeue,
    AvenirNext,
}

impl UiFont {
    pub const ALL: [UiFont; 4] = [UiFont::Inter, UiFont::SanFrancisco, UiFont::HelveticaNeue, UiFont::AvenirNext];

    pub fn key(self) -> &'static str {
        match self {
            UiFont::Inter => "inter",
            UiFont::SanFrancisco => "sf",
            UiFont::HelveticaNeue => "helvetica",
            UiFont::AvenirNext => "avenir",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            UiFont::Inter => "Inter",
            UiFont::SanFrancisco => "San Francisco",
            UiFont::HelveticaNeue => "Helvetica Neue",
            UiFont::AvenirNext => "Avenir Next",
        }
    }

    /// The system file (face 0 for a .ttc), none for the built-in font.
    pub fn path(self) -> Option<&'static str> {
        match self {
            UiFont::Inter => None,
            UiFont::SanFrancisco => Some("/System/Library/Fonts/SFNS.ttf"),
            UiFont::HelveticaNeue => Some("/System/Library/Fonts/HelveticaNeue.ttc"),
            UiFont::AvenirNext => Some("/System/Library/Fonts/Avenir Next.ttc"),
        }
    }

    pub fn parse(s: &str) -> Option<UiFont> {
        UiFont::ALL.into_iter().find(|f| f.key() == s.trim().to_lowercase())
    }
}

/// The code font: JetBrains Mono is built into the app, the others are read from the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CodeFont {
    #[default]
    JetBrainsMono,
    SfMono,
    Menlo,
    Monaco,
}

impl CodeFont {
    pub const ALL: [CodeFont; 4] = [CodeFont::JetBrainsMono, CodeFont::SfMono, CodeFont::Menlo, CodeFont::Monaco];

    pub fn key(self) -> &'static str {
        match self {
            CodeFont::JetBrainsMono => "jetbrains",
            CodeFont::SfMono => "sfmono",
            CodeFont::Menlo => "menlo",
            CodeFont::Monaco => "monaco",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CodeFont::JetBrainsMono => "JetBrains Mono",
            CodeFont::SfMono => "SF Mono",
            CodeFont::Menlo => "Menlo",
            CodeFont::Monaco => "Monaco",
        }
    }

    pub fn path(self) -> Option<&'static str> {
        match self {
            CodeFont::JetBrainsMono => None,
            CodeFont::SfMono => Some("/System/Library/Fonts/SFNSMono.ttf"),
            CodeFont::Menlo => Some("/System/Library/Fonts/Menlo.ttc"),
            CodeFont::Monaco => Some("/System/Library/Fonts/Monaco.ttf"),
        }
    }

    pub fn parse(s: &str) -> Option<CodeFont> {
        CodeFont::ALL.into_iter().find(|f| f.key() == s.trim().to_lowercase())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The font of the interface (`UiFont::Inter` by default).
    pub ui_font: UiFont,
    /// The font of code, diffs and hashes (`CodeFont::JetBrainsMono` by default).
    pub code_font: CodeFont,
    /// Fetch the open repository's remote in the background.
    pub auto_fetch: bool,
    /// One of `FETCH_MINUTES`.
    pub fetch_minutes: u32,
    /// Pull rebases the current branch onto its upstream instead of merging it.
    pub pull_rebase: bool,
    /// Pull puts uncommitted changes aside and back afterwards (`--autostash`).
    pub pull_autostash: bool,
    /// A stash's Apply also deletes it once applied without conflicts (`stash pop`).
    pub delete_stash_after_apply: bool,
    /// Lines of context around a change in a diff (`--unified=`).
    pub diff_context: u32,
    /// The commit subject counter turns orange above this, and Claude's soft limit.
    pub subject_low: usize,
    /// ... red above this, and Claude's hard limit. Always above `subject_low`.
    pub subject_high: usize,
    /// How many commits the history loads (`settings maxCommitCount <n>` in the palette sets it too).
    pub max_commits: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ui_font: UiFont::default(),
            code_font: CodeFont::default(),
            auto_fetch: DEFAULT_AUTO_FETCH,
            fetch_minutes: DEFAULT_FETCH_MINUTES,
            pull_rebase: DEFAULT_PULL_REBASE,
            pull_autostash: DEFAULT_PULL_AUTOSTASH,
            delete_stash_after_apply: DEFAULT_DELETE_STASH_AFTER_APPLY,
            diff_context: DEFAULT_DIFF_CONTEXT,
            subject_low: DEFAULT_SUBJECT_LOW,
            subject_high: DEFAULT_SUBJECT_HIGH,
            max_commits: DEFAULT_MAX_COMMITS,
        }
    }
}

impl Settings {
    /// Reads the `"settings"` object; each key that is missing or invalid keeps its default.
    pub fn from_json(v: &Value) -> Settings {
        let d = Settings::default();
        let n = |key: &str| v[key].as_u64().filter(|n| *n > 0);
        // The two subject limits only count together, and only when low is under high.
        let limits = match (n("subject_low"), n("subject_high")) {
            (Some(l), Some(h)) if l < h && SUBJECT_RANGE.contains(&(l as usize)) && SUBJECT_RANGE.contains(&(h as usize)) => (l as usize, h as usize),
            _ => (d.subject_low, d.subject_high),
        };
        Settings {
            ui_font: v["ui_font"].as_str().and_then(UiFont::parse).unwrap_or(d.ui_font),
            code_font: v["code_font"].as_str().and_then(CodeFont::parse).unwrap_or(d.code_font),
            auto_fetch: v["auto_fetch"].as_bool().unwrap_or(d.auto_fetch),
            fetch_minutes: v["fetch_minutes"].as_u64().map(|m| m as u32).filter(|m| FETCH_MINUTES.contains(m)).unwrap_or(d.fetch_minutes),
            pull_rebase: v["pull_rebase"].as_bool().unwrap_or(d.pull_rebase),
            pull_autostash: v["pull_autostash"].as_bool().unwrap_or(d.pull_autostash),
            delete_stash_after_apply: v["delete_stash_after_apply"].as_bool().unwrap_or(d.delete_stash_after_apply),
            diff_context: n("diff_context").filter(|c| *c <= 100).map(|c| c as u32).unwrap_or(d.diff_context),
            subject_low: limits.0,
            subject_high: limits.1,
            max_commits: n("max_commits").map(|m| m as usize).unwrap_or(d.max_commits),
        }
    }

    pub fn to_json(self) -> Value {
        json!({
            "ui_font": self.ui_font.key(),
            "code_font": self.code_font.key(),
            "auto_fetch": self.auto_fetch,
            "fetch_minutes": self.fetch_minutes,
            "pull_rebase": self.pull_rebase,
            "pull_autostash": self.pull_autostash,
            "delete_stash_after_apply": self.delete_stash_after_apply,
            "diff_context": self.diff_context,
            "subject_low": self.subject_low,
            "subject_high": self.subject_high,
            "max_commits": self.max_commits,
        })
    }

    pub fn fetch_secs(self) -> f32 {
        self.fetch_minutes as f32 * 60.0
    }
}

/// "1,000", "100,000".
pub fn count_label(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "5 min", "30 min", "1 hour".
pub fn minutes_label(m: u32) -> String {
    if m == 60 {
        "1 hour".into()
    } else {
        format!("{m} min")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_on_every_ten_minutes() {
        let d = Settings::default();
        assert!(d.auto_fetch);
        assert_eq!(d.fetch_minutes, 10);
        assert_eq!(d.fetch_secs(), 600.0);
        assert!(FETCH_MINUTES.contains(&d.fetch_minutes));
    }

    #[test]
    fn every_interval_survives_a_round_trip() {
        for m in FETCH_MINUTES {
            for on in [true, false] {
                let s = Settings { auto_fetch: on, fetch_minutes: m, ..Settings::default() };
                assert_eq!(Settings::from_json(&s.to_json()), s);
            }
        }
        assert_eq!(FETCH_MINUTES.map(minutes_label), ["5 min", "10 min", "30 min", "1 hour"]);
    }

    #[test]
    fn missing_or_invalid_values_fall_back_one_by_one() {
        assert_eq!(Settings::from_json(&Value::Null), Settings::default());
        assert_eq!(Settings::from_json(&json!({})), Settings::default());
        // An interval the window doesn't offer, or of the wrong type, goes back to 10 minutes;
        // the valid key next to it is kept.
        for bad in [json!(7), json!(0), json!(-5), json!("10"), json!(10.5), json!(null)] {
            let s = Settings::from_json(&json!({"auto_fetch": false, "fetch_minutes": bad}));
            assert_eq!(s, Settings { auto_fetch: false, fetch_minutes: 10, ..Settings::default() }, "{bad}");
        }
        let s = Settings::from_json(&json!({"auto_fetch": "no", "fetch_minutes": 30}));
        assert_eq!(s, Settings { auto_fetch: true, fetch_minutes: 30, ..Settings::default() });
    }

    #[test]
    fn git_settings_default_off_like_the_original() {
        let d = Settings::default();
        assert!(!d.pull_rebase, "Pull merges by default");
        assert!(!d.pull_autostash);
        assert!(!d.delete_stash_after_apply, "Apply keeps the stash by default");
    }

    #[test]
    fn git_settings_survive_a_round_trip_and_fall_back_one_by_one() {
        for bits in 0..8u8 {
            let s = Settings { pull_rebase: bits & 1 != 0, pull_autostash: bits & 2 != 0, delete_stash_after_apply: bits & 4 != 0, ..Settings::default() };
            assert_eq!(Settings::from_json(&s.to_json()), s, "{bits}");
        }
        // A file saved before these settings existed keeps its fetch choices and gets the defaults.
        let old = Settings::from_json(&json!({"auto_fetch": false, "fetch_minutes": 30}));
        assert_eq!(old, Settings { auto_fetch: false, fetch_minutes: 30, ..Settings::default() });
        // A value of the wrong type goes back to its default; the valid one next to it is kept.
        let s = Settings::from_json(&json!({"pull_rebase": "yes", "pull_autostash": 1, "delete_stash_after_apply": true}));
        assert_eq!(s, Settings { delete_stash_after_apply: true, ..Settings::default() });
    }

    #[test]
    fn the_diff_and_commit_settings_default_and_round_trip() {
        let d = Settings::default();
        assert_eq!((d.diff_context, d.subject_low, d.subject_high, d.max_commits), (3, 50, 70, 100_000));
        assert!(DIFF_CONTEXTS.contains(&d.diff_context) && MAX_COMMITS.contains(&d.max_commits));
        let s = Settings { diff_context: 10, subject_low: 40, subject_high: 60, max_commits: 5_000, ..d };
        assert_eq!(Settings::from_json(&s.to_json()), s);
        // An older file without these keys gets the defaults and keeps the rest.
        assert_eq!(Settings::from_json(&json!({"auto_fetch": false})), Settings { auto_fetch: false, ..d });
    }

    #[test]
    fn invalid_diff_and_commit_values_fall_back_one_by_one() {
        let d = Settings::default();
        for bad in [json!(0), json!(-1), json!("5"), json!(2.5), json!(null), json!(1000)] {
            let s = Settings::from_json(&json!({"diff_context": bad, "max_commits": bad, "subject_low": 40, "subject_high": 60}));
            assert_eq!(s.diff_context, d.diff_context, "{bad}");
            assert_eq!((s.subject_low, s.subject_high), (40, 60), "{bad}");
            assert_eq!(s.max_commits, if bad == json!(1000) { 1000 } else { d.max_commits }, "{bad}");
        }
        // Low at or above high, out of range, or only one of the two: both go back to 50 / 70.
        for (l, h) in [(70, 70), (80, 60), (5, 70), (50, 500)] {
            let s = Settings::from_json(&json!({"subject_low": l, "subject_high": h}));
            assert_eq!((s.subject_low, s.subject_high), (50, 70), "{l}/{h}");
        }
        let s = Settings::from_json(&json!({"subject_low": 40}));
        assert_eq!((s.subject_low, s.subject_high), (50, 70));
    }

    #[test]
    fn counts_read_with_thousands_separators() {
        assert_eq!(MAX_COMMITS.map(count_label), ["1,000", "10,000", "50,000", "100,000"]);
        assert_eq!(count_label(7), "7");
        assert_eq!(count_label(123_456_789), "123,456,789");
    }

    #[test]
    fn fonts_default_to_the_built_in_ones_and_round_trip() {
        let d = Settings::default();
        assert_eq!((d.ui_font, d.code_font), (UiFont::Inter, CodeFont::JetBrainsMono));
        assert!(UiFont::Inter.path().is_none() && CodeFont::JetBrainsMono.path().is_none());
        for u in UiFont::ALL {
            for c in CodeFont::ALL {
                let s = Settings { ui_font: u, code_font: c, ..d };
                assert_eq!(Settings::from_json(&s.to_json()), s);
            }
        }
        // The others come from /System/Library/Fonts.
        assert!(UiFont::ALL[1..].iter().all(|f| f.path().is_some_and(|p| p.starts_with("/System/Library/Fonts/"))));
        assert!(CodeFont::ALL[1..].iter().all(|f| f.path().is_some_and(|p| p.starts_with("/System/Library/Fonts/"))));
    }

    #[test]
    fn invalid_fonts_fall_back_one_by_one() {
        let d = Settings::default();
        for bad in [json!("comic"), json!(3), json!(null), json!("")] {
            let s = Settings::from_json(&json!({"ui_font": bad, "code_font": "menlo", "auto_fetch": false}));
            assert_eq!(s, Settings { code_font: CodeFont::Menlo, auto_fetch: false, ..d }, "{bad}");
        }
        // A code font's key in the interface slot is not valid there.
        assert_eq!(Settings::from_json(&json!({"ui_font": "menlo"})).ui_font, UiFont::Inter);
        // An older file without these keys gets the defaults.
        assert_eq!(Settings::from_json(&json!({"auto_fetch": false})), Settings { auto_fetch: false, ..d });
    }
}
