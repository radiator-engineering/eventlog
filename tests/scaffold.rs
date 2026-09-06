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
    assert!(gitignore.contains(".context/*.reactor.lock/"));
    let attrs = fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert!(attrs.contains(".context/events.jsonl -text"));
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
