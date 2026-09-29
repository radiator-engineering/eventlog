//! When to rebuild: the log's own task boundaries, with a fill backstop.

use chrono::DateTime;
use serde_json::{Value, json};

use crate::context::worktree::WorkTree;
use crate::model::config::ContextConfig;
use crate::model::event::Event;
use crate::query::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub rebuild: bool,
    pub reason: &'static str,
}

impl Verdict {
    pub fn to_json(&self) -> Value {
        json!({ "rebuild": self.rebuild, "reason": self.reason })
    }
}

/// First match wins: below the floor, never; at or above the backstop
/// (fill plus expected `growth`), always; at a task boundary, yes;
/// otherwise, no. `work` reads the working tree from git, which is not free;
/// it runs only when the floor and backstop rules do not already decide the
/// verdict, so a below-floor or backstop answer never shells out to git.
pub fn decide<F>(
    percent: u8,
    growth: u8,
    cfg: &ContextConfig,
    events: &[Event],
    state: &State,
    work: F,
) -> Verdict
where
    F: FnOnce() -> WorkTree,
{
    if percent < cfg.floor_percent {
        return Verdict {
            rebuild: false,
            reason: "below-floor",
        };
    }
    if percent as u16 + growth as u16 >= cfg.backstop_percent as u16 {
        return Verdict {
            rebuild: true,
            reason: "backstop",
        };
    }
    if at_boundary(events, state, &work()) {
        return Verdict {
            rebuild: true,
            reason: "boundary",
        };
    }
    Verdict {
        rebuild: false,
        reason: "mid-task",
    }
}

fn at_boundary(events: &[Event], state: &State, work: &WorkTree) -> bool {
    let last_rebuild = events
        .iter()
        .filter(|e| e.r#type == "rebuild")
        .map(|e| e.seq)
        .max()
        .unwrap_or(0);
    let Some(result) = events
        .iter()
        .filter(|e| e.r#type == "result" && e.writer() == "controller")
        .max_by_key(|e| e.seq)
    else {
        return false;
    };
    if result.seq <= last_rebuild {
        return false;
    }
    if state.intents.iter().any(|e| e.writer() == "controller") {
        return false;
    }
    let Ok(result_ts) = DateTime::parse_from_rfc3339(&result.ts) else {
        return false;
    };
    let result_secs = result_ts.timestamp().max(0) as u64;
    !work
        .changes
        .iter()
        .any(|c| c.owner.is_none() && c.mtime > result_secs)
}
