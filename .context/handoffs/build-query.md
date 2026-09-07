# Brief: build-query

You are the worker **build-query**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: Claude (opus) via claudewho-radiator.

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

The model contract is frozen (decision key=model-contract): use the public items in src/model/*.rs as they are. src/log/mod.rs provides `Log::open/read/tail/hash_line`; use it as it is. Your public items in src/query/mod.rs become the query contract for Tasks 10-13, so match the Interfaces block exactly. `globset` is in Cargo.toml. For the "this repo's log" fixture, copy .context/events.jsonl to tests/fixtures/self-log-2026-09-06.jsonl (read-only copy; never write to the original).

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-query` on every line (decision `log-writers`):

    append-event.sh progress by=build-query msg="<one line>" ref=<main file>
    append-event.sh result   by=build-query ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-query msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 9: `query::State` and fold **[radiator: opus]**

Spec section 8, and the allowlist-as-of-seq rule in section 4.

**Files:** Create `tests/query_fold.rs`; Modify `src/query/mod.rs`.

**Interfaces produced:**

```rust
pub struct AgentState { pub name: String, pub model: Option<String>, pub pane: Option<String>, pub phase: Phase, pub spawned_at: u64, pub retired_at: Option<u64>, pub claims: Vec<String> }
pub enum Phase { Spawned, Prompted, Claimed, Progressing, Resulted, Retired }
pub struct ReactorState { pub name: String, pub last_ack_seq: Option<u64>, pub last_ack_ts: Option<String>, pub open_intents: Vec<u64> }
pub struct State {
    pub at: u64,
    pub agents: BTreeMap<String, AgentState>,
    pub claims: Vec<(String /*path glob*/, String /*agent*/)>,   // live claims only
    pub decisions: BTreeMap<String, (String, u64)>,              // key -> (value, seq)
    pub escalations: Vec<Event>,                                 // open: no approval with the same subject after it
    pub intents: Vec<Event>,                                     // intent with no ack for= by the same writer
    pub reactors: BTreeMap<String, ReactorState>,
    pub allowlist: Allowlist,                                    // as of `at`
    pub open_lifecycles: Vec<String>,                            // result/retire/progress for an agent with no spawn, or spawn with no retire at tip
}
pub fn fold(events: &[Event], cfg: &Config) -> State;                        // to tip
pub fn fold_at(events: &[Event], cfg: &Config, at: u64) -> State;            // same fold, stopped at seq `at`
impl State { pub fn claims_for(&self, agent: &str) -> Vec<&str>; pub fn claim_owner(&self, path: &str) -> Option<&str>; pub fn unacked(&self, reactor: &str, on: &[&str], events: &[Event]) -> Vec<u64>; }
```

Claim matching: a claim entry is a literal path or a glob (`globset`); `claim_owner` returns the first live claim whose glob matches. `unacked` = seqs of events with type in `on` and seq greater than `last_ack_seq`.

- [ ] **Step 1:** tests on the fixture: `fold` counts `agents.len()` equal to the number of distinct `spawn` agents; every agent with a `retire` has `Phase::Retired`; `fold_at(events, 100).at == 100` and lists no agent spawned after 100; a synthetic log with `decision key=log-writers value=x:result` at seq 3 gives `fold_at(2).allowlist.permits("x","result") == false` and `fold_at(3)... == true`; `ReactorState.last_ack_seq` compares numerically (fixture with acks `"9"` and `"51"` yields 51).
- [ ] **Steps 2–5.** Then the controller appends `decision key=query-contract value=src/query/mod.rs@<sha>`.
