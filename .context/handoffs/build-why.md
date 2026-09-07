# Brief: build-why

You are the worker **build-why**, spawned by the controller of this repo (see
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

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are. src/log/mod.rs provides Log::open/read/tail/hash_line. Other workers are editing src/log/append.rs and src/cmd/view.rs right now: never touch those, and if the crate fails to build because of them, append escalate and wait rather than editing their files. The fixture tests/fixtures/self-log-2026-09-06.jsonl is this repo's log frozen at seq 125; seq 39 is a controller result acked by seq 42, seq 33 is a worker result with no ack.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-why` on every line (decision `log-writers`):

    append-event.sh progress by=build-why msg="<one line>" ref=<main file>
    append-event.sh result   by=build-why ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-why msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 10: `why`

**Files:** Create `src/query/why.rs`, `tests/query_why.rs`.

**Interfaces:** `pub struct WhyReport { pub event: Event, pub causes: Vec<Event>, pub effects: Vec<Event>, pub verdict: String }`, `pub fn why(events: &[Event], cfg: &Config, seq: u64) -> Option<WhyReport>`.

Causes: events this one references via `for`, `for_ack`, `seq_done`, `intent`, and the `origin` writer's latest `result` before it. Effects: events that reference this seq in those fields. Verdict: for a `result`/`decision`: "acted on by <reactor> at seq N (outcome=…)" from the effects, else "not matched by <reactor> (filter: type∉…)" for each reactor in `State.reactors` whose `--on` is unknown → say "no ack references this seq".

- [ ] **Step 1:** tests: on this repo's log copied to fixtures as `tests/fixtures/self-log-2026-09-06.jsonl`: `why(39)` effects include seq 42 and verdict contains `outcome=committed`; `why(33)` verdict contains "no ack".
- [ ] **Steps 2–5.**
