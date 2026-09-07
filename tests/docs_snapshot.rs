use assert_cmd::Command;
use std::{fs, process::Command as Git};
use tempfile::TempDir;

fn fixture(script: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.name", "test"],
        vec!["config", "user.email", "test@example.invalid"],
    ] {
        assert!(
            Git::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success()
        );
    }
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(dir.path().join("docs/page.md"), "before\n").unwrap();
    for args in [vec!["add", "docs"], vec!["commit", "-qm", "source"]] {
        assert!(
            Git::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success()
        );
    }
    let head = Git::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let head = String::from_utf8(head.stdout).unwrap();
    let log = dir.path().join(".context/custom.jsonl");
    fs::write(&log, format!("{{\"seq\":1,\"ts\":\"2026-09-07T00:00:00Z\",\"type\":\"result\",\"agent\":\"implementation\",\"paths\":\"docs/page.md\"}}\n{{\"seq\":2,\"ts\":\"2026-09-07T00:00:01Z\",\"type\":\"ack\",\"by\":\"committer\",\"seq_done\":\"1\",\"outcome\":\"committed\",\"ref\":\"{}\"}}\n", head.trim())).unwrap();
    let cfg = toml::toml! { [docs] roots = ["docs"] command = ["sh", "-c", script] };
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        toml::to_string(&cfg).unwrap(),
    )
    .unwrap();
    (dir, log)
}

fn action(dir: &TempDir, log: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("eventlog").unwrap();
    command
        .current_dir(dir.path())
        .env("EVENTLOG_LOG", log)
        .env("EVENTLOG_TYPE", "ack")
        .env("EVENTLOG_SEQ", "2")
        .env(
            "EVENTLOG_TEST_BIN",
            assert_cmd::cargo::cargo_bin("eventlog"),
        )
        .args(["action", "docs"]);
    command
}

#[test]
fn docs_snapshot_excludes_concurrent_log_and_reserved_sidecars() {
    let (dir, log) = fixture(
        r#""$EVENTLOG_TEST_BIN" --log "$EVENTLOG_LOG" append note msg=concurrent &
printf after > docs/page.md
mkdir -p "$EVENTLOG_LOG.docs.reactor.lock"
printf owner > "$EVENTLOG_LOG.docs.reactor.lock/owner"
wait"#,
    );
    action(&dir, &log)
        .assert()
        .success()
        .stdout(predicates::str::contains("outcome=updated"));
    let events = fs::read_to_string(&log).unwrap();
    assert!(events.contains("concurrent"));
    let last: serde_json::Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert_eq!(last["type"], "result");
    assert_eq!(last["paths"], "docs/page.md");
}

#[test]
fn docs_snapshot_still_rejects_other_context_edits() {
    let (dir, log) = fixture(
        r#""$EVENTLOG_TEST_BIN" --log "$EVENTLOG_LOG" append note msg=concurrent &
printf after > docs/page.md
printf unrelated > .context/unrelated.txt
wait"#,
    );
    action(&dir, &log)
        .assert()
        .failure()
        .stdout(predicates::str::contains(
            "outside configured roots: .context/unrelated.txt",
        ));
}

#[test]
fn docs_snapshot_no_changes_skips_without_result() {
    let (dir, log) = fixture("true");
    let before = fs::read(&log).unwrap();
    action(&dir, &log)
        .assert()
        .success()
        .stdout(predicates::str::contains("outcome=skipped"));
    assert_eq!(fs::read(log).unwrap(), before);
}

#[test]
fn docs_snapshot_log_only_changes_skip_without_result() {
    let (dir, log) =
        fixture(r#""$EVENTLOG_TEST_BIN" --log "$EVENTLOG_LOG" append note msg=concurrent"#);
    action(&dir, &log)
        .assert()
        .success()
        .stdout(predicates::str::contains("outcome=skipped"));
    let events = fs::read_to_string(log).unwrap();
    assert_eq!(events.lines().count(), 3);
    let last: serde_json::Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert_eq!(last["type"], "note");
}

#[test]
fn docs_snapshot_uses_configured_log_without_reactor_environment() {
    let (dir, log) = fixture(
        r#""$EVENTLOG_TEST_BIN" append note msg=concurrent
printf after > docs/page.md"#,
    );
    fs::write(
        dir.path().join(".context/eventlog.toml"),
        "[log]\npath = '.context/custom.jsonl'\n",
    )
    .unwrap();
    action(&dir, &log)
        .env_remove("EVENTLOG_LOG")
        .env_remove("EVENTLOG_TYPE")
        .env_remove("EVENTLOG_SEQ")
        .assert()
        .success()
        .stdout(predicates::str::contains("outcome=updated"));
    assert!(fs::read_to_string(log).unwrap().contains("concurrent"));
}
