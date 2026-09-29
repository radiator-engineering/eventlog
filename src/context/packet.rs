//! The context packet: a pure render of the log, state, and working tree.

use serde_json::{Map, Value, json};

use crate::cmd::agents::{REACTOR_ON, active_agents, phase_label};
use crate::context::age::age_between;
use crate::context::worktree::{Change, WorkTree};
use crate::model::config::ContextConfig;
use crate::model::event::Event;
use crate::query::State;

const HISTORY_TYPES: &[&str] = &["result", "decision", "violation", "observed", "note"];
const HISTORY_LEN: usize = 15;
const STALE_ACK_SECS: i64 = 86_400;
/// A line longer than this is cut; the packet points at the log for the rest.
const LINE_MAX: usize = 200;
/// Lines shown per section before older ones are summarized as a count.
const CAP_LISTS: usize = 20;
const CAP_AGENTS: usize = 10;
/// The budget loop never cuts decisions or open work below this many lines,
/// nor history below `MIN_HISTORY`, so no section a rebuilt controller needs
/// is emptied to make room for another.
const MIN_LINES: usize = 4;
const MIN_HISTORY: usize = 5;

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
    /// Older lines left out to fit; shown as a count line.
    pub omitted: usize,
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
            omitted: 0,
        },
        Section {
            key: "decisions",
            title: "Decisions in force",
            lines: decisions(input),
            omitted: 0,
        },
        Section {
            key: "agents",
            title: "Agents",
            lines: agents(input),
            omitted: 0,
        },
        Section {
            key: "history",
            title: "Recent history",
            lines: history.iter().map(|e| history_line(e, &tip_ts)).collect(),
            omitted: 0,
        },
        Section {
            key: "artifacts",
            title: "Artifact index",
            lines: artifacts(&history, input.exists),
            omitted: 0,
        },
    ];
    let reactors = reactor_health(input, &tip_ts);
    if !reactors.is_empty() {
        sections.push(Section {
            key: "reactors",
            title: "Reactor health",
            lines: reactors,
            omitted: 0,
        });
    }
    sections.push(Section {
        key: "open_work",
        title: "Open work",
        lines: open_work(input),
        omitted: 0,
    });
    if let Some(lines) = current_task(input, budget) {
        sections.push(Section {
            key: "current_task",
            title: "Current task",
            lines,
            omitted: 0,
        });
    }
    for s in &mut sections {
        if s.key != "current_task" {
            for l in &mut s.lines {
                *l = cut_line(l);
            }
        }
        let cap = match s.key {
            "decisions" | "open_work" => CAP_LISTS,
            "agents" => CAP_AGENTS,
            _ => usize::MAX,
        };
        if s.lines.len() > cap {
            s.omitted += s.lines.len() - cap;
            s.lines.truncate(cap);
        }
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
            if s.omitted > 0 {
                out.push_str(&format!(
                    "- (+{} older not shown; `eventlog state --json` lists them all)\n",
                    s.omitted
                ));
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

    /// Fit the markdown to the budget. Newest-first lists (decisions, open
    /// work) lose their oldest lines and history loses its oldest lines, but
    /// only down to a floor, so every section keeps something. Order: the
    /// artifact index, then the longer of decisions and open work, then
    /// history, then agents. Anything left is over budget and printed whole.
    fn fit(&mut self, budget: usize) {
        while self.markdown().chars().count() > budget {
            if self.trim("artifacts", false, 0) {
                continue;
            }
            let longer = if self.len_of("decisions") >= self.len_of("open_work") {
                ["decisions", "open_work"]
            } else {
                ["open_work", "decisions"]
            };
            if longer.iter().any(|k| self.trim(k, false, MIN_LINES)) {
                continue;
            }
            if self.trim("history", true, MIN_HISTORY) || self.trim("agents", false, MIN_LINES) {
                continue;
            }
            self.over_budget = true;
            return;
        }
    }

    fn len_of(&self, key: &str) -> usize {
        self.sections
            .iter()
            .find(|s| s.key == key)
            .map_or(0, |s| s.lines.len())
    }

    /// Drop one line from the front (`from_front`, for oldest-first history)
    /// or the back of a section, keeping at least `min` lines. Every dropped
    /// line but the artifact index's counts as omitted.
    fn trim(&mut self, key: &str, from_front: bool, min: usize) -> bool {
        let Some(s) = self.sections.iter_mut().find(|s| s.key == key) else {
            return false;
        };
        if s.lines.first().is_some_and(|l| l == "- (none)") || s.lines.len() <= min {
            return false;
        }
        if from_front {
            s.lines.remove(0);
        } else {
            s.lines.pop();
        }
        if key != "artifacts" {
            s.omitted += 1;
        }
        if s.lines.is_empty() {
            s.lines.push("- (none)".into());
        }
        true
    }
}

/// Cut a line to `LINE_MAX` characters, on a character boundary.
fn cut_line(l: &str) -> String {
    if l.chars().count() <= LINE_MAX {
        return l.to_string();
    }
    let cut = l.char_indices().nth(LINE_MAX).map_or(l.len(), |(i, _)| i);
    format!("{}…", &l[..cut])
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

/// A decision whose value is `retired` is off the list until the key is
/// decided again. Newest decisions come first.
fn decisions(input: &PacketInput) -> Vec<String> {
    let mut live: Vec<_> = input
        .state
        .decisions
        .iter()
        .filter(|(_, (v, _))| !v.trim().eq_ignore_ascii_case("retired"))
        .collect();
    live.sort_by_key(|(_, (_, seq))| std::cmp::Reverse(*seq));
    live.into_iter()
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
        .filter(|a| !a.name.trim().is_empty())
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

/// Working tree changes first (they are the live state), then open
/// intents, then open escalations, each newest first. The oldest lines are
/// the first to go when the packet is trimmed.
fn open_work(input: &PacketInput) -> Vec<String> {
    let mut lines = Vec::new();
    if !input.work.available {
        lines.push("- (working tree unavailable)".into());
    }
    lines.extend(change_lines(&input.work.changes));
    for e in input.state.intents.iter().rev() {
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
    for e in input.state.escalations.iter().rev() {
        let what = field(e, "subject")
            .or_else(|| field(e, "msg"))
            .unwrap_or("");
        lines.push(format!("- escalation seq {}: {what}", e.seq));
    }
    lines
}

/// Untracked files under one directory collapse to a single line once there
/// are more than `COLLAPSE_OVER` of them (a virtualenv or a build folder would
/// otherwise fill the packet). The directory is the first three path parts,
/// less the file name. Tracked changes always print one line each.
const COLLAPSE_OVER: usize = 3;

fn change_lines(changes: &[Change]) -> Vec<String> {
    let dir_of = |c: &Change| -> Option<String> {
        if c.status != "??" {
            return None;
        }
        let parts: Vec<&str> = c.path.split('/').collect();
        let keep = parts.len().saturating_sub(1).min(3);
        (keep > 0).then(|| parts[..keep].join("/"))
    };
    let mut counts: Vec<(String, usize)> = Vec::new();
    for c in changes {
        if let Some(d) = dir_of(c) {
            match counts.iter_mut().find(|(k, _)| *k == d) {
                Some((_, n)) => *n += 1,
                None => counts.push((d, 1)),
            }
        }
    }
    let mut shown: Vec<String> = Vec::new();
    let mut lines = Vec::new();
    for c in changes {
        if let Some(d) = dir_of(c)
            && let Some((_, n)) = counts.iter().find(|(k, _)| *k == d)
            && *n > COLLAPSE_OVER
        {
            if !shown.contains(&d) {
                let owners: Vec<&str> = changes
                    .iter()
                    .filter(|x| dir_of(x).as_deref() == Some(d.as_str()))
                    .filter_map(|x| x.owner.as_deref())
                    .collect();
                let mut l = format!("- ?? {d}/ ({n} new files)");
                if let Some(o) = owners.first()
                    && owners.len() == *n
                {
                    l.push_str(&format!(" [claimed by {o}]"));
                }
                lines.push(l);
                shown.push(d);
            }
            continue;
        }
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
