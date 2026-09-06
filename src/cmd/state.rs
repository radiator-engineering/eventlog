//! Folded state summary: active agents, claims, decisions, reactors.

use std::io::{self, Write};

use serde_json::{Map, Value};

use crate::cli::{Args as CliArgs, Command};
use crate::cmd::agents::{
    REACTOR_ON, active_agents, format_age, format_event_line, load_context, phase_label,
};
use crate::model::event::Event;

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::State(state_args) = &args.command else {
        anyhow::bail!("state::run called with wrong subcommand");
    };
    let ctx = load_context(args, state_args.at)?;
    if args.json {
        print_json(&ctx)?;
    } else {
        print_text(&ctx)?;
    }
    Ok(0)
}

fn print_text(ctx: &crate::cmd::agents::QueryContext) -> anyhow::Result<()> {
    let state = &ctx.state;
    let mut out = io::stdout().lock();

    writeln!(out, "active agents:")?;
    let active = active_agents(state);
    if active.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for agent in active {
            writeln!(
                out,
                "  {}  phase={}  spawned={}",
                agent.name,
                phase_label(agent.phase),
                agent.spawned_at
            )?;
        }
    }

    writeln!(out, "open claims:")?;
    if state.claims.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for (glob, owner) in &state.claims {
            writeln!(out, "  {owner}: {glob}")?;
        }
    }

    writeln!(out, "decisions in force:")?;
    if state.decisions.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for (key, (value, seq)) in &state.decisions {
            writeln!(out, "  {key}={value}  (seq {seq})")?;
        }
    }

    writeln!(out, "open escalations:")?;
    if state.escalations.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for event in &state.escalations {
            writeln!(out, "  {}", format_event_line(event))?;
        }
    }

    writeln!(out, "open intents:")?;
    if state.intents.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for event in &state.intents {
            writeln!(out, "  {}", format_event_line(event))?;
        }
    }

    writeln!(out, "reactors:")?;
    if state.reactors.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for reactor in state.reactors.values() {
            let last_ack = reactor
                .last_ack_seq
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".into());
            let age = reactor
                .last_ack_ts
                .as_deref()
                .map(format_age)
                .unwrap_or_else(|| "-".into());
            let unacked = state.unacked(&reactor.name, REACTOR_ON, &ctx.events).len();
            writeln!(
                out,
                "  {}  last_ack={}  age={}  unacked={}",
                reactor.name, last_ack, age, unacked
            )?;
        }
    }

    Ok(())
}

fn print_json(ctx: &crate::cmd::agents::QueryContext) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    let report = state_report(ctx);
    writeln!(out, "{}", serde_json::to_string(&report)?)?;
    Ok(())
}

fn state_report(ctx: &crate::cmd::agents::QueryContext) -> Value {
    let state = &ctx.state;
    let mut obj = Map::new();
    obj.insert("v".into(), Value::from(1));
    obj.insert("at".into(), state.at.into());
    obj.insert(
        "active_agents".into(),
        Value::Array(
            active_agents(state)
                .into_iter()
                .map(|a| {
                    let mut row = Map::new();
                    row.insert("name".into(), a.name.clone().into());
                    row.insert("phase".into(), phase_label(a.phase).into());
                    row.insert("spawned".into(), a.spawned_at.into());
                    Value::Object(row)
                })
                .collect(),
        ),
    );
    obj.insert(
        "open_claims".into(),
        Value::Array(
            state
                .claims
                .iter()
                .map(|(glob, owner)| {
                    let mut row = Map::new();
                    row.insert("agent".into(), owner.clone().into());
                    row.insert("path".into(), glob.clone().into());
                    Value::Object(row)
                })
                .collect(),
        ),
    );
    obj.insert(
        "decisions".into(),
        Value::Array(
            state
                .decisions
                .iter()
                .map(|(key, (value, seq))| {
                    let mut row = Map::new();
                    row.insert("key".into(), key.clone().into());
                    row.insert("value".into(), value.clone().into());
                    row.insert("seq".into(), (*seq).into());
                    Value::Object(row)
                })
                .collect(),
        ),
    );
    obj.insert("open_escalations".into(), events_array(&state.escalations));
    obj.insert("open_intents".into(), events_array(&state.intents));
    obj.insert(
        "reactors".into(),
        Value::Array(
            state
                .reactors
                .values()
                .map(|reactor| {
                    let mut row = Map::new();
                    row.insert("name".into(), reactor.name.clone().into());
                    if let Some(seq) = reactor.last_ack_seq {
                        row.insert("last_ack_seq".into(), seq.into());
                    }
                    if let Some(ts) = &reactor.last_ack_ts {
                        row.insert("last_ack_ts".into(), ts.clone().into());
                        row.insert("age".into(), format_age(ts).into());
                    }
                    let unacked = state.unacked(&reactor.name, REACTOR_ON, &ctx.events);
                    row.insert("unacked_count".into(), unacked.len().into());
                    row.insert(
                        "unacked".into(),
                        Value::Array(unacked.into_iter().map(Value::from).collect()),
                    );
                    Value::Object(row)
                })
                .collect(),
        ),
    );
    Value::Object(obj)
}

fn events_array(events: &[Event]) -> Value {
    Value::Array(
        events
            .iter()
            .map(crate::cmd::agents::event_to_json)
            .collect(),
    )
}
