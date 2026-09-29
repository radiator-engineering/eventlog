use assert_cmd::Command;
use tempfile::TempDir;

fn install(dir: &TempDir, extra: &[&str]) -> std::process::Output {
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(dir.path().join(".context/events.jsonl"), "").unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "install"])
        .args(extra)
        .current_dir(dir.path())
        .output()
        .unwrap()
}

#[test]
fn install_writes_the_mod_without_tests() {
    let dir = TempDir::new().unwrap();
    let out = install(&dir, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let base = dir.path().join(".claude/skills/eventlog-context");
    assert!(base.join(".claude-plugin/plugin.json").is_file());
    assert!(base.join("hooks/hooks.json").is_file());
    assert!(base.join("hooks/register.ts").is_file());
    assert!(base.join("hooks/policy.ts").is_file());
    assert!(!base.join("tests").exists());
    assert!(String::from_utf8_lossy(&out.stdout).contains("CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1"));
}

#[test]
fn install_is_repeatable() {
    let dir = TempDir::new().unwrap();
    assert!(install(&dir, &[]).status.success());
    assert!(install(&dir, &[]).status.success());
}

#[test]
fn install_refuses_when_classic_hook_present() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(
        dir.path().join(".claude/settings.json"),
        r#"{"hooks":{"SessionStart":[{"matcher":"compact","hooks":[{"type":"command","command":"eventlog context"}]}]}}"#,
    )
    .unwrap();
    let out = install(&dir, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("classic"));
    assert!(install(&dir, &["--force"]).status.success());
}

#[test]
fn install_refuses_when_classic_hook_present_in_settings_local() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(
        dir.path().join(".claude/settings.local.json"),
        r#"{"hooks":{"SessionStart":[{"matcher":"compact","hooks":[{"type":"command","command":"eventlog context"}]}]}}"#,
    )
    .unwrap();
    let out = install(&dir, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("classic"));
    assert!(install(&dir, &["--force"]).status.success());
}

#[test]
fn classic_prints_the_hook_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let out = install(&dir, &["--classic"]);
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains(r#""matcher": "compact""#));
    assert!(s.contains(r#""command": "eventlog context""#));
    assert!(!dir.path().join(".claude").exists());
}

#[test]
fn reinstall_removes_a_stray_file_the_mod_no_longer_ships() {
    let dir = TempDir::new().unwrap();
    assert!(install(&dir, &[]).status.success());
    let base = dir.path().join(".claude/skills/eventlog-context");
    let stray = base.join("hooks/stale-from-an-older-version.ts");
    std::fs::write(&stray, "// no longer shipped").unwrap();
    assert!(stray.is_file());
    assert!(install(&dir, &[]).status.success());
    assert!(!stray.exists());
    assert!(base.join("hooks/hooks.json").is_file());
}

#[test]
fn install_from_a_subdirectory_lands_at_the_git_repo_top_level() {
    let dir = TempDir::new().unwrap();
    let git = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(git.success());
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(dir.path().join(".context/events.jsonl"), "").unwrap();
    let sub = dir.path().join("sub/deeper");
    std::fs::create_dir_all(&sub).unwrap();
    let out = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "install"])
        .current_dir(&sub)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        dir.path()
            .join(".claude/skills/eventlog-context/hooks/hooks.json")
            .is_file()
    );
    assert!(!sub.join(".claude").exists());
}
