use assert_cmd::Command;
use std::{fs, thread, time::Duration};
use tempfile::TempDir;

/// The checked-in fixture lives inside this crate's own git worktree, so
/// running the CLI directly against it would pick up the whole worktree's
/// `git status`. Copy it into an isolated temp dir (outside any repo) first,
/// so `eventlog context` sees a deterministic, git-less working tree. The
/// fixture files themselves live outside any `.context/` dir (and the log
/// avoids the `events.jsonl` basename) so the coordination-log guard never
/// matches them; this function lays them back out at the paths
/// `eventlog context` expects to find them (`.context/events.jsonl`,
/// `.context/handoffs/task.md`), so the rendered `ref`/log-path text in the
/// golden file matches a real run.
fn isolated_fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let src = std::path::Path::new("tests/fixtures/context");
    fs::create_dir_all(dir.path().join(".context/handoffs")).unwrap();
    fs::copy(
        src.join("log.fixture.jsonl"),
        dir.path().join(".context/events.jsonl"),
    )
    .unwrap();
    fs::copy(
        src.join("handoffs/task.md"),
        dir.path().join(".context/handoffs/task.md"),
    )
    .unwrap();
    dir
}

fn repo_with_log(lines: &[&str]) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(
        dir.path().join(".context/events.jsonl"),
        lines.join("\n") + "\n",
    )
    .unwrap();
    dir
}

const LOG: &[&str] = &[
    r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"decision","key":"k","value":"v"}"#,
    r#"{"seq":2,"ts":"2026-09-01T01:00:00Z","type":"result","agent":"controller","ref":"a.md","summary":"did a"}"#,
];

#[test]
fn context_outside_git_still_renders() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog")
        .unwrap()
        .arg("context")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let md = String::from_utf8_lossy(&out.stdout);
    assert!(md.starts_with("# Context rebuilt from the event log"));
    assert!(md.contains("- k=v (seq 1)"));
    assert!(md.contains("(working tree unavailable)"));
}

#[test]
fn context_json_has_settings() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "--json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["as_of"], 2);
    assert_eq!(v["v"], 1);
    assert!(v["markdown"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(v["settings"]["keep_turns"], 3);
    assert!(v["settings"]["tail_chars"].is_number());
}

#[test]
fn check_prints_json_verdict() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "check", "--percent", "30"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"rebuild": true, "reason": "boundary"})
    );
}

#[test]
fn check_rejects_percent_over_100() {
    let dir = repo_with_log(LOG);
    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "check", "--percent", "101"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn check_growth_pushes_over_backstop() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "check", "--percent", "55", "--growth", "10"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"rebuild": true, "reason": "backstop"})
    );
}

#[test]
fn over_budget_warns_on_stderr() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "--budget", "50"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("over budget"));
}

#[test]
fn budget_conflicts_with_subcommand() {
    let dir = repo_with_log(LOG);
    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "--budget", "50", "check", "--percent", "10"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn matches_golden_fixture() {
    let expected = fs::read_to_string("tests/fixtures/context/context.golden.md").unwrap();
    let dir = isolated_fixture();
    let out = Command::cargo_bin("eventlog")
        .unwrap()
        .arg("context")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}

#[test]
fn same_input_same_bytes_a_second_apart() {
    // Guards against a future wall-clock read creeping into the render path:
    // nothing in it reads the current time today, so this is a regression
    // trip-wire, not a check of any existing behavior.
    let dir = isolated_fixture();
    let run = || {
        Command::cargo_bin("eventlog")
            .unwrap()
            .arg("context")
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout
    };
    let first = run();
    thread::sleep(Duration::from_secs(1));
    let second = run();
    assert_eq!(first, second);
}
