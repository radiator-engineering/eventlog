use eventlog::model::paths::validate_paths;
use eventlog::react::action::{ActionEnv, newly_dirty, outside, run, snapshot, touched};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn test_env(outcome_file: PathBuf) -> ActionEnv {
    ActionEnv {
        log: PathBuf::from(".context/events.jsonl"),
        seq: 42,
        r#type: "result".to_string(),
        agent: "build-action".to_string(),
        by: "build-action".to_string(),
        paths: validate_paths("src/a.rs").unwrap(),
        reference: Some(".context/handoffs/build-action.md".to_string()),
        resume: 41,
        outcome_file,
    }
}

#[test]
fn run_reads_outcome_from_outcome_file() {
    let outcome_file = tempfile::NamedTempFile::new().unwrap();
    let path = outcome_file.path().to_path_buf();
    let env = test_env(path.clone());

    let command = vec![
        "sh".into(),
        "-c".into(),
        "printf 'outcome=committed\\nref=abc' > \"$EVENTLOG_OUTCOME_FILE\"".into(),
    ];
    let outcome = run(&command, "{}", &env, Duration::from_secs(5)).unwrap();

    assert!(!outcome.timed_out);
    assert_eq!(
        outcome.fields,
        vec![
            ("outcome".to_string(), "committed".to_string()),
            ("ref".to_string(), "abc".to_string()),
        ]
    );
}

#[test]
fn run_falls_back_to_last_stdout_outcome_line() {
    let env = test_env(PathBuf::from("/tmp/eventlog-outcome-missing-for-test"));

    let command = vec![
        "sh".into(),
        "-c".into(),
        "echo noise; echo outcome=skipped".into(),
    ];
    let outcome = run(&command, "{}", &env, Duration::from_secs(5)).unwrap();

    assert!(!outcome.timed_out);
    assert_eq!(
        outcome.fields,
        vec![("outcome".to_string(), "skipped".to_string())]
    );
}

#[test]
fn run_marks_timeout() {
    let env = test_env(PathBuf::from("/tmp/eventlog-outcome-missing-for-test"));

    let command = vec!["sleep".into(), "5".into()];
    let outcome = run(&command, "{}", &env, Duration::from_secs(1)).unwrap();

    assert!(outcome.timed_out);
}

#[cfg(unix)]
#[test]
fn run_keeps_a_bounded_stderr_tail_for_a_failed_action() {
    let env = test_env(PathBuf::from("/tmp/eventlog-outcome-missing-for-test"));
    let command = vec![
        "sh".into(),
        "-c".into(),
        "dd if=/dev/zero bs=1048576 count=4 2>/dev/null | tr '\\000' x >&2; echo unique-stderr-tail >&2; exit 7".into(),
    ];

    let outcome = run(&command, "{}", &env, Duration::from_secs(3)).unwrap();
    let stderr = outcome
        .fields
        .iter()
        .find(|(key, _)| key == "stderr")
        .map(|(_, value)| value)
        .expect("failed action lost stderr diagnostic");

    assert_eq!(outcome.exit, 7);
    assert!(stderr.contains("unique-stderr-tail"));
    assert!(stderr.len() <= 1024, "stderr tail was not bounded");
}

#[cfg(unix)]
#[test]
fn run_timeout_discards_an_early_success_outcome() {
    let env = test_env(PathBuf::from("/tmp/eventlog-outcome-missing-for-test"));
    let command = vec![
        "sh".into(),
        "-c".into(),
        "echo outcome=committed; sleep 5".into(),
    ];

    let outcome = run(&command, "{}", &env, Duration::from_millis(250)).unwrap();

    assert!(outcome.timed_out);
    assert!(outcome.fields.is_empty());
}

/// Draining while the child is live prevents a full stdout pipe from blocking
/// the child before it can write its final outcome line.
#[cfg(unix)]
#[test]
fn run_drains_large_stdout_before_waiting_for_exit() {
    let env = test_env(PathBuf::from("/tmp/eventlog-outcome-missing-for-test"));
    let command = vec![
        "sh".into(),
        "-c".into(),
        "dd if=/dev/zero bs=1048576 count=4 2>/dev/null | tr '\\000' x; printf '\\n'; echo outcome=skipped".into(),
    ];

    let outcome = run(&command, "{}", &env, Duration::from_secs(3)).unwrap();

    assert!(!outcome.timed_out);
    assert_eq!(outcome.fields, vec![("outcome".into(), "skipped".into())]);
}

/// A shell may exit while its background child still owns stdout. The action
/// cannot be considered complete until that pipe closes, and the deadline has
/// to kill the whole process group rather than waiting for the child forever.
#[cfg(unix)]
#[test]
fn run_timeout_kills_descendant_that_holds_stdout_open() {
    let dir = tempfile::tempdir().unwrap();
    let env = test_env(dir.path().join("outcome"));
    let command = vec![
        "sh".into(),
        "-c".into(),
        "sleep 5 & echo outcome=skipped".into(),
    ];

    let start = Instant::now();
    let outcome = run(&command, "{}", &env, Duration::from_millis(250)).unwrap();

    assert!(outcome.timed_out);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "descendant-held stdout exceeded the outer test guard"
    );
}

/// A descendant may retain stderr after its parent exits too. The bounded
/// diagnostics collector must not turn that into an unbounded timeout wait.
#[cfg(unix)]
#[test]
fn run_timeout_kills_descendant_that_holds_stderr_open() {
    let dir = tempfile::tempdir().unwrap();
    let env = test_env(dir.path().join("outcome"));
    let command = vec![
        "sh".into(),
        "-c".into(),
        "echo timeout-stderr-marker >&2; sleep 5 & echo outcome=skipped".into(),
    ];

    let start = Instant::now();
    let outcome = run(&command, "{}", &env, Duration::from_millis(250)).unwrap();

    assert!(outcome.timed_out);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "descendant-held stderr exceeded the outer test guard"
    );
    assert!(
        outcome
            .fields
            .iter()
            .any(|(key, value)| key == "stderr" && value.contains("timeout-stderr-marker")),
        "timed-out action lost its available stderr tail: {:?}",
        outcome.fields
    );
}

/// The deadline covers a blocked writer too. A command that never reads stdin
/// must not keep the reactor stuck in write_all before it starts waiting.
#[cfg(unix)]
#[test]
fn run_timeout_covers_stdin_backpressure() {
    let env = test_env(PathBuf::from("/tmp/eventlog-outcome-missing-for-test"));
    let command = vec!["sh".into(), "-c".into(), "sleep 5".into()];
    let stdin = "x".repeat(4 * 1024 * 1024);

    let start = Instant::now();
    let outcome = run(&command, &stdin, &env, Duration::from_millis(250)).unwrap();

    assert!(outcome.timed_out);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "blocked stdin exceeded the outer test guard"
    );
}

#[test]
fn outside_flags_unauthorized_committed_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    Command::new("git")
        .args(["init"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "t@test"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "t"])
        .current_dir(root)
        .status()
        .unwrap();

    std::fs::write(root.join("a.rs"), "a\n").unwrap();
    std::fs::write(root.join("b.rs"), "b\n").unwrap();
    Command::new("git")
        .args(["add", "a.rs", "b.rs"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(root)
        .status()
        .unwrap();

    let before = snapshot(root).unwrap();
    std::fs::write(root.join("a.rs"), "a2\n").unwrap();
    std::fs::write(root.join("b.rs"), "b2\n").unwrap();
    Command::new("git")
        .args(["add", "a.rs", "b.rs"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "both"])
        .current_dir(root)
        .status()
        .unwrap();
    let after = snapshot(root).unwrap();

    let touched = touched(&before, &after, root);
    let authorized = validate_paths("a.rs").unwrap();
    assert_eq!(outside(&touched, &authorized), vec!["b.rs".to_string()]);
}

fn git(root: &std::path::Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .unwrap()
            .success(),
        "git {args:?} failed"
    );
}

fn init_repo(root: &std::path::Path) {
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "t@test"]);
    git(root, &["config", "user.name", "t"]);
    std::fs::write(root.join("a.rs"), "a\n").unwrap();
    git(root, &["add", "a.rs"]);
    git(root, &["commit", "-q", "-m", "init"]);
}

/// `git status --porcelain` prints a tracked modification as ` M a.rs`, with
/// a leading space. Trimming the whole output ate that space on the first
/// line and the parser then dropped the first letter of the path ("GENTS.md").
#[test]
fn snapshot_keeps_the_leading_status_column_of_the_first_line() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    std::fs::write(root.join("a.rs"), "a2\n").unwrap();
    let snap = snapshot(root).unwrap();

    assert_eq!(
        snap.dirty.into_iter().collect::<Vec<_>>(),
        vec!["a.rs".to_string()]
    );
}

/// `touched` is what the action committed, and nothing else: a file that
/// became dirty during the action is somebody's work in progress, reported
/// by `newly_dirty` for an `observed` line, never as a violation.
#[test]
fn touched_is_committed_files_only_and_newly_dirty_is_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    std::fs::write(root.join("pre.rs"), "dirty before\n").unwrap();

    let before = snapshot(root).unwrap();
    std::fs::write(root.join("a.rs"), "a2\n").unwrap();
    git(root, &["add", "a.rs"]);
    git(root, &["commit", "-q", "-m", "a"]);
    std::fs::write(root.join("stray.rs"), "someone else\n").unwrap();
    let after = snapshot(root).unwrap();

    let committed = touched(&before, &after, root);
    assert_eq!(
        committed.iter().cloned().collect::<Vec<_>>(),
        vec!["a.rs".to_string()]
    );
    let dirty = newly_dirty(&before, &after);
    assert_eq!(
        dirty.into_iter().collect::<Vec<_>>(),
        vec!["stray.rs".to_string()],
        "pre.rs was dirty before the action and must not be reported"
    );
}

/// NUL-delimited Git output keeps unusual valid names exact. In particular,
/// whitespace, quotes, newlines and a literal ` -> ` are not porcelain syntax.
#[test]
fn git_accounting_preserves_unusual_and_renamed_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);

    let old = "old -> name.rs";
    let new = "new \"quoted\"\nname.rs";
    std::fs::write(root.join(old), "old\n").unwrap();
    git(root, &["add", old]);
    git(root, &["commit", "-q", "-m", "add unusual name"]);
    let before = snapshot(root).unwrap();

    std::fs::rename(root.join(old), root.join(new)).unwrap();
    std::fs::write(root.join("space name.rs"), "untracked\n").unwrap();
    git(root, &["add", "-A"]);
    let staged = snapshot(root).unwrap();
    assert!(staged.dirty.contains(old));
    assert!(staged.dirty.contains(new));
    assert!(staged.dirty.contains("space name.rs"));

    git(root, &["commit", "-q", "-m", "rename unusual name"]);
    let after = snapshot(root).unwrap();
    let changed = touched(&before, &after, root);
    assert!(changed.contains(old));
    assert!(changed.contains(new));
}
