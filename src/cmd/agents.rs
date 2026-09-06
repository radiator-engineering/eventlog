//! Per-agent lifecycle table and lifecycle flags.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::{Map, Value};

use crate::cli::{Args as CliArgs, Command};
use crate::log::Log;
use crate::model::config::{self, Config};
use crate::model::event::Event;
use crate::query::{self, Phase, State};

pub(crate) struct QueryContext {
    pub events: Vec<Event>,
    pub cfg: Config,
    pub state: State,
}

pub(crate) fn load_context(args: &CliArgs, at: Option<u64>) -> anyhow::Result<QueryContext> {
    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&repo_root)?;
    let log_path = config::resolve_log(&cfg, args.log.as_deref());
    let log_path = absolutize(&repo_root, log_path);
    let report = Log::open(&log_path).read()?;
    for (line_no, reason) in &report.malformed {
        eprintln!("eventlog: skip malformed line {line_no}: {reason}");
    }
    let events = report.events;
    let tip = events.iter().map(|e| e.seq).max().unwrap_or(0);
    let at = at.unwrap_or(tip);
    let state = query::fold_at(&events, &cfg, at);
    Ok(QueryContext { events, cfg, state })
}

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::Agents(agents_args) = &args.command else {
        anyhow::bail!("agents::run called with wrong subcommand");
    };
    let ctx = load_context(args, agents_args.at)?;
    if args.json {
        print_json(&ctx.state)?;
    } else {
        print_text(&ctx.state)?;
    }
    Ok(0)
}

fn print_text(state: &State) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "{:<24}  {:<20}  {:<10}  {:<12}  {:<40}  {:>7}  {:>7}",
        "agent", "model", "pane", "phase", "claims", "spawned", "retired"
    )?;
    for agent in state.agents.values() {
        writeln!(
            out,
            "{:<24}  {:<20}  {:<10}  {:<12}  {:<40}  {:>7}  {:>7}",
            agent.name,
            agent.model.as_deref().unwrap_or("-"),
            agent.pane.as_deref().unwrap_or("-"),
            phase_label(agent.phase),
            agent.claims.join(","),
            agent.spawned_at,
            agent
                .retired_at
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".into()),
        )?;
    }
    writeln!(out)?;
    writeln!(out, "flags:")?;
    if state.open_lifecycles.is_empty() {
        writeln!(out, "  open_lifecycles: (none)")?;
    } else {
        writeln!(
            out,
            "  open_lifecycles: {}",
            state.open_lifecycles.join(", ")
        )?;
    }
    let unreleased = unreleased_claims(state);
    if unreleased.is_empty() {
        writeln!(out, "  unreleased claims: (none)")?;
    } else {
        writeln!(out, "  unreleased claims:")?;
        for (agent, glob) in unreleased {
            writeln!(out, "    {agent}: {glob}")?;
        }
    }
    Ok(())
}

fn print_json(state: &State) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    for agent in state.agents.values() {
        let row = agent_row(agent);
        writeln!(out, "{}", serde_json::to_string(&row)?)?;
    }
    Ok(())
}

fn agent_row(agent: &query::AgentState) -> Value {
    let mut obj = Map::new();
    obj.insert("v".into(), Value::from(1));
    obj.insert("agent".into(), agent.name.clone().into());
    if let Some(model) = &agent.model {
        obj.insert("model".into(), model.clone().into());
    }
    if let Some(pane) = &agent.pane {
        obj.insert("pane".into(), pane.clone().into());
    }
    obj.insert("phase".into(), phase_label(agent.phase).into());
    obj.insert("claims".into(), agent.claims.join(",").into());
    obj.insert("spawned".into(), agent.spawned_at.into());
    if let Some(retired) = agent.retired_at {
        obj.insert("retired".into(), retired.into());
    }
    Value::Object(obj)
}

fn unreleased_claims(state: &State) -> Vec<(String, String)> {
    state
        .claims
        .iter()
        .filter(|(_, owner)| {
            state
                .agents
                .get(owner)
                .is_some_and(|a| a.retired_at.is_some())
        })
        .map(|(glob, owner)| (owner.clone(), glob.clone()))
        .collect()
}

pub(crate) fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Spawned => "spawned",
        Phase::Prompted => "prompted",
        Phase::Claimed => "claimed",
        Phase::Progressing => "progressing",
        Phase::Resulted => "resulted",
        Phase::Retired => "retired",
    }
}

pub(crate) fn absolutize(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub(crate) fn event_summary(event: &Event) -> String {
    if let Some(msg) = event.fields.get("msg") {
        return msg.clone();
    }
    if event.r#type == "decision"
        && let (Some(key), Some(value)) = (event.fields.get("key"), event.fields.get("value"))
    {
        return format!("{key}={value}");
    }
    for key in [
        "summary",
        "verdict",
        "subject",
        "outcome",
        "detail",
        "disposition",
        "task",
    ] {
        if let Some(v) = event.fields.get(key) {
            return v.clone();
        }
    }
    if let Some(paths) = event.fields.get("paths") {
        return format!("paths: {paths}");
    }
    if let Some(reference) = event.fields.get("ref") {
        return format!("ref: {reference}");
    }
    String::new()
}

pub(crate) fn format_event_line(event: &Event) -> String {
    let summary = event_summary(event);
    if summary.is_empty() {
        format!("seq {} {}", event.seq, event.r#type)
    } else {
        format!("seq {} {}  {summary}", event.seq, event.r#type)
    }
}

pub(crate) fn event_to_json(event: &Event) -> Value {
    let mut obj = Map::new();
    obj.insert("v".into(), Value::from(1));
    obj.insert("seq".into(), event.seq.into());
    obj.insert("ts".into(), event.ts.clone().into());
    obj.insert("type".into(), event.r#type.clone().into());
    if let Some(v) = &event.prev {
        obj.insert("prev".into(), v.clone().into());
    }
    if let Some(v) = &event.by {
        obj.insert("by".into(), v.clone().into());
    }
    if let Some(v) = &event.agent {
        obj.insert("agent".into(), v.clone().into());
    }
    for (k, v) in &event.fields {
        obj.insert(k.clone(), v.clone().into());
    }
    Value::Object(obj)
}

pub(crate) fn active_agents(state: &State) -> Vec<&query::AgentState> {
    state
        .agents
        .values()
        .filter(|a| a.retired_at.is_none())
        .collect()
}

pub(crate) const REACTOR_ON: &[&str] = &["result", "decision"];

pub(crate) fn format_age(ts: &str) -> String {
    use chrono::{DateTime, Utc};
    let Ok(ack) = DateTime::parse_from_rfc3339(ts) else {
        return "?".into();
    };
    let now = Utc::now();
    let ack = ack.with_timezone(&Utc);
    let delta = now.signed_duration_since(ack);
    if delta.num_seconds() < 0 {
        return "0s".into();
    }
    if delta.num_days() > 0 {
        return format!("{}d", delta.num_days());
    }
    if delta.num_hours() > 0 {
        return format!("{}h", delta.num_hours());
    }
    if delta.num_minutes() > 0 {
        return format!("{}m", delta.num_minutes());
    }
    format!("{}s", delta.num_seconds())
}
