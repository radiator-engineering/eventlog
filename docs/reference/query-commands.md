# Query commands: `agents`, `state`, `why`

Status: implemented. All three fold the log with [`src/query`](query-module.md)
and print a view of the result: `agents` is a per-agent lifecycle table,
`state` is a full snapshot, and `why` explains one event. Shared loading and
formatting helpers live in `src/cmd/agents.rs`.

```sh
eventlog agents [--at <seq>] [--json]
eventlog state  [--at <seq>] [--json]
eventlog why <seq> [--json]
```

`--at <seq>` folds the log only up to that sequence number, using
[`query::fold_at`](query-module.md); without it, `agents` and `state` fold to
the tip. `why` always reads the whole log, since causes and effects can fall
on either side of the seq it explains.

## `agents`

Prints one row per agent that has ever appeared in a `spawn` line:

```
agent  model  pane  phase  claims  spawned  retired
```

`phase` is one of `spawned`, `prompted`, `claimed`, `progressing`, `resulted`,
`retired` (see [`Phase`](query-module.md#agentstate-and-phase)). `claims` is
the agent's live claim globs, comma-separated. `retired` is `-` for an agent
still active.

After the table, a `flags` section lists:

- `open_lifecycles` — agent names from `state.open_lifecycles` (an orphaned
  or still-running lifecycle).
- `unreleased claims` — claims still owned by an agent whose `retired_at` is
  set: a `retire` line that never gave up its claims.

`--json` prints one row per agent instead, each `{"v":1,"agent":...,"phase":...,
"claims":...,"spawned":...}` plus `model`, `pane`, and `retired` when present.
The `flags` section has no JSON form; read `open_lifecycles` and unreleased
claims from `state` directly if you need them as JSON.

## `state`

Prints a full snapshot in six sections:

- **active agents** — every agent with no `retired_at`, with its phase and
  spawn seq.
- **open claims** — `state.claims`: agent and path glob.
- **decisions in force** — `state.decisions`: key, value, and the seq that
  set it.
- **open escalations** / **open intents** — `state.escalations` and
  `state.intents`, one formatted event line each (see [View: text
  format](view.md#text-format) for the line format).
- **reactors** — one line per reactor: `last_ack_seq`, `age` (now minus
  `last_ack_ts`, as `Nd`/`Nh`/`Nm`/`Ns`), and `unacked` — the count from
  `state.unacked(reactor, ["result", "decision"], events)`, i.e. `result` and
  `decision` events above the reactor's last ack.

`--json` prints one object with `active_agents`, `open_claims`, `decisions`,
`open_escalations`, `open_intents`, and `reactors` (each reactor row also
carries the `unacked` seq list, not just the count).

## `why`

`why <seq>` explains one event:

- **causes** — events this one points back to: any of its `for`, `for_ack`,
  `seq_done`, or `intent` fields resolved to the event at that seq
  ([`Event::seq_ref`]), plus, if the event has an `origin` field, the latest
  `result` from that writer strictly before it.
- **event** — the event itself.
- **effects** — every event elsewhere in the log whose `for`, `for_ack`,
  `seq_done`, or `intent` field points back at this seq.
- **verdict** — set only when the event is a `result` or `decision`: if an
  effect is an `ack` with `seq_done` equal to this seq, the verdict is that
  ack's `outcome` (e.g. `committed`, `skipped`); otherwise empty.

Text output prints causes, the event, and effects as formatted lines, then
the verdict if non-empty. `--json` prints one object: `causes` and `effects`
as arrays of `--json`-shaped event rows, `event` as one row, and `verdict`
only when non-empty. Exit code is 1, with `no event at seq <seq>` on stderr,
if the seq isn't in the log.

## Tests

`tests/cmd_query.rs` runs the built binary against
`tests/fixtures/self-log-2026-09-06.jsonl` and checks: every `agents --json`
row has `"v":1` and an `agent` field; `state --at 20` lists agents alive at
seq 20 (`cursor-committer`, `doc-worker`) but not ones spawned later
(`build-scaffold`, `logact-deep-read`); `why 39` exits 0 and prints `seq 42`
(the ack that resolved it). Run them with:

```sh
cargo test
```

## See also

- [Query module](query-module.md) — `State`, `fold`, `fold_at`, and the
  methods these commands read from it.
- [View](view.md) — the formatted event line shared by `state`'s
  escalations/intents sections and `why`.
- [`eventlog` command list](eventlog-cli-surface.md)
