# Brief: build-model

You are the worker **build-model**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: Claude (opus) via claudewho-radiator.

Read first: `docs/superpowers/plans/2026-09-06-eventlog-cli.md` sections
"Global constraints" and "File structure", and the spec sections your task cites
in `docs/superpowers/specs/2026-09-06-event-log-cli-design.md`.

## Rules

Do only this task. Touch only the files it lists (plus `Cargo.lock`, which
cargo maintains). Write the failing test first, run it, make it pass, then run
`cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt`.
Do not `git commit`. Do not edit `Cargo.toml`, `src/lib.rs`, `src/main.rs`
or `src/cli.rs` unless this task lists them (exception: a command task may add
fields to its own `<Cmd>Args` struct in `src/cli.rs`, with a targeted edit only); if you need a change there,
append `escalate` (below) and stop. Do not start subagents. Never edit,
truncate or `rm` `.context/events.jsonl`.

## Contract

Task 1 (build-scaffold) already created the crate, `src/cli.rs` and every stub file; the CLI surface in `src/cli.rs` is frozen. Your public signatures become the model contract every later task builds against, so match the Interfaces block exactly. The crate uses `indexmap` for `Event.fields`; if it is not in `Cargo.toml`, escalate.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-model` on every line (decision `log-writers`):

    append-event.sh progress by=build-model msg="<one line>" ref=<main file>
    append-event.sh result   by=build-model ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-model msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 2: Model: Event, Config, Vocabulary, Allowlist, paths **[radiator: opus]**

Spec sections 3, 4.

**Files:**
- Create: `src/model/event.rs`, `src/model/config.rs`, `src/model/vocab.rs`, `src/model/allow.rs`, `src/model/paths.rs`, `tests/model_event.rs`, `tests/model_allow.rs`, `tests/model_paths.rs`, `tests/fixtures/eventlog.toml`
- Modify: `src/model/mod.rs`

**Interfaces produced (this is the model contract; later tasks build against it):**

```rust
// event.rs
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    pub r#type: String,
    pub prev: Option<String>,
    pub by: Option<String>,
    pub agent: Option<String>,
    pub fields: indexmap::IndexMap<String, String>, // every other field, in file order
}
impl Event {
    pub fn parse_line(line: &str) -> Result<Event, ParseError>;   // ParseError { line_hint: String, reason: String }
    pub fn to_line(&self) -> String;                               // serializes in spec order, no trailing newline
    pub fn seq_ref(&self, field: &str) -> Option<u64>;             // "seq_done" -> 51
    pub fn paths(&self) -> Vec<String>;                            // split "paths" on ','
    pub fn writer(&self) -> &str;                                  // by or "controller"
    pub fn subject(&self) -> &str;                                 // agent or "controller"
}
// config.rs
pub struct Config { pub log: LogConfig, pub vocabulary: Vocabulary, pub writers: Allowlist, pub view: ViewConfig, pub keys: KeysConfig }
pub struct LogConfig { pub path: PathBuf, pub fsync: bool, pub named: BTreeMap<String, PathBuf> }
pub fn load(repo_root: &Path) -> anyhow::Result<Config>;         // .context/eventlog.toml, then ~/.config/eventlog/config.toml, then defaults
pub fn resolve_log(cfg: &Config, selector: Option<&str>) -> PathBuf;  // None -> cfg.log.path; a named key; else a path
// vocab.rs
pub struct TypeSpec { pub fields: Vec<String>, pub optional: Vec<String> }
pub struct Vocabulary(BTreeMap<String, TypeSpec>);
impl Vocabulary { pub fn builtin() -> Self; pub fn merge_file(&mut self, extra: BTreeMap<String, TypeSpec>); pub fn get(&self, ty: &str) -> Option<&TypeSpec>; pub fn types(&self) -> Vec<&str>; }
pub const REFERENCE_FIELDS: &[&str] = &["seq_done", "for", "for_ack", "intent"];
// allow.rs
pub struct Allowlist(BTreeMap<String, Vec<String>>);            // type -> writers
impl Allowlist {
    pub fn builtin() -> Self;
    pub fn merge_file(&mut self, extra: BTreeMap<String, Vec<String>>);
    pub fn apply_decision(&mut self, value: &str);                 // full replacement per spec 4
    pub fn permits(&self, writer: &str, ty: &str) -> bool;
}
// paths.rs
pub struct RelPath(String);
pub fn validate_paths(field: &str) -> Result<Vec<RelPath>, PathError>;   // comma split, no comma inside, not absolute, no escape after ..
pub fn canonicalize(root: &Path, p: &RelPath) -> Result<PathBuf, PathError>;  // rejects symlinked components
pub fn is_log_or_lock(cfg_log: &Path, p: &Path) -> bool;        // <log> or <log>.*
```

`apply_decision` value grammar: `controller-plus-reactors` keeps the builtin; any other value is `name:type1|type2;name2:...` and replaces the whole map, with `controller` always retaining `spawn,prompt,claim,decision,retire,approval`. Document this in `vocab.rs` doc comment; `EVENTLOG.md` (Task 19) repeats it.

- [ ] **Step 1: failing tests.** `tests/model_event.rs`:

```rust
use eventlog::model::event::Event;
#[test]
fn parses_existing_line_with_quoted_seq_done() {
    let e = Event::parse_line(r#"{"seq":51,"ts":"2026-09-06T20:58:58Z","type":"ack","by":"cursor-committer","seq_done":"50","outcome":"committed"}"#).unwrap();
    assert_eq!(e.seq, 51); assert_eq!(e.seq_ref("seq_done"), Some(50)); assert_eq!(e.writer(), "cursor-committer"); assert_eq!(e.subject(), "controller");
}
#[test]
fn to_line_keeps_spec_order() {
    let e = Event::parse_line(r#"{"type":"result","seq":3,"agent":"w","ts":"2026-01-01T00:00:00Z","ref":"x"}"#).unwrap();
    assert_eq!(e.to_line(), r#"{"seq":3,"ts":"2026-01-01T00:00:00Z","type":"result","agent":"w","ref":"x"}"#);
}
#[test]
fn every_fixture_line_parses() {
    for (i, l) in include_str!("fixtures/drove-events.jsonl").lines().enumerate() { Event::parse_line(l).unwrap_or_else(|e| panic!("line {}: {:?}", i + 1, e)); }
}
```

`tests/model_allow.rs`:

```rust
use eventlog::model::allow::Allowlist;
#[test]
fn builtin_lets_only_controller_spawn() {
    let a = Allowlist::builtin();
    assert!(a.permits("controller", "spawn")); assert!(!a.permits("doc-worker", "spawn")); assert!(a.permits("doc-worker", "ack"));
}
#[test]
fn decision_replaces_in_full_but_keeps_controller_core() {
    let mut a = Allowlist::builtin();
    a.apply_decision("survey-x:progress|result");
    assert!(a.permits("survey-x", "result")); assert!(!a.permits("doc-worker", "ack")); assert!(a.permits("controller", "decision"));
}
```

`tests/model_paths.rs`:

```rust
use eventlog::model::paths::validate_paths;
#[test] fn rejects_absolute_and_escape() { assert!(validate_paths("/etc/hosts").is_err()); assert!(validate_paths("../x").is_err()); assert!(validate_paths("a/../../x").is_err()); }
#[test] fn splits_on_comma() { assert_eq!(validate_paths("a.md,b/c.rs").unwrap().len(), 2); }
```

- [ ] **Step 2:** run, confirm failures. **Step 3:** implement. **Step 4:** all green, clippy, fmt. **Step 5:** `result`.
