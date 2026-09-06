//! Human and JSON rendering of log events, with optional follow mode.

use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::cli::{Args as CliArgs, Command, ViewArgs};
use crate::log::Log;
use crate::model::config::{self, Config};
use crate::model::event::{Event, ParseError};

#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

struct ViewContext {
    log_path: PathBuf,
    opts: ViewArgs,
    json: bool,
    use_color: bool,
    palette: BTreeMap<String, String>,
    since: Option<DateTime<Utc>>,
    types: Option<Vec<String>>,
}

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::View(view_args) = &args.command else {
        anyhow::bail!("view::run called with wrong subcommand");
    };
    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&repo_root)?;
    let log_path = config::resolve_log(&cfg, args.log.as_deref());
    let log_path = if log_path.is_absolute() {
        log_path
    } else {
        repo_root.join(log_path)
    };
    run_with_paths(args, view_args.clone(), cfg, log_path)
}

fn run_with_paths(
    args: &CliArgs,
    opts: ViewArgs,
    cfg: Config,
    log_path: PathBuf,
) -> anyhow::Result<i32> {
    let use_color = match opts.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => io::stdout().is_terminal(),
    };
    let since = opts
        .since
        .as_deref()
        .map(parse_since)
        .transpose()
        .context("parsing --since")?;
    let types = opts.types.as_deref().map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()
    });
    let palette = build_palette(&cfg);
    let ctx = ViewContext {
        log_path,
        opts,
        json: args.json,
        use_color,
        palette,
        since,
        types,
    };
    if ctx.opts.follow {
        follow(&ctx)?;
    } else {
        let report = Log::open(&ctx.log_path).read()?;
        for (line_no, reason) in &report.malformed {
            eprintln!("eventlog view: skip malformed line {line_no}: {reason}");
        }
        let mut events = report.events;
        apply_filters(&ctx, &mut events);
        print_events(&ctx, &events)?;
    }
    Ok(0)
}

fn parse_since(raw: &str) -> anyhow::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .with_context(|| format!("invalid RFC 3339 timestamp: {raw}"))
}

fn build_palette(cfg: &Config) -> BTreeMap<String, String> {
    let mut palette = default_palette();
    for (k, v) in &cfg.view.colors {
        palette.insert(k.clone(), v.clone());
    }
    palette
}

fn default_palette() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("seq".into(), "2".into()),
        ("agent".into(), "1".into()),
        ("ref".into(), "2".into()),
        ("result".into(), "1;36".into()),
        ("decision".into(), "1;33".into()),
        ("ack".into(), "36".into()),
        ("spawn".into(), "1;32".into()),
        ("prompt".into(), "36".into()),
        ("retire".into(), "2".into()),
        ("claim".into(), "34".into()),
        ("progress".into(), "2".into()),
        ("escalate".into(), "1;35".into()),
        ("violation".into(), "1;31".into()),
        ("note".into(), "2".into()),
    ])
}

fn apply_filters(ctx: &ViewContext, events: &mut Vec<Event>) {
    events.retain(|event| matches_filters(ctx, event));
    if let Some(n) = ctx.opts.last
        && events.len() > n
    {
        *events = events.split_off(events.len() - n);
    }
}

fn matches_filters(ctx: &ViewContext, event: &Event) -> bool {
    if let Some(types) = &ctx.types
        && !types.iter().any(|t| t == &event.r#type)
    {
        return false;
    }
    if let Some(agent) = &ctx.opts.agent
        && !agent_matches(event, agent)
    {
        return false;
    }
    if let Some(by) = &ctx.opts.by
        && event.writer() != by.as_str()
    {
        return false;
    }
    if let Some(since) = ctx.since
        && let Ok(ts) = DateTime::parse_from_rfc3339(&event.ts)
        && ts.with_timezone(&Utc) < since
    {
        return false;
    }
    if let Some(grep) = &ctx.opts.grep {
        let formatted = format_event(ctx, event);
        if !formatted.contains(grep) && !event.to_line().contains(grep) {
            return false;
        }
    }
    true
}

fn agent_matches(event: &Event, agent: &str) -> bool {
    event.subject() == agent
        || event.writer() == agent
        || event.fields.get("from").is_some_and(|v| v == agent)
        || event.fields.get("to").is_some_and(|v| v == agent)
}

fn print_events(ctx: &ViewContext, events: &[Event]) -> io::Result<()> {
    let mut out = io::stdout().lock();
    for event in events {
        writeln!(out, "{}", render_row(ctx, event))?;
    }
    Ok(())
}

fn render_row(ctx: &ViewContext, event: &Event) -> String {
    if ctx.json {
        event_json_row(event)
    } else {
        format_event(ctx, event)
    }
}

fn event_json_row(event: &Event) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("v".into(), 1.into());
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
    serde_json::Value::Object(obj).to_string()
}

fn format_event(ctx: &ViewContext, event: &Event) -> String {
    let seq = paint(ctx, "seq", &pad(event.seq.to_string(), 4));
    let ty = paint(
        ctx,
        &event.r#type,
        &pad(event.r#type.to_ascii_uppercase(), 10),
    );
    let agent = paint(ctx, "agent", &pad(event.subject().to_string(), 14));
    let summary = summary_for(event);
    let mut line = format!("{seq}  {ty}  {agent}  {summary}");
    if let Some(reference) = event.fields.get("ref") {
        line.push_str("  ");
        line.push_str(&paint(ctx, "ref", &format!("→ {reference}")));
    }
    line
}

fn summary_for(event: &Event) -> String {
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
    if let Some(to) = event.fields.get("to") {
        return format!("→ {to}");
    }
    String::new()
}

fn pad(s: String, width: usize) -> String {
    if s.len() >= width {
        s
    } else {
        format!("{s}{}", " ".repeat(width - s.len()))
    }
}

fn paint(ctx: &ViewContext, key: &str, text: &str) -> String {
    if !ctx.use_color {
        return text.to_string();
    }
    let code = ctx.palette.get(key).map(String::as_str).unwrap_or("0");
    format!("\x1b[{code}m{text}\x1b[0m")
}

fn follow(ctx: &ViewContext) -> anyhow::Result<()> {
    let report = Log::open(&ctx.log_path).read()?;
    for (line_no, reason) in &report.malformed {
        eprintln!("eventlog view: skip malformed line {line_no}: {reason}");
    }
    let mut events = report.events;
    apply_filters(ctx, &mut events);
    print_events(ctx, &events)?;
    let mut offset = file_len(&ctx.log_path)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let watch_path = ctx.log_path.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        notify::Config::default(),
    )
    .context("creating file watcher")?;
    if watch_path.exists() {
        watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;
    } else if let Some(parent) = watch_path.parent() {
        watcher.watch(parent, RecursiveMode::NonRecursive)?;
    }
    let mut pending = String::new();
    loop {
        let _ = rx.recv_timeout(Duration::from_millis(500));
        if !ctx.log_path.exists() {
            continue;
        }
        if offset == 0 && file_len(&ctx.log_path)? > 0 {
            let _ = watcher.watch(&ctx.log_path, RecursiveMode::NonRecursive);
        }
        let chunk = read_from_offset(&ctx.log_path, offset)?;
        offset += chunk.len() as u64;
        if chunk.is_empty() {
            continue;
        }
        pending.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = pending.find('\n') {
            let line = pending[..pos].trim_end_matches('\r').to_string();
            pending = pending[pos + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            match Event::parse_line(&line) {
                Ok(event) if matches_filters(ctx, &event) => {
                    println!("{}", render_row(ctx, &event));
                }
                Ok(_) => {}
                Err(ParseError { line_hint, reason }) => {
                    eprintln!("eventlog view: skip malformed line: {reason}: {line_hint}");
                }
            }
        }
    }
}

fn file_len(path: &Path) -> anyhow::Result<u64> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(meta.len()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e.into()),
    }
}

fn read_from_offset(path: &Path, offset: u64) -> anyhow::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_filter_uses_by_field() {
        let event = Event::parse_line(
            r#"{"seq":5,"ts":"2026-09-06T14:21:52Z","type":"ack","by":"doc-worker","seq_done":"4","outcome":"skipped","detail":"baseline"}"#,
        )
        .unwrap();
        assert!(agent_matches(&event, "doc-worker"));
    }
}
