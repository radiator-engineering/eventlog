//! The context packet: a pure render of the log, state, and working tree.

use serde_json::{Map, Value, json};

use crate::cmd::agents::{REACTOR_ON, active_agents, phase_label};
use crate::context::age::age_between;
use crate::context::worktree::WorkTree;
use crate::model::config::ContextConfig;
use crate::model::event::Event;
use crate::query::State;

const HISTORY_TYPES: &[&str] = &["result", "decision", "violation", "observed", "note"];
const HISTORY_LEN: usize = 15;
const STALE_ACK_SECS: i64 = 86_400;

pub struct PacketInput<'a> {
    pub events: &'a [Event],
    pub state: &'a State,
    pub log_path: &'a str,
    pub work: &'a WorkTree,
    pub read_file: &'a dyn Fn(&str) -> Option<String>,
    pub exists: &'a dyn Fn(&str) -> bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub key: &'static str,
    pub title: &'static str,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub as_of: u64,
    pub sections: Vec<Section>,
    pub over_budget: bool,
}

pub fn build(input: &PacketInput, budget: usize) -> Packet {
    let tip = input.events.iter().max_by_key(|e| e.seq);
    let as_of = tip.map(|e| e.seq).unwrap_or(0);
    let tip_ts = tip.map(|e| e.ts.clone()).unwrap_or_default();

    let history: Vec<&Event> = {
        let mut h: Vec<&Event> = input
            .events
            .iter()
            .filter(|e| HISTORY_TYPES.contains(&e.r#type.as_str()))
            .collect();
        let skip = h.len().saturating_sub(HISTORY_LEN);
        h.drain(..skip);
        h
    };

    let mut sections = vec![
        Section {
            key: "header",
            title: "",
            lines: header(input.log_path, as_of),
        },
        Section {
            key: "decisions",
            title: "Decisions in force",
            lines: decisions(input),
        },
        Section {
            key: "agents",
            title: "Agents",
            lines: agents(input),
        },
        Section {
            key: "history",
            title: "Recent history",
            lines: history.iter().map(|e| history_line(e, &tip_ts)).collect(),
        },
        Section {
            key: "artifacts",
            title: "Artifact index",
            lines: artifacts(&history, input.exists),
        },
    ];
    let reactors = reactor_health(input, &tip_ts);
    if !reactors.is_empty() {
        sections.push(Section {
            key: "reactors",
            title: "Reactor health",
            lines: reactors,
        });
    }
    sections.push(Section {
        key: "open_work",
        title: "Open work",
        lines: open_work(input),
    });
    if let Some(lines) = current_task(input, budget) {
        sections.push(Section {
            key: "current_task",
            title: "Current task",
            lines,
        });
    }
    for s in &mut sections {
        if s.lines.is_empty() && s.key != "header" {
            s.lines.push("- (none)".into());
        }
    }

    let mut packet = Packet {
        as_of,
        sections,
        over_budget: false,
    };
    packet.fit(budget);
    packet
}

impl Packet {
    pub fn markdown(&self) -> String {
        let mut out = String::from("# Context rebuilt from the event log\n\n");
        for s in &self.sections {
            if !s.title.is_empty() {
                out.push_str(&format!("## {}\n\n", s.title));
            }
            for line in &s.lines {
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        out
    }

    pub fn to_json(&self, cfg: &ContextConfig) -> Value {
        let mut sections = Map::new();
        for s in &self.sections {
            sections.insert(s.key.into(), json!(s.lines));
        }
        json!({
            "v": 1,
            "as_of": self.as_of,
            "over_budget": self.over_budget,
            "markdown": self.markdown(),
            "sections": sections,
            "settings": { "keep_turns": cfg.keep_turns, "tail_chars": cfg.tail_chars },
        })
    }

    /// Drop the oldest history lines, then the oldest artifact lines, until
    /// the markdown fits. Nothing else is ever dropped.
    fn fit(&mut self, budget: usize) {
        while self.markdown().chars().count() > budget {
            if self.drop_first_line("history") || self.drop_last_line("artifacts") {
                continue;
            }
            self.over_budget = true;
            return;
        }
    }

    fn drop_first_line(&mut self, key: &str) -> bool {
        let Some(s) = self.sections.iter_mut().find(|s| s.key == key) else {
            return false;
        };
        if s.lines.first().is_some_and(|l| l != "- (none)") {
            s.lines.remove(0);
            if s.lines.is_empty() {
                s.lines.push("- (none)".into());
            }
            return true;
        }
        false
    }

    fn drop_last_line(&mut self, key: &str) -> bool {
        let Some(s) = self.sections.iter_mut().find(|s| s.key == key) else {
            return false;
        };
        if s.lines.last().is_some_and(|l| l != "- (none)") {
            s.lines.pop();
            if s.lines.is_empty() {
                s.lines.push("- (none)".into());
            }
            return true;
        }
        false
    }
}

fn header(log_path: &str, as_of: u64) -> Vec<String> {
    vec![
        "You are the controller of this repo. Claude Code rebuilt your context from the event log: keep working as the controller, not as a reviewer or worker.".into(),
        format!("Rebuilt from `{log_path}` as of seq {as_of}. The log is the source of truth."),
        "Read an event's artifact with `eventlog open <seq>`. Read recent events with `eventlog view --last 20`.".into(),
    ]
}

fn field<'a>(e: &'a Event, k: &str) -> Option<&'a str> {
    e.fields.get(k).map(String::as_str)
}

fn decisions(input: &PacketInput) -> Vec<String> {
    input
        .state
        .decisions
        .iter()
        .map(|(k, (v, seq))| {
            let r = input
                .events
                .iter()
                .find(|e| e.seq == *seq)
                .and_then(|e| field(e, "ref"));
            match r {
                Some(r) => format!("- {k}={v} (seq {seq}, {r})"),
                None => format!("- {k}={v} (seq {seq})"),
            }
        })
        .collect()
}

fn agents(input: &PacketInput) -> Vec<String> {
    active_agents(input.state)
        .into_iter()
        .map(|a| {
            let brief = input
                .events
                .iter()
                .rev()
                .find(|e| e.r#type == "prompt" && e.agent.as_deref() == Some(a.name.as_str()))
                .and_then(|e| field(e, "ref"))
                .unwrap_or("-");
            format!(
                "- {} phase={} claims={} brief={}",
                a.name,
                phase_label(a.phase),
                a.claims.join(","),
                brief
            )
        })
        .collect()
}

fn history_line(e: &Event, tip_ts: &str) -> String {
    let who = e.agent.as_deref().unwrap_or_else(|| e.writer());
    let what = field(e, "summary")
        .or_else(|| field(e, "msg"))
        .map(str::to_string)
        .or_else(|| Some(format!("{}={}", field(e, "key")?, field(e, "value")?)))
        .or_else(|| field(e, "paths").map(|p| format!("paths={p}")))
        .unwrap_or_default();
    let mut line = format!(
        "- seq {} ({} ago) {} {}: {}",
        e.seq,
        age_between(&e.ts, tip_ts),
        e.r#type,
        who,
        what
    );
    if e.r#type == "result"
        && let Some(p) = field(e, "paths")
    {
        line.push_str(&format!(" paths={p}"));
    }
    line
}

fn artifacts(history: &[&Event], exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    let mut lines = Vec::new();
    for e in history.iter().rev() {
        let Some(r) = field(e, "ref") else { continue };
        if seen.contains(&r) {
            continue;
        }
        seen.push(r);
        let missing = if exists(r) { "" } else { " (missing)" };
        lines.push(format!("- {r} (seq {}){missing}", e.seq));
    }
    lines
}

/// Reactor unacked counts and staleness, excluding the controller itself
/// (which never acks and would otherwise always look unacked).
fn reactor_health(input: &PacketInput, tip_ts: &str) -> Vec<String> {
    use chrono::DateTime;
    let secs_before_tip = |ts: &str| -> Option<i64> {
        let t = DateTime::parse_from_rfc3339(ts).ok()?;
        let tip = DateTime::parse_from_rfc3339(tip_ts).ok()?;
        Some(tip.signed_duration_since(t).num_seconds())
    };
    input
        .state
        .reactors
        .values()
        .filter(|r| r.name != "controller")
        .filter_map(|r| {
            let unacked = input.state.unacked(&r.name, REACTOR_ON, input.events).len();
            let stale = r
                .last_ack_ts
                .as_deref()
                .and_then(secs_before_tip)
                .is_some_and(|s| s > STALE_ACK_SECS);
            (unacked > 0 || stale).then(|| {
                let last_ack = match r.last_ack_ts.as_deref() {
                    Some(t) => format!("{} ago", age_between(t, tip_ts)),
                    None => "never".to_string(),
                };
                format!("- {} unacked={} last_ack={}", r.name, unacked, last_ack)
            })
        })
        .collect()
}

fn open_work(input: &PacketInput) -> Vec<String> {
    let mut lines = Vec::new();
    for e in &input.state.intents {
        let mut l = format!(
            "- intent seq {} {}: {}",
            e.seq,
            e.writer(),
            field(e, "msg").unwrap_or("")
        );
        if let Some(p) = field(e, "paths") {
            l.push_str(&format!(" paths={p}"));
        }
        if let Some(r) = field(e, "ref") {
            l.push_str(&format!(" ref={r}"));
        }
        lines.push(l);
    }
    for e in &input.state.escalations {
        let what = field(e, "subject")
            .or_else(|| field(e, "msg"))
            .unwrap_or("");
        lines.push(format!("- escalation seq {}: {what}", e.seq));
    }
    if !input.work.available {
        lines.push("- (working tree unavailable)".into());
    }
    for c in &input.work.changes {
        let mut l = format!("- {} {} {}", c.status, c.path, c.stat)
            .trim_end()
            .to_string();
        if let Some(o) = &c.owner {
            l.push_str(&format!(" [claimed by {o}]"));
        }
        lines.push(l);
    }
    lines
}

/// The controller's newest open intent, if it has a `ref`: the only file the
/// packet inlines. Reactor and worker intents never count.
fn current_task(input: &PacketInput, budget: usize) -> Option<Vec<String>> {
    let intent = input
        .state
        .intents
        .iter()
        .rev()
        .find(|e| e.writer() == "controller")?;
    let r = field(intent, "ref")?;
    let Some(text) = (input.read_file)(r) else {
        return Some(vec![format!("- {r} (missing)")]);
    };
    let mut lines = vec![format!("From `{r}`:"), String::new()];
    // Spec: "the command inlines at most half of budget_chars characters of
    // it, so history and the artifact index keep room".
    let cap = budget / 2;
    if text.chars().count() > cap {
        let cut = text
            .char_indices()
            .nth(cap)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        lines.extend(text[..cut].lines().map(str::to_string));
        lines.push(format!("(truncated; read `{r}` for the rest)"));
    } else {
        lines.extend(text.lines().map(str::to_string));
    }
    Some(lines)
}
