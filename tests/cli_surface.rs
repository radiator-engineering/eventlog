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
        "react",
        "guard",
        "init",
        "setup",
        "action",
        "lifecycle",
        "doctor",
        "protect",
        "schema",
        "skill",
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
