use assert_cmd::Command;
use std::path::Path;
use std::process::Command as StdCommand;

fn setup_repo(dir: &Path) {
    std::fs::create_dir_all(dir.join(".context")).unwrap();
}

#[test]
fn append_result_prints_seq_one_line() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "result", "ref=x"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("{\"seq\":1"));
    assert!(dir.path().join(".context/events.jsonl").is_file());
}

#[test]
fn append_dry_run_prints_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    let log = dir.path().join(".context/events.jsonl");

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "--dry-run", "result", "ref=x"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("{\"seq\":1"));
    assert!(!log.exists());
}

#[test]
fn append_missing_required_field_exits_one() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "result"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("missing field") && stderr.contains("ref"),
        "stderr: {stderr}"
    );
}

#[test]
fn vocab_result_json_lists_fields() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["vocab", "result", "--json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"fields\":[\"agent\",\"ref\"]"));
}

#[test]
fn append_contention_from_two_processes() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    let program =
        std::env::var("CARGO_BIN_EXE_eventlog").expect("CARGO_BIN_EXE_eventlog must be set");

    let mut children = Vec::new();
    for thread_id in 0..2 {
        let script = format!(
            "for j in $(seq 0 24); do {program} append note msg=t{thread_id}-$j || exit 1; done"
        );
        children.push(
            StdCommand::new("sh")
                .current_dir(dir.path())
                .args(["-c", &script])
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        let status = child.wait().unwrap();
        assert!(status.success(), "child failed");
    }

    let content = std::fs::read_to_string(dir.path().join(".context/events.jsonl")).unwrap();
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 50);
    let seqs: Vec<u64> = lines
        .iter()
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            v["seq"].as_u64().unwrap()
        })
        .collect();
    assert_eq!(seqs, (1..=50).collect::<Vec<_>>());
}

#[test]
fn strict_claim_accepts_a_glob_that_matches_under_the_repo_root() {
    let dir = tempfile::tempdir().unwrap();
    setup_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("src/api")).unwrap();
    std::fs::write(dir.path().join("src/api/ping.rs"), "").unwrap();

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "spawn", "agent=w1"])
        .assert()
        .success();

    // A directory glob matches a file under it, judged relative to the root.
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "--dry-run", "claim", "agent=w1", "paths=src/api/**"])
        .assert()
        .success();

    // A glob under a directory that does not exist is still rejected.
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "--dry-run", "claim", "agent=w1", "paths=nope/**"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("claim-path-missing"));
}
