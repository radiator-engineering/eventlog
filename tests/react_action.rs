use eventlog::model::paths::validate_paths;
use eventlog::react::action::{ActionEnv, outside, run, snapshot, touched};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

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
