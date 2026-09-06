use assert_cmd::Command;
use std::path::Path;
use std::process::Command as SysCommand;
use tempfile::TempDir;

fn init_git_repo(root: &Path) {
    SysCommand::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root)
        .output()
        .unwrap();
    SysCommand::new("git")
        .args(["config", "user.email", "t@test"])
        .current_dir(root)
        .output()
        .unwrap();
    SysCommand::new("git")
        .args(["config", "user.name", "t"])
        .current_dir(root)
        .output()
        .unwrap();
}

fn git_commit(root: &Path, message: &str) {
    SysCommand::new("git")
        .args(["add", "-A"])
        .current_dir(root)
        .output()
        .unwrap();
    SysCommand::new("git")
        .args(["commit", "-m", message])
        .current_dir(root)
        .output()
        .unwrap();
}

fn git_rev(root: &Path) -> String {
    String::from_utf8(
        SysCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string()
}

#[test]
fn lists_unclaimed_changed_and_untracked() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    init_git_repo(root);

    std::fs::create_dir_all(root.join(".context")).unwrap();
    std::fs::write(root.join(".gitignore"), ".context/events.jsonl\n").unwrap();
    let log = root.join(".context/events.jsonl");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/.gitkeep"), "").unwrap();
    git_commit(root, "init");
    let base = git_rev(root);

    let claim = concat!(
        r#"{"seq":1,"ts":"2026-01-01T00:00:00Z","type":"claim","prev":"genesis","#,
        r#""agent":"worker","paths":"src/**"}"#
    );
    std::fs::write(&log, claim).unwrap();

    std::fs::write(root.join("src/a.rs"), "fn main() {}").unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/b.md"), "doc").unwrap();
    git_commit(root, "change");

    std::fs::write(root.join("docs/c.md"), "new").unwrap();

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(root)
        .args(["claims", "worker"])
        .arg(&base)
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1, got:\n{combined}"
    );
    assert!(
        combined.contains("docs/b.md"),
        "expected docs/b.md in output:\n{combined}"
    );
    assert!(
        combined.contains("docs/c.md"),
        "expected docs/c.md in output:\n{combined}"
    );
    assert!(
        !combined.contains("src/a.rs"),
        "src/a.rs is claimed and must not appear:\n{combined}"
    );
}
