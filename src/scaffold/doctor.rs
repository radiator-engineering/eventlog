//! `eventlog doctor`: diagnose setup problems in a repo.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use crate::guard::{self, Agent};
use crate::log::Log;
use crate::model::config::{self, Config};
use crate::model::event::Event;
use crate::model::vocab::REFERENCE_FIELDS;
use crate::query::{self, Phase, State};
use crate::react::lock::Token;
use crate::skill;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub fix: bool,
    pub protect: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Fail,
}

struct Row {
    status: Status,
    label: String,
    detail: Option<String>,
}

pub fn run(repo_root: &Path, cfg: &Config, opts: &Options) -> anyhow::Result<i32> {
    if opts.fix {
        apply_fixes(repo_root)?;
    }
    if opts.protect {
        let log = log_path(repo_root, cfg);
        crate::scaffold::protect(&log, true)?;
    }

    let mut rows = Vec::new();
    rows.push(check_binary_on_path());
    rows.extend(check_guards(repo_root));
    rows.push(check_protection(repo_root, cfg));
    rows.extend(check_log(repo_root, cfg));
    rows.push(check_skill_stamp());
    rows.extend(check_old_scripts_on_path());

    let mut out = io::stdout().lock();
    let mut failed = false;
    for row in &rows {
        if row.status == Status::Fail {
            failed = true;
        }
        print_row(&mut out, row)?;
    }
    Ok(if failed { 1 } else { 0 })
}

fn apply_fixes(repo_root: &Path) -> anyhow::Result<()> {
    let bin = guard::current_bin();
    for agent in Agent::ALL {
        if agent_config_dir(repo_root, agent).is_some() {
            guard::install(agent, repo_root, &bin)?;
        }
    }
    remove_old_script_symlinks()?;
    skill::install(&skill::default_dir(), false)?;
    Ok(())
}

fn check_binary_on_path() -> Row {
    let found = Command::new("sh")
        .args(["-c", "command -v eventlog >/dev/null 2>&1"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    row(
        if found { Status::Ok } else { Status::Warn },
        "binary on PATH",
        if found {
            None
        } else {
            Some("eventlog not found".into())
        },
    )
}

fn check_guards(repo_root: &Path) -> Vec<Row> {
    Agent::ALL
        .into_iter()
        .map(|agent| {
            let hook = repo_root.join(guard_hook_rel(agent));
            if !hook.is_file() {
                return row(
                    Status::Warn,
                    format!("guard {}", agent.as_str()),
                    Some("hook file missing".into()),
                );
            }
            let text = fs::read_to_string(&hook).unwrap_or_default();
            let installed = text.contains("eventlog guard");
            row(
                if installed { Status::Ok } else { Status::Warn },
                format!("guard {}", agent.as_str()),
                if installed {
                    None
                } else {
                    Some("eventlog guard not installed".into())
                },
            )
        })
        .collect()
}

fn guard_hook_rel(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => ".claude/settings.json",
        Agent::Cursor => ".cursor/hooks.json",
        Agent::Codex => ".codex/hooks.json",
    }
}

fn check_protection(repo_root: &Path, cfg: &Config) -> Row {
    let log = log_path(repo_root, cfg);
    match crate::scaffold::is_protected(&log) {
        Ok(true) => row(Status::Ok, "protection", None),
        Ok(false) => row(
            Status::Warn,
            "protection",
            Some(format!("{} is not append-only", log.display())),
        ),
        Err(err) => row(
            Status::Warn,
            "protection",
            Some(format!("could not read protection: {err}")),
        ),
    }
}

fn check_log(repo_root: &Path, cfg: &Config) -> Vec<Row> {
    let log = log_path(repo_root, cfg);
    let report = match Log::open(&log).read() {
        Ok(r) => r,
        Err(err) => {
            return vec![row(Status::Fail, "log read", Some(err.to_string()))];
        }
    };

    let mut rows = Vec::new();
    if !report.malformed.is_empty() {
        rows.push(row(
            Status::Fail,
            "malformed lines",
            Some(format!("{} bad line(s)", report.malformed.len())),
        ));
    }

    rows.extend(check_unsanctioned(&report.events, cfg));
    rows.extend(check_strict_history(&report.events, cfg, repo_root));
    rows.extend(check_references(&report.events));
    rows.extend(check_open_lifecycles(&report.events, cfg));
    rows.extend(check_stale_locks(&log));
    rows
}

fn check_unsanctioned(events: &[Event], cfg: &Config) -> Vec<Row> {
    let mut rows = Vec::new();
    for event in events {
        let state = query::fold_at(events, cfg, event.seq);
        let writer = event.writer();
        let ty = &event.r#type;
        if !state.allowlist.permits(writer, ty) {
            rows.push(row(
                Status::Fail,
                "unsanctioned writer",
                Some(format!("seq {} by={} type={}", event.seq, writer, ty)),
            ));
        }
    }
    rows
}

fn check_strict_history(events: &[Event], cfg: &Config, repo_root: &Path) -> Vec<Row> {
    let mut rows = Vec::new();
    for event in events {
        let prior_at = event.seq.saturating_sub(1);
        let prior = query::fold_at(events, cfg, prior_at);
        if let Some(rule) = strict_violation(&prior, event, repo_root) {
            rows.push(row(
                Status::Fail,
                "strict rule",
                Some(format!("seq {}: {rule}", event.seq)),
            ));
        }
    }
    rows
}

fn strict_violation(prior: &State, event: &Event, repo_root: &Path) -> Option<String> {
    if matches!(
        event.r#type.as_str(),
        "result" | "progress" | "claim" | "retire"
    ) {
        let agent = event.subject();
        if agent != "controller" && !prior.agent_is_open(agent) {
            return Some("open-spawn".into());
        }
    }

    if event.r#type == "claim"
        && let Some(paths_val) = event.fields.get("paths")
    {
        if let Err(e) = claim_paths_exist(repo_root, paths_val) {
            return Some(e);
        }
        let claimer = event.subject();
        for entry in paths_val
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if let Some(owner) = prior.claim_owner(entry)
                && owner != claimer
            {
                return Some("claim-conflict".into());
            }
        }
    }

    None
}

impl State {
    fn agent_is_open(&self, agent: &str) -> bool {
        self.agents
            .get(agent)
            .is_some_and(|a| a.phase != Phase::Retired)
    }
}

fn claim_paths_exist(repo_root: &Path, paths_val: &str) -> Result<(), String> {
    use crate::model::paths;
    let entries = paths::validate_paths(paths_val).map_err(|e| e.to_string())?;
    for entry in entries {
        let path = repo_root.join(entry.as_str());
        if path.exists() {
            continue;
        }
        if entry.as_str().contains('*') || entry.as_str().contains('?') {
            continue;
        }
        return Err("claim-path-missing".into());
    }
    Ok(())
}

fn check_references(events: &[Event]) -> Vec<Row> {
    let seqs: BTreeSet<u64> = events.iter().map(|e| e.seq).collect();
    let mut rows = Vec::new();
    for event in events {
        for field in REFERENCE_FIELDS {
            if let Some(target) = event.seq_ref(field)
                && (!seqs.contains(&target) || target >= event.seq)
            {
                rows.push(row(
                    Status::Fail,
                    "reference",
                    Some(format!("seq {} field {field}={target} invalid", event.seq)),
                ));
            }
        }
    }
    rows
}

fn check_open_lifecycles(events: &[Event], cfg: &Config) -> Vec<Row> {
    let state = query::fold(events, cfg);
    if state.open_lifecycles.is_empty() {
        vec![row(Status::Ok, "open lifecycles", None)]
    } else {
        vec![row(
            Status::Warn,
            "open lifecycles",
            Some(state.open_lifecycles.join(", ")),
        )]
    }
}

fn check_stale_locks(log: &Path) -> Vec<Row> {
    let parent = match log.parent() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let prefix = match log.file_name().and_then(|n| n.to_str()) {
        Some(name) => format!("{name}."),
        None => return Vec::new(),
    };
    let me = Token::current();
    let mut rows = Vec::new();
    let read = match fs::read_dir(parent) {
        Ok(r) => r,
        Err(_) => return rows,
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".reactor.lock") {
            continue;
        }
        let lock_dir = entry.path();
        if !lock_dir.is_dir() {
            continue;
        }
        let token = fs::read_to_string(lock_dir.join("token"))
            .ok()
            .and_then(|t| Token::from_json(&t));
        let stale = token.as_ref().is_none_or(|t| !t.is_live(&me));
        rows.push(row(
            if stale { Status::Warn } else { Status::Ok },
            format!("reactor lock {name}"),
            if stale {
                Some("stale or unreadable".into())
            } else {
                None
            },
        ));
    }
    rows
}

fn check_skill_stamp() -> Row {
    let stamp_path = skill::default_dir()
        .join("event-log-coordination")
        .join(".eventlog-version");
    let current = env!("CARGO_PKG_VERSION");
    match fs::read_to_string(&stamp_path) {
        Ok(existing) if existing.trim() == current => row(Status::Ok, "skill stamp", None),
        Ok(existing) => row(
            Status::Warn,
            "skill stamp",
            Some(format!("installed {} != binary {current}", existing.trim())),
        ),
        Err(_) => row(
            Status::Warn,
            "skill stamp",
            Some("skill not installed".into()),
        ),
    }
}

const OLD_SCRIPTS: &[&str] = &[
    "append-event.sh",
    "eventlog-view.sh",
    "check-claims.sh",
    "init-eventlog.sh",
    "safety-check.sh",
    "setup.sh",
    "link-scripts.sh",
    "protect-log.sh",
    "eventlog-guard.sh",
    "install-guard.sh",
    "run-reactor.sh",
];

fn check_old_scripts_on_path() -> Vec<Row> {
    OLD_SCRIPTS
        .iter()
        .filter_map(|script| {
            let path = which(script)?;
            Some(row(
                Status::Warn,
                "old script on PATH",
                Some(format!("{script} -> {}", path.display())),
            ))
        })
        .collect()
}

fn remove_old_script_symlinks() -> anyhow::Result<()> {
    for script in OLD_SCRIPTS {
        let Some(path) = which(script) else {
            continue;
        };
        if fs::symlink_metadata(&path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            fs::remove_file(&path)
                .with_context(|| format!("removing symlink {}", path.display()))?;
        }
    }
    Ok(())
}

fn which(name: &str) -> Option<PathBuf> {
    let output = Command::new("sh")
        .args(["-c", &format!("command -v {name}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn agent_config_dir(repo_root: &Path, agent: Agent) -> Option<PathBuf> {
    let dir = match agent {
        Agent::Claude => repo_root.join(".claude"),
        Agent::Cursor => repo_root.join(".cursor"),
        Agent::Codex => repo_root.join(".codex"),
    };
    dir.is_dir().then_some(dir)
}

fn log_path(repo_root: &Path, cfg: &Config) -> PathBuf {
    let path = config::resolve_log(cfg, None);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}

fn row(status: Status, label: impl Into<String>, detail: Option<String>) -> Row {
    Row {
        status,
        label: label.into(),
        detail,
    }
}

fn print_row(out: &mut impl Write, row: &Row) -> io::Result<()> {
    let tag = match row.status {
        Status::Ok => "[ OK ]",
        Status::Warn => "[WARN]",
        Status::Fail => "[FAIL]",
    };
    write!(out, "{tag} {}", row.label)?;
    if let Some(detail) = &row.detail {
        write!(out, ": {detail}")?;
    }
    writeln!(out)
}
