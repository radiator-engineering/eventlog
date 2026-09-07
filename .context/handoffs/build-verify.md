# Brief: build-verify

You are the worker **build-verify**, spawned by the controller of this repo (see
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

The model contract is frozen (decision key=model-contract): use the public items in src/model/*.rs as they are. Task 1 created stubs for every file you will fill. Task 3 (build-log) landed `Log` (open, read, tail, hash_line) in src/log/mod.rs; use it as it is.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-verify` on every line (decision `log-writers`):

    append-event.sh progress by=build-verify msg="<one line>" ref=<main file>
    append-event.sh result   by=build-verify ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-verify msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 5: Verify

Spec section 3 hash chain.

**Files:** Create `src/log/verify.rs`, `tests/log_verify.rs`, `tests/fixtures/chain_ok.jsonl`, `tests/fixtures/chain_break.jsonl`, `tests/fixtures/chain_unchained_after.jsonl`; Modify `src/cmd/verify.rs` (prints `ok: N events, chain intact` or the failure, exit 1).

**Interfaces produced:** `pub struct VerifyReport { pub checked: u64, pub last_good: u64, pub failure: Option<String> }`, `pub fn verify(log: &Log) -> anyhow::Result<VerifyReport>`.

Rules: pre-chain lines pass until the first `prev`; that line's `prev` must equal hash of the last pre-chain line (or `genesis` if it is line 1); every later line must have `prev` equal to hash of its predecessor; a later line without `prev` is a failure "unchained line at seq N".

- [ ] **Step 1:** tests for each fixture: ok → `failure None`, break → `last_good` is the seq before the break, unchained-after → failure names the seq. Also: stripping every `prev` from `chain_ok` must fail with "unchained" (write the mutation in the test).
- [ ] **Steps 2–5.**
