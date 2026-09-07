#![cfg(unix)]

use assert_cmd::Command;
use std::fs;
use std::process::Command as Process;
use tempfile::TempDir;

fn git(dir: &TempDir, args: &[&str]) -> String {
    let output = Process::new("git")
        .current_dir(dir.path())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn fixture(script: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.name", "eventlog test"]);
    git(&dir, &["config", "user.email", "test@example.invalid"]);
    fs::write(dir.path().join("target.txt"), "before\n").unwrap();
    fs::write(dir.path().join("unrelated.txt"), "before\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "baseline"]);
    fs::write(dir.path().join("target.txt"), "after\n").unwrap();
    fs::write(dir.path().join("unrelated.txt"), "staged\n").unwrap();
    git(&dir, &["add", "unrelated.txt"]);
    fs::create_dir(dir.path().join(".context")).unwrap();
    fs::write(dir.path().join(".context/eventlog-setup.toml"), format!(
        "[commit]\nmodel = \"configured-model\"\ncommand = [\"sh\", \"-c\", {}, \"stub\", \"{{model}}\"]\n",
        serde_json::to_string(script).unwrap()
    )).unwrap();
    dir
}

#[test]
fn configured_commit_runs_model_and_reports_commit_despite_late_failure() {
    let dir = fixture(
        "test \"$EVENTLOG_MODEL\" = configured-model && test \"$1\" = configured-model && git commit -qm 'feat: model-authored change' && exit 7",
    );
    let staged = git(&dir, &["diff", "--cached", "--binary"]);
    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let outcome = String::from_utf8(output).unwrap();
    assert_eq!(
        git(&dir, &["log", "-1", "--format=%s"]),
        "feat: model-authored change"
    );
    assert!(outcome.contains("outcome=committed"), "{outcome}");
    assert!(
        outcome.contains(&format!("ref={}", git(&dir, &["rev-parse", "HEAD"]))),
        "{outcome}"
    );
    assert!(outcome.contains("exit 7"), "{outcome}");
    assert_eq!(
        git(&dir, &["show", "--format=", "--name-only", "HEAD"]),
        "target.txt"
    );
    assert_eq!(git(&dir, &["diff", "--cached", "--binary"]), staged);
    assert_eq!(
        git(&dir, &["status", "--porcelain", "--", "target.txt"]),
        ""
    );
}

#[test]
fn configured_commit_cannot_publish_out_of_scope_changes() {
    let dir =
        fixture("printf unexpected > outside.txt; git add outside.txt; git commit -qm 'bad scope'");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    let staged = git(&dir, &["diff", "--cached", "--binary"]);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .failure();
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&dir, &["diff", "--cached", "--binary"]), staged);
    assert!(!dir.path().join("outside.txt").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("target.txt")).unwrap(),
        "after\n"
    );
}

#[test]
fn configured_command_failure_without_commit_preserves_source() {
    let dir = fixture("exit 9");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    let staged = git(&dir, &["diff", "--cached", "--binary"]);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .failure();
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&dir, &["diff", "--cached", "--binary"]), staged);
}

#[test]
fn configured_relative_command_is_overlaid_before_bootstrap() {
    let dir = fixture("unused");
    fs::write(
        dir.path().join(".context/model.sh"),
        "test \"$EVENTLOG_MODEL\" = configured-model && git commit -qm 'feat: bootstrap adapter'\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        "[commit]\nmodel = \"configured-model\"\ncommand = [\"sh\", \".context/model.sh\"]\n",
    )
    .unwrap();
    let staged = git(&dir, &["diff", "--cached", "--binary"]);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env(
            "EVENTLOG_PATHS",
            "target.txt,.context/model.sh,.context/eventlog-setup.toml",
        )
        .args(["action", "commit"])
        .assert()
        .success();
    assert_eq!(
        git(&dir, &["log", "-1", "--format=%s"]),
        "feat: bootstrap adapter"
    );
    assert_eq!(git(&dir, &["diff", "--cached", "--binary"]), staged);
    assert_eq!(git(&dir, &["status", "--porcelain", "--", ".context"]), "");
}

#[test]
fn configured_commit_rejects_outside_changes_even_if_later_reverted() {
    let dir = fixture(
        "git commit -qm target; printf bad > outside.txt; git add outside.txt; git commit -qm outside; git rm -q outside.txt; git commit -qm revert",
    );
    let head = git(&dir, &["rev-parse", "HEAD"]);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .failure()
        .stdout(predicates::str::contains("unauthorized path"));
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]), head);
}

#[test]
fn configured_commit_preserves_concurrent_source_edits() {
    let dir =
        fixture("printf concurrent > \"$SOURCE_TEST_ROOT/target.txt\"; git commit -qm target");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("SOURCE_TEST_ROOT", dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .failure()
        .stdout(predicates::str::contains("source path changed"));
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read_to_string(dir.path().join("target.txt")).unwrap(),
        "concurrent"
    );
}

#[test]
fn configured_commit_reports_all_commits_and_syncs_authorized_content() {
    let dir = fixture(
        "git commit -qm first; printf formatted > target.txt; git add target.txt; git commit -qm second",
    );
    let staged = git(&dir, &["diff", "--cached", "--binary"]);
    let result = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let refs = git(&dir, &["rev-list", "--reverse", "HEAD~2..HEAD"]).replace('\n', ",");
    assert!(
        String::from_utf8(result)
            .unwrap()
            .contains(&format!("ref={refs}"))
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("target.txt")).unwrap(),
        "formatted"
    );
    assert_eq!(git(&dir, &["diff", "--cached", "--binary"]), staged);
    assert_eq!(
        git(&dir, &["status", "--porcelain", "--", "target.txt"]),
        ""
    );
}

#[test]
fn clean_authorized_scope_skips_model_execution() {
    let dir = fixture("exit 9");
    fs::write(dir.path().join("target.txt"), "before\n").unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .success()
        .stdout(predicates::str::contains("outcome=skipped"));
}

#[test]
fn configured_commit_can_create_the_first_commit() {
    let dir = TempDir::new().unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.name", "eventlog test"]);
    git(&dir, &["config", "user.email", "test@example.invalid"]);
    fs::create_dir(dir.path().join(".context")).unwrap();
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        "[commit]\ncommand = [\"git\", \"commit\", \"-qm\", \"first commit\"]\n",
    )
    .unwrap();
    fs::write(dir.path().join("target.txt"), "first\n").unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .success();
    assert_eq!(git(&dir, &["log", "-1", "--format=%s"]), "first commit");
}

#[test]
fn configured_commit_inherits_the_driving_event_on_stdin() {
    let dir = fixture("test \"$(cat)\" = '{\"seq\":42}' && git commit -qm 'from stdin'");
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .write_stdin("{\"seq\":42}")
        .args(["action", "commit"])
        .assert()
        .success();
    assert_eq!(git(&dir, &["log", "-1", "--format=%s"]), "from stdin");
}

#[test]
fn configured_commit_syncs_deletions_without_consuming_other_staging() {
    let dir = fixture("git commit -qm deletion");
    fs::remove_file(dir.path().join("target.txt")).unwrap();
    let staged = git(&dir, &["diff", "--cached", "--binary"]);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .success();
    assert_eq!(git(&dir, &["diff", "--cached", "--binary"]), staged);
    assert!(!dir.path().join("target.txt").exists());
    assert_eq!(
        git(&dir, &["status", "--porcelain", "--", "target.txt"]),
        ""
    );
}

#[test]
fn configured_commit_uses_repo_relative_paths_from_a_subdirectory() {
    let dir = fixture("git commit -qm 'feat: root policy from subdirectory'");
    fs::create_dir(dir.path().join("nested")).unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path().join("nested"))
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit"])
        .assert()
        .success()
        .stdout(predicates::str::contains("outcome=committed"));
    assert_eq!(
        git(&dir, &["log", "-1", "--format=%s"]),
        "feat: root policy from subdirectory"
    );
    assert_eq!(
        git(&dir, &["diff", "--cached", "--name-only"]),
        "unrelated.txt"
    );
}
