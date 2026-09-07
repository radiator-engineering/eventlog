---
name: event-log-coordination
description: >-
  Use when several coding agents share one repo and need one ordered record of
  who was spawned, what they own, what they reported and what was decided:
  spawning workers, claiming files, accepting results, recording decisions or
  approvals, or running a committer that acts on log events. Also use when a
  repo has a .context/events.jsonl, when the eventlog guard blocks a command,
  or when a reactor duplicates or skips work.
metadata:
  author: jjm@radiator.live
---

# Event-log coordination

The `eventlog` CLI keeps an append-only JSONL log at `.context/events.jsonl`.
The controller is the only writer. Workers report back; the controller
records. Reactors act on events and record their own `ack`. Every event is
small and points at artifacts by `ref=`.

**You run every command below. The user never does.** On first use in a
session run `eventlog doctor --fix` (idempotent). It installs the guard hook
for each agent config dir present (`.claude`, `.cursor`, `.codex`), removes
old script symlinks, and installs this skill. It never protects the log;
that is `eventlog protect`, run only when the user says so.

## Command shapes that work

Flags go **before** the type. Fields are `key=value`.

```sh
eventlog init                                   # log, EVENTLOG.md, eventlog.toml, gitignore lines; never overwrites
eventlog setup preview                          # show reusable-owned configuration changes, writes nothing
eventlog setup apply                            # init non-destructively and create .context/eventlog-setup.toml
eventlog setup upgrade                          # same validation; refuses a customized owned setup file
eventlog append spawn agent=t2 model=sonnet role=impl
eventlog append claim agent=t2 paths=src/api,docs/index.md
eventlog append prompt agent=t2 ref=.context/handoffs/t2.md
eventlog append result agent=t2 ref=.context/handoffs/t2.md paths=src/api/ping.rs summary="add ping"
eventlog append retire agent=t2 disposition=accepted
eventlog append decision key=agent-topology value=parallel-worktrees ref=.context/DECISIONS.md
eventlog append --dry-run --as committer note msg=check   # validate without writing
```

`eventlog vocab [type]` prints the fields each type accepts. The 18 types are
`spawn prompt message drain result decision escalate approval retire claim
progress seam violation observed ack note intent veto`. `append` rejects a field
the vocabulary does not list. Declare a new type or field in
`.context/eventlog.toml`:

```toml
[vocabulary.deploy]
fields = ["env"]        # required
optional = ["ref"]
```

## Rules `append` enforces

| Rule | What fails |
|---|---|
| `open-spawn` | `result`, `progress`, `claim`, `retire` for an agent with no open `spawn` |
| `claim-path-missing` | a claim path (or glob) that matches nothing in the repo |
| `claim-conflict` | a path another open agent already claims |
| `open-escalation` | any append while an `escalate` for the writer is open |
| `by does not match writer` | the controller passing `by=`; reactors pass `--as` instead |
| unknown type / field | anything not in `eventlog vocab` |

Paths are repo-relative: no `/`, `~`, or `..`. `--no-strict` is for repair
only.

## Worker lifecycle

1. Land any shared foundation first, then fan out.
2. One worktree per worker (`git worktree add .worktrees/<name> -b <name> main`).
3. Brief in `.context/handoffs/<name>.md`: the task, its paths, and the line
   "do not append to the log". `spawn`, `claim`, `prompt`.
4. On report, in the worker's worktree: `eventlog claims <name> main` lists
   changed files no claim covers (exit 1 on a gap). Record a gap as
   `violation agent=<name> paths=<list>`.
5. `result` with `paths=`, then `retire`. A `seam agents=a,b subject=...`
   the moment a report names a cross-worker dependency.

## Reactor

A reactor acts on matching events with no prompt. The runtime owns lock,
resume, intent, voter, action, violation check and ack; you supply the
action script. Drive a committer on `result`, which carries `paths=`
(`decision` has no `paths` field):

```sh
eventlog react --as committer --on result --git -- bash .context/bin/commit-action.sh
eventlog react test <seq> --as committer --git -- bash .context/bin/commit-action.sh   # dry run
```

## Reusable setup, actions, and lifecycle

For a new repository, run `eventlog setup preview`, inspect the output, then
run `eventlog setup apply`. The project-managed `.context/eventlog-setup.toml` declares
the committer/doc-worker identities, default models (`composer-2.5-fast` and
`claude-sonnet`), timeouts, documentation roots, and an argv-form docs command.
`setup` never rewrites an existing `Drovefile`, restarts a reactor, removes a
lock/history file, or advances a checkpoint. `upgrade` is intentionally
conservative: it preserves valid project edits and fails before writing when
the configuration is malformed.

Use argv directly in a Drove hook; do not concatenate shell strings:

```python
on_start = ["eventlog", "lifecycle", "start", agent, "--model", model,
            "--paths", paths]
on_stop = ["eventlog", "lifecycle", "stop", agent]
```

`lifecycle start` is idempotent and restores its own claims after a prior
`stop`; it leaves every other agent's claims intact. The supervisor invokes it
as the controller, so do not run it from a worker brief.

Package reactor actions as direct argv too:

```sh
eventlog react --as committer --on result --git -- eventlog action commit
eventlog react --as doc-worker --on ack --filter by=committer --filter outcome=committed --git -- eventlog action docs
```

`action commit` stages and commits only validated `EVENTLOG_PATHS`; it rejects
target paths that were already staged and keeps unrelated staged work intact.
`action docs` executes `[docs].command` as argv (no shell interpolation) and
reports the added, deleted, or modified files below `[docs].roots`, including
files already dirty before the command. Set `docs.command` to a local stub in
tests. An empty command or a failing model command is a failed action, never a
successful no-op. Under a reactor (when `EVENTLOG_LOG` and `EVENTLOG_REF` are
set), it appends one `result` attributed to `[docs].identity`; when that result
comes back through the doc worker it is skipped, preventing the doc-result
loop. Native `react` still owns intent/ack/resume handling.

Sanction it once: `decision key=log-writers value=controller-plus-reactors`.
Run it in a terminal pane that outlives any agent's turn, and record its
`spawn` and `claim`. Details: `references/log-reactors.md`; script:
`references/reactor-example.md`.

## Reading the log

`eventlog view --last 20`, `view -f`, `view --since <seq|rfc3339>`,
`view --grep <text>`, `state`, `agents`, `why <seq>`, `open <seq>`, `tui`.

**Run any read that names the log file as its own command.** The guard
denies a compound command (`;`, `&&`, `|`, `$(`, newline) that names
`events.jsonl`, even a read. It allows `eventlog view --last 10 | grep x`
and denies `jq . .context/events.jsonl | grep x`. It also denies any `Edit` or
`Write` on the log and any mutating shell shape naming it (`>`, `sed -i`,
`rm`, `mv`, `git checkout`). It parses Claude, Cursor and Codex payloads and
fails open only on non-JSON. `eventlog protect` adds the OS append-only flag
for anything the hook does not see.

## Red flags

- Writing `by=` as the controller, or `--as` after the fields.
- `git add -A` in a committer. Stage `EVENTLOG_PATHS` only.
- Restarting a reactor from an agent's tool shell. It dies with the turn.
- A `result` with no `spawn`, or a `retire` with no `result`.
- Answering "add a type" with a new `append`; edit `eventlog.toml` first.
