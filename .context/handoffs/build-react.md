# Brief: build-react

You are the worker **build-react**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: Claude (opus) via claudewho-radiator.

Read first: `docs/superpowers/plans/2026-09-06-eventlog-cli.md` sections
"Global constraints" and "File structure", and the spec sections your task cites
in `docs/superpowers/specs/2026-09-06-event-log-cli-design.md`.

## Rules

Do only this task. Touch only the files it lists (plus `Cargo.lock`, which
cargo maintains). Write the failing test first, run it, make it pass, then run
`cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt`.
Do not `git commit`. Do not edit `Cargo.toml`, `src/lib.rs`, `src/main.rs`
or `src/cli.rs` unless this task lists them; if you need a change there,
append `escalate` (below) and stop. Do not start subagents. Never edit,
truncate or `rm` `.context/events.jsonl`.

## Contract

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are; src/log/mod.rs provides Log and Lock, src/log/append.rs provides append. Other workers are editing other files right now: never touch a file outside your list; if the crate fails to build because of someone else's file, append escalate and wait. Tasks 15 (src/react/voter.rs) and 16 (src/react/action.rs) are being built in parallel by other workers: put the calls to them behind the `Steps` trait as the task says, and do not create or edit those two files. Your public items in src/react/mod.rs become the react contract for Task 17.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-react` on every line (decision `log-writers`):

    append-event.sh progress by=build-react msg="<one line>" ref=<main file>
    append-event.sh result   by=build-react ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-react msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

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
