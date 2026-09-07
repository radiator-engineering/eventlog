# Brief: build-schema

You are the worker **build-schema**, spawned by the controller of this repo (see
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

The model contract is frozen (decision key=model-contract): use the public items in src/model/*.rs as they are. Task 1 created stubs for every file you will fill.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-schema` on every line (decision `log-writers`):

    append-event.sh progress by=build-schema msg="<one line>" ref=<main file>
    append-event.sh result   by=build-schema ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-schema msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 8: `schema`

**Files:** Create `src/cmd/schema.rs`, `tests/cmd_schema.rs`.

Behavior: `eventlog schema --events` prints a JSON Schema (schemars) for an event line: `seq` integer, `ts` string date-time, `type` string, `prev` optional string, `by` optional string, `agent` optional string, `additionalProperties: {type: string}`. `--output` prints the `--json` row schema (event plus `v`). Default `--events`.

- [ ] **Step 1:** test: output parses as JSON, has `properties.seq.type == "integer"`, `additionalProperties.type == "string"`.
- [ ] **Steps 2–5.**
