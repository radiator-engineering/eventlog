# Brief: build-append-cmd

You are the worker **build-append-cmd**, spawned by the controller of this repo (see
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

The model contract is frozen (decision key=model-contract): use the public items in src/model/*.rs as they are. src/log/mod.rs provides `Log::open/read/tail/hash_line`; use it as it is. src/log/append.rs (Task 4) provides `AppendRequest`, `AppendError`, `StrictContext`, `append`; use them as they are, and implement `StrictContext` for `query::State` (Task 9, src/query/mod.rs) in src/cmd/append.rs. The command stub is src/cmd/append.rs; `vocab` lives in src/cmd/vocab.rs (also a stub you own).

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-append-cmd` on every line (decision `log-writers`):

    append-event.sh progress by=build-append-cmd msg="<one line>" ref=<main file>
    append-event.sh result   by=build-append-cmd ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-append-cmd msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 6: `append` and `vocab` commands

**Files:** Modify `src/cmd/append.rs`, `src/cmd/vocab.rs` (replace stubs); Create `tests/cmd_append.rs`.

Behavior: implement `log::append::StrictContext` for `query::State` in `src/cmd/append.rs` (this is the only place `log` state meets `query` state). `eventlog append <type> k=v... [--as n] [--dry-run] [--no-strict]` folds the log once and passes the state as the context; `EVENTLOG_AS` env fallback; prints the line on stdout; errors map to exit 1 (`Lock(Busy)` → 2). `eventlog vocab [type] [--json]` prints required and optional fields from `Config`; `append --help` epilogue includes the same table (build the help string at runtime with `clap::Command::after_help`).

- [ ] **Step 1:** integration tests with `assert_cmd` in a `tempfile` repo containing `.context/`: `append result ref=x` → stdout starts with `{"seq":1`; `append --dry-run result ref=x` prints the line and leaves the file absent; `append result` (no ref) → exit 1, stderr contains `missing field ref`; `vocab result --json` contains `"fields":["agent","ref"]`; contention: spawn 2 child processes × 25 appends, assert 50 lines, seqs 1..50.
- [ ] **Steps 2–5.**
