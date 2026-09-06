use eventlog::model::event::Event;

#[test]
fn parses_existing_line_with_quoted_seq_done() {
    let e = Event::parse_line(
        r#"{"seq":51,"ts":"2026-09-06T20:58:58Z","type":"ack","by":"cursor-committer","seq_done":"50","outcome":"committed"}"#,
    )
    .unwrap();
    assert_eq!(e.seq, 51);
    assert_eq!(e.seq_ref("seq_done"), Some(50));
    assert_eq!(e.writer(), "cursor-committer");
    assert_eq!(e.subject(), "controller");
}

#[test]
fn to_line_keeps_spec_order() {
    let e = Event::parse_line(
        r#"{"type":"result","seq":3,"agent":"w","ts":"2026-01-01T00:00:00Z","ref":"x"}"#,
    )
    .unwrap();
    assert_eq!(
        e.to_line(),
        r#"{"seq":3,"ts":"2026-01-01T00:00:00Z","type":"result","agent":"w","ref":"x"}"#
    );
}

#[test]
fn every_fixture_line_parses() {
    for (i, l) in include_str!("fixtures/drove-events.jsonl")
        .lines()
        .enumerate()
    {
        Event::parse_line(l).unwrap_or_else(|e| panic!("line {}: {:?}", i + 1, e));
    }
}

#[test]
fn fixture_lines_round_trip_byte_for_byte() {
    for (i, l) in include_str!("fixtures/drove-events.jsonl")
        .lines()
        .enumerate()
    {
        let e = Event::parse_line(l).unwrap();
        assert_eq!(e.to_line(), l, "line {} did not round-trip", i + 1);
    }
}

#[test]
fn to_line_emits_prev_and_by_in_spec_order() {
    let src = r#"{"seq":9,"ts":"2026-01-01T00:00:00Z","type":"ack","prev":"abc","by":"doc-worker","agent":"w","outcome":"ok"}"#;
    let e = Event::parse_line(src).unwrap();
    assert_eq!(e.prev.as_deref(), Some("abc"));
    assert_eq!(e.to_line(), src);
}

#[test]
fn rejects_malformed_lines_without_panicking() {
    for bad in [
        "not json at all",
        "[1,2,3]",
        r#"{"ts":"2026-01-01T00:00:00Z","type":"ack"}"#,
        r#"{"seq":"x","ts":"2026-01-01T00:00:00Z","type":"ack"}"#,
        r#"{"seq":1,"type":"ack"}"#,
        r#"{"seq":1,"ts":"2026-01-01T00:00:00Z"}"#,
        r#"{"seq":1,"ts":"2026-01-01T00:00:00Z","type":"ack","nested":{"a":1}}"#,
    ] {
        let err = Event::parse_line(bad).unwrap_err();
        assert!(!err.reason.is_empty(), "no reason for {bad}");
        assert!(!err.line_hint.is_empty(), "no hint for {bad}");
    }
}

#[test]
fn seq_ref_rejects_non_numeric_and_paths_split_on_comma() {
    let e = Event::parse_line(
        r#"{"seq":2,"ts":"2026-01-01T00:00:00Z","type":"result","agent":"w","paths":"a.md,b/c.rs","seq_done":"nope"}"#,
    )
    .unwrap();
    assert_eq!(e.seq_ref("seq_done"), None);
    assert_eq!(e.seq_ref("absent"), None);
    assert_eq!(e.paths(), vec!["a.md".to_string(), "b/c.rs".to_string()]);
    assert_eq!(e.subject(), "w");
    assert_eq!(e.writer(), "controller");
}

#[test]
fn escapes_are_preserved() {
    let src = r#"{"seq":1,"ts":"2026-01-01T00:00:00Z","type":"note","msg":"a \"quoted\" line\nwith break"}"#;
    let e = Event::parse_line(src).unwrap();
    assert_eq!(e.fields["msg"], "a \"quoted\" line\nwith break");
    assert_eq!(e.to_line(), src);
}
