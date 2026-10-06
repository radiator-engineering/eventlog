//! A repo that never opts into reactors: the log's own commands create no
//! reactor files and print nothing about reactors.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// Run `eventlog <args>` in `dir`; return its exit code and stdout+stderr.
fn eventlog(dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().unwrap_or(-1), text)
}

fn assert_no_reactor_output(command: &str, text: &str) {
    assert!(
        !text.to_lowercase().contains("reactor") && !text.contains("unacked"),
        "`eventlog {command}` mentioned reactors:\n{text}"
    );
}

#[test]
fn a_repo_without_reactor_config_hears_nothing_about_reactors() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    git(
        root,
        &["config", "user.email", "eventlog-test@example.invalid"],
    );
    git(root, &["config", "user.name", "eventlog test"]);
    fs::write(root.join("a.txt"), "a\n").unwrap();
    git(root, &["add", "a.txt"]);
    git(root, &["commit", "-qm", "base"]);

    let steps: &[&[&str]] = &[
        &["setup", "preview"],
        &["setup", "apply"],
        &["setup", "upgrade"],
        &["doctor"],
        &["append", "decision", "key=k", "value=v"],
        &["append", "spawn", "agent=w1"],
        &["append", "claim", "agent=w1", "paths=a.txt"],
        &[
            "append",
            "result",
            "agent=w1",
            "ref=a.txt",
            "paths=a.txt",
            "summary=hello",
        ],
        &["view"],
        &["state"],
        &["agents"],
        &["why", "4"],
        &["claims", "w1", "HEAD"],
        &["context"],
    ];
    for args in steps {
        let (code, text) = eventlog(root, args);
        let command = args.join(" ");
        // doctor warns about guards and protection on a bare repo, and
        // claims reports the setup files as unclaimed; neither fails here.
        if !matches!(args[0], "doctor" | "claims") {
            assert_eq!(code, 0, "`eventlog {command}` failed:\n{text}");
        }
        assert_no_reactor_output(&command, &text);
    }

    for file in [
        ".context/eventlog-setup.toml",
        ".context/eventlog-reactors.star",
    ] {
        assert!(!root.join(file).exists(), "setup created {file}");
    }
    let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert_no_reactor_output("setup apply (.gitignore)", &ignore);

    let (_, json) = eventlog(root, &["state", "--json"]);
    let state: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(state["reactors"], serde_json::json!([]));
}

#[test]
fn state_lists_reactors_once_the_log_has_an_ack_writer() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let (code, _) = eventlog(root, &["init"]);
    assert_eq!(code, 0);
    eventlog(root, &["append", "result", "ref=a.txt", "summary=one"]);
    let (code, text) = eventlog(
        root,
        &[
            "append",
            "ack",
            "--as",
            "committer",
            "seq_done=1",
            "outcome=committed",
            "ref=HEAD",
        ],
    );
    assert_eq!(code, 0, "{text}");
    eventlog(root, &["append", "result", "ref=b.txt", "summary=two"]);
    let (_, text) = eventlog(root, &["state"]);
    assert!(
        text.contains("reactors:\n  committer  last_ack=1"),
        "{text}"
    );
    assert!(text.contains("unacked=1"), "{text}");
}
