//! Hook guard: read one agent's tool-call payload, decide, install the hooks.
//!
//! Three agents send three shapes. `--agent` (baked in by `guard install`)
//! names the sender; shape detection is the fallback when the flag is absent.
//! Input that is not JSON fails open — the guard can never brick a tool chain.
//! JSON in an unknown shape fails closed: a payload we cannot read is a payload
//! we cannot clear. Spec section 9.

pub mod deny;

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::{Value, json};

pub use deny::{Decision, decide, is_simple_sanctioned_writer};

/// The agents whose hooks this guard speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lower")]
pub enum Agent {
    Claude,
    Cursor,
    Codex,
}

impl Agent {
    pub fn as_str(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Cursor => "cursor",
            Agent::Codex => "codex",
        }
    }

    pub const ALL: [Agent; 3] = [Agent::Claude, Agent::Cursor, Agent::Codex];
}

/// What the agent is about to do, reduced to the shapes the denylist judges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Edit {
        path: String,
    },
    Write {
        path: String,
    },
    Shell {
        command: String,
    },
    /// A tool the guard has no opinion about.
    Other,
}

/// Why a payload could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseFail {
    /// Not JSON at all: fail open.
    #[error("stdin is not JSON")]
    NotJson,
    /// JSON, but no shape we know: fail closed.
    #[error("unrecognized payload")]
    UnknownShape,
}

/// Read one hook payload. `agent` comes from `--agent`; without it the shape
/// decides (Codex carries `turn_id`, Cursor carries `command` plus `cwd`,
/// Claude Code carries `tool_name` plus `tool_input`).
pub fn parse(payload: &str, agent: Option<Agent>) -> Result<(Agent, Action), ParseFail> {
    let value: Value = serde_json::from_str(payload).map_err(|_| ParseFail::NotJson)?;
    let object = value.as_object().ok_or(ParseFail::NotJson)?;

    let has_tool = object.contains_key("tool_name");
    let has_shell = object.get("command").is_some_and(Value::is_string);
    let detected = if object.contains_key("turn_id") && has_tool {
        Some(Agent::Codex)
    } else if has_tool {
        Some(Agent::Claude)
    } else if has_shell {
        Some(Agent::Cursor)
    } else {
        None
    };

    match agent.or(detected) {
        // A Cursor payload is a shell call and nothing else.
        Some(Agent::Cursor) if !has_tool => {
            let command = object
                .get("command")
                .and_then(Value::as_str)
                .ok_or(ParseFail::UnknownShape)?;
            Ok((
                Agent::Cursor,
                Action::Shell {
                    command: command.to_string(),
                },
            ))
        }
        Some(agent) if has_tool => Ok((agent, tool_action(object)?)),
        // --agent said Claude or Codex, but the payload is Cursor-shaped.
        Some(_) if has_shell => {
            let command = object["command"].as_str().unwrap_or_default().to_string();
            Ok((Agent::Cursor, Action::Shell { command }))
        }
        _ => Err(ParseFail::UnknownShape),
    }
}

/// Claude Code and Codex both send `tool_name` plus `tool_input`.
fn tool_action(object: &serde_json::Map<String, Value>) -> Result<Action, ParseFail> {
    let tool = object
        .get("tool_name")
        .and_then(Value::as_str)
        .ok_or(ParseFail::UnknownShape)?;
    let input = object.get("tool_input").unwrap_or(&Value::Null);

    match tool {
        "Bash" | "shell" | "local_shell" | "exec_command" | "run_terminal_cmd" => {
            let command = command_string(input).ok_or(ParseFail::UnknownShape)?;
            Ok(Action::Shell { command })
        }
        "Write" | "create_file" => Ok(Action::Write {
            path: file_path(input).ok_or(ParseFail::UnknownShape)?,
        }),
        "Edit" | "MultiEdit" | "NotebookEdit" | "str_replace_editor" => Ok(Action::Edit {
            path: file_path(input).ok_or(ParseFail::UnknownShape)?,
        }),
        "apply_patch" => Ok(Action::Edit {
            path: patch_target(input).ok_or(ParseFail::UnknownShape)?,
        }),
        _ => Ok(Action::Other),
    }
}

/// `command` is a string for Claude Code and Cursor, an argv array for Codex.
fn command_string(input: &Value) -> Option<String> {
    match input.get("command")? {
        Value::String(s) => Some(s.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

fn file_path(input: &Value) -> Option<String> {
    for key in ["file_path", "notebook_path", "path", "target_file"] {
        if let Some(p) = input.get(key).and_then(Value::as_str) {
            return Some(p.to_string());
        }
    }
    None
}

/// The first file an `apply_patch` envelope touches.
fn patch_target(input: &Value) -> Option<String> {
    if let Some(p) = file_path(input) {
        return Some(p);
    }
    let text = ["input", "patch", "diff"]
        .into_iter()
        .find_map(|k| input.get(k).and_then(Value::as_str))?;
    for line in text.lines() {
        for marker in ["*** Update File:", "*** Delete File:", "*** Add File:"] {
            if let Some(rest) = line.strip_prefix(marker) {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

/// Write the hook entry for `agent` under `root`, calling `bin`. Returns true
/// when the file changed; running it twice is a no-op the second time.
pub fn install(agent: Agent, root: &Path, bin: &Path) -> anyhow::Result<bool> {
    let command = format!("{} guard --agent {}", bin.display(), agent.as_str());
    let (rel, existing) = hook_file(agent, root);
    let path = root.join(rel);
    let before = std::fs::read_to_string(&path).ok();
    let mut doc: Value = match &before {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str(text).with_context(|| format!("parsing {}", path.display()))?
        }
        _ => existing,
    };
    if !doc.is_object() {
        anyhow::bail!("{} is not a JSON object", path.display());
    }

    let entry = hook_entry(agent, &command);
    let list = hook_list(agent, &mut doc);
    if list.iter().any(|e| mentions(e, &command)) {
        return Ok(false);
    }
    list.push(entry);

    let text = format!("{}\n", serde_json::to_string_pretty(&doc)?);
    if before.as_deref() == Some(text.as_str()) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, &text).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

/// Where each agent keeps its hooks, and the skeleton for a fresh file.
fn hook_file(agent: Agent, _root: &Path) -> (&'static str, Value) {
    match agent {
        Agent::Claude => (
            ".claude/settings.json",
            json!({ "hooks": { "PreToolUse": [] } }),
        ),
        Agent::Cursor => (
            ".cursor/hooks.json",
            json!({ "version": 1, "hooks": { "beforeShellExecution": [] } }),
        ),
        Agent::Codex => (
            ".codex/hooks.json",
            json!({ "hooks": { "PreToolUse": [] } }),
        ),
    }
}

fn hook_event(agent: Agent) -> &'static str {
    match agent {
        Agent::Cursor => "beforeShellExecution",
        _ => "PreToolUse",
    }
}

/// The array this agent's entries live in, created if the file lacks it.
fn hook_list(agent: Agent, doc: &mut Value) -> &mut Vec<Value> {
    let hooks = doc
        .as_object_mut()
        .expect("checked object")
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let event = hooks
        .as_object_mut()
        .expect("just made an object")
        .entry(hook_event(agent))
        .or_insert_with(|| json!([]));
    if !event.is_array() {
        *event = json!([]);
    }
    event.as_array_mut().expect("just made an array")
}

fn hook_entry(agent: Agent, command: &str) -> Value {
    match agent {
        Agent::Claude => json!({
            "matcher": "Edit|Write|Bash",
            "hooks": [{ "type": "command", "command": command }],
        }),
        Agent::Cursor => json!({ "command": command }),
        Agent::Codex => json!({
            "matcher": "Edit|Write|Bash",
            "command": command,
        }),
    }
}

/// Is this entry already ours? Compared by the command string, wherever the
/// agent's schema puts it.
fn mentions(entry: &Value, command: &str) -> bool {
    match entry {
        Value::String(s) => s == command,
        Value::Object(map) => map.values().any(|v| mentions(v, command)),
        Value::Array(items) => items.iter().any(|v| mentions(v, command)),
        _ => false,
    }
}

/// The binary to bake into a hook entry: this executable, else plain
/// `eventlog` from `PATH`.
pub fn current_bin() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("eventlog"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_detection_picks_the_agent() {
        let claude = r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
        let cursor = r#"{"command":"ls","cwd":"/repo"}"#;
        let codex = r#"{"turn_id":"t","tool_name":"shell","tool_input":{"command":["ls"]}}"#;
        assert_eq!(parse(claude, None).unwrap().0, Agent::Claude);
        assert_eq!(parse(cursor, None).unwrap().0, Agent::Cursor);
        assert_eq!(parse(codex, None).unwrap().0, Agent::Codex);
    }

    #[test]
    fn not_json_fails_open_unknown_shape_fails_closed() {
        assert_eq!(parse("hello", None), Err(ParseFail::NotJson));
        assert_eq!(parse(r#"{"a":1}"#, None), Err(ParseFail::UnknownShape));
    }

    #[test]
    fn an_unknown_tool_is_other() {
        let payload = r#"{"tool_name":"WebFetch","tool_input":{"url":"https://x"}}"#;
        assert_eq!(parse(payload, None).unwrap().1, Action::Other);
    }

    #[test]
    fn apply_patch_names_the_file_it_rewrites() {
        let payload = r#"{"turn_id":"t","tool_name":"apply_patch","tool_input":{"input":"*** Begin Patch\n*** Update File: a/b.rs\n"}}"#;
        assert_eq!(
            parse(payload, None).unwrap().1,
            Action::Edit {
                path: "a/b.rs".into()
            }
        );
    }
}
