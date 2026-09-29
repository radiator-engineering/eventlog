use assert_cmd::Command;
use eventlog::model::config;

fn write_cfg(body: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(dir.path().join(".context/eventlog.toml"), body).unwrap();
    dir
}

#[test]
fn context_defaults() {
    let dir = write_cfg("");
    let cfg = config::load(dir.path()).unwrap();
    assert_eq!(cfg.context.floor_percent, 25);
    assert_eq!(cfg.context.backstop_percent, 60);
    assert_eq!(cfg.context.keep_turns, 3);
    assert_eq!(cfg.context.tail_chars, 40_000);
    assert_eq!(cfg.context.budget_chars, 12_000);
}

#[test]
fn context_table_overrides() {
    let dir = write_cfg("[context]\nfloor_percent = 10\nbudget_chars = 500\n");
    let cfg = config::load(dir.path()).unwrap();
    assert_eq!(cfg.context.floor_percent, 10);
    assert_eq!(cfg.context.budget_chars, 500);
    assert_eq!(cfg.context.backstop_percent, 60);
}

#[test]
fn context_floor_at_or_above_backstop_still_loads() {
    // A bad [context] table must not break config load: every other command
    // (react's reactors included) has to keep working. Only `validate`,
    // which `eventlog context` and `eventlog context check` call, rejects it.
    let dir = write_cfg("[context]\nfloor_percent = 70\nbackstop_percent = 60\n");
    let cfg = config::load(dir.path()).unwrap();
    assert_eq!(cfg.context.floor_percent, 70);
    assert_eq!(cfg.context.backstop_percent, 60);
    let err = cfg.context.validate().unwrap_err().to_string();
    assert!(err.contains("floor_percent"), "{err}");
}

#[test]
fn context_validate_rejects_percents_above_100() {
    let dir = write_cfg("[context]\nfloor_percent = 101\n");
    let cfg = config::load(dir.path()).unwrap();
    let err = cfg.context.validate().unwrap_err().to_string();
    assert!(err.contains("0-100"), "{err}");

    let dir = write_cfg("[context]\nbackstop_percent = 150\n");
    let cfg = config::load(dir.path()).unwrap();
    let err = cfg.context.validate().unwrap_err().to_string();
    assert!(err.contains("0-100"), "{err}");
}

#[test]
fn context_unknown_key_is_an_error() {
    let dir = write_cfg("[context]\nkeep_turn = 3\n");
    assert!(config::load(dir.path()).is_err());
}

#[test]
fn eventlog_context_and_check_refuse_a_bad_context_table() {
    let dir = write_cfg("[context]\nfloor_percent = 70\nbackstop_percent = 60\n");
    std::fs::write(dir.path().join(".context/events.jsonl"), "").unwrap();

    Command::cargo_bin("eventlog")
        .unwrap()
        .arg("context")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("floor_percent"));

    Command::cargo_bin("eventlog")
        .unwrap()
        .args(["context", "check", "--percent", "30"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("floor_percent"));
}

#[test]
fn a_bad_context_table_does_not_break_other_commands() {
    let dir = write_cfg("[context]\nfloor_percent = 70\nbackstop_percent = 60\n");
    std::fs::write(dir.path().join(".context/events.jsonl"), "").unwrap();

    Command::cargo_bin("eventlog")
        .unwrap()
        .arg("state")
        .current_dir(dir.path())
        .assert()
        .success();
}
