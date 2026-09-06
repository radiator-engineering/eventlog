use eventlog::model::allow::Allowlist;
use eventlog::model::config;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn builtin_lets_only_controller_spawn() {
    let a = Allowlist::builtin();
    assert!(a.permits("controller", "spawn"));
    assert!(!a.permits("doc-worker", "spawn"));
    assert!(a.permits("doc-worker", "ack"));
}

#[test]
fn decision_replaces_in_full_but_keeps_controller_core() {
    let mut a = Allowlist::builtin();
    a.apply_decision("survey-x:progress|result");
    assert!(a.permits("survey-x", "result"));
    assert!(!a.permits("doc-worker", "ack"));
    assert!(a.permits("controller", "decision"));
}

#[test]
fn controller_plus_reactors_keeps_the_builtin() {
    let mut a = Allowlist::builtin();
    a.apply_decision("controller-plus-reactors");
    assert!(a.permits("doc-worker", "ack"));
    assert!(!a.permits("doc-worker", "spawn"));
    assert!(a.permits("controller", "approval"));
}

#[test]
fn file_writers_override_one_type_only() {
    let mut a = Allowlist::builtin();
    let mut extra = BTreeMap::new();
    extra.insert("ack".to_string(), vec!["cursor-committer".to_string()]);
    a.merge_file(extra);
    assert!(a.permits("cursor-committer", "ack"));
    assert!(!a.permits("doc-worker", "ack"));
    assert!(a.permits("controller", "spawn"));
}

#[test]
fn config_loads_the_fixture_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/eventlog.toml"),
        dir.path().join(".context/eventlog.toml"),
    )
    .unwrap();

    let cfg = config::load(dir.path()).unwrap();
    assert_eq!(cfg.log.path, PathBuf::from(".context/events.jsonl"));
    assert!(cfg.log.fsync);
    assert_eq!(
        cfg.log.named.get("review"),
        Some(&PathBuf::from(".context/review.jsonl"))
    );

    // the file adds a type and adds a field to a built-in one
    assert!(cfg.vocabulary.get("survey").is_some());
    let result = cfg.vocabulary.get("result").unwrap();
    assert!(result.optional.contains(&"pr".to_string()));
    assert!(result.optional.contains(&"summary".to_string()));
    assert!(
        cfg.vocabulary.get("spawn").is_some(),
        "built-in types survive"
    );

    assert!(cfg.writers.permits("survey-x", "survey"));
    assert!(cfg.writers.permits("controller", "spawn"));
    assert!(!cfg.writers.permits("someone-else", "ack"));

    assert_eq!(cfg.view.columns[0], "seq");
    assert_eq!(
        cfg.view.colors.get("result").map(String::as_str),
        Some("1;36")
    );
    assert_eq!(cfg.keys.follow, "f");

    assert_eq!(
        config::resolve_log(&cfg, None),
        PathBuf::from(".context/events.jsonl")
    );
    assert_eq!(
        config::resolve_log(&cfg, Some("review")),
        PathBuf::from(".context/review.jsonl")
    );
    assert_eq!(
        config::resolve_log(&cfg, Some("other/log.jsonl")),
        PathBuf::from("other/log.jsonl")
    );
}

#[test]
fn vocabulary_builtin_covers_the_documented_types() {
    let v = eventlog::model::vocab::Vocabulary::builtin();
    for ty in [
        "spawn",
        "prompt",
        "message",
        "drain",
        "result",
        "decision",
        "escalate",
        "approval",
        "retire",
        "claim",
        "progress",
        "seam",
        "violation",
        "ack",
        "note",
        "intent",
        "veto",
    ] {
        assert!(v.get(ty).is_some(), "missing built-in type {ty}");
    }
    assert!(v.types().contains(&"ack"));
    assert_eq!(
        eventlog::model::vocab::REFERENCE_FIELDS,
        &["seq_done", "for", "for_ack", "intent"]
    );
}

#[test]
fn controller_may_write_every_type_including_result() {
    let a = Allowlist::builtin();
    assert!(a.permits("controller", "result"));
    assert!(a.permits("controller", "note"));
    assert!(a.permits("controller", "custom-type"));
}

#[test]
fn decision_value_without_clauses_changes_nothing() {
    let mut a = Allowlist::builtin();
    a.apply_decision("controller-plus-reactors-plus-briefed-workers");
    assert_eq!(a, Allowlist::builtin());
    assert!(a.permits("doc-worker", "ack"));
}
