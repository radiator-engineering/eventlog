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

#[test]
fn a_pipe_inside_a_quoted_jq_filter_is_a_read_not_a_compound_command() {
    let cmd = r#"jq -c 'select(.type=="ack") | {seq,outcome}' .context/events.jsonl"#;
    let action = eventlog::guard::Action::Shell {
        command: cmd.to_string(),
    };
    let dec = eventlog::guard::deny::decide(&action, std::path::Path::new(".context/events.jsonl"));
    assert_eq!(dec, eventlog::guard::deny::Decision::Allow);
    let cmd = r#"jq -c 'select(.type=="ack")' .context/events.jsonl | head"#;
    let action = eventlog::guard::Action::Shell {
        command: cmd.to_string(),
    };
    let dec = eventlog::guard::deny::decide(&action, std::path::Path::new(".context/events.jsonl"));
    assert!(matches!(dec, eventlog::guard::deny::Decision::Deny(_)));
}

// ---------------------------------------------------------------------------
// Mutation-killing tests for src/guard/deny.rs. Each test pins one branch that
// `cargo mutants` found unobserved. Decisions are taken in-process through
// `decide`, except where a mutant would hang (then the binary runs under a
// timeout) or where the behaviour depends on the environment.
// ---------------------------------------------------------------------------

use eventlog::guard::{Action, Decision, decide, is_simple_sanctioned_writer};
use std::time::Duration;

fn decision(command: &str) -> Decision {
    decide(
        &Action::Shell {
            command: command.to_string(),
        },
        Path::new(".context/events.jsonl"),
    )
}

fn assert_allowed_cmd(command: &str) {
    assert_eq!(decision(command), Decision::Allow, "{command} should be allowed");
}

fn assert_denied_cmd(command: &str, reason_fragment: &str) {
    match decision(command) {
        Decision::Deny(reason) => assert!(
            reason.contains(reason_fragment),
            "{command}: denied, but reason {reason:?} does not mention {reason_fragment:?}"
        ),
        Decision::Allow => panic!("{command} should be denied"),
    }
}

/// `eventlog guard` in a scratch repo with `env` set, killed after `timeout`.
fn guard_cmd_with(
    dir: &Path,
    payload: &str,
    env: &[(&str, &str)],
    timeout: Duration,
) -> std::process::Output {
    let mut cmd = Command::cargo_bin("eventlog").unwrap();
    cmd.current_dir(dir)
        .args(["guard", "--log", ".context/events.jsonl"])
        .write_stdin(payload.to_string())
        .timeout(timeout);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

// protected_basename: EVENTLOG_GUARD_BASENAME wins when set and non-empty.

#[test]
fn guard_basename_env_var_overrides_the_log_path() {
    let dir = scratch();
    let env = [("EVENTLOG_GUARD_BASENAME", "custom.jsonl")];
    let out = guard_cmd_with(
        dir.path(),
        &bash_payload("rm custom.jsonl"),
        &env,
        Duration::from_secs(30),
    );
    assert_denied(&out, "rm custom.jsonl with EVENTLOG_GUARD_BASENAME=custom.jsonl");
    let out = guard_cmd_with(
        dir.path(),
        &bash_payload("rm .context/events.jsonl"),
        &env,
        Duration::from_secs(30),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "with the basename overridden, events.jsonl is no longer protected"
    );
}

#[test]
fn an_empty_guard_basename_env_var_falls_back_to_the_log_path() {
    let dir = scratch();
    let env = [("EVENTLOG_GUARD_BASENAME", "")];
    let out = guard_cmd_with(
        dir.path(),
        &bash_payload("rm .context/events.jsonl"),
        &env,
        Duration::from_secs(30),
    );
    assert_denied(&out, "rm events.jsonl with EVENTLOG_GUARD_BASENAME empty");
}

// is_simple_sanctioned_writer: each redirect and background character alone
// disqualifies the writer.

#[test]
fn a_sanctioned_writer_with_any_redirect_or_background_is_not_simple() {
    assert!(is_simple_sanctioned_writer("eventlog append note x=1"));
    assert!(!is_simple_sanctioned_writer("eventlog append note x=1 > out.txt"));
    assert!(!is_simple_sanctioned_writer("eventlog append note x=1 < seed.txt"));
    assert!(!is_simple_sanctioned_writer("eventlog append note x=1 &"));
    assert!(!is_simple_sanctioned_writer("append-event.sh note x=1 & rm y"));
}

#[test]
fn a_sanctioned_writer_redirected_into_the_log_is_denied() {
    assert_denied_cmd(
        "eventlog append note x=1 > .context/events.jsonl",
        "truncating redirect",
    );
}

#[test]
fn a_backgrounded_sanctioned_writer_does_not_launder_a_following_rm() {
    assert_denied_cmd("eventlog append note x=1 & rm .context/events.jsonl", "'rm'");
}

// truncating_redirect_into_log: a `>` inside quotes is text, not a redirect.

#[test]
fn a_redirect_inside_single_quotes_is_not_a_redirect() {
    assert_allowed_cmd(
        r#"jq -c 'select(.ref == "> .context/events.jsonl")' .context/events.jsonl"#,
    );
}

#[test]
fn a_redirect_inside_double_quotes_is_not_a_redirect() {
    assert_allowed_cmd(r#"grep -F "> .context/events.jsonl" .context/events.jsonl"#);
}

#[test]
fn an_unquoted_truncating_redirect_into_the_log_is_denied() {
    assert_denied_cmd("echo x > .context/events.jsonl", "truncating redirect");
    // `>|` is caught earlier by the compound-command rule (the `|`), so only
    // the decision is pinned here, not the reason.
    assert!(matches!(
        decision("echo x >| .context/events.jsonl"),
        Decision::Deny(_)
    ));
    assert_allowed_cmd("echo x >> .context/events.jsonl");
}

// opens_for_writing: a read-only open() must be allowed, and scanning must
// move past each match (a stuck scan would hang the guard).

#[test]
fn an_interpreter_opening_the_log_for_reading_is_allowed() {
    let dir = scratch();
    let out = guard_cmd_with(
        dir.path(),
        &bash_payload(r#"python3 -c "print(open('.context/events.jsonl').read())""#),
        &[],
        Duration::from_secs(20),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "read-only open() should be allowed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = guard_cmd_with(
        dir.path(),
        &bash_payload(r#"python3 -c "open('x').read(); open('.context/events.jsonl','w')""#),
        &[],
        Duration::from_secs(20),
    );
    assert_denied(&out, "a later open(...,'w') after a read-only open()");
}

// mutation_reason: the in-place edit check per interpreter.

#[test]
fn perl_flag_clusters_containing_i_are_in_place_edits() {
    assert_denied_cmd(
        "perl -pi -e s/a/b/ .context/events.jsonl",
        "in-place 'perl -i'",
    );
}

#[test]
fn only_perl_treats_an_i_anywhere_in_a_flag_as_in_place() {
    // `--quiet` carries an `i` but is not `sed -i`.
    assert_allowed_cmd("sed --quiet 1p .context/events.jsonl");
    assert_denied_cmd("sed -i.bak d .context/events.jsonl", "in-place 'sed -i'");
}

#[test]
fn a_quoted_perl_switch_word_is_judged_whole() {
    // The quoted argument is one word starting with `-` that carries an `i`,
    // so it is judged as a perl flag cluster.
    assert_denied_cmd("perl '-e print' .context/events.jsonl", "in-place 'perl -i'");
}

// mutation_reason: the tee append check, clause by clause.

#[test]
fn tee_without_append_into_the_log_is_denied() {
    assert_denied_cmd("tee .context/events.jsonl", "non-append 'tee'");
}

#[test]
fn tee_with_append_into_the_log_is_allowed() {
    assert_allowed_cmd("tee -a .context/events.jsonl");
    assert_allowed_cmd("tee --append .context/events.jsonl");
    assert_allowed_cmd("tee -ai .context/events.jsonl");
}

#[test]
fn a_tee_flag_without_a_is_not_append() {
    assert_denied_cmd("tee -i .context/events.jsonl", "non-append 'tee'");
}
