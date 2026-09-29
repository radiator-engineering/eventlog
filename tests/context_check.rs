use eventlog::context::check::decide;
use eventlog::context::worktree::{Change, WorkTree};
use eventlog::log::Log;
use eventlog::log::append::{AppendRequest, append};
use eventlog::model::config::{Config, ContextConfig};
use eventlog::model::event::Event;
use eventlog::query;

fn ev(l: &str) -> Event {
    Event::parse_line(l).unwrap()
}
const RESULT: &str = r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"result","agent":"controller","ref":"a","summary":"s"}"#;
const RESULT_TS: u64 = 1_788_220_800; // 2026-09-01T00:00:00Z

fn run(percent: u8, events: &[Event], work: &WorkTree) -> (bool, &'static str) {
    let state = query::fold(events, &Config::default());
    let v = decide(
        percent,
        0,
        &ContextConfig::default(),
        events,
        &state,
        || work.clone(),
    );
    (v.rebuild, v.reason)
}
fn clean() -> WorkTree {
    WorkTree {
        available: true,
        changes: vec![],
    }
}
fn change(mtime: u64, owner: Option<&str>) -> WorkTree {
    WorkTree {
        available: true,
        changes: vec![Change {
            path: "f".into(),
            status: "M".into(),
            stat: String::new(),
            mtime,
            owner: owner.map(Into::into),
        }],
    }
}

#[test]
fn below_floor_never_rebuilds() {
    assert_eq!(run(24, &[ev(RESULT)], &clean()), (false, "below-floor"));
}

#[test]
fn below_floor_and_backstop_never_call_the_collector() {
    // The collector (git status/diff) is not free; `decide` must not run it
    // when the floor or backstop rule already answers the verdict.
    let events = [ev(RESULT)];
    let state = query::fold(&events, &Config::default());
    let panics = || -> WorkTree { panic!("the collector ran but the verdict did not need it") };

    let below = decide(24, 0, &ContextConfig::default(), &events, &state, panics);
    assert_eq!((below.rebuild, below.reason), (false, "below-floor"));

    let backstop = decide(80, 0, &ContextConfig::default(), &events, &state, panics);
    assert_eq!((backstop.rebuild, backstop.reason), (true, "backstop"));
}
#[test]
fn backstop_rebuilds_mid_task() {
    let events = [
        ev(RESULT),
        ev(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"intent","msg":"m"}"#),
    ];
    assert_eq!(run(60, &events, &clean()), (true, "backstop"));
}
#[test]
fn boundary_after_result() {
    assert_eq!(run(30, &[ev(RESULT)], &clean()), (true, "boundary"));
}
#[test]
fn no_result_is_mid_task() {
    assert_eq!(run(30, &[], &clean()), (false, "mid-task"));
}
#[test]
fn boundary_needs_result_after_last_rebuild() {
    let events = [
        ev(RESULT),
        ev(
            r#"{"seq":2,"ts":"2026-09-01T00:00:02Z","type":"rebuild","trigger":"auto","reason":"engine-fallback"}"#,
        ),
    ];
    assert_eq!(run(30, &events, &clean()), (false, "mid-task"));
}
#[test]
fn open_controller_intent_blocks_boundary() {
    let events = [
        ev(RESULT),
        ev(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"intent","msg":"m"}"#),
    ];
    assert_eq!(run(30, &events, &clean()), (false, "mid-task"));
}
#[test]
fn closed_controller_intent_allows_boundary() {
    let events = [
        ev(r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"intent","msg":"m"}"#),
        ev(
            r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"ack","seq_done":"1","outcome":"done","for":"1"}"#,
        ),
        ev(
            r#"{"seq":3,"ts":"2026-09-01T00:00:02Z","type":"result","agent":"controller","ref":"a"}"#,
        ),
    ];
    assert_eq!(run(30, &events, &clean()), (true, "boundary"));
}
#[test]
fn reactor_intent_does_not_block_boundary() {
    let events = [
        ev(RESULT),
        ev(
            r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"intent","by":"cursor-committer","for":"1"}"#,
        ),
    ];
    assert_eq!(run(30, &events, &clean()), (true, "boundary"));
}
#[test]
fn unreported_change_blocks_boundary() {
    assert_eq!(
        run(30, &[ev(RESULT)], &change(RESULT_TS + 60, None)),
        (false, "mid-task")
    );
}
#[test]
fn change_older_than_result_is_reported() {
    assert_eq!(
        run(30, &[ev(RESULT)], &change(RESULT_TS - 60, None)),
        (true, "boundary")
    );
}
#[test]
fn change_under_other_claim_is_not_ours() {
    assert_eq!(
        run(30, &[ev(RESULT)], &change(RESULT_TS + 60, Some("w1"))),
        (true, "boundary")
    );
}
#[test]
fn unavailable_tree_counts_as_clean() {
    assert_eq!(
        run(
            30,
            &[ev(RESULT)],
            &WorkTree {
                available: false,
                changes: vec![]
            }
        ),
        (true, "boundary")
    );
}
#[test]
fn result_by_a_reactor_is_not_a_controller_result() {
    let events = [ev(
        r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"result","by":"doc-worker","agent":"doc-worker","ref":"a"}"#,
    )];
    assert_eq!(run(30, &events, &clean()), (false, "mid-task"));
}

#[test]
fn growth_pushes_sub_backstop_percent_over() {
    let events = [
        ev(RESULT),
        ev(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"intent","msg":"m"}"#),
    ];
    let state = query::fold(&events, &Config::default());
    let v = decide(50, 15, &ContextConfig::default(), &events, &state, || {
        clean()
    });
    assert_eq!((v.rebuild, v.reason), (true, "backstop"));
}

#[test]
fn growth_does_not_lift_below_floor_into_rebuild() {
    let events = [ev(RESULT)];
    let state = query::fold(&events, &Config::default());
    let v = decide(24, 50, &ContextConfig::default(), &events, &state, || {
        clean()
    });
    assert_eq!((v.rebuild, v.reason), (false, "below-floor"));
}

/// `ack ... for=<seq>` must both pass real `append` validation (the vocabulary
/// must allow `for` on `ack`) and, once appended, close the controller's open
/// `intent` so `decide` finds a task boundary. Exercises the real
/// `eventlog::log::append::append` path, not just `Event::parse_line`, so a
/// vocabulary regression (`for` missing from `ack`'s allowed fields) fails
/// this test even though `Event::parse_line` never enforces the vocabulary.
#[test]
fn ack_for_accepted_by_append_closes_open_intent() {
    let dir = tempfile::tempdir().unwrap();
    let log = Log::open(dir.path().join("events.jsonl"));
    let cfg = Config::default();

    fn req(ty: &str, fields: &[(&str, &str)]) -> AppendRequest {
        AppendRequest {
            r#type: ty.to_string(),
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            writer: "controller".to_string(),
            strict: false,
            dry_run: false,
        }
    }

    let intent = append(&log, &cfg, req("intent", &[("msg", "m")]), None).unwrap();
    let ack = append(
        &log,
        &cfg,
        req(
            "ack",
            &[
                ("seq_done", &intent.seq.to_string()),
                ("outcome", "done"),
                ("for", &intent.seq.to_string()),
            ],
        ),
        None,
    )
    .unwrap();
    assert_eq!(ack.fields.get("for").map(String::as_str), Some("1"));
    append(&log, &cfg, req("result", &[("ref", "a")]), None).unwrap();

    let events = log.read().unwrap().events;
    let state = query::fold(&events, &cfg);
    let v = decide(30, 0, &cfg.context, &events, &state, clean);
    assert_eq!((v.rebuild, v.reason), (true, "boundary"));
}
