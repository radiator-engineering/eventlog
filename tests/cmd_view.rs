use assert_cmd::Command;
use assert_cmd::cargo::CommandCargoExt;
use predicates::str::contains;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn fixture_log(dir: &TempDir) -> std::path::PathBuf {
    let log = dir.path().join("events.jsonl");
    std::fs::copy("tests/fixtures/drove-events.jsonl", &log).unwrap();
    log
}

#[test]
fn last_three_prints_three_lines() {
    let dir = TempDir::new().unwrap();
    let log = fixture_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["view", "--last", "3", "--log"])
        .arg(&log)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3, "stdout:\n{stdout}");
    assert!(stdout.contains("285"));
    assert!(stdout.contains("286"));
    assert!(stdout.contains("287"));
}

#[test]
fn type_ack_json_rows_have_version_and_type() {
    let dir = TempDir::new().unwrap();
    let log = fixture_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["view", "--type", "ack", "--json", "--log"])
        .arg(&log)
        .output()
        .unwrap();

    assert!(output.status.success());
    for line in output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let row: serde_json::Value = serde_json::from_slice(line).unwrap();
        assert_eq!(row.get("v").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(row.get("type").and_then(|v| v.as_str()), Some("ack"));
    }
}

#[test]
fn agent_filter_matches_by_field() {
    let dir = TempDir::new().unwrap();
    let log = fixture_log(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["view", "--agent", "doc-worker", "--log"])
        .arg(&log)
        .assert()
        .success()
        .stdout(contains("5     ACK"));
}

#[test]
fn follow_prints_appended_line_within_two_seconds() {
    let dir = TempDir::new().unwrap();
    let log = fixture_log(&dir);

    let mut child = std::process::Command::cargo_bin("eventlog")
        .unwrap()
        .args(["view", "-f", "--log"])
        .arg(&log)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    std::thread::sleep(Duration::from_millis(300));

    let new_line =
        r#"{"seq":288,"ts":"2026-09-06T20:16:00Z","type":"note","msg":"follow test line"}"#;
    OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all(format!("{new_line}\n").as_bytes())
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut found = false;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) if line.contains("288") || line.contains("follow test line") => {
                found = true;
                break;
            }
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(found, "appended line did not appear within 2s");
}
