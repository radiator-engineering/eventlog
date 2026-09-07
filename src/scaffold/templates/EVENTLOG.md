# Coordination event log

`.context/events.jsonl` is the append-only, ordered truth for multi-agent
work in this repo. One JSON object per line. It is **not** a transcript — it
holds small coordination facts and points at bigger artifacts by path.

## Rules

- **Append-only.** Lines are never edited or deleted. The log up to event N
  is exactly what the system knew at event N.
- **Small events.** No model output, no file contents. Reference artifacts by
  path: `ref=.context/handoffs/foo.md`.
- **Sanctioned writers.** Only the controller writes by default. Any other
  writer must carry `by=<name>` and be named in a `decision key=log-writers`.

## Event shape

Every line has `seq` (monotonic), `ts` (UTC), `type`, plus type-specific fields:

| type       | fields (besides seq/ts/type)                    | meaning                          |
|------------|--------------------------------------------------|----------------------------------|
| `spawn`    | agent, model, runtime, tab, pane, workspace, role, kind | a worker was launched     |
| `prompt`   | agent, ref                                        | a delegation packet was sent     |
| `message`  | from, to, subject, ref                            | inter-agent routing              |
| `drain`    | agent                                             | inbox/turn processed             |
| `result`   | agent, ref, verdict                               | worker reported; artifact at ref |
| `decision` | key, value, ref                                   | a choice others must follow      |
| `escalate` | subject, ref                                      | queued for human approval        |
| `approval` | subject, by, decision                             | human decided                    |
| `retire`   | agent, disposition                                | worker tab closed                |

Parallel-work events:

| type        | fields (besides seq/ts/type)                     | meaning                                        |
|-------------|--------------------------------------------------|------------------------------------------------|
| `claim`     | agent, paths (comma-separated globs)             | the files this worker owns                     |
| `progress`  | agent, msg, ref                                  | short note while a worker runs                 |
| `seam`      | agents, subject, ref                             | a cross-worker dependency found mid-work       |
| `violation` | agent, paths                                     | worker changed files outside its claim         |
| `ack`       | by, seq_done, outcome, ref                       | a reactor acted on event seq_done              |
| `note`      | msg                                              | reactor or operator note                       |
| `intent`    | by, for, paths                                     | reactor declared intent before acting          |
| `veto`      | by, for, reason                                    | reactor blocked an action                      |
| `observed`  | by, for, paths                                     | unclaimed files turned dirty while a reactor acted (no blame) |

Any line not written by the controller carries `by=<writer>`. Reactors resume
from their own `ack` lines, never a side file.

## `decision key=log-writers`

The value `controller-plus-reactors` keeps the built-in allowlist (controller
writes coordination types; any `by=`-tagged reactor writes `ack`, `note`,
`escalate`, `violation`, `intent`, `veto`, `result`, `progress`, …).

Any other value is `name:type1|type2;name2:type3` and **replaces the whole
map** — revoking a writer is a new decision that omits it. `controller`
always keeps `spawn`, `prompt`, `claim`, `decision`, `retire` and `approval`.

Example: `decision key=log-writers value=build-worker:result|progress` lets
`build-worker` append `result` and `progress` with `by=build-worker`.

## Commands

| command | purpose |
|---------|---------|
| `eventlog append <type> k=v …` | validate and append one line |
| `eventlog view [-f]` | colored aligned rows; `-f` follows |
| `eventlog verify` | walk the hash chain |
| `eventlog agents` / `state` / `why` | folded views |
| `eventlog claims <agent> <base> [head]` | claim coverage vs git diff |
| `eventlog react --as <name> --on t1,t2 -- cmd…` | reactor runtime |
| `eventlog guard` / `guard install` | hook guard for Claude, Cursor, Codex |
| `eventlog init` | create this scaffold |
| `eventlog doctor [--fix] [--protect]` | diagnose setup |
| `eventlog protect [--off] [--status]` | OS append-only on the log |
| `eventlog skill install` | install the coordination skill |

Run `eventlog vocab` for required and optional fields per type.

## Read it

```bash
eventlog view -f
tail -f .context/events.jsonl
jq -c 'select(.type=="decision")' .context/events.jsonl
eventlog claims reviewer main
```
