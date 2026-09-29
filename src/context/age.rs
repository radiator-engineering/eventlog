//! Ages measured back from the newest event, so a packet is deterministic.

use chrono::{DateTime, Utc};

/// How long before `tip` the time `ts` was: `0s`, `42s`, `5m`, `3h`, `2d`.
/// `?` when either time does not parse.
pub fn age_between(ts: &str, tip: &str) -> String {
    let (Ok(t), Ok(tip)) = (
        DateTime::parse_from_rfc3339(ts),
        DateTime::parse_from_rfc3339(tip),
    ) else {
        return "?".into();
    };
    let secs = tip
        .with_timezone(&Utc)
        .signed_duration_since(t.with_timezone(&Utc))
        .num_seconds()
        .max(0);
    match secs {
        s if s >= 86_400 => format!("{}d", s / 86_400),
        s if s >= 3_600 => format!("{}h", s / 3_600),
        s if s >= 60 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}
