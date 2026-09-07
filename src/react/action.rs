//! Run reactor action commands and detect git violations (spec section 7, steps 4.5–4.7).

use std::collections::{BTreeSet, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::model::paths::RelPath;

const DROPPED_OUTCOME_KEYS: &[&str] = &["seq", "ts", "prev", "by"];

/// Environment passed to an action command.
pub struct ActionEnv {
    pub log: PathBuf,
    pub seq: u64,
    pub r#type: String,
    pub agent: String,
    pub by: String,
    pub paths: Vec<RelPath>,
    pub reference: Option<String>,
    pub resume: u64,
    pub outcome_file: PathBuf,
}

/// Result of running an action command.
pub struct Outcome {
    pub exit: i32,
    pub fields: Vec<(String, String)>,
    pub timed_out: bool,
}

/// Git state before or after an action.
pub struct Snapshot {
    pub head: Option<String>,
    pub dirty: BTreeSet<String>,
}

/// Run `command` with `stdin_json` on stdin and reactor env vars set.
pub fn run(
    command: &[String],
    stdin_json: &str,
    env: &ActionEnv,
    timeout: Duration,
) -> Result<Outcome> {
    let (program, args) = command.split_first().context("action command is empty")?;

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("EVENTLOG_LOG", &env.log)
        .env("EVENTLOG_SEQ", env.seq.to_string())
        .env("EVENTLOG_TYPE", &env.r#type)
        .env("EVENTLOG_AGENT", &env.agent)
        .env("EVENTLOG_BY", &env.by)
        .env(
            "EVENTLOG_PATHS",
            env.paths
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(","),
        )
        .env("EVENTLOG_REF", env.reference.as_deref().unwrap_or_default())
        .env("EVENTLOG_RESUME", env.resume.to_string())
        .env("EVENTLOG_OUTCOME_FILE", &env.outcome_file)
        .spawn()
        .with_context(|| format!("spawn action command {program}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_json.as_bytes())
            .context("write action stdin")?;
    }

    let (timed_out, status) = wait_with_timeout(&mut child, timeout)?;
    let exit = status.map(|s| s.code().unwrap_or(1)).unwrap_or(1);

    let stdout = if timed_out {
        String::new()
    } else {
        let mut out = String::new();
        if let Some(mut pipe) = child.stdout.take() {
            use std::io::Read;
            pipe.read_to_string(&mut out)
                .context("read action stdout")?;
        }
        out
    };

    let fields = if env.outcome_file.is_file() {
        let content = std::fs::read_to_string(&env.outcome_file)
            .with_context(|| format!("read {}", env.outcome_file.display()))?;
        parse_outcome_file(&content)
    } else {
        parse_stdout_fallback(&stdout)
    };

    Ok(Outcome {
        exit,
        fields,
        timed_out,
    })
}

/// Capture `HEAD` and porcelain status under `root`.
pub fn snapshot(root: &Path) -> Result<Snapshot> {
    let head = git_output(root, &["rev-parse", "HEAD"]).ok();
    let status = git_output(root, &["status", "--porcelain"]).unwrap_or_default();
    Ok(Snapshot {
        head,
        dirty: parse_porcelain(&status),
    })
}

/// Files changed by the commits the action made between the snapshots:
/// `git diff --name-only before..after`. Only what was committed counts as
/// the action's doing; the working tree is shared with every other agent,
/// so a file that merely became dirty is reported by [`newly_dirty`], never
/// blamed on the action.
pub fn touched(before: &Snapshot, after: &Snapshot, root: &Path) -> BTreeSet<String> {
    commit_touched(before.head.as_deref(), after.head.as_deref(), root)
}

/// Paths dirty after the action that were not dirty before. Somebody's work
/// in progress, observed while the action ran; not the action's own writes.
pub fn newly_dirty(before: &Snapshot, after: &Snapshot) -> BTreeSet<String> {
    after.dirty.difference(&before.dirty).cloned().collect()
}

/// Paths in `touched` that are not covered by `authorized`.
pub fn outside(touched: &BTreeSet<String>, authorized: &[RelPath]) -> Vec<String> {
    let allowed: HashSet<&str> = authorized.iter().map(|p| p.as_str()).collect();
    touched
        .iter()
        .filter(|p| !allowed.contains(p.as_str()))
        .cloned()
        .collect()
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<(bool, Option<std::process::ExitStatus>)> {
    let start = Instant::now();
    loop {
        match child.try_wait().context("wait on action command")? {
            Some(status) => return Ok((false, Some(status))),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok((true, None));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn parse_outcome_file(content: &str) -> Vec<(String, String)> {
    let dropped: HashSet<&str> = DROPPED_OUTCOME_KEYS.iter().copied().collect();
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            if dropped.contains(key) {
                return None;
            }
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn parse_stdout_fallback(stdout: &str) -> Vec<(String, String)> {
    let Some(line) = stdout.lines().rev().find(|l| l.contains("outcome=")) else {
        return Vec::new();
    };
    parse_kv_tokens(line)
}

fn parse_kv_tokens(line: &str) -> Vec<(String, String)> {
    let dropped: HashSet<&str> = DROPPED_OUTCOME_KEYS.iter().copied().collect();
    line.split_whitespace()
        .filter_map(|token| {
            let (key, value) = token.split_once('=')?;
            if dropped.contains(key) {
                return None;
            }
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn git_output(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // Only the tail is trimmed: porcelain's first line starts with its status
    // columns (` M path`), and trimming that space would eat the path's
    // first character when the parser skips the column.
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

fn parse_porcelain(status: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in status.lines() {
        if line.len() < 4 {
            continue;
        }
        let path = line[3..].trim();
        let path = path
            .rsplit(" -> ")
            .next()
            .unwrap_or(path)
            .trim()
            .to_string();
        if !path.is_empty() {
            out.insert(path);
        }
    }
    out
}

fn commit_touched(before: Option<&str>, after: Option<&str>, root: &Path) -> BTreeSet<String> {
    let (Some(before), Some(after)) = (before, after) else {
        return BTreeSet::new();
    };
    if before == after {
        return BTreeSet::new();
    }
    git_output(root, &["diff", "--name-only", before, after])
        .map(|out| out.lines().map(str::to_string).collect())
        .unwrap_or_default()
}
