use eventlog::log::Log;

#[test]
fn hash_line_ignores_trailing_crlf() {
    assert_eq!(Log::hash_line(b"abc\r\n"), Log::hash_line(b"abc"));
    assert_eq!(Log::hash_line(b"abc\n"), Log::hash_line(b"abc"));
}

#[test]
fn tail_reads_last_seq_of_fixture() {
    let log = Log::open("tests/fixtures/drove-events.jsonl");
    let tail = log.tail().unwrap();
    assert_eq!(tail.last_seq, 287);
    assert!(!tail.chained);
    assert!(tail.torn.is_none());
    assert!(tail.last_line.is_some());
}

#[test]
fn read_parses_the_whole_fixture_with_no_malformed_lines() {
    let log = Log::open("tests/fixtures/drove-events.jsonl");
    let report = log.read().unwrap();
    assert_eq!(report.events.len(), 287);
    assert!(report.malformed.is_empty());
    assert_eq!(report.events.last().unwrap().seq, 287);
}

#[test]
fn tail_reports_a_torn_last_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("torn.jsonl");
    std::fs::write(
        &path,
        "{\"seq\":1,\"ts\":\"2026-09-06T00:00:00Z\",\"type\":\"note\"}\n\
         {\"seq\":2,\"ts\":\"2026-09-06T00:00:01Z\",\"type\":\"note\"}\n\
         {\"seq\":5,\"ts\":\"...",
    )
    .unwrap();

    let log = Log::open(&path);
    let tail = log.tail().unwrap();
    assert_eq!(tail.torn, Some(3));
    assert_eq!(tail.last_seq, 0);
}

#[test]
fn read_reports_malformed_lines_by_number() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("malformed.jsonl");
    std::fs::write(
        &path,
        "{\"seq\":1,\"ts\":\"2026-09-06T00:00:00Z\",\"type\":\"note\"}\n\
         not json\n\
         {\"seq\":3,\"ts\":\"2026-09-06T00:00:02Z\",\"type\":\"note\"}\n",
    )
    .unwrap();

    let log = Log::open(&path);
    let report = log.read().unwrap();
    assert_eq!(report.events.len(), 2);
    assert_eq!(report.malformed.len(), 1);
    assert_eq!(report.malformed[0].0, 2);
}
