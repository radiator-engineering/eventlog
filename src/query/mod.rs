//! The fold: every event read once, in `seq` order, into one [`State`].
//!
//! Spec section 8. `fold` runs to the tip, `fold_at` stops at a `seq`; they
//! are the same fold, so `--at N` never has a second code path. The allowlist
//! is rebuilt as it goes, so `State::allowlist` is the allowlist as of `at`
//! and a later revocation never turns history into a breach (spec section 4).

pub mod why;

use std::collections::BTreeMap;

use globset::{Glob, GlobMatcher};

use crate::model::allow::Allowlist;
use crate::model::config::Config;
use crate::model::event::Event;

/// How far an agent got. Ordered: the fold only ever moves an agent forward,
/// except a re-`spawn`, which starts its lifecycle again at [`Phase::Spawned`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Spawned,
    Prompted,
    Claimed,
    Progressing,
    Resulted,
    Retired,
}

impl Phase {
    /// The phase a line of this type puts its subject in, if any.
    fn of_type(ty: &str) -> Option<Phase> {
        match ty {
            "spawn" => Some(Phase::Spawned),
            "prompt" => Some(Phase::Prompted),
            "claim" => Some(Phase::Claimed),
            "progress" => Some(Phase::Progressing),
            "result" => Some(Phase::Resulted),
            "retire" => Some(Phase::Retired),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentState {
    pub name: String,
    pub model: Option<String>,
    pub pane: Option<String>,
    pub phase: Phase,
    pub spawned_at: u64,
    pub retired_at: Option<u64>,
    pub claims: Vec<String>,
}

/// A `by=`-tagged writer of `ack`, `intent` or `veto`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReactorState {
    pub name: String,
    /// The highest `seq_done` this reactor has acked, compared as an integer.
    pub last_ack_seq: Option<u64>,
    /// The `ts` of the ack that set `last_ack_seq`.
    pub last_ack_ts: Option<String>,
    /// Seqs of this reactor's `intent` lines with no `ack for=` behind them.
    pub open_intents: Vec<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    /// The seq this fold stopped at: the tip for [`fold`].
    pub at: u64,
    pub agents: BTreeMap<String, AgentState>,
    /// Live claims only, in the order they were claimed: `(glob, agent)`.
    pub claims: Vec<(String, String)>,
    /// `key` -> `(value, the seq that set it)`.
    pub decisions: BTreeMap<String, (String, u64)>,
    /// Open escalations: no `approval` with the same subject after them.
    pub escalations: Vec<Event>,
    /// Open intents: no `ack for=<seq>` by the same writer after them.
    pub intents: Vec<Event>,
    pub reactors: BTreeMap<String, ReactorState>,
    /// The allowlist as of `at`.
    pub allowlist: Allowlist,
    /// Agents whose lifecycle does not close: a `result`, `retire` or
    /// `progress` naming an agent that was never spawned, and an agent
    /// spawned but not retired at `at`.
    pub open_lifecycles: Vec<String>,
}

/// Fold every event, to the tip.
pub fn fold(events: &[Event], cfg: &Config) -> State {
    let tip = events.iter().map(|e| e.seq).max().unwrap_or(0);
    fold_at(events, cfg, tip)
}

/// The same fold, stopped after the line with `seq == at`. Lines above `at`
/// are not read, so nothing later can change what the log said at `at`.
pub fn fold_at(events: &[Event], cfg: &Config, at: u64) -> State {
    let mut state = State {
        at,
        allowlist: cfg.writers.clone(),
        ..State::default()
    };
    // Escalation subject -> its index in `escalations`, so an approval can
    // close it without a second pass.
    let mut open_escalations: BTreeMap<String, usize> = BTreeMap::new();
    let mut closed_escalations: Vec<usize> = Vec::new();
    // (writer, intent seq) -> index in `intents`.
    let mut open_intents: BTreeMap<(String, u64), usize> = BTreeMap::new();
    let mut closed_intents: Vec<usize> = Vec::new();
    // Agents named by a result/retire/progress before any spawn.
    let mut orphans: Vec<String> = Vec::new();

    for event in events.iter().filter(|e| e.seq <= at) {
        match event.r#type.as_str() {
            "spawn" => on_spawn(&mut state, event),
            "claim" => on_claim(&mut state, event),
            "retire" => on_retire(&mut state, event),
            "decision" => on_decision(&mut state, event),
            "approval" => {
                if let Some(subject) = event.fields.get("subject")
                    && let Some(index) = open_escalations.remove(subject)
                {
                    closed_escalations.push(index);
                }
            }
            "escalate" => {
                state.escalations.push(event.clone());
                open_escalations.insert(escalation_subject(event), state.escalations.len() - 1);
            }
            "intent" => {
                state.intents.push(event.clone());
                open_intents.insert(
                    (event.writer().to_string(), event.seq),
                    state.intents.len() - 1,
                );
            }
            "ack" => {
                on_ack(&mut state, event);
                if let Some(intent) = event.seq_ref("for")
                    && let Some(index) = open_intents.remove(&(event.writer().to_string(), intent))
                {
                    closed_intents.push(index);
                }
            }
            _ => {}
        }
        if matches!(event.r#type.as_str(), "intent" | "veto") {
            reactor(&mut state, event.writer());
        }
        note_orphan(&state, event, &mut orphans);
        advance_phase(&mut state, event);
    }

    retain_open(&mut state.escalations, &closed_escalations);
    retain_open(&mut state.intents, &closed_intents);
    let open: Vec<(String, u64)> = state
        .intents
        .iter()
        .map(|i| (i.writer().to_string(), i.seq))
        .collect();
    for (writer, seq) in open {
        reactor(&mut state, &writer).open_intents.push(seq);
    }

    state.open_lifecycles = orphans;
    for agent in state.agents.values() {
        if agent.retired_at.is_none() {
            state.open_lifecycles.push(agent.name.clone());
        }
    }
    state.open_lifecycles.dedup();
    state
}

fn on_spawn(state: &mut State, event: &Event) {
    let Some(name) = event.agent.clone() else {
        return;
    };
    state.claims.retain(|(_, owner)| owner != &name);
    state.agents.insert(
        name.clone(),
        AgentState {
            name,
            model: event.fields.get("model").cloned(),
            pane: event.fields.get("pane").cloned(),
            phase: Phase::Spawned,
            spawned_at: event.seq,
            retired_at: None,
            claims: Vec::new(),
        },
    );
}

/// A `claim` adds to what the agent already claims; it never narrows it. That
/// is what `check-claims.sh` has always done, and a widening claim is the only
/// way the controller records a boundary change.
fn on_claim(state: &mut State, event: &Event) {
    let Some(name) = event.agent.clone() else {
        return;
    };
    for path in event.paths() {
        if let Some(agent) = state.agents.get_mut(&name)
            && !agent.claims.contains(&path)
        {
            agent.claims.push(path.clone());
        }
        if !state
            .claims
            .iter()
            .any(|(glob, owner)| glob == &path && owner == &name)
        {
            state.claims.push((path, name.clone()));
        }
    }
}

fn on_retire(state: &mut State, event: &Event) {
    let Some(name) = event.agent.clone() else {
        return;
    };
    state.claims.retain(|(_, owner)| owner != &name);
    if let Some(agent) = state.agents.get_mut(&name) {
        agent.retired_at = Some(event.seq);
    }
}

fn on_decision(state: &mut State, event: &Event) {
    let (Some(key), Some(value)) = (event.fields.get("key"), event.fields.get("value")) else {
        return;
    };
    state
        .decisions
        .insert(key.clone(), (value.clone(), event.seq));
    if key == "log-writers" {
        state.allowlist.apply_decision(value);
    }
}

fn on_ack(state: &mut State, event: &Event) {
    let done = event.seq_ref("seq_done");
    let ts = event.ts.clone();
    let r = reactor(state, event.writer());
    // `seq_done` is a JSON string on disk; comparing it as one would rank
    // "9" above "51".
    if let Some(done) = done
        && r.last_ack_seq.is_none_or(|seen| done > seen)
    {
        r.last_ack_seq = Some(done);
        r.last_ack_ts = Some(ts);
    }
}

fn reactor<'a>(state: &'a mut State, name: &str) -> &'a mut ReactorState {
    state
        .reactors
        .entry(name.to_string())
        .or_insert_with(|| ReactorState {
            name: name.to_string(),
            ..ReactorState::default()
        })
}

/// The escalation's `subject`, falling back to the agent it is about, so an
/// `approval subject=<agent>` closes it.
fn escalation_subject(event: &Event) -> String {
    event
        .fields
        .get("subject")
        .cloned()
        .unwrap_or_else(|| event.subject().to_string())
}

/// A `result`, `retire` or `progress` naming an agent that has no `spawn`
/// behind it. Lines with no `agent` field are the controller's own and are not
/// a lifecycle.
fn note_orphan(state: &State, event: &Event, orphans: &mut Vec<String>) {
    if !matches!(event.r#type.as_str(), "result" | "retire" | "progress") {
        return;
    }
    let Some(name) = event.agent.as_deref() else {
        return;
    };
    if name == "controller" || state.agents.contains_key(name) {
        return;
    }
    if !orphans.iter().any(|o| o == name) {
        orphans.push(name.to_string());
    }
}

fn advance_phase(state: &mut State, event: &Event) {
    let (Some(phase), Some(name)) = (Phase::of_type(&event.r#type), event.agent.as_deref()) else {
        return;
    };
    if let Some(agent) = state.agents.get_mut(name)
        && phase > agent.phase
    {
        agent.phase = phase;
    }
}

fn retain_open<T>(items: &mut Vec<T>, closed: &[usize]) {
    let mut index = 0;
    items.retain(|_| {
        let keep = !closed.contains(&index);
        index += 1;
        keep
    });
}

impl State {
    /// Every live claim of `agent`, in claim order.
    pub fn claims_for(&self, agent: &str) -> Vec<&str> {
        self.claims
            .iter()
            .filter(|(_, owner)| owner == agent)
            .map(|(glob, _)| glob.as_str())
            .collect()
    }

    /// Who owns `path`: the first live claim that matches it. A claim is a
    /// literal path, a glob, or a directory that contains the path.
    pub fn claim_owner(&self, path: &str) -> Option<&str> {
        self.claims
            .iter()
            .find(|(glob, _)| covers(glob, path))
            .map(|(_, owner)| owner.as_str())
    }

    /// The seqs of `events` with a type in `on` that this reactor has not
    /// acked: above its `last_ack_seq`, and no higher than `at`.
    pub fn unacked(&self, reactor: &str, on: &[&str], events: &[Event]) -> Vec<u64> {
        let floor = self
            .reactors
            .get(reactor)
            .and_then(|r| r.last_ack_seq)
            .unwrap_or(0);
        events
            .iter()
            .filter(|e| e.seq > floor && e.seq <= self.at)
            .filter(|e| on.contains(&e.r#type.as_str()))
            .map(|e| e.seq)
            .collect()
    }
}

/// Does the claim `glob` cover `path`? Literal equality, a glob match, or a
/// directory prefix (`src/model` covers `src/model/mod.rs`).
fn covers(glob: &str, path: &str) -> bool {
    if glob == path {
        return true;
    }
    if path.starts_with(&format!("{}/", glob.trim_end_matches('/'))) {
        return true;
    }
    matcher(glob).is_some_and(|m| m.is_match(path))
}

fn matcher(glob: &str) -> Option<GlobMatcher> {
    Glob::new(glob).ok().map(|g| g.compile_matcher())
}
