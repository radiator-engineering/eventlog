# eventlog CLI Implementation Plan

> **For agentic workers:** This plan is executed by herdr peer agents coordinated through `.context/events.jsonl`. Each task below is one worker brief. The controller uses the "Execution model" section; a worker reads only its own task plus "Global constraints" and "Worker protocol". Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the `eventlog` Rust binary that replaces the shell toolkit, phase by phase, with this repo running each phase on itself.

**Architecture:** One crate, `lib.rs` plus `main.rs`. `model` defines events, config, vocabulary and allowlist. `log` appends and verifies. `query` folds events into `State`. `react`, `guard`, `scaffold`, `tui` and `skill` sit on those three. A serial scaffold task creates every module file and all shared wiring first, so later tasks run in parallel on disjoint files.

**Tech Stack:** Rust 1.88+ (edition 2024), clap 4 (derive), serde, serde_json, toml, sha2, hex, ratatui + crossterm, notify, include_dir, schemars, assert_cmd, predicates, tempfile, cargo-mutants, cargo-dist.

**Spec:** `docs/superpowers/specs/2026-09-06-event-log-cli-design.md`. The plan argues from the spec; read both.

## Global constraints

- Crate and binary name `eventlog`. Edition 2024, `rust-version = "1.88"`, license Apache-2.0, repository `https://github.com/radiator-engineering/event-log`.
- The on-disk log format is unchanged for existing types: `seq` integer, `ts` string, `type` string, other fields strings. Reference fields (`seq_done`, `for`, `for_ack`, `intent`) stay JSON strings on disk and are parsed as integers in memory.
- `prev` = hex sha256 of the previous line's bytes with trailing `\r` and `\n` stripped; `genesis` only on an empty log.
- Field cap 2 KB, event cap 4 KB. Writer rejects user-supplied `seq`, `ts`, `prev`, and `by` unless equal to `--as`.
- Exit codes: 0 success, 1 check failed, 2 guard deny or lock conflict. One stderr line per failure. No panics on malformed input.
- `--json` rows carry `"v":1`. Command names in spec section 6 are frozen.
- Never write to `.context/events.jsonl` except through `append-event.sh` (during the build) with `by=<your worker name>`. Never `git commit`; the commit reactor does that.
- Tests: `cargo test` must pass before a worker reports `result`. `cargo clippy -- -D warnings` and `cargo fmt --check` must pass.

---

## Execution model (controller reads this)

**Who runs what.** The controller (the Claude in `control › coordinator`) spawns one herdr peer per task in the `build` workspace. Default runtime is `cursor-agent` on auto. Tasks marked **[radiator: sonnet]** or **[radiator: opus]** run under `claudewho-radiator --model <m>` because they define contracts or carry safety claims.

**Per task, the controller:**

1. Writes the brief to `.context/handoffs/<task-slug>.md` from the "Worker brief template" below, pasting the task section verbatim.
2. Creates a tab in workspace `build` labeled `<runtime>: <task words>`, renames the pane to the slug, exports `CLAUDE_CONFIG_DIR=/Users/jjmartin/.claudewho-radiator` in the pane for radiator tasks, then `herdr agent start <slug> --kind cursor|claude --pane <p>` and `herdr agent prompt <slug> "<pointer to brief>"`.
3. Appends `spawn agent=<slug> model=<m> runtime=<r> pane=<p> role=build`, `prompt agent=<slug> ref=<brief>`, `claim agent=<slug> paths=<the task's Files list>`.
4. Waits on the log for `result by=<slug>` (monitor `tail -F | jq select(.by==...)`).
5. Runs `check-claims.sh <slug> <base-sha>` (later: `eventlog claims`), runs `cargo test` itself, reads the diff.
6. Appends `result ref=<main file> paths=<changed files> summary=...` (controller-written, no `by`), then `retire agent=<slug> disposition=accepted`. The committer commits; the doc worker documents.
7. On a `violation` or failed tests: `escalate` or re-prompt the same agent with the fix; never widen the claim silently.

**Parallel groups.** Tasks in the same group have disjoint `Files` and may run at once, one pane each. A group starts only after every task it depends on has a controller `result` and `retire`. `cargo` serializes on the target-dir lock; that is delay, not conflict.

**Shared files are frozen after Task 1.** `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/cli.rs` are claimed by Task 1 only. A later task that needs a new dependency or a new `mod` line appends `escalate by=<slug> msg="needs <dep> in Cargo.toml"`; the controller makes the one-line change and records its own `result`.

**Contracts are decisions.** Before group 1B starts, the controller appends `decision key=model-contract value=src/model/mod.rs@<sha> ref=.context/DECISIONS.md`. Every later brief says "build against the model contract at that sha; do not change public signatures; escalate if you must".

### Worker protocol (every brief includes this)

You are the worker **<slug>**, spawned by the controller (see AGENTS.md, "Spawned worker"). Do only this task. Touch only the files it lists. Read the spec sections it cites. Write the failing test first, run it, make it pass, run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt`. Do not `git commit`. Do not edit `Cargo.toml`, `src/lib.rs`, `src/main.rs` or `src/cli.rs`; if you need a change there, append `escalate by=<slug> msg="..."` and stop. Report through the log, only with `append-event.sh` and only with `by=<slug>`:

    append-event.sh progress by=<slug> msg="<one line>" ref=<main file>
    append-event.sh result   by=<slug> ref=<main file> paths=<files you changed> summary="<one line>"

End your final message with the word DONE.

### Worker brief template

```
# Brief: <slug>
<Worker protocol, verbatim>
## Contract
Build against the model contract at <sha> (decision key=model-contract). Spec: <sections>.
## Task
<the task section from this plan, verbatim>
```

---

## File structure

```
Cargo.toml                      Task 1
dist-workspace.toml             Task 20
rustfmt.toml                    Task 1
.gitattributes                  Task 1 (log marked -text)
src/main.rs                     Task 1: calls eventlog::cli::run()
src/lib.rs                      Task 1: pub mod declarations only
src/cli.rs                      Task 1: clap enums for every command (frozen names); dispatch stubs returning "not implemented" exit 1
src/model/mod.rs                Task 2: pub use of the four files
src/model/event.rs              Task 2: Event, parse_line, to_line, seq_ref, paths
src/model/config.rs             Task 2: Config, load, defaults
src/model/vocab.rs              Task 2: Vocabulary, TypeSpec, builtin table
src/model/allow.rs              Task 2: Allowlist, builtin, apply_decision, permits
src/model/paths.rs              Task 2: RelPath, validate, canonicalize
src/log/mod.rs                  Task 3: Log, open, read, tail, hash_line
src/log/lock.rs                 Task 3: Lock (mkdir, token, atomic-rename reclaim)
src/log/append.rs               Task 4: AppendRequest, append, strict validation
src/log/verify.rs               Task 5: verify, VerifyReport
src/cmd/append.rs               Task 6: append, vocab, --dry-run
src/cmd/view.rs                 Task 7: view, follow, filters, --json
src/cmd/schema.rs               Task 8: schema
src/query/mod.rs                Task 9: State, fold, fold_at
src/query/why.rs                Task 10: why
src/cmd/agents.rs               Task 11: agents
src/cmd/state.rs                Task 11: state
src/cmd/why.rs                  Task 11: why
src/cmd/claims.rs               Task 12: claims (alias check-claims)
src/cmd/open.rs                 Task 12: open
src/tui/mod.rs                  Task 13: app loop
src/tui/views.rs                Task 13: follow, agents, state panes
src/react/mod.rs                Task 14: Reactor, run loop, baseline, resume
src/react/lock.rs               Task 14: reactor lock with pid/start/host/boot
src/react/voter.rs              Task 15: authorized set, rules, veto
src/react/action.rs             Task 16: run command, env, outcome file, violation snapshot
src/cmd/react.rs                Task 17: react, react test
src/guard/mod.rs                Task 18: payload parsing, agent detection
src/guard/deny.rs               Task 18: denylist, simple-command parser
src/cmd/guard.rs                Task 18: guard, guard install
src/scaffold/mod.rs             Task 19: init, protect
src/scaffold/doctor.rs          Task 19: doctor
src/skill/mod.rs                Task 20: include_dir, install, stamp
src/cmd/skill.rs                Task 20
skill/event-log-coordination/   Task 21: rewrite SKILL.md and references to subcommands
tests/fixtures/                 Task 1 copies drove-events.jsonl; later tasks add files
tests/*.rs                      one integration test file per cmd, named in each task
```

`src/cmd/mod.rs` (Task 1) declares one stub file per command: `append, vocab, verify, view, agents, state, why, claims, open, tui, react, guard, init, doctor, protect, schema, skill, completions` (18 files, each `pub fn run(args: &crate::cli::Args) -> anyhow::Result<i32>`), so `cli.rs` compiles from day one and each later task replaces one stub. `cmd/verify.rs` and `cmd/tui.rs` are filled by Tasks 5 and 13 respectively.

---

## Phase 1: writer, verify, view

### Group 1A (serial)

### Task 1: Crate scaffold and frozen CLI surface **[radiator: sonnet]**

**Files:**
- Create: `Cargo.toml`, `rustfmt.toml`, `.gitattributes`, `src/main.rs`, `src/lib.rs`, `src/cli.rs`, `src/cmd/mod.rs`, one stub per `src/cmd/*.rs` and per module `mod.rs` listed in File structure, `tests/fixtures/drove-events.jsonl` (copy of `.context/research/samples/drove-events.jsonl`), `tests/cli_surface.rs`
- Modify: `.gitignore` (add `target/`)

**Interfaces produced:**
- `pub fn eventlog::cli::run() -> i32` (parses argv, dispatches, returns exit code).
- `pub enum Command` in `cli.rs` with exactly these subcommands: `Append`, `Vocab`, `Verify`, `View`, `Agents`, `State`, `Why`, `Claims` (alias `check-claims`, hidden), `Open`, `Tui`, `React` (subcommand `Test`), `Guard` (subcommand `Install`), `Init`, `Doctor`, `Protect`, `Schema`, `Skill` (subcommand `Install`), `Completions`.
- Global flags on every command: `--log <name|path>`, `--json`.
- Every `cmd::<x>::run` stub returns `Ok(1)` after printing `eventlog <x>: not implemented` to stderr.

- [ ] **Step 1: write the failing test** in `tests/cli_surface.rs`:

```rust
use assert_cmd::Command;
#[test]
fn every_frozen_command_is_recognized() {
    for c in ["append","vocab","verify","view","agents","state","why","claims","open","tui","react","guard","init","doctor","protect","schema","skill","completions"] {
        Command::cargo_bin("eventlog").unwrap().arg(c).arg("--help").assert().success();
    }
}
#[test]
fn check_claims_is_a_hidden_alias() {
    Command::cargo_bin("eventlog").unwrap().args(["check-claims","--help"]).assert().success();
    let help = Command::cargo_bin("eventlog").unwrap().arg("--help").output().unwrap();
    assert!(!String::from_utf8_lossy(&help.stdout).contains("check-claims"));
}
```

- [ ] **Step 2:** `cargo test` fails (no crate).
- [ ] **Step 3:** write `Cargo.toml` with the dependencies in Tech Stack, `[[bin]] name = "eventlog"`, `[lib]`; write `cli.rs` with the clap derive; write every stub file; `lib.rs` declares `pub mod cli; pub mod cmd; pub mod model; pub mod log; pub mod query; pub mod react; pub mod guard; pub mod scaffold; pub mod tui; pub mod skill;`.
- [ ] **Step 4:** `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt` pass.
- [ ] **Step 5:** report `result` naming every created file.

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

### Group 1B (parallel after Task 2 and the `model-contract` decision)

### Task 3: Log open, read, tail, hash, lock **[radiator: sonnet]**

Spec section 3 (hash chain, malformed lines).

**Files:** Create `src/log/lock.rs`, `tests/log_read.rs`, `tests/log_lock.rs`; Modify `src/log/mod.rs`.

**Interfaces produced:**

```rust
pub struct Log { pub path: PathBuf }
pub struct ReadReport { pub events: Vec<Event>, pub malformed: Vec<(usize, String)> } // (line number, reason)
pub struct Tail { pub last_seq: u64, pub last_line: Option<Vec<u8>>, pub chained: bool, pub torn: Option<usize> }
impl Log {
    pub fn open(path: impl Into<PathBuf>) -> Log;
    pub fn read(&self) -> anyhow::Result<ReadReport>;
    pub fn tail(&self) -> anyhow::Result<Tail>;            // reads last line only; torn = Some(line no) if it does not parse
    pub fn hash_line(bytes: &[u8]) -> String;              // strip trailing \r\n, sha256 hex
}
pub struct Lock { dir: PathBuf }
impl Lock { pub fn acquire(dir: &Path, wait: std::time::Duration) -> Result<Lock, LockError>; } // mkdir, pid inside, reclaim if pid dead by atomic rename; LockError::Busy -> exit 2
impl Drop for Lock { fn drop(&mut self) { /* remove dir */ } }
```

- [ ] **Step 1:** tests: `hash_line(b"abc\r\n") == hash_line(b"abc")`; `tail` on the fixture returns `last_seq == 287`; `tail` on a file whose last line is `{"seq":5,"ts":"...` (cut) returns `torn == Some(n)`; lock: acquire twice in one process, second returns `Busy` within 200 ms; write a lock dir with pid 999999 (dead), acquire succeeds.
- [ ] **Step 2–5** as above.

### Task 4: Append with strict validation

Spec sections 3, 4, 6 "Strict append". Depends on Task 3 for `Log`/`Lock` — start when Task 3 has its `result`, or stub against the signatures above.

**Files:** Create `src/log/append.rs`, `tests/log_append.rs`.

**Interfaces produced:**

```rust
pub struct AppendRequest { pub r#type: String, pub fields: Vec<(String, String)>, pub writer: String /* --as */, pub strict: bool, pub dry_run: bool }
pub enum AppendError { Reserved(String), ByMismatch, FieldTooLarge(String), EventTooLarge, NotPermitted{writer:String, ty:String}, MissingField(String), UnknownType(String), BadPath(String), TornTail(usize), Strict(String) /* rule name */, Lock(LockError) }
pub fn append(log: &Log, cfg: &Config, req: AppendRequest, fold: Option<&dyn Fn() -> query::State>) -> Result<Event, AppendError>;
```

Rules, in order: reserved fields; `by` equals writer or absent; `agent` default `controller` when writer is controller and type is `result`; type known; required fields present; paths valid; sizes; allowlist (`cfg.writers` after `fold().allowlist_at_tip()` when a fold is supplied); strict rules (spec section 6) using the fold; tail torn → refuse; then lock, `seq = tail.last_seq + 1`, `ts` now, `prev = genesis | hash(last_line)`, write one `write_all` of line + `\n`, `fsync` if configured, unlock. `dry_run` runs everything except lock and write and returns the would-be event.

- [ ] **Step 1:** tests: append to an empty temp log yields `seq 1, prev "genesis"`; second append has `prev == hash(first line)`; `k=v` with `seq=9` → `Reserved`; `by=x` with writer `controller` → `ByMismatch`; `--as doc-worker spawn` → `NotPermitted`; a 3000-byte field → `FieldTooLarge`; existing pre-chain fixture: first append's `prev == hash(last fixture line)`; torn tail → `TornTail`; two processes appending 50 lines each via `std::process::Command` on the built binary produce seq 1..100 with no gaps (this test lives in Task 6's integration file if the binary is needed; here use two threads with `Log::open` on the same path).
- [ ] **Steps 2–5.**

### Task 5: Verify

Spec section 3 hash chain.

**Files:** Create `src/log/verify.rs`, `tests/log_verify.rs`, `tests/fixtures/chain_ok.jsonl`, `tests/fixtures/chain_break.jsonl`, `tests/fixtures/chain_unchained_after.jsonl`; Modify `src/cmd/verify.rs` (prints `ok: N events, chain intact` or the failure, exit 1).

**Interfaces produced:** `pub struct VerifyReport { pub checked: u64, pub last_good: u64, pub failure: Option<String> }`, `pub fn verify(log: &Log) -> anyhow::Result<VerifyReport>`.

Rules: pre-chain lines pass until the first `prev`; that line's `prev` must equal hash of the last pre-chain line (or `genesis` if it is line 1); every later line must have `prev` equal to hash of its predecessor; a later line without `prev` is a failure "unchained line at seq N".

- [ ] **Step 1:** tests for each fixture: ok → `failure None`, break → `last_good` is the seq before the break, unchained-after → failure names the seq. Also: stripping every `prev` from `chain_ok` must fail with "unchained" (write the mutation in the test).
- [ ] **Steps 2–5.**

### Group 1C (parallel after 1B)

### Task 6: `append` and `vocab` commands

**Files:** Create `src/cmd/append.rs` (replace stub), `tests/cmd_append.rs`.

Behavior: `eventlog append <type> k=v... [--as n] [--dry-run] [--no-strict]`; `EVENTLOG_AS` env fallback; prints the line on stdout; errors map to exit 1 (`Lock(Busy)` → 2). `eventlog vocab [type] [--json]` prints required and optional fields from `Config`; `append --help` epilogue includes the same table (build the help string at runtime with `clap::Command::after_help`).

- [ ] **Step 1:** integration tests with `assert_cmd` in a `tempfile` repo containing `.context/`: `append result ref=x` → stdout starts with `{"seq":1`; `append --dry-run result ref=x` prints the line and leaves the file absent; `append result` (no ref) → exit 1, stderr contains `missing field ref`; `vocab result --json` contains `"fields":["agent","ref"]`; contention: spawn 2 child processes × 25 appends, assert 50 lines, seqs 1..50.
- [ ] **Steps 2–5.**

### Task 7: `view` with follow and filters

Spec section 6 (`view`, `--agent` matching, `--json v:1`).

**Files:** Create `src/cmd/view.rs`, `tests/cmd_view.rs`.

Behavior: one line per event: `seq  TYPE  agent  <summary or msg or key=value>  → ref`, colored per `[view.colors]` (SGR string) when stdout is a tty or `--color always`; `--type a,b`, `--agent x` (matches agent|by|from|to), `--by x`, `--since <rfc3339>`, `--last N`, `--grep s`, `--json` (`{"v":1, ...event}`), `-f` follows with `notify` and a 500 ms poll fallback, printing new lines and skipping malformed ones with a stderr note.

- [ ] **Step 1:** tests on the fixture copied into a temp log: `--last 3` prints 3 lines; `--type ack --json` lines all have `"type":"ack"` and `"v":1`; `--agent doc-worker` includes a line whose only doc-worker field is `by`; `-f` in a child process, append one line, assert it appears within 2 s, kill the child.
- [ ] **Steps 2–5.**

### Task 8: `schema`

**Files:** Create `src/cmd/schema.rs`, `tests/cmd_schema.rs`.

Behavior: `eventlog schema --events` prints a JSON Schema (schemars) for an event line: `seq` integer, `ts` string date-time, `type` string, `prev` optional string, `by` optional string, `agent` optional string, `additionalProperties: {type: string}`. `--output` prints the `--json` row schema (event plus `v`). Default `--events`.

- [ ] **Step 1:** test: output parses as JSON, has `properties.seq.type == "integer"`, `additionalProperties.type == "string"`.
- [ ] **Steps 2–5.**

**Phase 1 exit:** controller runs `eventlog append` and `eventlog view -f` in this repo alongside the scripts for one day; `eventlog verify` passes on this repo's log after the first chained append.

---

## Phase 2: query, views, TUI

### Group 2A (serial)

### Task 9: `query::State` and fold **[radiator: opus]**

Spec section 8, and the allowlist-as-of-seq rule in section 4.

**Files:** Create `tests/query_fold.rs`; Modify `src/query/mod.rs`.

**Interfaces produced:**

```rust
pub struct AgentState { pub name: String, pub model: Option<String>, pub pane: Option<String>, pub phase: Phase, pub spawned_at: u64, pub retired_at: Option<u64>, pub claims: Vec<String> }
pub enum Phase { Spawned, Prompted, Claimed, Progressing, Resulted, Retired }
pub struct ReactorState { pub name: String, pub last_ack_seq: Option<u64>, pub last_ack_ts: Option<String>, pub open_intents: Vec<u64> }
pub struct State {
    pub at: u64,
    pub agents: BTreeMap<String, AgentState>,
    pub claims: Vec<(String /*path glob*/, String /*agent*/)>,   // live claims only
    pub decisions: BTreeMap<String, (String, u64)>,              // key -> (value, seq)
    pub escalations: Vec<Event>,                                 // open: no approval with the same subject after it
    pub intents: Vec<Event>,                                     // intent with no ack for= by the same writer
    pub reactors: BTreeMap<String, ReactorState>,
    pub allowlist: Allowlist,                                    // as of `at`
    pub open_lifecycles: Vec<String>,                            // result/retire/progress for an agent with no spawn, or spawn with no retire at tip
}
pub fn fold(events: &[Event], cfg: &Config) -> State;                        // to tip
pub fn fold_at(events: &[Event], cfg: &Config, at: u64) -> State;            // same fold, stopped at seq `at`
impl State { pub fn claims_for(&self, agent: &str) -> Vec<&str>; pub fn claim_owner(&self, path: &str) -> Option<&str>; pub fn unacked(&self, reactor: &str, on: &[&str], events: &[Event]) -> Vec<u64>; }
```

Claim matching: a claim entry is a literal path or a glob (`globset`); `claim_owner` returns the first live claim whose glob matches. `unacked` = seqs of events with type in `on` and seq greater than `last_ack_seq`.

- [ ] **Step 1:** tests on the fixture: `fold` counts `agents.len()` equal to the number of distinct `spawn` agents; every agent with a `retire` has `Phase::Retired`; `fold_at(events, 100).at == 100` and lists no agent spawned after 100; a synthetic log with `decision key=log-writers value=x:result` at seq 3 gives `fold_at(2).allowlist.permits("x","result") == false` and `fold_at(3)... == true`; `ReactorState.last_ack_seq` compares numerically (fixture with acks `"9"` and `"51"` yields 51).
- [ ] **Steps 2–5.** Then the controller appends `decision key=query-contract value=src/query/mod.rs@<sha>`.

### Group 2B (parallel after Task 9)

### Task 10: `why`

**Files:** Create `src/query/why.rs`, `tests/query_why.rs`.

**Interfaces:** `pub struct WhyReport { pub event: Event, pub causes: Vec<Event>, pub effects: Vec<Event>, pub verdict: String }`, `pub fn why(events: &[Event], cfg: &Config, seq: u64) -> Option<WhyReport>`.

Causes: events this one references via `for`, `for_ack`, `seq_done`, `intent`, and the `origin` writer's latest `result` before it. Effects: events that reference this seq in those fields. Verdict: for a `result`/`decision`: "acted on by <reactor> at seq N (outcome=…)" from the effects, else "not matched by <reactor> (filter: type∉…)" for each reactor in `State.reactors` whose `--on` is unknown → say "no ack references this seq".

- [ ] **Step 1:** tests: on this repo's log copied to fixtures as `tests/fixtures/self-log-2026-09-06.jsonl`: `why(39)` effects include seq 42 and verdict contains `outcome=committed`; `why(33)` verdict contains "no ack".
- [ ] **Steps 2–5.**

### Task 11: `agents`, `state`, `why` commands

**Files:** Create `src/cmd/agents.rs`, `src/cmd/state.rs`, `src/cmd/why.rs`, `tests/cmd_query.rs`.

Behavior: `agents [--at N] [--json]`: table `agent  model  pane  phase  claims  spawned  retired`, then a "flags" section listing `open_lifecycles` and claims of retired agents never released. `state [--at N] [--json]`: sections active agents, open claims, decisions in force, open escalations, open intents, per reactor `last ack seq, age (now − ts), unacked count`. `why <seq> [--json]`: causes, the event, effects, verdict.

- [ ] **Step 1:** tests: `agents --json` rows have `"v":1`; `state --at 20` on fixture lists only agents alive at 20; `why 39` exit 0 and stdout contains `seq 42`.
- [ ] **Steps 2–5.**

### Task 12: `claims` and `open`

**Files:** Create `src/cmd/claims.rs`, `src/cmd/open.rs`, `tests/cmd_claims.rs`.

Behavior: `claims <agent> <base> [head]` runs `git diff --name-only --find-renames <base> <head|HEAD>` plus untracked from `git status --porcelain`, canonicalizes, and lists paths no live claim of `<agent>` covers; exit 1 if any. `check-claims` hidden alias. `open <seq> [--pager]` resolves the event's `ref`, runs `$EDITOR` (or `$PAGER` with `--pager`, default `less`), exit 1 if no ref.

- [ ] **Step 1:** tests in a temp git repo: a claim on `src/**`, a commit touching `src/a.rs` and `docs/b.md` → output lists `docs/b.md` only, exit 1; an untracked `docs/c.md` also listed.
- [ ] **Steps 2–5.**

### Task 13: TUI **[radiator: sonnet]**

Spec section 8.

**Files:** Create `src/tui/views.rs`, `tests/tui_render.rs`; Modify `src/tui/mod.rs`, `src/cmd/tui.rs`.

Behavior: ratatui app; top pane = follow view with the same filters as `view`; bottom pane toggles agents table / state summary; keys from `[keys]`: filter (opens an input line), follow toggle, open (runs `open` on the selected row, suspends the terminal), `w` why (bottom pane shows the `why` report), `tab` pane switch, `q` quit. Re-folds on every new line (fold is cheap at this scale; keep an `events: Vec<Event>` and refold).

- [ ] **Step 1:** render tests with `ratatui::backend::TestBackend`: the follow view shows the last N lines of the fixture; pressing the filter key then typing `ack` and Enter leaves only ack rows; the agents pane shows a retired agent with phase `retired`.
- [ ] **Steps 2–5.**

**Phase 2 exit:** `eventlog tui` replaces `eventlog-view.sh -f` in this repo's coordinator pane (`layout.sh` change is the controller's, recorded as its own `result`).

---

## Phase 3: reactor runtime

### Group 3A (serial)

### Task 14: Reactor loop, lock, baseline, resume, interrupted intents **[radiator: opus]**

Spec section 7 steps 1–3, 5.

**Files:** Create `src/react/lock.rs`, `tests/react_loop.rs`; Modify `src/react/mod.rs`.

**Interfaces produced:**

```rust
pub struct ReactorConfig { pub name: String, pub on: Vec<String>, pub filter: Vec<(String,String)>, pub window: Duration, pub git: bool, pub command: Vec<String>, pub pass_timeout: Duration }
pub struct Reactor { cfg: ReactorConfig, log: Log, config: Config, root: PathBuf }
impl Reactor {
    pub fn new(cfg: ReactorConfig, log: Log, config: Config, root: PathBuf) -> Reactor;
    pub fn run(&mut self) -> anyhow::Result<()>;                 // acquire lock, baseline, then loop: read, find unacked, handle each, watch for new lines
    pub fn handle(&mut self, driving: &Event, dry: bool) -> anyhow::Result<Vec<Event>>;  // steps 4.1-4.7; with dry=true returns the events it would append, writes none
}
pub struct ReactorLock;  // token = {pid, start_time, hostname, boot_id}; live iff pid+start match on this host; reclaim by rename to <dir>.stale.<rand>
impl ReactorLock { pub fn acquire(dir: &Path) -> Result<ReactorLock, LockError>; }
pub fn supervise(cfg: ReactorConfig, ...) -> !;                   // respawn on panic/exit, note per restart, escalate + exit after 5 restarts in 10 minutes
```

`handle` calls `voter::authorize`, `voter::check`, `action::run`, `action::snapshot` from Tasks 15–16; in this task stub them behind a trait `Steps` so the loop is testable with a fake.

- [ ] **Step 1:** tests with a fake `Steps`: empty-ack start writes baseline ack at tip and handles nothing older; a log with own ack `seq_done "9"` and events 10..12 matching `--on` handles exactly 10, 11, 12 in order; an own `intent for=11` with no ack → appends `ack seq_done=11 outcome=interrupted` and an `escalate`, never calls the action; lock: a dir holding a pid that is alive but with a different start time is reclaimed; two threads reclaiming a dead lock → exactly one acquires.
- [ ] **Steps 2–5.**

### Group 3B (parallel after Task 14)

### Task 15: Voter **[radiator: opus]**

Spec section 7 steps 4.1, 4.3, 4.4.

**Files:** Create `src/react/voter.rs`, `tests/react_voter.rs`.

**Interfaces:**

```rust
pub struct Authorized { pub paths: Vec<RelPath>, pub excess: Vec<RelPath> }
pub fn authorize(driving: &Event, state: &State) -> Authorized;   // no `by` -> all its paths; else intersect with state.claims_for(driving.writer())
pub enum Veto { UnclaimedPaths(Vec<RelPath>), LogOrLock(RelPath), ClaimedByOther{path: RelPath, owner: String}, OpenEscalation(u64) }
pub fn check(reactor: &str, auth: &Authorized, state: &State, cfg: &Config) -> Result<(), Veto>;
pub fn veto_binds(events: &[Event], driving_seq: u64, since_seq: u64) -> Option<&Event>;  // any veto with for=driving_seq after since_seq
```

- [ ] **Step 1:** tests: controller `result paths=src/a.rs` → authorized `[src/a.rs]`, no excess; `result by=doc-worker paths=docs,README.md,src/x.rs` with doc-worker claim `docs,README.md,AGENTS.md` → excess `[src/x.rs]` and `check` returns `UnclaimedPaths`; `paths=.context/events.jsonl` → `LogOrLock`; `paths=.context/DECISIONS.md` passes; path claimed by another open agent → `ClaimedByOther`; an open `escalate agent=<reactor>` → `OpenEscalation`; `veto_binds` finds a veto whose `for` names the driving seq even when `intent=` names an older intent.
- [ ] **Steps 2–5.**

### Task 16: Action runner and violation snapshot

Spec section 7 steps 4.5–4.7.

**Files:** Create `src/react/action.rs`, `tests/react_action.rs`.

**Interfaces:**

```rust
pub struct ActionEnv { pub log: PathBuf, pub seq: u64, pub r#type: String, pub agent: String, pub by: String, pub paths: Vec<RelPath>, pub reference: Option<String>, pub resume: u64, pub outcome_file: PathBuf }
pub struct Outcome { pub exit: i32, pub fields: Vec<(String,String)>, pub timed_out: bool }  // fields from outcome file (k=v lines; seq/ts/prev/by dropped), fallback last stdout "outcome=..."
pub fn run(command: &[String], stdin_json: &str, env: &ActionEnv, timeout: Duration) -> anyhow::Result<Outcome>;
pub struct Snapshot { pub head: Option<String>, pub dirty: BTreeSet<String> }
pub fn snapshot(root: &Path) -> anyhow::Result<Snapshot>;        // git rev-parse HEAD, git status --porcelain
pub fn touched(before: &Snapshot, after: &Snapshot, root: &Path) -> BTreeSet<String>;  // files in commits before..after plus dirty delta
pub fn outside(touched: &BTreeSet<String>, authorized: &[RelPath]) -> Vec<String>;
```

Retry: `run` never retries; the loop (Task 14) retries once only if `fields` contains `outcome=retryable`.

- [ ] **Step 1:** tests: a shell script command that writes `outcome=committed\nref=abc` to `$EVENTLOG_OUTCOME_FILE` → `Outcome.fields == [("outcome","committed"),("ref","abc")]`; a command that prints `outcome=skipped` as last stdout line with no file → fallback works; `sleep 5` with 1 s timeout → `timed_out`; in a temp git repo, a command that commits `a.rs` and `b.rs` with authorized `[a.rs]` → `outside == ["b.rs"]`.
- [ ] **Steps 2–5.**

### Group 3C (serial after 3B)

### Task 17: `react` and `react test` commands; switch this repo's reactors

**Files:** Create `src/cmd/react.rs`, `tests/cmd_react.rs`, `.context/bin/commit-action.sh`, `.context/bin/doc-action.sh`; Modify `.context/handoffs/cursor-committer.md` (replace "never stage anything under .context/" with "never stage the log or lock dirs"), `.context/workspace.env` (add `REACTOR_RUNTIME=eventlog`).

Behavior: `eventlog react --as n --on t1,t2 [--filter k=v]... [--window 0s] [--git] -- cmd...` builds `ReactorConfig` and calls `supervise`; `eventlog react test <seq> --as n [--git] -- cmd...` calls `handle(dry=true)` and prints each would-be event. `commit-action.sh` is the `cursor-agent` invocation from `cursor-commit-reactor.sh` lines that build the prompt and run it, staging only `$EVENTLOG_PATHS`, with its HEAD-moved check, writing `outcome=committed ref=<shas>` or `outcome=skipped detail=...` to `$EVENTLOG_OUTCOME_FILE`. `doc-action.sh` likewise wraps the `claude -p` call and writes `outcome=updated files=N` or `outcome=skipped`. Neither script touches the log.

- [ ] **Step 1:** tests: `react test <seq>` against a fixture with a fake `cmd` prints an `intent` and an `ack` line and leaves the log unchanged; `react --as t --on result -- sh -c 'echo outcome=committed'` in a temp repo, then an `append result paths=a.md` from another process → within 3 s the log has `intent`, `ack seq_done=<n> outcome=committed`; a `veto for=<n>` appended before the window closes (use `--window 2s` in this test) → `outcome=vetoed`.
- [ ] **Steps 2–5.** Controller then updates `layout.sh`'s reactor commands to `eventlog react ...` in the two `maintenance` panes (its own `result`), records `claim agent=doc-worker paths=docs,README.md,AGENTS.md`, and runs `teardown.sh` + `layout.sh`.

**Phase 3 exit:** both reactors run under `eventlog react`; one real `result` produces `intent → ack`; `eventlog state` shows zero open intents.

---

## Phase 4: guard, scaffold, skill, release

### Group 4A (parallel after Phase 3)

### Task 18: Guard for three agents **[radiator: opus]**

Spec section 9.

**Files:** Create `src/guard/deny.rs`, `src/cmd/guard.rs`, `tests/guard.rs`, `tests/fixtures/hooks/claude_bash_deny.json`, `claude_edit_deny.json`, `cursor_shell_deny.json`, `codex_bash_deny.json`, `codex_apply_patch_deny.json`, `claude_append_allow.json`, `adaptive_cases.txt`; Modify `src/guard/mod.rs`.

**Interfaces:**

```rust
pub enum Agent { Claude, Cursor, Codex }
pub enum Action { Edit{path: String}, Write{path: String}, Shell{command: String}, Other }
pub fn parse(payload: &str, agent: Option<Agent>) -> Result<(Agent, Action), ParseFail>;  // ParseFail::NotJson -> fail open; ParseFail::UnknownShape -> fail closed
pub fn decide(action: &Action, log_path: &Path) -> Decision;   // Decision::Allow | Decision::Deny(reason)
pub fn is_simple_sanctioned_writer(command: &str) -> bool;      // single simple command, argv[0] in {eventlog, append-event.sh}; false on ; && || | $( ` newline
pub fn install(agent: Agent, root: &Path, bin: &Path) -> anyhow::Result<bool>;  // idempotent; returns true if changed
```

`decide` for Shell: allow if `is_simple_sanctioned_writer`; deny if the command names the log basename (or `EVENTLOG_GUARD_BASENAME` regex) together with any of: `>`/`>|`/`>>`-less redirect into it, `sed -i`, `rm`, `mv`, `truncate`, `tee` without `-a`, `cp` onto it, `perl -i`, `python … open(… 'w')`, `git checkout -- <log>`, `git restore <log>`; deny compound commands naming the log. Deny output: stderr reason, stdout `{"permission":"deny","userMessage":"<reason>"}`, exit 2. `install` writes `.claude/settings.json` PreToolUse entry (matcher `Edit|Write|Bash`), `.cursor/hooks.json` `beforeShellExecution` and `beforeReadFile`-less entry, `.codex/hooks.json` `PreToolUse`, each with `eventlog guard --agent <a>`.

- [ ] **Step 1:** tests: each deny fixture → exit 2 and `permission: deny`; `claude_append_allow.json` (command `append-event.sh result ref=x`) → exit 0; `eventlog append note x=1; : > .context/events.jsonl` → deny; every line of `adaptive_cases.txt` (start with 12: `cat .context/events.jsonl | tee .context/events.jsonl`, `python3 -c "open('.context/events.jsonl','w')"`, `env X=1 rm .context/events.jsonl`, `bash -c 'rm .context/events.jsonl'`, `git checkout -- .context/events.jsonl`, `cp /dev/null .context/events.jsonl`, `truncate -s0 .context/events.jsonl`, `perl -i -pe s/a/b/ .context/events.jsonl`, `sed -i '' d .context/events.jsonl`, `mv .context/events.jsonl x`, `: >| .context/events.jsonl`, `eventlog append a b=c && rm .context/events.jsonl`) → deny; a JSON payload with no known fields → exit 2 "unrecognized payload"; non-JSON stdin → exit 0; `install` twice → second returns false and file unchanged.
- [ ] **Steps 2–5.**

### Task 19: `init`, `protect`, `doctor`

Spec section 6 rows for these three.

**Files:** Create `src/scaffold/doctor.rs`, `tests/scaffold.rs`, `src/scaffold/templates/EVENTLOG.md`, `src/scaffold/templates/eventlog.toml`; Modify `src/scaffold/mod.rs`, `src/cmd/init.rs`, `src/cmd/doctor.rs`, `src/cmd/protect.rs` (stubs from Task 1).

Behavior: `init` creates `.context/events.jsonl` (empty), `.context/EVENTLOG.md` (template updated to subcommands and the `log-writers` grammar), `.context/eventlog.toml`, appends gitignore lines `.context/events.jsonl`, `.context/events.jsonl.lock`, `.context/*.reactor.lock/`, `.context/layout.json` and a `.gitattributes` line `.context/events.jsonl -text`; idempotent. `protect [--off] [--status]` runs `chflags uappnd|nouappnd` on macOS, `chattr +a|-a` on Linux; `--status` exit 0 protected / 1 not. `doctor [--fix] [--protect]` prints `[ OK ]`/`[WARN]`/`[FAIL]` rows: binary on PATH; guard installed per agent (reads the three hook files); protection; unsanctioned writers (fold as-of-seq); strict-rule violations over history; open lifecycles; stale reactor locks; skill stamp vs `--version`; old script symlinks on PATH. `--fix` installs guards for agents whose config dir exists, removes the old symlinks, installs the skill; `--protect` runs protect. Exit 1 on any `[FAIL]`.

- [ ] **Step 1:** tests in a temp repo: `init` twice leaves identical files; `doctor` on a log whose seq 3 is `result by=x` with no grant → `[FAIL] unsanctioned`; after appending `decision key=log-writers value=x:result` at seq 2 (rebuild the fixture) → OK; `protect --status` exit 1 before, 0 after `protect` (skip on CI without permission; mark `#[ignore]` with a reason).
- [ ] **Steps 2–5.**

### Task 20: Skill install and release **[radiator: sonnet]**

**Files:** Create `src/cmd/skill.rs`, `src/cmd/completions.rs`, `tests/skill.rs`, `dist-workspace.toml`, `.github/workflows/release.yml` (generated by `dist init`); Modify `src/skill/mod.rs`.

Behavior: `include_dir!("$CARGO_MANIFEST_DIR/skill")`; `skill install [--dir ~/.claude/skills]` writes `event-log-coordination/` with a `.eventlog-version` file containing the binary version; refuses (exit 1, message) if the existing stamp is newer; `--force` overrides. `completions <shell>` via `clap_complete`. `dist-workspace.toml` copied from `~/Development/Drove/dist-workspace.toml` with the tap and targets unchanged.

- [ ] **Step 1:** tests: install into a temp dir creates `SKILL.md` and the stamp equal to `env!("CARGO_PKG_VERSION")`; a pre-existing stamp `99.0.0` → exit 1; `completions zsh` output contains `_eventlog`.
- [ ] **Steps 2–5.**

### Task 21: Rewrite the skill to subcommands

**Files:** Modify `skill/event-log-coordination/SKILL.md`, `skill/event-log-coordination/README.md`, `skill/event-log-coordination/references/*.md`; Delete `skill/event-log-coordination/scripts/` (its `.gitignore` too).

Behavior: every script name becomes the subcommand from spec section 6; the "Setup" section becomes `eventlog init` + `eventlog doctor --fix`; the reactor section describes `eventlog react` with intent, voter, veto, violation and the outcome file; the "honest limit" section gains the unauthenticated-`--as` statement from spec section 3; `references/reactor-example.sh` becomes `references/reactor-example.md` showing a 15-line action script. Keep the section order. Run the `plain-technical-english` final gate.

- [ ] **Step 1:** test: `grep -rn '\.sh' skill/` returns nothing except inside a fenced block that shows the migration table.
- [ ] **Steps 2–5.**

**Phase 4 exit:** `eventlog doctor` exits 0 in this repo with all three guards installed; `cargo dist plan` succeeds; tag `v0.1.0`; `brew install radiator-engineering/tap/eventlog` works on the user's machine; `layout.sh` and `setup.sh` in the setup skill switch to `eventlog` (separate repo, out of scope here).

---

## Controller checklist per phase

- [ ] Freeze the contract decision for the phase's serial task.
- [ ] Spawn the group; one pane per task; record spawn, prompt, claim.
- [ ] On each `result by=<slug>`: claims check, `cargo test`, diff read, controller `result`, `retire`.
- [ ] Run the phase exit check on this repo; record it as a `note`.
- [ ] Run `cargo mutants --in-diff` on `query`, `react/voter.rs`, `guard/deny.rs` at the end of Phases 2, 3, 4; surviving mutants become a follow-up task with the same claims.
