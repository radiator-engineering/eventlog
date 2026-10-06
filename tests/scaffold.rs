use assert_cmd::Command;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn init_git_repo(dir: &TempDir) {
    StdCommand::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "eventlog-test@example.invalid"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "eventlog test"])
        .current_dir(dir.path())
        .status()
        .unwrap();
}

fn snapshot(dir: &TempDir) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in [
        ".context/events.jsonl",
        ".context/EVENTLOG.md",
        ".context/eventlog.toml",
        ".gitignore",
        ".gitattributes",
    ] {
        let path = dir.path().join(rel);
        let content = fs::read_to_string(&path).unwrap_or_default();
        out.push((rel.to_string(), content));
    }
    out
}

#[test]
fn init_twice_leaves_identical_files() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    let first = snapshot(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    let second = snapshot(&dir);

    assert_eq!(first, second);
    assert!(dir.path().join(".context/events.jsonl").exists());
    assert!(dir.path().join(".context/EVENTLOG.md").exists());
    assert!(dir.path().join(".context/eventlog.toml").exists());
    let gitignore = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gitignore.contains(".context/events.jsonl"));
    assert!(!gitignore.contains("reactor"));
    let attrs = fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert!(attrs.contains(".context/events.jsonl -text"));
}

#[test]
fn lifecycle_start_stop_start_restores_only_its_own_claim() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::write(dir.path().join("owned.txt"), "x\n").unwrap();
    let run = |args: &[&str]| {
        Command::cargo_bin("eventlog")
            .unwrap()
            .current_dir(dir.path())
            .args(args)
            .assert()
            .success()
    };
    run(&["init"]);
    run(&[
        "lifecycle",
        "start",
        "worker",
        "--model",
        "stub",
        "--paths",
        "owned.txt",
    ]);
    run(&[
        "lifecycle",
        "start",
        "worker",
        "--model",
        "stub",
        "--paths",
        "owned.txt",
    ]);
    run(&["lifecycle", "stop", "worker"]);
    run(&[
        "lifecycle",
        "start",
        "worker",
        "--model",
        "stub",
        "--paths",
        "owned.txt",
    ]);
    let log = fs::read_to_string(dir.path().join(".context/events.jsonl")).unwrap();
    assert_eq!(log.matches("\"type\":\"spawn\"").count(), 2);
    assert_eq!(log.matches("\"type\":\"claim\"").count(), 2);
    assert_eq!(log.matches("\"type\":\"retire\"").count(), 1);
}

#[test]
fn lifecycle_rejects_invalid_claim_before_spawning_and_keeps_other_claims() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::write(dir.path().join("first.txt"), "x\n").unwrap();
    fs::write(dir.path().join("second.txt"), "x\n").unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["lifecycle", "start", "first", "--paths", "first.txt"])
        .assert()
        .success();
    let log = dir.path().join(".context/events.jsonl");
    let before = fs::read_to_string(&log).unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["lifecycle", "start", "broken", "--paths", "missing.txt"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("claim-path-missing"));
    assert_eq!(fs::read_to_string(&log).unwrap(), before);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["lifecycle", "start", "second", "--paths", "second.txt"])
        .assert()
        .success();
    let after = fs::read_to_string(&log).unwrap();
    assert!(after.contains("\"agent\":\"first\""));
    assert!(after.contains("\"agent\":\"second\""));
}

fn write_unsanctioned_log(dir: &TempDir) {
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(dir.path().join("foo.rs"), "// fixture\n").unwrap();
    let log = dir.path().join(".context/events.jsonl");
    fs::write(
        &log,
        concat!(
            r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"x"}"#,
            "\n",
            r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"prompt","agent":"x","ref":"x.md"}"#,
            "\n",
            r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"claim","by":"x","agent":"x","paths":"foo.rs"}"#,
            "\n",
        ),
    )
    .unwrap();
}

fn write_sanctioned_log(dir: &TempDir) {
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(dir.path().join("foo.rs"), "// fixture\n").unwrap();
    let log = dir.path().join(".context/events.jsonl");
    fs::write(
        &log,
        concat!(
            r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"x"}"#,
            "\n",
            r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"decision","key":"log-writers","value":"x:claim"}"#,
            "\n",
            r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"claim","by":"x","agent":"x","paths":"foo.rs"}"#,
            "\n",
        ),
    )
    .unwrap();
}

#[test]
fn doctor_flags_unsanctioned_writer_without_grant() {
    let dir = TempDir::new().unwrap();
    write_unsanctioned_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["doctor"])
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("[FAIL]") && combined.to_lowercase().contains("unsanctioned"),
        "expected unsanctioned FAIL, got:\n{combined}"
    );
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn doctor_ok_when_log_writers_grants_the_writer() {
    let dir = TempDir::new().unwrap();
    write_sanctioned_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["doctor"])
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.to_lowercase().contains("unsanctioned"),
        "unexpected unsanctioned row:\n{combined}"
    );
    assert!(
        !combined.contains("[FAIL]"),
        "expected no FAIL rows:\n{combined}"
    );
}

#[test]
#[ignore = "needs chflags/chattr permission; run locally with: cargo test --test scaffold -- --ignored"]
fn protect_status_before_and_after_protect() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["protect", "--status"])
        .assert()
        .failure()
        .code(1);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("protect")
        .assert()
        .success();

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["protect", "--status"])
        .assert()
        .success()
        .code(0);
}

#[test]
fn bracket_globs_work_in_lifecycle_and_strict_append() {
    for lifecycle in [false, true] {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "fixture").unwrap();
        let mut command = Command::cargo_bin("eventlog").unwrap();
        command.current_dir(dir.path());
        if lifecycle {
            command.args(["lifecycle", "start", "bracket", "--paths", "src/[ab].rs"]);
        } else {
            Command::cargo_bin("eventlog")
                .unwrap()
                .current_dir(dir.path())
                .args(["append", "spawn", "agent=bracket"])
                .assert()
                .success();
            command.args(["append", "claim", "agent=bracket", "paths=src/[ab].rs"]);
        }
        command.assert().success();
    }
}

#[test]
#[cfg(unix)]
fn claim_globs_do_not_follow_directory_symlink_cycles() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir(dir.path().join("src")).unwrap();
    std::os::unix::fs::symlink(dir.path(), dir.path().join("src/cycle")).unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["lifecycle", "start", "cycle", "--paths", "src/[ab].rs"])
        .timeout(std::time::Duration::from_secs(3))
        .assert()
        .failure()
        .stderr(predicates::str::contains("claim-path-missing"));
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "spawn", "agent=cycle"])
        .assert()
        .success();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["append", "claim", "agent=cycle", "paths=src/[ab].rs"])
        .timeout(std::time::Duration::from_secs(3))
        .assert()
        .failure()
        .stderr(predicates::str::contains("claim-path-missing"));
}
