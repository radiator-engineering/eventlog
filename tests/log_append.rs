use eventlog::log::Log;
use eventlog::log::append::{AppendError, AppendRequest, append};
use eventlog::model::config::Config;
use std::sync::{Arc, Barrier};
use std::thread;

fn req(type_name: &str, fields: Vec<(&str, &str)>) -> AppendRequest {
    AppendRequest {
        r#type: type_name.to_string(),
        fields: fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        writer: "controller".to_string(),
        strict: false,
        dry_run: false,
    }
}

#[test]
fn append_to_empty_log_yields_seq_one_and_genesis_prev() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let log = Log::open(&path);
    let cfg = Config::default();

    let event = append(&log, &cfg, req("note", vec![("msg", "hello")]), None).unwrap();

    assert_eq!(event.seq, 1);
    assert_eq!(event.prev.as_deref(), Some("genesis"));
    assert_eq!(event.r#type, "note");
    assert!(event.by.is_none());

    let tail = log.tail().unwrap();
    assert_eq!(tail.last_seq, 1);
}

#[test]
fn second_append_prev_is_hash_of_first_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let log = Log::open(&path);
    let cfg = Config::default();

    let first = append(&log, &cfg, req("note", vec![("msg", "one")]), None).unwrap();
    let first_line = std::fs::read(&path).unwrap();
    let expected_prev = Log::hash_line(&first_line);

    let second = append(&log, &cfg, req("note", vec![("msg", "two")]), None).unwrap();

    assert_eq!(second.seq, 2);
    assert_eq!(second.prev.as_deref(), Some(expected_prev.as_str()));
    assert_ne!(second.prev.as_deref(), Some("genesis"));
    assert_ne!(first.prev, second.prev);
}

#[test]
fn user_supplied_seq_is_reserved() {
    let dir = tempfile::tempdir().unwrap();
    let log = Log::open(dir.path().join("events.jsonl"));
    let err = append(
        &log,
        &Config::default(),
        req("note", vec![("seq", "9"), ("msg", "nope")]),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, AppendError::Reserved(field) if field == "seq"));
}

#[test]
fn by_mismatch_when_controller_supplies_by() {
    let dir = tempfile::tempdir().unwrap();
    let log = Log::open(dir.path().join("events.jsonl"));
    let err = append(
        &log,
        &Config::default(),
        req("note", vec![("by", "x"), ("msg", "nope")]),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, AppendError::ByMismatch));
}

#[test]
fn doc_worker_cannot_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let log = Log::open(dir.path().join("events.jsonl"));
    let err = append(
        &log,
        &Config::default(),
        AppendRequest {
            r#type: "spawn".to_string(),
            fields: vec![("agent".to_string(), "worker".to_string())],
            writer: "doc-worker".to_string(),
            strict: false,
            dry_run: false,
        },
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        AppendError::NotPermitted { writer, ty } if writer == "doc-worker" && ty == "spawn"
    ));
}

#[test]
fn field_over_two_kb_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let log = Log::open(dir.path().join("events.jsonl"));
    let big = "x".repeat(3000);
    let err = append(
        &log,
        &Config::default(),
        req("note", vec![("msg", &big)]),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, AppendError::FieldTooLarge(field) if field == "msg"));
}

#[test]
fn first_append_on_prechain_fixture_hashes_last_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    std::fs::copy("tests/fixtures/drove-events.jsonl", &path).unwrap();

    let fixture_tail = Log::open(&path).tail().unwrap();
    let last_line = fixture_tail.last_line.as_ref().unwrap();
    let expected_prev = Log::hash_line(last_line);

    let log = Log::open(&path);
    let event = append(
        &log,
        &Config::default(),
        req("note", vec![("msg", "after fixture")]),
        None,
    )
    .unwrap();

    assert_eq!(event.seq, 288);
    assert_eq!(event.prev.as_deref(), Some(expected_prev.as_str()));
}

#[test]
fn torn_tail_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    std::fs::write(
        &path,
        "{\"seq\":1,\"ts\":\"2026-09-06T00:00:00Z\",\"type\":\"note\",\"msg\":\"ok\"}\n\
{\"seq\":2,\"ts\":\"2026-09-06T00:00:01Z\",\"type\":\"note\",\"msg\":\"ok\"}\n\
{\"seq\":5,\"ts\":\"...",
    )
    .unwrap();

    let log = Log::open(&path);
    let err = append(
        &log,
        &Config::default(),
        req("note", vec![("msg", "nope")]),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, AppendError::TornTail(3)));
}

#[test]
fn two_threads_append_one_hundred_lines_without_gaps() {
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let path = dir.path().join("events.jsonl");
    let log_path = Arc::new(path);
    let cfg = Arc::new(Config::default());
    let barrier = Arc::new(Barrier::new(2));

    let mut handles = Vec::new();
    for thread_id in 0..2 {
        let log_path = Arc::clone(&log_path);
        let cfg = Arc::clone(&cfg);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let log = Log::open(log_path.as_path());
            for i in 0..50 {
                append(
                    &log,
                    &cfg,
                    req("note", vec![("msg", &format!("t{thread_id}-{i}"))]),
                    None,
                )
                .unwrap();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let report = Log::open(log_path.as_path()).read().unwrap();
    assert_eq!(report.events.len(), 100);
    assert!(report.malformed.is_empty());
    let seqs: Vec<u64> = report.events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, (1..=100).collect::<Vec<_>>());
}
