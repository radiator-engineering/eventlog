# Query module: `src/query`

Status: `fold` and `fold_at` are implemented, with `State` and its lookup
methods. Spec section 8, and the allowlist-as-of-seq rule in [Why the model
layer is frozen as a contract](../explanation/model-contract-precedence.md#why-the-evaluation-order-matters).

```rust
pub fn fold(events: &[Event], cfg: &Config) -> State;
pub fn fold_at(events: &[Event], cfg: &Config, at: u64) -> State;
```

`fold` and `fold_at` are the same fold: `fold` runs it to the tip (the
highest `seq` in `events`), and `fold_at` stops after the line with
`seq == at`. Lines with a higher `seq` are never read, so nothing later in
the log can change what `fold_at` reports for an earlier point. `fold(events,
cfg)` is `fold_at(events, cfg, tip)`.

## `State`

```rust
pub struct State {
    pub at: u64,
    pub agents: BTreeMap<String, AgentState>,
    pub claims: Vec<(String, String)>,           // (path glob, agent) — live claims only
    pub decisions: BTreeMap<String, (String, u64)>, // key -> (value, the seq that set it)
    pub escalations: Vec<Event>,
    pub intents: Vec<Event>,
    pub reactors: BTreeMap<String, ReactorState>,
    pub allowlist: Allowlist,
    pub open_lifecycles: Vec<String>,
}
```

| Field | Meaning |
|---|---|
| `at` | The `seq` this fold stopped at. |
| `agents` | Every agent that has appeared in a `spawn` line, keyed by name. |
| `claims` | Live claims only, in the order they were claimed. A `retire` or a re-`spawn` drops an agent's claims. |
| `decisions` | The latest value for each decision `key`, and the `seq` that set it. |
| `escalations` | `escalate` lines with no `approval` naming the same subject after them. |
| `intents` | `intent` lines with no `ack for=<that seq>` by the same writer after them. |
| `reactors` | One `ReactorState` per agent that has written `ack`, `intent`, or `veto`. |
| `allowlist` | The write allowlist as of `at` — see below. |
| `open_lifecycles` | Agents with an unfinished lifecycle: see below. |

### `AgentState` and `Phase`

```rust
pub struct AgentState {
    pub name: String,
    pub model: Option<String>,
    pub pane: Option<String>,
    pub phase: Phase,
    pub spawned_at: u64,
    pub retired_at: Option<u64>,
    pub claims: Vec<String>,
}

pub enum Phase { Spawned, Prompted, Claimed, Progressing, Resulted, Retired }
```

`Phase` is ordered, and the fold only moves an agent forward through it — a
`progress` line can't move an agent from `Resulted` back to `Progressing`.
The one exception is a re-`spawn`: it starts the agent's lifecycle over at
`Phase::Spawned`, with a fresh `spawned_at` and `retired_at` reset to `None`.

### `ReactorState`

```rust
pub struct ReactorState {
    pub name: String,
    pub last_ack_seq: Option<u64>,
    pub last_ack_ts: Option<String>,
    pub open_intents: Vec<u64>,
}
```

`last_ack_seq` is the highest `seq_done` this reactor has acked, compared as
an integer — `seq_done` is stored as a JSON string on disk, so a naive string
compare would rank `"9"` above `"51"`. `open_intents` lists the seqs of this
reactor's `intent` lines with no matching `ack for=` after them.

### The allowlist is evaluated as of `at`

`state.allowlist` starts from `cfg.writers` and replays every `decision
key=log-writers` line up to `at`, in order. A `decision` after `at` never
affects it, so revoking a writer's permission later never turns an earlier,
legitimate line into a breach when you ask what the allowlist looked like at
that earlier `seq`.

### `open_lifecycles`

An agent name goes in `open_lifecycles` if either:

- a `result`, `retire`, or `progress` line names it before any `spawn` does
  (an orphaned lifecycle), or
- it was spawned but has no `retire` by `at` (a lifecycle still running).

Lines with no `agent` field, including the controller's own, are never a
lifecycle and never appear here.

## Methods on `State`

```rust
impl State {
    pub fn claims_for(&self, agent: &str) -> Vec<&str>;
    pub fn claim_owner(&self, path: &str) -> Option<&str>;
    pub fn unacked(&self, reactor: &str, on: &[&str], events: &[Event]) -> Vec<u64>;
}
```

| Method | Behavior |
|---|---|
| `claims_for(agent)` | Every live claim glob owned by `agent`, in the order it was claimed. |
| `claim_owner(path)` | The agent whose claim covers `path`: the first live claim that is a literal match, a glob match (via `globset`), or a directory prefix of `path` (a claim on `src/model` covers `src/model/mod.rs`). |
| `unacked(reactor, on, events)` | The seqs of `events` with a type in `on`, above `reactor`'s `last_ack_seq` and no higher than `self.at`. |

## Fixture and tests

`tests/fixtures/self-log-2026-09-06.jsonl` is a read-only copy of this
repo's own `.context/events.jsonl`, used so the fold is tested against a
real coordination log rather than only hand-built lines. `tests/query_fold.rs`
covers claim accumulation, phase advancement through a retire and a
re-spawn, numeric `last_ack_seq` comparison, and allowlist replay through a
`log-writers` decision. Run them with:

```sh
cargo test
```

## See also

- [Log module](log-module.md) — `Log::open` and `Log::read`, which produce
  the `Event` slice this module folds.
- [Model contract](model-contract.md) — `Event`, `Config`, and `Allowlist`,
  used as-is by this module.
- [Why the model layer is frozen as a contract](../explanation/model-contract-precedence.md) —
  why the allowlist is replayed in `seq` order instead of checked against
  one fixed table.
