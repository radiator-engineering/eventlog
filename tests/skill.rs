use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn install_writes_skill_and_stamp() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["skill", "install", "--dir"])
        .arg(dir.path())
        .assert()
        .success();

    let skill_dir = dir.path().join("event-log-coordination");
    assert!(skill_dir.join("SKILL.md").exists());

    let stamp = fs::read_to_string(skill_dir.join(".eventlog-version")).unwrap();
    assert_eq!(stamp.trim(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn install_refuses_a_newer_existing_stamp() {
    let dir = tempdir().unwrap();
    let skill_dir = dir.path().join("event-log-coordination");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join(".eventlog-version"), "99.0.0").unwrap();

    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["skill", "install", "--dir"])
        .arg(dir.path())
        .assert()
        .failure()
        .code(1);

    // Refused: the newer stamp is left untouched.
    let stamp = fs::read_to_string(skill_dir.join(".eventlog-version")).unwrap();
    assert_eq!(stamp.trim(), "99.0.0");
}

#[test]
fn install_force_overrides_a_newer_stamp() {
    let dir = tempdir().unwrap();
    let skill_dir = dir.path().join("event-log-coordination");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join(".eventlog-version"), "99.0.0").unwrap();

    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["skill", "install", "--force", "--dir"])
        .arg(dir.path())
        .assert()
        .success();

    let stamp = fs::read_to_string(skill_dir.join(".eventlog-version")).unwrap();
    assert_eq!(stamp.trim(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn completions_zsh_output_contains_the_completion_function() {
    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["completions", "zsh"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("_eventlog"));
}
