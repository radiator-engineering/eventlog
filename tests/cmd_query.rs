//! `agents`, `state`, and `why` commands. Task 11.

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

const FIXTURE: &str = "tests/fixtures/self-log-2026-09-06.jsonl";

fn fixture_log(dir: &TempDir) -> std::path::PathBuf {
    let log = dir.path().join("events.jsonl");
    std::fs::copy(FIXTURE, &log).unwrap();
    log
}

#[test]
fn agents_json_rows_have_version() {
    let dir = TempDir::new().unwrap();
    let log = fixture_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["agents", "--json", "--log"])
        .arg(&log)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let rows: Vec<_> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(!rows.is_empty(), "expected agent rows");
    for line in rows {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(row.get("v").and_then(|v| v.as_i64()), Some(1));
        assert!(row.get("agent").is_some());
    }
}

#[test]
fn state_at_20_lists_only_agents_alive_then() {
    let dir = TempDir::new().unwrap();
    let log = fixture_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["state", "--at", "20", "--log"])
        .arg(&log)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cursor-committer"));
    assert!(stdout.contains("doc-worker"));
    assert!(!stdout.contains("build-scaffold"));
    assert!(!stdout.contains("logact-deep-read"));
}

#[test]
fn why_39_reports_the_committer_ack() {
    let dir = TempDir::new().unwrap();
    let log = fixture_log(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["why", "39", "--log"])
        .arg(&log)
        .assert()
        .success()
        .stdout(contains("seq 42"));
}
