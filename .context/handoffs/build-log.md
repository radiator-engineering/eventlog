# Brief: build-log

You are the worker **build-log**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: Claude (sonnet) via claudewho-radiator.

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

The model contract is frozen (decision key=model-contract): use the public items in src/model/*.rs as they are. Task 1 created stubs for every file you will fill.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-log` on every line (decision `log-writers`):

    append-event.sh progress by=build-log msg="<one line>" ref=<main file>
    append-event.sh result   by=build-log ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-log msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

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

- [ ] **Step 1:** tests: `hash_line(b"abc\r\n") == hash_line(b"abc")`; `tail` on the fixture `tests/fixtures/drove-events.jsonl` returns `last_seq == 287`; `tail` on a file whose last line is `{"seq":5,"ts":"...` (cut) returns `torn == Some(n)`; lock: acquire twice in one process, second returns `Busy` within 200 ms; write a lock dir with pid 999999 (dead), acquire succeeds.
- [ ] **Step 2–5** as above.
