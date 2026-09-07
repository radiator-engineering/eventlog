//! `eventlog react` and `eventlog react test` (spec section 7, plan task 17).
//!
//! `react test` is a dry run: it prints the events one reaction would append
//! and leaves the log byte-for-byte alone. `react` is the live loop: it
//! baselines, then turns each matching event into `intent` and `ack`, and
//! honours a `veto` that lands inside the window.

use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::Command;

fn setup_repo(dir: &Path) {
    std::fs::create_dir_all(dir.join(".context")).unwrap();
}

fn bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("eventlog")
}

/// Append one event through the CLI and return the seq it was given.
fn append(dir: &Path, args: &[&str]) -> u64 {
    let output = Command::new(bin())
        .current_dir(dir)
        .arg("append")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "append {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap();
    let json: serde_json::Value = serde_json::from_str(line).unwrap();
    json["seq"].as_u64().unwrap()
}

fn read_log(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".context/events.jsonl")).unwrap_or_default()
}

/// Every log line as JSON, ignoring a torn tail mid-write.
fn events(dir: &Path) -> Vec<serde_json::Value> {
    read_log(dir)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Poll the log until `pred` matches a line, or fail after `within`.
fn wait_for(
    dir: &Path,
    within: Duration,
    what: &str,
    pred: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = Instant::now() + within;
    loop {
        if let Some(found) = events(dir).into_iter().find(&pred) {
            return found;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for {what}; log:\n{}", read_log(dir));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A running reactor, killed when the test drops it.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_reactor(dir: &Path, args: &[&str]) -> Running {
    let child = StdCommand::new(bin())
        .current_dir(dir)
        .arg("react")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    Running(child)
}

fn str_field(event: &serde_json::Value, key: &str) -> String {
    event[key].as_str().unwrap_or_default().to_string()
}

#[test]
fn react_test_prints_intent_and_ack_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    append(dir.path(), &["note", "msg=start"]);
    let seq = append(
        dir.path(),
        &["result", "ref=a.md", "paths=a.md", "summary=landed a.md"],
    );
    let before = read_log(dir.path());

    let output = Command::new(bin())
        .current_dir(dir.path())
        .args([
            "react",
            "test",
            &seq.to_string(),
            "--as",
            "t",
            "--",
            "sh",
            "-c",
            "echo outcome=committed",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let printed: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    assert!(
        printed.iter().any(|e| str_field(e, "type") == "intent"
            && e["for"].as_str() == Some(seq.to_string().as_str())),
        "no intent in: {stdout}"
    );
    assert!(
        printed
            .iter()
            .any(|e| str_field(e, "type") == "ack" && str_field(e, "outcome") == "committed"),
        "no committed ack in: {stdout}"
    );
    assert_eq!(before, read_log(dir.path()), "react test wrote to the log");
}

#[test]
fn react_acks_a_result_it_watches() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    append(dir.path(), &["note", "msg=start"]);

    let _reactor = start_reactor(
        dir.path(),
        &[
            "--as",
            "t",
            "--on",
            "result",
            "--",
            "sh",
            "-c",
            "echo outcome=committed",
        ],
    );
    // The baseline ack tells us the reactor is watching; anything appended
    // before it would be skipped as history.
    wait_for(
        dir.path(),
        Duration::from_secs(10),
        "the baseline ack",
        |e| str_field(e, "type") == "ack" && str_field(e, "by") == "t",
    );

    let seq = append(
        dir.path(),
        &["result", "ref=a.md", "paths=a.md", "summary=landed a.md"],
    );

    let want = seq.to_string();
    wait_for(dir.path(), Duration::from_secs(3), "the intent", |e| {
        str_field(e, "type") == "intent" && str_field(e, "by") == "t" && str_field(e, "for") == want
    });
    let ack = wait_for(dir.path(), Duration::from_secs(3), "the ack", |e| {
        str_field(e, "type") == "ack"
            && str_field(e, "by") == "t"
            && str_field(e, "seq_done") == want
    });
    assert_eq!(str_field(&ack, "outcome"), "committed");
}

#[cfg(unix)]
#[test]
fn a_failed_action_keeps_stderr_in_its_ack_detail() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    append(dir.path(), &["note", "msg=start"]);

    let _reactor = start_reactor(
        dir.path(),
        &[
            "--as",
            "t",
            "--on",
            "result",
            "--",
            "sh",
            "-c",
            "echo unique-reactor-stderr >&2; exit 7",
        ],
    );
    wait_for(
        dir.path(),
        Duration::from_secs(10),
        "the baseline ack",
        |e| str_field(e, "type") == "ack" && str_field(e, "by") == "t",
    );

    let seq = append(
        dir.path(),
        &["result", "ref=a.md", "paths=a.md", "summary=landed a.md"],
    );
    let want = seq.to_string();
    let ack = wait_for(dir.path(), Duration::from_secs(3), "the failed ack", |e| {
        str_field(e, "type") == "ack"
            && str_field(e, "by") == "t"
            && str_field(e, "seq_done") == want
    });

    assert_eq!(str_field(&ack, "outcome"), "failed");
    assert!(str_field(&ack, "detail").contains("unique-reactor-stderr"));
    assert!(str_field(&ack, "detail").contains("exit 7"));
}

/// Invalid stderr bytes expand during lossy UTF-8 decoding. The reactor must
/// still append its failed ack rather than exceeding the log field limit.
#[cfg(unix)]
#[test]
fn a_failed_action_with_invalid_utf8_stderr_still_acks() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    append(dir.path(), &["note", "msg=start"]);

    let _reactor = start_reactor(
        dir.path(),
        &[
            "--as",
            "binary-diag",
            "--on",
            "result",
            "--",
            "python3",
            "-c",
            "import os; os.write(2, bytes([255]) * 1024 + b'binary-stderr-tail'); raise SystemExit(7)",
        ],
    );
    wait_for(
        dir.path(),
        Duration::from_secs(10),
        "the baseline ack",
        |e| str_field(e, "type") == "ack" && str_field(e, "by") == "binary-diag",
    );

    let seq = append(
        dir.path(),
        &["result", "ref=a.md", "paths=a.md", "summary=landed a.md"],
    );
    let want = seq.to_string();
    let ack = wait_for(dir.path(), Duration::from_secs(3), "the failed ack", |e| {
        str_field(e, "type") == "ack"
            && str_field(e, "by") == "binary-diag"
            && str_field(e, "seq_done") == want
    });

    assert_eq!(str_field(&ack, "outcome"), "failed");
    let detail = str_field(&ack, "detail");
    assert!(
        detail.len() <= 2048,
        "oversized detail: {} bytes",
        detail.len()
    );
    assert!(detail.contains("binary-stderr-tail"), "detail: {detail}");
    assert!(detail.contains("exit 7"), "detail: {detail}");
}

#[test]
fn the_outcome_file_names_the_outcome_and_its_extras_land_in_detail() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    append(dir.path(), &["note", "msg=start"]);

    let _reactor = start_reactor(
        dir.path(),
        &[
            "--as",
            "t",
            "--on",
            "result",
            "--",
            "sh",
            "-c",
            "printf 'outcome=updated\\nfiles=3\\nref=abc\\n' > \"$EVENTLOG_OUTCOME_FILE\"",
        ],
    );
    wait_for(
        dir.path(),
        Duration::from_secs(10),
        "the baseline ack",
        |e| str_field(e, "type") == "ack" && str_field(e, "by") == "t",
    );

    let seq = append(
        dir.path(),
        &["result", "ref=a.md", "paths=a.md", "summary=landed a.md"],
    );
    let want = seq.to_string();
    let ack = wait_for(dir.path(), Duration::from_secs(3), "the ack", |e| {
        str_field(e, "type") == "ack"
            && str_field(e, "by") == "t"
            && str_field(e, "seq_done") == want
    });
    assert_eq!(str_field(&ack, "outcome"), "updated");
    assert_eq!(str_field(&ack, "ref"), "abc");
    // `files` is not an `ack` field, so it is carried rather than dropped.
    assert!(
        str_field(&ack, "detail").contains("files=3"),
        "detail lost the extra field: {ack}"
    );
}

#[test]
fn a_veto_inside_the_window_stops_the_action() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    append(dir.path(), &["note", "msg=start"]);
    let marker = dir.path().join("ran");

    let _reactor = start_reactor(
        dir.path(),
        &[
            "--as",
            "t",
            "--on",
            "result",
            "--window",
            "2s",
            "--",
            "sh",
            "-c",
            &format!("touch {}; echo outcome=committed", marker.display()),
        ],
    );
    wait_for(
        dir.path(),
        Duration::from_secs(10),
        "the baseline ack",
        |e| str_field(e, "type") == "ack" && str_field(e, "by") == "t",
    );

    let seq = append(
        dir.path(),
        &["result", "ref=a.md", "paths=a.md", "summary=landed a.md"],
    );
    let want = seq.to_string();
    wait_for(dir.path(), Duration::from_secs(3), "the intent", |e| {
        str_field(e, "type") == "intent" && str_field(e, "for") == want
    });

    append(
        dir.path(),
        &[
            "veto",
            &format!("for={seq}"),
            "role=voter",
            "reason=not-yet",
        ],
    );

    let ack = wait_for(dir.path(), Duration::from_secs(5), "the vetoed ack", |e| {
        str_field(e, "type") == "ack"
            && str_field(e, "by") == "t"
            && str_field(e, "seq_done") == want
    });
    assert_eq!(str_field(&ack, "outcome"), "vetoed");
    assert!(!marker.exists(), "the action ran despite the veto");
}
