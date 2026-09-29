use eventlog::context::packet::{PacketInput, build};
use eventlog::context::worktree::{Change, WorkTree};
use eventlog::model::config::{Config, ContextConfig};
use eventlog::model::event::Event;
use eventlog::query;

fn ev(line: &str) -> Event {
    Event::parse_line(line).unwrap()
}

fn log() -> Vec<Event> {
    vec![
        ev(
            r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"decision","key":"crate-name","value":"eventlog-cli","ref":".context/DECISIONS.md"}"#,
        ),
        ev(r#"{"seq":2,"ts":"2026-09-01T01:00:00Z","type":"spawn","agent":"w1"}"#),
        ev(
            r#"{"seq":3,"ts":"2026-09-01T01:01:00Z","type":"prompt","agent":"w1","ref":".context/handoffs/w1.md"}"#,
        ),
        ev(r#"{"seq":4,"ts":"2026-09-01T01:02:00Z","type":"claim","agent":"w1","paths":"src/a"}"#),
        ev(
            r#"{"seq":5,"ts":"2026-09-01T02:00:00Z","type":"result","agent":"controller","ref":"src/lib.rs","paths":"src/lib.rs","summary":"add lib"}"#,
        ),
        ev(
            r#"{"seq":6,"ts":"2026-09-01T03:00:00Z","type":"intent","msg":"write the packet","ref":".context/handoffs/task.md"}"#,
        ),
    ]
}

fn render(
    events: &[Event],
    work: &WorkTree,
    budget: usize,
    files: &[(&str, &str)],
) -> eventlog::context::packet::Packet {
    let state = query::fold(events, &Config::default());
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    let read = |p: &str| files.iter().find(|(k, _)| k == p).map(|(_, v)| v.clone());
    let exists = |p: &str| files.iter().any(|(k, _)| k == p);
    build(
        &PacketInput {
            events,
            state: &state,
            log_path: ".context/events.jsonl",
            work,
            read_file: &read,
            exists: &exists,
        },
        budget,
    )
}

fn clean() -> WorkTree {
    WorkTree {
        available: true,
        changes: vec![],
    }
}

#[test]
fn sections_in_attention_order() {
    let p = render(
        &log(),
        &clean(),
        12_000,
        &[(".context/handoffs/task.md", "Task body")],
    );
    let keys: Vec<&str> = p.sections.iter().map(|s| s.key).collect();
    assert_eq!(
        keys,
        [
            "header",
            "decisions",
            "agents",
            "history",
            "artifacts",
            "open_work",
            "current_task"
        ]
    );
    assert_eq!(p.as_of, 6);
}

#[test]
fn decisions_carry_seq_and_ref() {
    let md = render(&log(), &clean(), 12_000, &[]).markdown();
    assert!(
        md.contains("- crate-name=eventlog-cli (seq 1, .context/DECISIONS.md)"),
        "{md}"
    );
}

#[test]
fn agents_show_phase_claims_and_brief() {
    let md = render(&log(), &clean(), 12_000, &[]).markdown();
    assert!(
        md.contains("- w1 phase=claimed claims=src/a brief=.context/handoffs/w1.md"),
        "{md}"
    );
}

#[test]
fn history_ages_are_relative_to_the_tip() {
    let md = render(&log(), &clean(), 12_000, &[]).markdown();
    assert!(
        md.contains("- seq 5 (1h ago) result controller: add lib paths=src/lib.rs"),
        "{md}"
    );
}

#[test]
fn artifact_index_marks_missing_paths() {
    let md = render(&log(), &clean(), 12_000, &[(".context/DECISIONS.md", "x")]).markdown();
    assert!(md.contains("- .context/DECISIONS.md (seq 1)\n"), "{md}");
    assert!(md.contains("- src/lib.rs (seq 5) (missing)"), "{md}");
}

#[test]
fn open_work_lists_intents_and_changes_with_owner() {
    let work = WorkTree {
        available: true,
        changes: vec![
            Change {
                path: "src/a/x.rs".into(),
                status: "M".into(),
                stat: "+3 -1".into(),
                mtime: 0,
                owner: Some("w1".into()),
            },
            Change {
                path: "notes.md".into(),
                status: "??".into(),
                stat: "new".into(),
                mtime: 0,
                owner: None,
            },
        ],
    };
    let md = render(&log(), &work, 12_000, &[]).markdown();
    assert!(
        md.contains("- intent seq 6 controller: write the packet ref=.context/handoffs/task.md"),
        "{md}"
    );
    assert!(md.contains("- M src/a/x.rs +3 -1 [claimed by w1]"), "{md}");
    assert!(md.contains("- ?? notes.md new"), "{md}");
}

#[test]
fn working_tree_unavailable_is_said() {
    let work = WorkTree {
        available: false,
        changes: vec![],
    };
    let md = render(&log(), &work, 12_000, &[]).markdown();
    assert!(md.contains("(working tree unavailable)"), "{md}");
}

#[test]
fn current_task_inlines_the_intent_ref_last() {
    let md = render(
        &log(),
        &clean(),
        12_000,
        &[(".context/handoffs/task.md", "Task body")],
    )
    .markdown();
    assert!(md.trim_end().ends_with("Task body"), "{md}");
}

#[test]
fn current_task_is_capped() {
    let big = "x".repeat(1_000_000);
    let budget = 12_000;
    let p = render(
        &log(),
        &clean(),
        budget,
        &[(".context/handoffs/task.md", &big)],
    );
    // The spec caps the inlined file at budget/2 chars, not the whole
    // budget: pin that half directly, so a regression to `budget` (or to
    // dropping the cap) fails here even though it would still pass a loose
    // "the whole packet is smallish" check.
    let section = p.sections.iter().find(|s| s.key == "current_task").unwrap();
    let inlined: usize = section
        .lines
        .iter()
        .filter(|l| l.chars().all(|c| c == 'x') && !l.is_empty())
        .map(|l| l.chars().count())
        .sum();
    assert_eq!(
        inlined,
        budget / 2,
        "inlined body should be exactly budget/2 chars"
    );
    assert!(inlined < budget);

    let md = p.markdown();
    assert!(md.len() < 30_000, "len {}", md.len());
    assert!(md.contains("(truncated; read `.context/handoffs/task.md` for the rest)"));
}

#[test]
fn reactor_health_pins_never_acked_wording() {
    let mut events = log();
    // A non-controller reactor that has acted (via `intent by=`) but never
    // acked anything: `last_ack_ts` stays `None`, and the wording must read
    // `last_ack=never`, not `last_ack=- ago`.
    events.push(ev(
        r#"{"seq":7,"ts":"2026-09-01T05:00:00Z","type":"intent","by":"committer","msg":"acted"}"#,
    ));
    let md = render(&events, &clean(), 12_000, &[]).markdown();
    assert!(md.contains("- committer unacked=2 last_ack=never"), "{md}");
}

#[test]
fn budget_drops_history_before_artifacts_and_keeps_decisions() {
    let mut events = log();
    for i in 0..40u64 {
        events.push(ev(&format!(
            r#"{{"seq":{},"ts":"2026-09-01T04:00:00Z","type":"note","msg":"note number {i} with some padding text","ref":"docs/n{i}.md"}}"#,
            7 + i
        )));
    }
    let full = render(&events, &clean(), 1_000_000, &[]);
    // Two history lines are about 150 characters: this forces two or three
    // of the oldest out and no more.
    let small = render(&events, &clean(), full.markdown().len() - 150, &[]);
    assert!(!small.over_budget);
    let hist = |p: &eventlog::context::packet::Packet| {
        p.sections
            .iter()
            .find(|s| s.key == "history")
            .unwrap()
            .lines
            .len()
    };
    assert!(hist(&small) < hist(&full));
    let md = small.markdown();
    assert!(md.contains("crate-name=eventlog-cli"));
    assert!(md.contains("write the packet"));
    // History is the last 15 of 42: notes 25..39. The oldest go first.
    assert!(full.markdown().contains("note number 25 "));
    assert!(md.contains("note number 39"));
    assert!(!md.contains("note number 25 "));
    // Artifacts are untouched while history still has lines to drop.
    let arts = |p: &eventlog::context::packet::Packet| {
        p.sections
            .iter()
            .find(|s| s.key == "artifacts")
            .unwrap()
            .lines
            .len()
    };
    assert_eq!(arts(&small), arts(&full));
}

#[test]
fn over_budget_when_kept_sections_alone_exceed_it() {
    let p = render(&log(), &clean(), 50, &[]);
    assert!(p.over_budget);
}

#[test]
fn same_input_same_bytes() {
    let a = render(&log(), &clean(), 12_000, &[]).markdown();
    let b = render(&log(), &clean(), 12_000, &[]).markdown();
    assert_eq!(a, b);
}

#[test]
fn empty_log_renders() {
    let p = render(&[], &clean(), 12_000, &[]);
    assert_eq!(p.as_of, 0);
    assert!(p.markdown().contains("- (none)"));
}

#[test]
fn json_carries_markdown_and_settings() {
    let p = render(&log(), &clean(), 12_000, &[]);
    let v = p.to_json(&ContextConfig::default());
    assert_eq!(v["v"], 1);
    assert_eq!(v["as_of"], 6);
    assert_eq!(v["settings"]["keep_turns"], 3);
    assert_eq!(v["settings"]["tail_chars"], 40_000);
    assert_eq!(v["markdown"].as_str().unwrap(), p.markdown());
    assert!(v["sections"]["decisions"].is_array());
}
