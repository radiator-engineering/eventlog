//! Append one validated line to the log (spec sections 3, 4, 6).

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use globset::{Glob, GlobSetBuilder};
use indexmap::IndexMap;

use crate::log::lock::{Lock, LockError};
use crate::log::{Log, Tail};
use crate::model::allow::Allowlist;
use crate::model::config::Config;
use crate::model::event::Event;
use crate::model::paths;

const FIELD_CAP: usize = 2048;
const EVENT_CAP: usize = 4096;
const RESERVED: [&str; 3] = ["seq", "ts", "prev"];
const LOCK_WAIT: Duration = Duration::from_secs(5);

pub struct AppendRequest {
    pub r#type: String,
    pub fields: Vec<(String, String)>,
    pub writer: String,
    pub strict: bool,
    pub dry_run: bool,
}

/// What strict append rules need from folded state. `log` never imports `query`;
/// `cmd/append` implements this for `query::State`.
pub trait StrictContext {
    fn allowlist(&self) -> &Allowlist;
    fn agent_is_open(&self, agent: &str) -> bool;
    fn claim_owner(&self, path: &str) -> Option<String>;
    fn has_open_escalation(&self, agent: &str) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum AppendError {
    #[error("reserved field: {0}")]
    Reserved(String),
    #[error("by does not match writer")]
    ByMismatch,
    #[error("field too large: {0}")]
    FieldTooLarge(String),
    #[error("event too large")]
    EventTooLarge,
    #[error("not permitted: writer {writer} type {ty}")]
    NotPermitted { writer: String, ty: String },
    #[error("missing field: {0}")]
    MissingField(String),
    #[error("unknown type: {0}")]
    UnknownType(String),
    #[error("bad path: {0}")]
    BadPath(String),
    #[error("torn tail at line {0}")]
    TornTail(usize),
    #[error("strict: {0}")]
    Strict(String),
    #[error("lock error")]
    Lock(#[from] LockError),
}

pub fn append(
    log: &Log,
    cfg: &Config,
    req: AppendRequest,
    ctx: Option<&dyn StrictContext>,
) -> Result<Event, AppendError> {
    let mut fields = validate_and_build_fields(&req)?;
    let agent = resolve_agent(&req, &mut fields);
    validate_type_and_fields(cfg, &req.r#type, &fields, agent.is_some())?;
    validate_paths_field(&fields)?;
    validate_sizes(&fields)?;
    let event = build_event(&req, fields, agent, 0, String::new(), None);
    check_allowlist(cfg, &req, &event, ctx)?;
    if req.strict
        && let Some(ctx) = ctx
    {
        check_strict(ctx, &req, &event, &repo_root(log)).map_err(AppendError::Strict)?;
    }

    let tail = log
        .tail()
        .map_err(|e| AppendError::BadPath(e.to_string()))?;
    if let Some(line) = tail.torn {
        return Err(AppendError::TornTail(line));
    }

    if req.dry_run {
        let event = finalize_event(log, &tail, event);
        if event.to_line().len() > EVENT_CAP {
            return Err(AppendError::EventTooLarge);
        }
        return Ok(event);
    }

    write_locked(log, cfg, event)
}

fn validate_and_build_fields(req: &AppendRequest) -> Result<IndexMap<String, String>, AppendError> {
    let mut fields = IndexMap::new();
    for (key, value) in &req.fields {
        if RESERVED.contains(&key.as_str()) {
            return Err(AppendError::Reserved(key.clone()));
        }
        if key == "by" {
            if req.writer == "controller" || value != &req.writer {
                return Err(AppendError::ByMismatch);
            }
            continue;
        }
        fields.insert(key.clone(), value.clone());
    }
    Ok(fields)
}

fn resolve_agent(req: &AppendRequest, fields: &mut IndexMap<String, String>) -> Option<String> {
    // `agent` is a top-level event field; it must not also stay in `fields`,
    // or `to_line` would emit the key twice.
    if let Some(agent) = fields.shift_remove("agent") {
        return Some(agent);
    }
    if req.writer == "controller" && req.r#type == "result" {
        return Some("controller".to_string());
    }
    None
}

fn validate_type_and_fields(
    cfg: &Config,
    ty: &str,
    fields: &IndexMap<String, String>,
    has_agent: bool,
) -> Result<(), AppendError> {
    let spec = cfg
        .vocabulary
        .get(ty)
        .ok_or_else(|| AppendError::UnknownType(ty.to_string()))?;
    for required in &spec.fields {
        if required == "agent" && has_agent {
            continue;
        }
        if !fields.contains_key(required) {
            return Err(AppendError::MissingField(required.clone()));
        }
    }
    let allowed: BTreeMap<&str, ()> = spec
        .fields
        .iter()
        .chain(&spec.optional)
        .map(|f| (f.as_str(), ()))
        .collect();
    for key in fields.keys() {
        if !allowed.contains_key(key.as_str()) {
            return Err(AppendError::MissingField(format!("unknown field {key}")));
        }
    }
    Ok(())
}

fn validate_paths_field(fields: &IndexMap<String, String>) -> Result<(), AppendError> {
    if let Some(paths_val) = fields.get("paths") {
        paths::validate_paths(paths_val).map_err(|e| AppendError::BadPath(e.to_string()))?;
    }
    Ok(())
}

fn validate_sizes(fields: &IndexMap<String, String>) -> Result<(), AppendError> {
    for (key, value) in fields {
        if value.len() > FIELD_CAP {
            return Err(AppendError::FieldTooLarge(key.clone()));
        }
    }
    Ok(())
}

fn build_event(
    req: &AppendRequest,
    fields: IndexMap<String, String>,
    agent: Option<String>,
    seq: u64,
    ts: String,
    prev: Option<String>,
) -> Event {
    let by = if req.writer == "controller" {
        None
    } else {
        Some(req.writer.clone())
    };
    Event {
        seq,
        ts,
        r#type: req.r#type.clone(),
        prev,
        by,
        agent,
        fields,
    }
}

fn check_allowlist(
    cfg: &Config,
    req: &AppendRequest,
    event: &Event,
    ctx: Option<&dyn StrictContext>,
) -> Result<(), AppendError> {
    let allowlist = match ctx {
        Some(ctx) => ctx.allowlist(),
        None => &cfg.writers,
    };
    if !allowlist.permits(&req.writer, &event.r#type) {
        return Err(AppendError::NotPermitted {
            writer: req.writer.clone(),
            ty: event.r#type.clone(),
        });
    }
    Ok(())
}

fn check_strict(
    ctx: &dyn StrictContext,
    req: &AppendRequest,
    event: &Event,
    repo_root: &Path,
) -> Result<(), String> {
    if ctx.has_open_escalation(&req.writer) {
        return Err("open-escalation".into());
    }

    if matches!(
        event.r#type.as_str(),
        "result" | "progress" | "claim" | "retire"
    ) {
        let agent = event.subject();
        if !ctx.agent_is_open(agent) {
            return Err("open-spawn".into());
        }
    }

    if event.r#type == "claim"
        && let Some(paths_val) = event.fields.get("paths")
    {
        check_claim_paths_exist(repo_root, paths_val)?;
        let claimer = event.subject();
        for entry in paths::validate_paths(paths_val).map_err(|e| e.to_string())? {
            if let Some(owner) = ctx.claim_owner(entry.as_str())
                && owner != claimer
            {
                return Err("claim-conflict".into());
            }
        }
    }

    Ok(())
}

fn check_claim_paths_exist(repo_root: &Path, paths_val: &str) -> Result<(), String> {
    let entries = paths::validate_paths(paths_val).map_err(|e| e.to_string())?;
    for entry in entries {
        let path = repo_root.join(entry.as_str());
        if path.exists() {
            continue;
        }
        if entry.as_str().contains('*') || entry.as_str().contains('?') {
            let mut builder = GlobSetBuilder::new();
            builder.add(Glob::new(entry.as_str()).map_err(|e| e.to_string())?);
            let set = builder.build().map_err(|e| e.to_string())?;
            if glob_matches_under(repo_root, &set)? {
                continue;
            }
        }
        return Err("claim-path-missing".into());
    }
    Ok(())
}

fn glob_matches_under(repo_root: &Path, set: &globset::GlobSet) -> Result<bool, String> {
    // Match every file against its path relative to the repo root, so a
    // claim like `src/api/**` matches `src/api/ping.rs` the way the fold does.
    fn walk(root: &Path, dir: &Path, set: &globset::GlobSet) -> Result<bool, String> {
        let read = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
        for entry in read {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                if walk(root, &path, set)? {
                    return Ok(true);
                }
            } else {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                if set.is_match(rel) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    walk(repo_root, repo_root, set)
}

fn finalize_event(log: &Log, tail: &Tail, mut event: Event) -> Event {
    event.seq = tail.last_seq + 1;
    event.ts = now_ts();
    event.prev = Some(prev_for_tail(log, tail));
    event
}

fn write_locked(log: &Log, cfg: &Config, draft: Event) -> Result<Event, AppendError> {
    let lock_dir = lock_path(log);
    if let Some(parent) = lock_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LockError::Io {
            dir: lock_dir.clone(),
            source: e,
        })?;
    }
    let _lock = Lock::acquire(&lock_dir, LOCK_WAIT)?;

    let tail = log
        .tail()
        .map_err(|e| AppendError::BadPath(e.to_string()))?;
    if let Some(line) = tail.torn {
        return Err(AppendError::TornTail(line));
    }

    let mut event = draft;
    event.seq = tail.last_seq + 1;
    event.ts = now_ts();
    event.prev = Some(prev_for_tail(log, &tail));

    let line = event.to_line();
    if line.len() > EVENT_CAP {
        return Err(AppendError::EventTooLarge);
    }

    if let Some(parent) = log.path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppendError::BadPath(e.to_string()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log.path)
        .map_err(|e| AppendError::BadPath(e.to_string()))?;
    file.write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|e| AppendError::BadPath(e.to_string()))?;
    if cfg.log.fsync {
        file.sync_all()
            .map_err(|e| AppendError::BadPath(e.to_string()))?;
    }
    Ok(event)
}

fn prev_for_tail(_log: &Log, tail: &Tail) -> String {
    match &tail.last_line {
        Some(bytes) => Log::hash_line(bytes),
        None => "genesis".to_string(),
    }
}

fn lock_path(log: &Log) -> PathBuf {
    PathBuf::from(format!("{}.lock", log.path.display()))
}

fn repo_root(log: &Log) -> PathBuf {
    log.path
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn now_ts() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
