# Brief: build-query-cmds

You are the worker **build-query-cmds**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: cursor-agent, auto model.

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

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are. src/log/mod.rs provides Log::open/read/tail/hash_line. Other workers are editing src/log/append.rs and src/cmd/view.rs right now: never touch those, and if the crate fails to build because of them, append escalate and wait rather than editing their files. The fixture tests/fixtures/self-log-2026-09-06.jsonl is this repo's log frozen at seq 125; seq 39 is a controller result acked by seq 42, seq 33 is a worker result with no ack. src/query/why.rs (Task 10) provides `WhyReport` and `why`; use them as they are. You may add fields to AgentsArgs, StateArgs and WhyArgs in src/cli.rs (targeted edits).

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-query-cmds` on every line (decision `log-writers`):

    append-event.sh progress by=build-query-cmds msg="<one line>" ref=<main file>
    append-event.sh result   by=build-query-cmds ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-query-cmds msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 11: `agents`, `state`, `why` commands

**Files:** Create `src/cmd/agents.rs`, `src/cmd/state.rs`, `src/cmd/why.rs`, `tests/cmd_query.rs`.

Behavior: `agents [--at N] [--json]`: table `agent  model  pane  phase  claims  spawned  retired`, then a "flags" section listing `open_lifecycles` and claims of retired agents never released. `state [--at N] [--json]`: sections active agents, open claims, decisions in force, open escalations, open intents, per reactor `last ack seq, age (now − ts), unacked count`. `why <seq> [--json]`: causes, the event, effects, verdict.

- [ ] **Step 1:** tests: `agents --json` rows have `"v":1`; `state --at 20` on fixture lists only agents alive at 20; `why 39` exit 0 and stdout contains `seq 42`.
- [ ] **Steps 2–5.**
