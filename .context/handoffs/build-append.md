# Brief: build-append

You are the worker **build-append**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: cursor-agent, auto model.

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

The model contract is frozen (decision key=model-contract): use the public items in src/model/*.rs as they are. Task 1 created stubs for every file you will fill. Task 3 (build-log) landed `Log`, `Tail`, `ReadReport`, `Lock` in src/log/mod.rs and src/log/lock.rs; use them as they are.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-append` on every line (decision `log-writers`):

    append-event.sh progress by=build-append msg="<one line>" ref=<main file>
    append-event.sh result   by=build-append ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-append msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

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
