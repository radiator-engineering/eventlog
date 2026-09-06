//! End-to-end guard tests: real hook payloads in, deny honored on the way out.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use eventlog::guard::{self, Agent};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hooks")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Run `eventlog guard` in a scratch repo with `payload` on stdin.
fn guard_cmd(dir: &Path, payload: &str) -> std::process::Output {
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir)
        .args(["guard", "--log", ".context/events.jsonl"])
        .write_stdin(payload.to_string())
        .output()
        .unwrap()
}

fn scratch() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(dir.path().join(".context/events.jsonl"), "").unwrap();
    dir
}

fn assert_denied(out: &std::process::Output, what: &str) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "{what}: expected exit 2, got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        out.status.code()
    );
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{what}: stdout not JSON ({e}): {stdout}"));
    assert_eq!(json["permission"], "deny", "{what}: {stdout}");
    assert!(
        !json["userMessage"].as_str().unwrap_or("").is_empty(),
        "{what}: empty userMessage"
    );
    assert!(!stderr.trim().is_empty(), "{what}: no reason on stderr");
}

#[test]
fn every_deny_fixture_is_blocked() {
    let dir = scratch();
    for name in [
        "claude_bash_deny.json",
        "claude_edit_deny.json",
        "cursor_shell_deny.json",
        "codex_bash_deny.json",
        "codex_apply_patch_deny.json",
    ] {
        let out = guard_cmd(dir.path(), &fixture(name));
        assert_denied(&out, name);
    }
}

#[test]
fn the_sanctioned_writer_is_allowed() {
    let dir = scratch();
    let out = guard_cmd(dir.path(), &fixture("claude_append_allow.json"));
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn bash_payload(command: &str) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": command },
    })
    .to_string()
}

#[test]
fn a_sanctioned_writer_followed_by_a_truncate_is_denied() {
    let dir = scratch();
    let command = format!("eventlog append note x=1; : {} .context/events.jsonl", ">");
    let out = guard_cmd(dir.path(), &bash_payload(&command));
    assert_denied(&out, "compound append-then-truncate");
}

#[test]
fn every_adaptive_case_is_denied() {
    let dir = scratch();
    let cases = fixture("adaptive_cases.txt");
    let cases: Vec<&str> = cases
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(cases.len() >= 12, "expected at least 12 adaptive cases");
    for case in cases {
        let out = guard_cmd(dir.path(), &bash_payload(case));
        assert_denied(&out, case);
    }
}

#[test]
fn reads_of_the_log_are_allowed() {
    let dir = scratch();
    for command in [
        "cat .context/events.jsonl",
        "grep spawn .context/events.jsonl",
    ] {
        let out = guard_cmd(dir.path(), &bash_payload(command));
        assert_eq!(out.status.code(), Some(0), "{command} should be allowed");
    }
}

#[test]
fn an_unrecognized_json_payload_fails_closed() {
    let dir = scratch();
    let out = guard_cmd(dir.path(), r#"{"nothing":"we know"}"#);
    assert_denied(&out, "unknown shape");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unrecognized payload"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn non_json_stdin_fails_open() {
    let dir = scratch();
    let out = guard_cmd(dir.path(), "not json at all\n");
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn install_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let bin = PathBuf::from("/usr/local/bin/eventlog");
    for (agent, rel) in [
        (Agent::Claude, ".claude/settings.json"),
        (Agent::Cursor, ".cursor/hooks.json"),
        (Agent::Codex, ".codex/hooks.json"),
    ] {
        assert!(
            guard::install(agent, dir.path(), &bin).unwrap(),
            "{rel}: first install should change the file"
        );
        let after_first = std::fs::read_to_string(dir.path().join(rel)).unwrap();
        assert!(
            after_first.contains("eventlog guard --agent"),
            "{rel}: {after_first}"
        );
        assert!(
            !guard::install(agent, dir.path(), &bin).unwrap(),
            "{rel}: second install should be a no-op"
        );
        assert_eq!(
            after_first,
            std::fs::read_to_string(dir.path().join(rel)).unwrap(),
            "{rel}: file changed on the second install"
        );
    }
}

#[test]
fn install_keeps_settings_it_did_not_write() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(
        dir.path().join(".claude/settings.json"),
        r#"{"model":"opus","hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"other.sh"}]}]}}"#,
    )
    .unwrap();
    assert!(guard::install(Agent::Claude, dir.path(), Path::new("eventlog")).unwrap());
    let text = std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["model"], "opus");
    let entries = json["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "{text}");
}
