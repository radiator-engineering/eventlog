//! The reactor loop: baseline, resume, interrupted intents, and the lock that
//! keeps two runtimes off the same log. Spec section 7 steps 1-3.
//!
//! The voter and the action are stubbed behind `Steps`, so these tests drive
//! the loop without running a command or touching git.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eventlog::log::Log;
use eventlog::model::config::Config;
use eventlog::model::event::Event;
use eventlog::query::State;
use eventlog::react::lock::{ReactorLock, Token};
use eventlog::react::{GitSnapshot, Outcome, Reactor, ReactorConfig, Steps};

/// Records the seq of every driving event the loop asked it to act on.
#[derive(Clone, Default)]
struct Fake {
    runs: Arc<Mutex<Vec<u64>>>,
    veto: Option<String>,
    unauthorized: Option<String>,
    /// Snapshots handed back in order; empty means a clean, unmoving tree.
    snapshots: Arc<Mutex<Vec<GitSnapshot>>>,
    /// What the action "committed" between the two snapshots.
    committed: Vec<String>,
}

impl Steps for Fake {
    fn authorize(&mut self, _state: &State, driving: &Event) -> Result<Vec<String>, String> {
        match &self.unauthorized {
            Some(reason) => Err(reason.clone()),
            None => Ok(driving.paths()),
        }
    }

    fn check(
        &mut self,
        _state: &State,
        _driving: &Event,
        _authorized: &[String],
    ) -> Option<String> {
        self.veto.clone()
    }

    fn run(
        &mut self,
        _cfg: &ReactorConfig,
        driving: &Event,
        _authorized: &[String],
        _resume: u64,
    ) -> anyhow::Result<Outcome> {
        self.runs.lock().unwrap().push(driving.seq);
        Ok(Outcome {
            outcome: "committed".to_string(),
            fields: Vec::new(),
        })
    }

    fn snapshot(&mut self) -> anyhow::Result<GitSnapshot> {
        let mut scripted = self.snapshots.lock().unwrap();
        if scripted.is_empty() {
            return Ok(GitSnapshot::default());
        }
        Ok(scripted.remove(0))
    }

    fn committed(
        &mut self,
        _before: &GitSnapshot,
        _after: &GitSnapshot,
    ) -> anyhow::Result<Vec<String>> {
        Ok(self.committed.clone())
    }
}

/// One fixture line: `seq`, `type`, and its fields.
type Row<'a> = (u64, &'a str, &'a [(&'a str, &'a str)]);

/// Write a log with `prev` chained, so these fixtures verify like a real log.
fn write_log(path: &Path, rows: &[Row<'_>]) {
    let mut prev = "genesis".to_string();
    let mut out = String::new();
    for (seq, ty, kvs) in rows {
        let mut event = Event {
            seq: *seq,
            ts: "2026-09-06T00:00:00Z".to_string(),
            r#type: (*ty).to_string(),
            prev: Some(prev.clone()),
            by: None,
            agent: None,
            fields: Default::default(),
        };
        for (k, v) in kvs.iter() {
            match *k {
                "by" => event.by = Some((*v).to_string()),
                "agent" => event.agent = Some((*v).to_string()),
                _ => {
                    event.fields.insert((*k).to_string(), (*v).to_string());
                }
            }
        }
        let line = event.to_line();
        prev = Log::hash_line(line.as_bytes());
        out.push_str(&line);
        out.push('\n');
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, out).unwrap();
}

fn reactor(path: &Path, root: &Path, fake: Fake) -> Reactor {
    reactor_with(path, root, fake, false)
}

fn reactor_with(path: &Path, root: &Path, fake: Fake, git: bool) -> Reactor {
    let cfg = ReactorConfig {
        name: "commit".to_string(),
        on: vec!["result".to_string()],
        command: vec!["true".to_string()],
        git,
        ..ReactorConfig::default()
    };
    Reactor::new(cfg, Log::open(path), Config::default(), root.to_path_buf())
        .with_steps(Box::new(fake))
}

fn events(path: &Path) -> Vec<Event> {
    Log::open(path).read().unwrap().events
}

/// A `result` line the reactor's `--on` matches.
const RESULT: &[(&str, &str)] = &[("agent", "w"), ("ref", "src/a.rs"), ("paths", "src/a.rs")];

#[test]
fn a_first_start_baselines_at_the_tip_and_handles_nothing_older() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(".context/events.jsonl");
    write_log(
        &log,
        &[
            (1, "spawn", &[("agent", "w")]),
            (2, "result", RESULT),
            (3, "result", RESULT),
        ],
    );

    let fake = Fake::default();
    let runs = fake.runs.clone();
    reactor(&log, dir.path(), fake).catch_up().unwrap();

    assert!(runs.lock().unwrap().is_empty(), "acted on an older event");
    let written: Vec<Event> = events(&log).into_iter().filter(|e| e.seq > 3).collect();
    assert_eq!(written.len(), 1, "expected one baseline line: {written:?}");
    let ack = &written[0];
    assert_eq!(ack.r#type, "ack");
    assert_eq!(ack.writer(), "commit");
    assert_eq!(ack.seq_ref("seq_done"), Some(3));
    assert_eq!(
        ack.fields.get("outcome").map(String::as_str),
        Some("skipped")
    );
    assert_eq!(
        ack.fields.get("detail").map(String::as_str),
        Some("baseline")
    );
}

#[test]
fn a_resumed_reactor_handles_every_matching_event_above_its_ack_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(".context/events.jsonl");
    let mut rows: Vec<Row<'_>> = vec![(1, "spawn", &[("agent", "w")])];
    for seq in 2..=8 {
        rows.push((seq, "note", &[("msg", "filler")]));
    }
    rows.push((
        9,
        "ack",
        &[("by", "commit"), ("seq_done", "9"), ("outcome", "skipped")],
    ));
    for seq in 10..=12 {
        rows.push((seq, "result", RESULT));
    }
    write_log(&log, &rows);

    let fake = Fake::default();
    let runs = fake.runs.clone();
    reactor(&log, dir.path(), fake).catch_up().unwrap();

    assert_eq!(*runs.lock().unwrap(), vec![10, 11, 12]);
    let acked: Vec<u64> = events(&log)
        .iter()
        .filter(|e| e.seq > 12 && e.r#type == "ack" && e.writer() == "commit")
        .filter_map(|e| e.seq_ref("seq_done"))
        .collect();
    assert_eq!(acked, vec![10, 11, 12]);
}

#[test]
fn an_own_intent_with_no_ack_is_acked_interrupted_and_escalated_without_acting() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(".context/events.jsonl");
    let mut rows: Vec<Row<'_>> = vec![(1, "spawn", &[("agent", "w")])];
    for seq in 2..=8 {
        rows.push((seq, "note", &[("msg", "filler")]));
    }
    rows.push((
        9,
        "ack",
        &[("by", "commit"), ("seq_done", "9"), ("outcome", "skipped")],
    ));
    rows.push((10, "result", RESULT));
    rows.push((11, "result", RESULT));
    rows.push((12, "intent", &[("by", "commit"), ("for", "11")]));
    write_log(&log, &rows);

    let fake = Fake::default();
    let runs = fake.runs.clone();
    reactor(&log, dir.path(), fake).catch_up().unwrap();

    assert!(
        !runs.lock().unwrap().contains(&11),
        "re-ran an event whose effect may already have happened"
    );
    let new: Vec<Event> = events(&log).into_iter().filter(|e| e.seq > 12).collect();
    let interrupted = new
        .iter()
        .find(|e| e.r#type == "ack" && e.seq_ref("seq_done") == Some(11))
        .expect("no ack for the interrupted seq");
    assert_eq!(
        interrupted.fields.get("outcome").map(String::as_str),
        Some("interrupted")
    );
    assert_eq!(
        interrupted.seq_ref("for"),
        Some(12),
        "ack must close the intent"
    );
    assert!(
        new.iter()
            .any(|e| e.r#type == "escalate" && e.writer() == "commit"),
        "no escalate: {new:?}"
    );
}

#[test]
fn a_lock_whose_pid_is_alive_with_a_different_start_time_is_reclaimed() {
    let dir = tempfile::tempdir().unwrap();
    let lock_dir = dir.path().join("events.jsonl.commit.reactor.lock");
    std::fs::create_dir(&lock_dir).unwrap();
    let mut stale = Token::current();
    stale.start_time = format!("{} (not this process)", stale.start_time);
    std::fs::write(lock_dir.join("token"), stale.to_json()).unwrap();

    let lock = ReactorLock::acquire(&lock_dir);
    assert!(lock.is_ok(), "a recycled pid held the lock forever");
}

#[test]
fn exactly_one_of_two_racing_reclaimers_takes_a_dead_lock() {
    let dir = tempfile::tempdir().unwrap();
    let lock_dir = dir.path().join("events.jsonl.commit.reactor.lock");
    std::fs::create_dir(&lock_dir).unwrap();
    let mut dead = Token::current();
    dead.pid = 999_999;
    dead.start_time = "long gone".to_string();
    std::fs::write(lock_dir.join("token"), dead.to_json()).unwrap();

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let dir = lock_dir.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let held = ReactorLock::acquire(&dir);
                // Hold what we took until both threads have tried.
                std::thread::sleep(Duration::from_millis(300));
                held.is_ok()
            })
        })
        .collect();

    let winners = handles
        .into_iter()
        .filter(|_| true)
        .map(|h| h.join().unwrap())
        .filter(|ok| *ok)
        .count();
    assert_eq!(winners, 1, "expected exactly one holder");
}

#[test]
fn an_unauthorized_driving_event_is_vetoed_and_acked_without_acting() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(".context/events.jsonl");
    write_log(
        &log,
        &[
            (1, "spawn", &[("agent", "w")]),
            (
                2,
                "ack",
                &[("by", "commit"), ("seq_done", "2"), ("outcome", "skipped")],
            ),
            (3, "result", RESULT),
        ],
    );

    let fake = Fake {
        unauthorized: Some("unclaimed-paths".to_string()),
        ..Fake::default()
    };
    let runs = fake.runs.clone();
    reactor(&log, dir.path(), fake).catch_up().unwrap();

    assert!(runs.lock().unwrap().is_empty(), "acted despite the veto");
    let new: Vec<Event> = events(&log).into_iter().filter(|e| e.seq > 3).collect();
    let veto = new.iter().find(|e| e.r#type == "veto").expect("no veto");
    assert_eq!(veto.seq_ref("for"), Some(3));
    assert_eq!(
        veto.fields.get("reason").map(String::as_str),
        Some("unclaimed-paths")
    );
    let ack = new.iter().find(|e| e.r#type == "ack").expect("no ack");
    assert_eq!(
        ack.fields.get("outcome").map(String::as_str),
        Some("vetoed")
    );
}

#[test]
fn a_dry_handle_returns_the_events_it_would_append_and_writes_none() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(".context/events.jsonl");
    write_log(
        &log,
        &[
            (1, "spawn", &[("agent", "w")]),
            (
                2,
                "ack",
                &[("by", "commit"), ("seq_done", "2"), ("outcome", "skipped")],
            ),
            (3, "result", RESULT),
        ],
    );
    let before = std::fs::read_to_string(&log).unwrap();

    let fake = Fake::default();
    let runs = fake.runs.clone();
    let driving = events(&log).into_iter().find(|e| e.seq == 3).unwrap();
    let planned = reactor(&log, dir.path(), fake)
        .handle(&driving, true)
        .unwrap();

    assert_eq!(
        *runs.lock().unwrap(),
        vec![3],
        "test mode still runs the command"
    );
    let types: Vec<&str> = planned.iter().map(|e| e.r#type.as_str()).collect();
    assert_eq!(types, vec!["intent", "ack"]);
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        before,
        "log was written"
    );
}

/// Spec 7.4.6 as amended by decision `violation-scope` (seq 474): a
/// `violation` names only files the action committed outside the authorized
/// set. Files that merely became dirty during the pass are somebody else's
/// work in progress: silent when an open claim covers them, an `observed`
/// line when nothing does. Never a violation.
#[test]
fn a_git_pass_blames_only_what_it_committed_and_observes_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(".context/events.jsonl");
    write_log(
        &log,
        &[
            (1, "spawn", &[("agent", "w")]),
            (2, "spawn", &[("agent", "other")]),
            (3, "claim", &[("agent", "other"), ("paths", "src/other.rs")]),
            (
                4,
                "ack",
                &[("by", "commit"), ("seq_done", "4"), ("outcome", "skipped")],
            ),
            (5, "result", RESULT),
        ],
    );

    let fake = Fake {
        snapshots: Arc::new(Mutex::new(vec![
            GitSnapshot {
                head: "before".to_string(),
                status: vec!["src/pre.rs".to_string()],
            },
            GitSnapshot {
                head: "after".to_string(),
                status: vec![
                    "src/pre.rs".to_string(),
                    "src/a.rs".to_string(),
                    "src/other.rs".to_string(),
                    "src/stray.rs".to_string(),
                ],
            },
        ])),
        committed: vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
        ..Fake::default()
    };
    let mut r = reactor_with(&log, dir.path(), fake, true);
    r.catch_up().unwrap();

    let new: Vec<Event> = events(&log).into_iter().filter(|e| e.seq > 5).collect();
    let violation = new
        .iter()
        .find(|e| e.r#type == "violation")
        .expect("no violation for the unauthorized commit");
    assert_eq!(
        violation.fields.get("paths").map(String::as_str),
        Some("src/b.rs"),
        "only the committed, unauthorized file is a violation"
    );
    assert_eq!(violation.seq_ref("for"), Some(5));

    let observed = new
        .iter()
        .find(|e| e.r#type == "observed")
        .expect("no observed line for the unclaimed stray file");
    assert_eq!(
        observed.fields.get("paths").map(String::as_str),
        Some("src/stray.rs"),
        "src/pre.rs was dirty before, src/a.rs is authorized, src/other.rs is claimed"
    );
    assert_eq!(observed.seq_ref("for"), Some(5));
    assert_eq!(observed.writer(), "commit");

    let ack = new.iter().find(|e| e.r#type == "ack").expect("no ack");
    assert_eq!(
        ack.fields.get("outcome").map(String::as_str),
        Some("committed")
    );
}
