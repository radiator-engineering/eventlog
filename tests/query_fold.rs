//! The fold over the log: agents, claims, decisions, reactors and the
//! allowlist as of a seq. Spec section 8.

use eventlog::log::Log;
use eventlog::model::config::Config;
use eventlog::model::event::Event;
use eventlog::query::{self, Phase};
use std::collections::BTreeSet;

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

fn synth(lines: &[&str]) -> Vec<Event> {
    lines
        .iter()
        .map(|l| Event::parse_line(l).expect("synthetic line parses"))
        .collect()
}

#[test]
fn one_agent_per_distinct_spawn() {
    let events = self_log();
    let spawned: BTreeSet<&str> = events
        .iter()
        .filter(|e| e.r#type == "spawn")
        .filter_map(|e| e.agent.as_deref())
        .collect();
    let state = query::fold(&events, &Config::default());
    assert_eq!(state.agents.len(), spawned.len());
    for name in spawned {
        assert!(state.agents.contains_key(name), "missing agent {name}");
    }
}

#[test]
fn a_retired_agent_is_retired() {
    let events = self_log();
    let cfg = Config::default();
    let mut checked = 0;
    for e in events.iter().filter(|e| e.r#type == "retire") {
        let Some(name) = e.agent.as_deref() else {
            continue;
        };
        // As of the retire itself: a later re-spawn starts a new lifecycle,
        // so the tip is the wrong place to ask.
        let state = query::fold_at(&events, &cfg, e.seq);
        let Some(agent) = state.agents.get(name) else {
            continue; // retired with no spawn behind it: an open lifecycle
        };
        assert_eq!(agent.phase, Phase::Retired, "{name} is not retired");
        assert_eq!(agent.retired_at, Some(e.seq), "{name} has no retired_at");
        assert!(
            state.claims_for(name).is_empty(),
            "{name} still holds claims after retiring"
        );
        checked += 1;
    }
    assert!(checked > 0, "fixture has no retired agent to check");
}

#[test]
fn a_respawn_starts_a_new_lifecycle() {
    let events = self_log();
    let cfg = Config::default();
    // cursor-committer is retired at 10 and spawned again at 13.
    let retired = query::fold_at(&events, &cfg, 12);
    assert_eq!(retired.agents["cursor-committer"].phase, Phase::Retired);
    let again = query::fold_at(&events, &cfg, 13);
    assert_eq!(again.agents["cursor-committer"].phase, Phase::Spawned);
    assert_eq!(again.agents["cursor-committer"].spawned_at, 13);
    assert_eq!(again.agents["cursor-committer"].retired_at, None);
}

#[test]
fn fold_at_stops_at_the_seq() {
    let events = self_log();
    let state = query::fold_at(&events, &Config::default(), 100);
    assert_eq!(state.at, 100);
    for e in events.iter().filter(|e| e.r#type == "spawn" && e.seq > 100) {
        let name = e.agent.as_deref().unwrap();
        assert!(
            !state.agents.contains_key(name),
            "{name} was spawned at {} but appears at 100",
            e.seq
        );
    }
    assert!(
        state.agents.contains_key("build-scaffold"),
        "an agent spawned before 100 is missing"
    );
}

#[test]
fn the_allowlist_is_as_of_the_seq() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"x"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"prompt","agent":"x","ref":"b.md"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"decision","key":"log-writers","value":"x:spawn|result"}"#,
    ]);
    let cfg = Config::default();

    let before = query::fold_at(&events, &cfg, 2);
    assert!(!before.allowlist.permits("x", "spawn"));
    assert!(before.allowlist.permits("y", "result"));

    // The decision replaces the map in full: x gains spawn, y loses result.
    let after = query::fold_at(&events, &cfg, 3);
    assert!(after.allowlist.permits("x", "spawn"));
    assert!(after.allowlist.permits("x", "result"));
    assert!(!after.allowlist.permits("y", "result"));
    assert!(after.allowlist.permits("controller", "decision"));

    assert_eq!(after.decisions["log-writers"].1, 3);
    assert!(query::fold(&events, &cfg).allowlist.permits("x", "spawn"));
}

#[test]
fn last_ack_seq_compares_numerically() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"result","ref":"a.md"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"ack","by":"r","seq_done":"9","outcome":"committed"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"ack","by":"r","seq_done":"51","outcome":"committed"}"#,
        r#"{"seq":4,"ts":"2026-09-06T00:00:03Z","type":"ack","by":"r","seq_done":"9","outcome":"skipped"}"#,
    ]);
    let state = query::fold(&events, &Config::default());
    let reactor = state.reactors.get("r").expect("reactor r");
    assert_eq!(reactor.last_ack_seq, Some(51));
    assert_eq!(reactor.last_ack_ts.as_deref(), Some("2026-09-06T00:00:02Z"));
}

#[test]
fn claims_accumulate_and_match_globs() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"w"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"claim","agent":"w","paths":"src/query/**,README.md"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"spawn","agent":"v"}"#,
        r#"{"seq":4,"ts":"2026-09-06T00:00:03Z","type":"claim","agent":"v","paths":"src/log"}"#,
        r#"{"seq":5,"ts":"2026-09-06T00:00:04Z","type":"claim","agent":"w","paths":"tests/query_fold.rs"}"#,
    ]);
    let state = query::fold(&events, &Config::default());

    // A later claim adds to the earlier one; it never narrows it.
    assert_eq!(
        state.claims_for("w"),
        vec!["src/query/**", "README.md", "tests/query_fold.rs"]
    );
    assert_eq!(state.claim_owner("src/query/mod.rs"), Some("w"));
    assert_eq!(state.claim_owner("README.md"), Some("w"));
    assert_eq!(state.claim_owner("tests/query_fold.rs"), Some("w"));
    // A claimed directory covers what is under it.
    assert_eq!(state.claim_owner("src/log/lock.rs"), Some("v"));
    assert_eq!(state.claim_owner("src/model/mod.rs"), None);
}

#[test]
fn retiring_frees_the_claim() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"w"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"claim","agent":"w","paths":"src/query/**"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"retire","agent":"w","disposition":"accepted"}"#,
    ]);
    let cfg = Config::default();
    assert_eq!(
        query::fold_at(&events, &cfg, 2).claim_owner("src/query/mod.rs"),
        Some("w")
    );
    let after = query::fold(&events, &cfg);
    assert_eq!(after.claim_owner("src/query/mod.rs"), None);
    assert!(after.claims.is_empty());
    // The retired agent keeps the claims it held, for the record.
    assert_eq!(after.agents["w"].claims, vec!["src/query/**".to_string()]);
}

#[test]
fn an_approval_closes_the_escalation_it_names() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"escalate","by":"w","subject":"cargo-toml","msg":"needs globset"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"escalate","by":"v","subject":"schema","msg":"needs a dep"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"approval","subject":"cargo-toml","decision":"granted"}"#,
    ]);
    let cfg = Config::default();
    assert_eq!(query::fold_at(&events, &cfg, 2).escalations.len(), 2);
    let open = query::fold(&events, &cfg).escalations;
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].fields["subject"], "schema");
}

#[test]
fn an_ack_closes_only_its_own_writers_intent() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"intent","by":"r","msg":"commit 1"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"intent","by":"s","msg":"document 1"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"ack","by":"r","for":"2","seq_done":"1","outcome":"committed"}"#,
        r#"{"seq":4,"ts":"2026-09-06T00:00:03Z","type":"ack","by":"r","for":"1","seq_done":"1","outcome":"committed"}"#,
    ]);
    let state = query::fold(&events, &Config::default());
    // The ack at 3 names s's intent, so it closes nothing; the ack at 4 does.
    assert_eq!(state.intents.len(), 1);
    assert_eq!(state.intents[0].seq, 2);
    assert_eq!(state.reactors["s"].open_intents, vec![2]);
    assert!(state.reactors["r"].open_intents.is_empty());
}

#[test]
fn unacked_lists_what_a_reactor_has_not_seen() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"result","ref":"a.md"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"ack","by":"r","seq_done":"1","outcome":"committed"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"result","ref":"b.md"}"#,
        r#"{"seq":4,"ts":"2026-09-06T00:00:03Z","type":"note","msg":"hi"}"#,
        r#"{"seq":5,"ts":"2026-09-06T00:00:04Z","type":"result","ref":"c.md"}"#,
    ]);
    let cfg = Config::default();
    let state = query::fold(&events, &cfg);
    assert_eq!(state.unacked("r", &["result"], &events), vec![3, 5]);
    assert_eq!(
        state.unacked("r", &["result", "note"], &events),
        vec![3, 4, 5]
    );
    // A reactor that has never acked owes everything.
    assert_eq!(state.unacked("s", &["result"], &events), vec![1, 3, 5]);
    // A fold stopped early never reports work above its own tip.
    assert_eq!(
        query::fold_at(&events, &cfg, 3).unacked("r", &["result"], &events),
        vec![3]
    );
}

#[test]
fn open_lifecycles_name_both_kinds_of_gap() {
    let events = synth(&[
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"w"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"result","agent":"ghost","ref":"a.md"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"result","ref":"b.md"}"#,
    ]);
    let state = query::fold(&events, &Config::default());
    // `ghost` never spawned; `w` never retired; the bare result at 3 is the
    // controller's own and is not a lifecycle.
    assert_eq!(
        state.open_lifecycles,
        vec!["ghost".to_string(), "w".to_string()]
    );
}

#[test]
fn the_drove_sample_folds_the_same_way_at_every_seq() {
    let report = Log::open("tests/fixtures/drove-events.jsonl")
        .read()
        .expect("drove fixture reads");
    let events = report.events;
    let cfg = Config::default();
    let tip = query::fold(&events, &cfg);
    assert!(tip.at > 280, "the sample is 287 lines");
    assert!(!tip.agents.is_empty());
    assert!(!tip.decisions.is_empty());

    // Folding to N is folding stopped at N: no state from above N leaks in.
    for at in 1..=tip.at {
        let state = query::fold_at(&events, &cfg, at);
        assert_eq!(state.at, at);
        for agent in state.agents.values() {
            assert!(agent.spawned_at <= at);
            assert!(agent.retired_at.is_none_or(|r| r <= at));
        }
        for (_, seq) in state.decisions.values() {
            assert!(*seq <= at);
        }
    }
}

#[test]
fn approval_for_seq_closes_only_that_escalation_and_by_is_the_default_subject() {
    let lines = [
        r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"escalate","by":"build-a","msg":"blocked on b"}"#,
        r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"escalate","by":"build-b","msg":"blocked on a"}"#,
        r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"escalate","by":"build-b","msg":"still blocked"}"#,
        r#"{"seq":4,"ts":"2026-09-06T00:00:03Z","type":"approval","for":"2","decision":"resolved"}"#,
        r#"{"seq":5,"ts":"2026-09-06T00:00:04Z","type":"approval","subject":"build-a","decision":"resolved"}"#,
    ];
    let events = synth(&lines);
    let cfg = Config::default();
    // Three worker escalations with no explicit subject stay separately open.
    let open_at_3: Vec<u64> = query::fold_at(&events, &cfg, 3)
        .escalations
        .iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(open_at_3, vec![1, 2, 3]);
    // `for=2` closes seq 2 only; `subject=build-a` closes seq 1; seq 3 remains.
    let open: Vec<u64> = query::fold(&events, &cfg)
        .escalations
        .iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(open, vec![3]);
}
