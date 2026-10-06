use assert_cmd::Command;

#[test]
fn every_frozen_command_is_recognized() {
    for c in [
        "append",
        "vocab",
        "verify",
        "view",
        "agents",
        "state",
        "why",
        "claims",
        "open",
        "tui",
        "guard",
        "init",
        "setup",
        "lifecycle",
        "doctor",
        "protect",
        "schema",
        "skill",
        "context",
        "completions",
    ] {
        Command::cargo_bin("eventlog")
            .unwrap()
            .arg(c)
            .arg("--help")
            .assert()
            .success();
    }
}

/// The reactor runtime and its actions live in `eventlog-reactors`.
#[test]
fn reactor_commands_are_not_part_of_the_log() {
    for c in ["react", "action"] {
        Command::cargo_bin("eventlog")
            .unwrap()
            .arg(c)
            .arg("--help")
            .assert()
            .failure();
    }
    let help = Command::cargo_bin("eventlog")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&help.stdout)
            .to_lowercase()
            .contains("reactor")
    );
}

#[test]
fn check_claims_is_a_hidden_alias() {
    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["check-claims", "--help"])
        .assert()
        .success();
    let help = Command::cargo_bin("eventlog")
        .unwrap()
        .arg("--help")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&help.stdout).contains("check-claims"));
}
