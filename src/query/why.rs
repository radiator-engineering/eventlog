//! Explain one event: what led to it, what it drove, and how reactors acted on it.
//!
//! Spec section 8 (`why`). Reference fields are walked numerically via
//! [`Event::seq_ref`]; `origin` on an ack names the writer whose latest
//! `result` before the ack is a cause.

use crate::model::config::Config;
use crate::model::event::Event;
use crate::query::{State, fold};

const REF_FIELDS: [&str; 4] = ["for", "for_ack", "seq_done", "intent"];

#[derive(Clone, Debug, PartialEq)]
pub struct WhyReport {
    pub event: Event,
    pub causes: Vec<Event>,
    pub effects: Vec<Event>,
    pub verdict: String,
}

/// Walk causes and effects for the event at `seq`. `None` when `seq` is absent.
pub fn why(events: &[Event], cfg: &Config, seq: u64) -> Option<WhyReport> {
    let by_seq: std::collections::BTreeMap<u64, &Event> =
        events.iter().map(|e| (e.seq, e)).collect();
    let event = (*by_seq.get(&seq)?).clone();

    let mut causes = referenced_events(&by_seq, &event);
    if let Some(origin) = origin_result(events, &event) {
        push_unique(&mut causes, origin);
    }
    causes.sort_by_key(|e| e.seq);

    let mut effects: Vec<Event> = events
        .iter()
        .filter(|e| references_seq(e, seq))
        .cloned()
        .collect();
    effects.sort_by_key(|e| e.seq);

    let state = fold(events, cfg);
    let verdict = verdict_for(&event, &effects, &state);

    Some(WhyReport {
        event,
        causes,
        effects,
        verdict,
    })
}

fn referenced_events(
    by_seq: &std::collections::BTreeMap<u64, &Event>,
    event: &Event,
) -> Vec<Event> {
    let mut out = Vec::new();
    for field in REF_FIELDS {
        if let Some(target) = event.seq_ref(field)
            && let Some(&cause) = by_seq.get(&target)
        {
            push_unique(&mut out, (*cause).clone());
        }
    }
    out
}

fn references_seq(event: &Event, seq: u64) -> bool {
    REF_FIELDS
        .iter()
        .any(|field| event.seq_ref(field) == Some(seq))
}

/// Latest `result` from the `origin` writer strictly before `event`.
fn origin_result(events: &[Event], event: &Event) -> Option<Event> {
    let origin = event.fields.get("origin")?;
    events
        .iter()
        .filter(|e| e.r#type == "result" && e.writer() == origin.as_str() && e.seq < event.seq)
        .max_by_key(|e| e.seq)
        .cloned()
}

fn push_unique(out: &mut Vec<Event>, event: Event) {
    if !out.iter().any(|e| e.seq == event.seq) {
        out.push(event);
    }
}

fn verdict_for(event: &Event, effects: &[Event], state: &State) -> String {
    if !matches!(event.r#type.as_str(), "result" | "decision") {
        return String::new();
    }

    for effect in effects {
        if effect.r#type != "ack" {
            continue;
        }
        if effect.seq_ref("seq_done") != Some(event.seq) {
            continue;
        }
        let outcome = effect
            .fields
            .get("outcome")
            .map(String::as_str)
            .unwrap_or("unknown");
        return format!(
            "acted on by {} at seq {} (outcome={})",
            effect.writer(),
            effect.seq,
            outcome
        );
    }

    // `--on` lives in reactor config (Task 14); until it is loaded, every
    // reactor filter is unknown and we report the missing ack plainly.
    let _ = state;
    "no ack references this seq".to_string()
}
