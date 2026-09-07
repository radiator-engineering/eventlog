# Brief: build-voter

You are the worker **build-voter**, spawned by the controller of this repo (see
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

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are; src/log/mod.rs provides Log and Lock, src/log/append.rs provides append. Other workers are editing other files right now: never touch a file outside your list; if the crate fails to build because of someone else's file, append escalate and wait. Task 14 (src/react/mod.rs) is being built in parallel; do not edit it. voter.rs imports only model and query.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-voter` on every line (decision `log-writers`):

    append-event.sh progress by=build-voter msg="<one line>" ref=<main file>
    append-event.sh result   by=build-voter ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-voter msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

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
