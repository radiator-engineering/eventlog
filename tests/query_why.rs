//! `why`: causes, effects and verdict for one event. Task 10.

use eventlog::log::Log;
use eventlog::model::config::Config;
use eventlog::model::event::Event;
use eventlog::query::why::why;

const FIXTURE: &str = "tests/fixtures/self-log-2026-09-06.jsonl";

fn self_log() -> Vec<Event> {
    let report = Log::open(FIXTURE).read().expect("fixture reads");
    assert!(
        report.malformed.is_empty(),
        "fixture has malformed lines: {:?}",
        report.malformed
    );
    report.events
}

#[test]
fn why_39_is_acked_by_the_committer() {
    let events = self_log();
    let cfg = Config::default();
    let report = why(&events, &cfg, 39).expect("seq 39 exists");
    assert_eq!(report.event.seq, 39);
    assert!(
        report.effects.iter().any(|e| e.seq == 42),
        "effects should include the committer ack at seq 42: {:?}",
        report.effects.iter().map(|e| e.seq).collect::<Vec<_>>()
    );
    assert!(
        report.verdict.contains("outcome=committed"),
        "verdict: {}",
        report.verdict
    );
}

#[test]
fn why_33_has_no_ack() {
    let events = self_log();
    let cfg = Config::default();
    let report = why(&events, &cfg, 33).expect("seq 33 exists");
    assert_eq!(report.event.seq, 33);
    assert!(
        report.verdict.contains("no ack"),
        "verdict: {}",
        report.verdict
    );
}

#[test]
fn why_returns_none_for_a_missing_seq() {
    let events = self_log();
    assert!(why(&events, &Config::default(), 999_999).is_none());
}
