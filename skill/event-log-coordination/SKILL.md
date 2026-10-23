---
name: event-log-coordination
description: >-
  Use when several coding agents share one repo and need one ordered record of
  who was spawned, what they own, what they reported and what was decided:
  spawning workers, claiming files, accepting results, recording decisions or
  or approvals. Also use when a repo has a .context/events.jsonl or when the
  eventlog guard blocks a command.
metadata:
  author: jjm@radiator.live
---

# Event-log coordination

The `eventlog` CLI keeps an append-only JSONL log at `.context/events.jsonl`.
The controller is the only writer. Workers report back; the controller
records. Every event is small and points at artifacts by `ref=`. The log
needs no other process. Agents that act on events and record their own `ack`
live in the separate eventlog-reactors project.

**You run every command below. The user never does.** On first use in a
session run `eventlog doctor --fix` (idempotent). It installs the guard hook
for each agent config dir present (`.claude`, `.cursor`, `.codex`), removes
old script symlinks, and installs this skill. It never protects the log;
that is `eventlog protect`, run only when the user says so.

## Command shapes that work

Flags go **before** the type. Fields are `key=value`.

```sh
eventlog setup preview                          # what the log setup would add; writes nothing
eventlog setup apply                            # same as init: log, EVENTLOG.md, eventlog.toml, git lines; never overwrites
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
| `by does not match writer` | the controller passing `by=`; any other writer passes `--as` instead |
| unknown type / field | anything not in `eventlog vocab` |

Paths are repo-relative: separators and interior `..` are allowed, but
absolute paths, leading `~`, and traversal above the root are rejected.
`--no-strict` is for repair only.

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
6. Commit the files the `result` names.

## Supervised agents: `eventlog lifecycle`

A supervisor (a process-manager hook) records a long-running
agent's spawn, claim and retire with `eventlog lifecycle`. Use argv directly
in the hook; do not concatenate shell strings:

```python
on_start = ["eventlog", "lifecycle", "start", agent, "--model", model,
            "--paths", paths]
on_stop = ["eventlog", "lifecycle", "stop", agent]
```

`lifecycle start` is idempotent. It records `role=agent` unless you pass
`--role`. After `stop`, it claims the paths supplied again with `--paths`; it
does not recover historical claims automatically. It leaves every other
agent's claims intact. The supervisor invokes it as the controller, so do not
run it from a worker brief.

## Reading the log

`eventlog view --last 20`, `view -f`, `view --since <seq|rfc3339>`,
`view --grep <text>`, `state`, `agents`, `why <seq>`, `open <seq>`, `tui`.
`state` adds a `reactors:` section, and `why` a `verdict:`, only when the log
has `ack`, `intent` or `veto` lines from a `by=` writer.

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
- A `result` with no open `spawn`. Accepted worker work needs a result before
  retirement; supervisor `lifecycle stop` needs no preceding result.
- Answering "add a type" with a new `append`; edit `eventlog.toml` first.
