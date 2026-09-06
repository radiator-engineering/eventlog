//! Explain one event: causes, effects, and reactor verdict.

use std::io::{self, Write};

use serde_json::{Map, Value};

use crate::cli::{Args as CliArgs, Command};
use crate::cmd::agents::{event_to_json, format_event_line, load_context};
use crate::query::why::{self, WhyReport};

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::Why(why_args) = &args.command else {
        anyhow::bail!("why::run called with wrong subcommand");
    };
    let ctx = load_context(args, None)?;
    let Some(report) = why::why(&ctx.events, &ctx.cfg, why_args.seq) else {
        anyhow::bail!("no event at seq {}", why_args.seq);
    };
    if args.json {
        print_json(&report)?;
    } else {
        print_text(&report)?;
    }
    Ok(0)
}

fn print_text(report: &WhyReport) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(out, "causes:")?;
    if report.causes.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for event in &report.causes {
            writeln!(out, "  {}", format_event_line(event))?;
        }
    }
    writeln!(out, "event:")?;
    writeln!(out, "  {}", format_event_line(&report.event))?;
    writeln!(out, "effects:")?;
    if report.effects.is_empty() {
        writeln!(out, "  (none)")?;
    } else {
        for event in &report.effects {
            writeln!(out, "  {}", format_event_line(event))?;
        }
    }
    if !report.verdict.is_empty() {
        writeln!(out, "verdict:")?;
        writeln!(out, "  {}", report.verdict)?;
    }
    Ok(())
}

fn print_json(report: &WhyReport) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(out, "{}", serde_json::to_string(&report_json(report))?)?;
    Ok(())
}

fn report_json(report: &WhyReport) -> Value {
    let mut obj = Map::new();
    obj.insert("v".into(), Value::from(1));
    obj.insert(
        "causes".into(),
        Value::Array(report.causes.iter().map(event_to_json).collect()),
    );
    obj.insert("event".into(), event_to_json(&report.event));
    obj.insert(
        "effects".into(),
        Value::Array(report.effects.iter().map(event_to_json).collect()),
    );
    if !report.verdict.is_empty() {
        obj.insert("verdict".into(), report.verdict.clone().into());
    }
    Value::Object(obj)
}
