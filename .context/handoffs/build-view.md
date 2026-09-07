# Brief: build-view

You are the worker **build-view**, spawned by the controller of this repo (see
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

The model contract is frozen (decision key=model-contract): use the public items in src/model/*.rs as they are. Task 1 created stubs for every file you will fill. Task 3 (build-log) landed `Log::read` in src/log/mod.rs; use it as it is. Append is being built in parallel; for the -f test, append a line to the temp log with plain file append (std::fs OpenOptions), not the CLI.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-view` on every line (decision `log-writers`):

    append-event.sh progress by=build-view msg="<one line>" ref=<main file>
    append-event.sh result   by=build-view ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-view msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 7: `view` with follow and filters

Spec section 6 (`view`, `--agent` matching, `--json v:1`).

**Files:** Create `src/cmd/view.rs`, `tests/cmd_view.rs`.

Behavior: one line per event: `seq  TYPE  agent  <summary or msg or key=value>  → ref`, colored per `[view.colors]` (SGR string) when stdout is a tty or `--color always`; `--type a,b`, `--agent x` (matches agent|by|from|to), `--by x`, `--since <rfc3339>`, `--last N`, `--grep s`, `--json` (`{"v":1, ...event}`), `-f` follows with `notify` and a 500 ms poll fallback, printing new lines and skipping malformed ones with a stderr note.

- [ ] **Step 1:** tests on the fixture copied into a temp log: `--last 3` prints 3 lines; `--type ack --json` lines all have `"type":"ack"` and `"v":1`; `--agent doc-worker` includes a line whose only doc-worker field is `by`; `-f` in a child process, append one line, assert it appears within 2 s, kill the child.
- [ ] **Steps 2–5.**
