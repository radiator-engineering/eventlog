//! The rule voter: the authorized set, the veto rules, and the veto window.
//! Spec section 7, steps 4.1, 4.3 and 4.4.

use eventlog::model::config::Config;
use eventlog::model::event::Event;
use eventlog::model::paths::validate_paths;
use eventlog::query::{self, State};
use eventlog::react::voter::{self, Veto};

fn synth(lines: &[&str]) -> Vec<Event> {
    lines
        .iter()
        .map(|l| Event::parse_line(l).expect("synthetic line parses"))
        .collect()
}

fn state_of(lines: &[&str]) -> State {
    query::fold(&synth(lines), &Config::default())
}

fn event(line: &str) -> Event {
    Event::parse_line(line).expect("synthetic line parses")
}

fn names(paths: &[eventlog::model::paths::RelPath]) -> Vec<&str> {
    paths.iter().map(|p| p.as_str()).collect()
}

/// A live doc-worker with its doc roots claimed, plus one other open agent.
const WORLD: &[&str] = &[
    r#"{"seq":1,"ts":"2026-09-06T10:00:00Z","type":"spawn","agent":"doc-worker","role":"reactor"}"#,
    r#"{"seq":2,"ts":"2026-09-06T10:00:01Z","type":"claim","agent":"doc-worker","paths":"docs,README.md,AGENTS.md"}"#,
    r#"{"seq":3,"ts":"2026-09-06T10:00:02Z","type":"spawn","agent":"build-x","role":"build"}"#,
    r#"{"seq":4,"ts":"2026-09-06T10:00:03Z","type":"claim","agent":"build-x","paths":"src/x.rs"}"#,
];

// --- 4.1: the authorized set -------------------------------------------------

#[test]
fn a_controller_event_authorizes_all_its_paths() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":"src/a.rs","ref":"src/a.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(names(&auth.paths), ["src/a.rs"]);
    assert!(auth.excess.is_empty(), "controller paths are never excess");
}

#[test]
fn a_writer_is_held_to_its_claims() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","by":"doc-worker","paths":"docs,README.md,src/x.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(names(&auth.paths), ["docs", "README.md"]);
    assert_eq!(names(&auth.excess), ["src/x.rs"]);
}

#[test]
fn a_claim_covers_files_under_a_claimed_directory() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","by":"doc-worker","paths":"docs/reference/append.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(names(&auth.paths), ["docs/reference/append.md"]);
    assert!(auth.excess.is_empty());
}

#[test]
fn a_writer_with_no_claims_is_authorized_for_nothing() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","by":"stranger","paths":"src/a.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert!(auth.paths.is_empty());
    assert_eq!(names(&auth.excess), ["src/a.rs"]);
}

// --- 4.3: the rules ----------------------------------------------------------

#[test]
fn excess_paths_are_a_veto() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","by":"doc-worker","paths":"docs,README.md,src/x.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    match voter::check("doc-worker", &auth, &state, &Config::default()) {
        Err(Veto::UnclaimedPaths(paths)) => assert_eq!(names(&paths), ["src/x.rs"]),
        other => panic!("expected unclaimed-paths, got {other:?}"),
    }
}

#[test]
fn the_log_itself_is_a_veto() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":".context/events.jsonl"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    match voter::check("commit", &auth, &state, &Config::default()) {
        Err(Veto::LogOrLock(path)) => assert_eq!(path.as_str(), ".context/events.jsonl"),
        other => panic!("expected log-or-lock, got {other:?}"),
    }
}

#[test]
fn a_reactor_lock_dir_is_a_veto() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":".context/events.jsonl.commit.reactor.lock"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert!(matches!(
        voter::check("commit", &auth, &state, &Config::default()),
        Err(Veto::LogOrLock(_))
    ));
}

#[test]
fn other_tracked_files_under_context_pass() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":".context/DECISIONS.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(
        voter::check("commit", &auth, &state, &Config::default()),
        Ok(())
    );
}

#[test]
fn a_path_claimed_by_another_open_agent_is_a_veto() {
    let state = state_of(WORLD);
    let driving =
        event(r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":"src/x.rs"}"#);
    let auth = voter::authorize(&driving, &state);
    match voter::check("commit", &auth, &state, &Config::default()) {
        Err(Veto::ClaimedByOther { path, owner }) => {
            assert_eq!(path.as_str(), "src/x.rs");
            assert_eq!(owner, "build-x");
        }
        other => panic!("expected claimed-by-other, got {other:?}"),
    }
}

#[test]
fn a_reactors_own_claim_is_not_another_agents() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","by":"doc-worker","paths":"docs,README.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(
        voter::check("doc-worker", &auth, &state, &Config::default()),
        Ok(())
    );
}

#[test]
fn a_retired_agents_claim_is_released() {
    let mut lines = WORLD.to_vec();
    lines.push(
        r#"{"seq":5,"ts":"2026-09-06T10:00:04Z","type":"retire","agent":"build-x","disposition":"accepted"}"#,
    );
    let state = state_of(&lines);
    let driving =
        event(r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":"src/x.rs"}"#);
    let auth = voter::authorize(&driving, &state);
    assert_eq!(
        voter::check("commit", &auth, &state, &Config::default()),
        Ok(())
    );
}

#[test]
fn an_open_escalation_naming_the_reactor_is_a_veto() {
    let mut lines = WORLD.to_vec();
    lines.push(
        r#"{"seq":5,"ts":"2026-09-06T10:00:04Z","type":"escalate","agent":"commit","msg":"HEAD did not move"}"#,
    );
    let state = state_of(&lines);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":".context/DECISIONS.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    match voter::check("commit", &auth, &state, &Config::default()) {
        Err(Veto::OpenEscalation(seq)) => assert_eq!(seq, 5),
        other => panic!("expected open-escalation, got {other:?}"),
    }
}

#[test]
fn an_approved_escalation_no_longer_vetoes() {
    let mut lines = WORLD.to_vec();
    lines.push(
        r#"{"seq":5,"ts":"2026-09-06T10:00:04Z","type":"escalate","agent":"commit","subject":"commit","msg":"HEAD did not move"}"#,
    );
    lines.push(
        r#"{"seq":6,"ts":"2026-09-06T10:00:05Z","type":"approval","subject":"commit","msg":"go on"}"#,
    );
    let state = state_of(&lines);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":".context/DECISIONS.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(
        voter::check("commit", &auth, &state, &Config::default()),
        Ok(())
    );
}

#[test]
fn an_escalation_about_another_agent_does_not_veto() {
    let mut lines = WORLD.to_vec();
    lines.push(
        r#"{"seq":5,"ts":"2026-09-06T10:00:04Z","type":"escalate","agent":"build-x","msg":"needs a dep"}"#,
    );
    let state = state_of(&lines);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","paths":".context/DECISIONS.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(
        voter::check("commit", &auth, &state, &Config::default()),
        Ok(())
    );
}

#[test]
fn unclaimed_paths_outrank_the_other_rules() {
    let state = state_of(WORLD);
    // Both an excess path and the log itself; the excess is reported.
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","by":"doc-worker","paths":"docs,src/x.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert!(matches!(
        voter::check("doc-worker", &auth, &state, &Config::default()),
        Err(Veto::UnclaimedPaths(_))
    ));
}

#[test]
fn every_veto_has_a_reason_word() {
    let path = validate_paths("src/a.rs").expect("a valid path").remove(0);
    let owner = "build-x".to_string();
    let reasons = [
        Veto::UnclaimedPaths(Vec::new()).reason(),
        Veto::LogOrLock(path.clone()).reason(),
        Veto::ClaimedByOther { path, owner }.reason(),
        Veto::OpenEscalation(1).reason(),
    ];
    assert_eq!(
        reasons,
        [
            "unclaimed-paths",
            "log-or-lock",
            "claimed-by-other",
            "open-escalation"
        ]
    );
}

// --- 4.4: the veto window ----------------------------------------------------

#[test]
fn a_veto_naming_the_driving_seq_binds_whatever_intent_it_saw() {
    let events = synth(&[
        r#"{"seq":10,"ts":"2026-09-06T10:02:00Z","type":"intent","by":"commit","for":"9","action":"commit"}"#,
        r#"{"seq":11,"ts":"2026-09-06T10:02:01Z","type":"veto","by":"human","role":"voter","for":"9","intent":"7","reason":"hold"}"#,
    ]);
    let found = voter::veto_binds(&events, 9, 10).expect("the veto binds");
    assert_eq!(found.seq, 11);
}

#[test]
fn a_veto_for_another_event_does_not_bind() {
    let events = synth(&[
        r#"{"seq":11,"ts":"2026-09-06T10:02:01Z","type":"veto","by":"human","for":"8","reason":"hold"}"#,
    ]);
    assert!(voter::veto_binds(&events, 9, 10).is_none());
}

#[test]
fn a_veto_from_before_the_window_does_not_bind() {
    let events = synth(&[
        r#"{"seq":5,"ts":"2026-09-06T10:00:04Z","type":"veto","by":"human","for":"9","reason":"stale"}"#,
    ]);
    assert!(voter::veto_binds(&events, 9, 10).is_none());
}

#[test]
fn the_first_binding_veto_wins() {
    let events = synth(&[
        r#"{"seq":11,"ts":"2026-09-06T10:02:01Z","type":"veto","by":"human","for":"9","reason":"first"}"#,
        r#"{"seq":12,"ts":"2026-09-06T10:02:02Z","type":"veto","by":"other","for":"9","reason":"second"}"#,
    ]);
    let found = voter::veto_binds(&events, 9, 10).expect("a veto binds");
    assert_eq!(
        found.fields.get("reason").map(String::as_str),
        Some("first")
    );
}

#[test]
fn a_workers_own_result_on_its_claimed_paths_is_not_claimed_by_other() {
    let state = state_of(WORLD);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","by":"build-x","agent":"build-x","paths":"src/x.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(auth.subject, "build-x");
    assert_eq!(
        voter::check("commit", &auth, &state, &Config::default()),
        Ok(())
    );
    // The controller recording that worker's result is the same case.
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","agent":"build-x","paths":"src/x.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert_eq!(
        voter::check("commit", &auth, &state, &Config::default()),
        Ok(())
    );
}

// --- 4.3: the controller may cross an idle reactor's claim ---------------------

/// WORLD plus a doc-worker that has acked once, so the fold knows it as a
/// reactor. `extra` lines follow.
fn world_with_doc_reactor(extra: &[&str]) -> State {
    let mut lines = WORLD.to_vec();
    lines.push(
        r#"{"seq":5,"ts":"2026-09-06T10:00:05Z","type":"ack","by":"doc-worker","seq_done":"4","outcome":"skipped"}"#,
    );
    lines.extend_from_slice(extra);
    state_of(&lines)
}

/// The controller grants every claim, and the docs are not in any worker's
/// worktree: a controller `result` naming README.md must not be stopped by
/// the doc worker's claim while the doc worker is idle.
#[test]
fn a_controller_event_passes_over_an_idle_reactors_claim() {
    let state = world_with_doc_reactor(&[]);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","agent":"controller","paths":"README.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert!(auth.from_controller);
    assert_eq!(
        voter::check("commit", &auth, &state, &Config::default()),
        Ok(())
    );
}

/// While the doc worker has an intent open, a pass is in flight and a commit
/// could sweep up its half-written files: the claim binds again.
#[test]
fn a_reactor_with_an_open_intent_still_holds_its_claim_against_the_controller() {
    let state = world_with_doc_reactor(&[
        r#"{"seq":6,"ts":"2026-09-06T10:00:06Z","type":"intent","by":"doc-worker","for":"5","action":"doc","paths":"docs"}"#,
    ]);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","agent":"controller","paths":"README.md"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    match voter::check("commit", &auth, &state, &Config::default()) {
        Err(Veto::ClaimedByOther { path, owner }) => {
            assert_eq!(path.as_str(), "README.md");
            assert_eq!(owner, "doc-worker");
        }
        other => panic!("expected claimed-by-other, got {other:?}"),
    }
}

/// A worker is not a reactor: its claim binds the controller's own events
/// whether or not anything is in flight. Its files reach the log through
/// `result agent=<worker>`, which is the subject exemption, not this one.
#[test]
fn a_workers_claim_still_binds_a_controller_event() {
    let state = world_with_doc_reactor(&[]);
    let driving = event(
        r#"{"seq":9,"ts":"2026-09-06T10:01:00Z","type":"result","agent":"controller","paths":"src/x.rs"}"#,
    );
    let auth = voter::authorize(&driving, &state);
    assert!(matches!(
        voter::check("commit", &auth, &state, &Config::default()),
        Err(Veto::ClaimedByOther { .. })
    ));
}
