//! `eventlog worktree-facts`: what the log says about worktree paths.
//!
//! Reads absolute paths on stdin, one per line. For each path the log can
//! place, prints one JSON line: `{"path", "verdict": "hold"|"done", "reason"}`.
//! A path the log cannot place gets no line (no opinion). Read-only.
//!
//! A path belongs to an agent when a `result` names it in `worktree=`, or when
//! it is `<repo>/.worktrees/<agent>` (the layout the controller's briefs use).
//! `done`: the agent is retired and its last `result` is on the log.
//! `hold`: the agent is not retired, or it was retired without a `result`.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::json;

use crate::cli::{Args, Command};
use crate::cmd::agents::load_context;
use crate::model::event::Event;
use crate::query::{Phase, State};

#[derive(Debug, PartialEq, Eq)]
pub struct Fact {
    pub path: PathBuf,
    pub verdict: &'static str,
    pub reason: String,
}

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::WorktreeFacts(_) = &args.command else {
        anyhow::bail!("worktree_facts::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    let q = load_context(args, None)?;
    let paths: Vec<PathBuf> = std::io::stdin()
        .lock()
        .lines()
        .map_while(Result::ok)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();
    let mut out = std::io::stdout().lock();
    for f in facts(&q.events, &q.state, &root, &paths) {
        let line = json!({
            "path": f.path.display().to_string(),
            "verdict": f.verdict,
            "reason": f.reason,
        });
        writeln!(out, "{line}")?;
    }
    Ok(0)
}

/// Lexical absolute path: join a relative path onto `root`, drop `.` parts.
fn absolute(root: &Path, p: &str) -> PathBuf {
    let joined = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        root.join(p)
    };
    joined
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect()
}

pub fn facts(events: &[Event], state: &State, root: &Path, paths: &[PathBuf]) -> Vec<Fact> {
    // path -> agent, from `result worktree=`; the newest result wins.
    let mut named: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut has_result: BTreeMap<&str, ()> = BTreeMap::new();
    for e in events.iter().filter(|e| e.r#type == "result") {
        let Some(agent) = e.agent.as_deref() else {
            continue;
        };
        has_result.insert(agent, ());
        if let Some(w) = e.fields.get("worktree") {
            named.insert(absolute(root, w), agent.to_string());
        }
    }
    let by_layout = |p: &Path| -> Option<String> {
        let rel = p.strip_prefix(root.join(".worktrees")).ok()?;
        let name = rel.components().next()?.as_os_str().to_str()?;
        state.agents.contains_key(name).then(|| name.to_string())
    };

    let mut out = Vec::new();
    for p in paths {
        let Some(agent) = named.get(p).cloned().or_else(|| by_layout(p)) else {
            continue;
        };
        let Some(a) = state.agents.get(&agent) else {
            continue;
        };
        if a.phase != Phase::Retired {
            out.push(Fact {
                path: p.clone(),
                verdict: "hold",
                reason: format!("agent {agent} is not retired"),
            });
        } else if has_result.contains_key(agent.as_str()) {
            out.push(Fact {
                path: p.clone(),
                verdict: "done",
                reason: format!("agent {agent} retired after a result"),
            });
        } else {
            out.push(Fact {
                path: p.clone(),
                verdict: "hold",
                reason: format!("agent {agent} retired with no result on the log"),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::Config;
    use crate::query;

    fn log(lines: &[&str]) -> (Vec<Event>, State) {
        let events: Vec<Event> = lines
            .iter()
            .map(|l| Event::parse_line(l).unwrap())
            .collect();
        let state = query::fold(&events, &Config::default());
        (events, state)
    }

    fn l(seq: u64, rest: &str) -> String {
        format!(r#"{{"seq":{seq},"ts":"2026-09-30T00:00:00Z",{rest}}}"#)
    }

    fn run_facts(lines: &[String], paths: &[&str]) -> Vec<(String, &'static str)> {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let (events, state) = log(&refs);
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        facts(&events, &state, Path::new("/r"), &paths)
            .into_iter()
            .map(|f| (f.path.display().to_string(), f.verdict))
            .collect()
    }

    #[test]
    fn a_retired_agent_with_a_result_is_done() {
        let lines = [
            l(1, r#""type":"spawn","agent":"api""#),
            l(2, r#""type":"result","agent":"api","ref":"b.md""#),
            l(3, r#""type":"retire","agent":"api""#),
        ];
        assert_eq!(
            run_facts(&lines, &["/r/.worktrees/api"]),
            vec![("/r/.worktrees/api".to_string(), "done")]
        );
    }

    #[test]
    fn a_live_agent_is_held() {
        let lines = [l(1, r#""type":"spawn","agent":"api""#)];
        assert_eq!(
            run_facts(&lines, &["/r/.worktrees/api"]),
            vec![("/r/.worktrees/api".to_string(), "hold")]
        );
    }

    #[test]
    fn retired_with_no_result_is_held() {
        let lines = [
            l(1, r#""type":"spawn","agent":"api""#),
            l(2, r#""type":"retire","agent":"api""#),
        ];
        assert_eq!(
            run_facts(&lines, &["/r/.worktrees/api"]),
            vec![("/r/.worktrees/api".to_string(), "hold")]
        );
    }

    #[test]
    fn a_respawned_agent_is_held_again() {
        let lines = [
            l(1, r#""type":"spawn","agent":"api""#),
            l(2, r#""type":"result","agent":"api","ref":"b.md""#),
            l(3, r#""type":"retire","agent":"api""#),
            l(4, r#""type":"spawn","agent":"api""#),
        ];
        assert_eq!(
            run_facts(&lines, &["/r/.worktrees/api"]),
            vec![("/r/.worktrees/api".to_string(), "hold")]
        );
    }

    #[test]
    fn a_result_can_name_a_worktree_anywhere() {
        let lines = [
            l(1, r#""type":"spawn","agent":"ui""#),
            l(
                2,
                r#""type":"result","agent":"ui","ref":"b.md","worktree":"../elsewhere/ui""#,
            ),
            l(3, r#""type":"retire","agent":"ui""#),
        ];
        assert_eq!(
            run_facts(&lines, &["/elsewhere/ui"]),
            Vec::<(String, &str)>::new(),
            "a relative path resolves under the repo root: /r/../elsewhere/ui"
        );
        assert_eq!(
            run_facts(&lines, &["/r/../elsewhere/ui"]),
            vec![("/r/../elsewhere/ui".to_string(), "done")]
        );
    }

    #[test]
    fn a_path_the_log_cannot_place_gets_no_line() {
        let lines = [l(1, r#""type":"spawn","agent":"api""#)];
        assert!(run_facts(&lines, &["/r/.worktrees/other", "/somewhere/else"]).is_empty());
    }
}
