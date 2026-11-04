//! The person's settings, chosen in the Settings window (Cmd+,) and kept in repos.json under
//! `"settings"`, next to `"appearance"`. A missing or invalid value falls back to its default.
use serde_json::{json, Value};

/// "fetchRemotesAutomatically" default true (sub_10031b7cc).
pub const DEFAULT_AUTO_FETCH: bool = true;
/// How often the automatic fetch runs. Not found in the original's code (estimated).
pub const DEFAULT_FETCH_MINUTES: u32 = 10;
/// The intervals the Settings window offers, in minutes.
pub const FETCH_MINUTES: [u32; 4] = [5, 10, 30, 60];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Fetch the open repository's remote in the background.
    pub auto_fetch: bool,
    /// One of `FETCH_MINUTES`.
    pub fetch_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { auto_fetch: DEFAULT_AUTO_FETCH, fetch_minutes: DEFAULT_FETCH_MINUTES }
    }
}

impl Settings {
    /// Reads the `"settings"` object; each key that is missing or invalid keeps its default.
    pub fn from_json(v: &Value) -> Settings {
        let d = Settings::default();
        Settings {
            auto_fetch: v["auto_fetch"].as_bool().unwrap_or(d.auto_fetch),
            fetch_minutes: v["fetch_minutes"].as_u64().map(|m| m as u32).filter(|m| FETCH_MINUTES.contains(m)).unwrap_or(d.fetch_minutes),
        }
    }

    pub fn to_json(self) -> Value {
        json!({"auto_fetch": self.auto_fetch, "fetch_minutes": self.fetch_minutes})
    }

    pub fn fetch_secs(self) -> f32 {
        self.fetch_minutes as f32 * 60.0
    }
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
                let s = Settings { auto_fetch: on, fetch_minutes: m };
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
            assert_eq!(s, Settings { auto_fetch: false, fetch_minutes: 10 }, "{bad}");
        }
        let s = Settings::from_json(&json!({"auto_fetch": "no", "fetch_minutes": 30}));
        assert_eq!(s, Settings { auto_fetch: true, fetch_minutes: 30 });
    }
}
