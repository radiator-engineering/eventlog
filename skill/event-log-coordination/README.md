# event-log-coordination

A skill for coordinating several coding agents through one append-only JSONL
event log. A single controller writes it. Every agent and every human reads it.
Walk the log to event N and you know exactly what the system knew at event N.

The log and its tooling work under any agent that can run a shell: Claude Code,
Cursor, Codex, aider, plain scripts. Enforcement comes in two forms. A Claude
Code PreToolUse hook blocks rewrites before they run. An OS-level append-only
flag holds under any agent.

The full guide is in [`SKILL.md`](SKILL.md). This README is the map.

## What is in the box

| Path | What it is |
|------|------------|
| `SKILL.md` | The skill: the pattern, the rules, enforcement, parallel-worker rules, and the operating model |
| `references/herdr-integration.md` | Mapping herdr controller and worker actions to events |
| `references/log-reactors.md` | Rules for a process that acts on events (a committer, a deployer) without repeating itself |
| `references/reactor-example.md` | A 15-line committer action script for `eventlog react` |

Install with `eventlog skill install`. The binary embeds this directory and
writes it to your skills path, stamped with the binary version.

## Operating model

The agent runs every `eventlog` command. The user speaks in natural language.
On first use in a session the agent runs `eventlog doctor --fix` once, then does
not mention commands again unless asked. See `SKILL.md` for the full contract.

## Quick start

```bash
eventlog doctor --fix          # once per session: register guards, report posture
cd /path/to/the/repo
eventlog init                  # creates .context/events.jsonl + EVENTLOG.md

eventlog append spawn  agent=t2 model=claude pane=w1:p9 role=implement
eventlog append claim  agent=t2 paths=Sources/App/SettingsView.swift,Sources/Theme
eventlog append prompt agent=t2 ref=.context/handoffs/t2-brief.md
eventlog claims t2 feature/base   # before accepting: did t2 stay inside its claim?
eventlog append result agent=t2 verdict=ACCEPT commit=8eae01f ref=.context/handoffs/t2-brief.md
eventlog append retire agent=t2 disposition=merged

eventlog view -f               # watch it in a pane
```

## Migration from shell scripts

If you used the old skill with shell scripts on PATH, map them to subcommands:

```
| Old script              | eventlog subcommand              |
|-------------------------|----------------------------------|
| append-event.sh         | eventlog append                  |
| eventlog-view.sh        | eventlog view                    |
| check-claims.sh         | eventlog claims                  |
| init-eventlog.sh        | eventlog init                    |
| safety-check.sh         | eventlog doctor                  |
| setup.sh                | eventlog doctor --fix            |
| link-scripts.sh         | (removed; doctor --fix cleans PATH) |
| protect-log.sh          | eventlog protect                 |
| eventlog-guard.sh       | eventlog guard                   |
| install-guard.sh        | eventlog guard install           |
| run-reactor.sh + loop   | eventlog react                   |
```

## Event vocabulary

Every line carries `seq`, `ts`, and `type`. The rest is flat key=value fields.

| Type | Meaning |
|------|---------|
| `spawn`, `prompt`, `result`, `retire` | A worker's lifecycle. Every agent that reports a `result` must also have the other three |
| `decision` | A choice others must follow. Points at `DECISIONS.md` |
| `escalate`, `approval` | A question queued for a human, and the answer |
| `message`, `drain` | Inter-agent routing and inbox processing |
| `claim` | The repo-relative paths a worker owns. Nobody else edits them |
| `progress` | The controller polled a working agent and noted one line |
| `seam` | A cross-worker dependency found mid-work |
| `violation` | A worker changed files outside its claim |
| `intent`, `veto`, `ack` | Reactor runtime: declared action, voter block, completed action |
| `ack` | A reactor acted on event `seq_done`. Carries `by=<reactor>`, `outcome`, and `ref` to the effect |

A line with `by=<agent>` was not written by the controller. Only writers
sanctioned by a `decision key=log-writers` may set it.

Add new types freely. Keep fields flat and under 2KB each. Reference big
artifacts by path with `ref=`, never by inlining them.

## Keeping parallel workers apart

The log records coordination. It does not create it. `SKILL.md` lists eight
rules. The short version:

1. Land the shared foundation first, then fan out.
2. One git worktree per worker.
3. `claim` files at spawn. Ownership in brief prose cannot be replayed. A claim can.
4. Scope the brief: do only this task, no subagents, do not append to the log.
5. Run `eventlog claims` before accepting a `result`.
6. Append a `seam` the moment a report names a cross-worker dependency.
7. Workers are silent by design. Poll and append `progress` if you want a live signal.
8. Close every lifecycle.

## Reactors

A reactor tails the log and acts on events with no prompt, so a bug here is a
duplicate commit, not a stale note. `eventlog react` owns the runtime; you supply
the action script. Four rules, detailed in `references/log-reactors.md`:

1. Resume from your own `ack` events in the log, never from a cursor file or a
   `git log --grep` marker.
2. One instance, held by the runtime's lock dir.
3. Foreground it in a dedicated terminal pane. A `nohup … &` from an agent's
   tool-call shell dies with the turn.
4. A committer stages only the authorized paths the runtime passes in
   `EVENTLOG_PATHS`.

```bash
eventlog react --as committer --on decision --filter key=commit-message --git -- \
  bash path/to/commit-action
```

See `references/reactor-example.md` for a minimal commit action.

## Viewing the log

`eventlog view` renders one colored, aligned line per event:

```
seq   time      TYPE        agent           summary                        → ref
```

- `-f` follows. `--last N` limits or sets the follow start.
- `--type result,decision` and `--agent t2` filter.
- `--json` prints one object per row with `"v":1`.
- Colors come from `.context/eventlog.toml` `[view.colors]` or env overrides.

`eventlog tui` opens ratatui panes (follow, agents, state) in the coordinator
terminal.

## Install as a skill

```bash
eventlog skill install              # writes to ~/.claude/skills/event-log-coordination
eventlog skill install --dir <path> # custom skills directory
```

Start a new session after install. Under Claude Code the guard activates on the
next session start, since hooks load at startup.

## Portability: Cursor, Codex, and other agents

| Piece | Portable? |
|-------|-----------|
| The pattern, the log, and every `eventlog` subcommand except the guard hook | Yes. Any agent that runs a shell |
| `eventlog protect` (OS-level append-only) | Yes. macOS and Linux, any agent |
| `eventlog guard` PreToolUse hook | Claude Code only. It reads Claude Code's hook payload |

Under Cursor, Codex, or aider, use the log tooling normally and get enforcement
from `eventlog protect` instead of the hook.

OS protection is per-log and opt-in. Nothing is protected until someone runs
`eventlog protect` on a file. `eventlog doctor` reports the real state and
`eventlog doctor --fix` heals each gap. It prompts before it protects a log.
`eventlog init` runs the report right after creating a log, so no log is
silently assumed safe.

## Enforcement, honestly

- **Airtight** for `Edit` and `Write` under Claude Code. They can only overwrite or patch, so any hit is denied.
- **Best-effort** for `Bash` under Claude Code. A strong denylist covers truncating redirects, `sed -i`, `rm` and `mv`, non-append `tee`, and inline interpreter writes. It is not a proof.
- **Agent-independent** with `eventlog protect`. The kernel refuses truncate, overwrite, and rm under any agent. Run `eventlog verify` if you need tamper evidence on top.
- **Single writer is a convention for herdr peers.** A Task subagent cannot reach the log. A peer with a shell can append, and the OS flag allows appends. The brief must forbid it. See the honest limit in `SKILL.md`.
- **Not authenticated.** `--as` is a declaration, not proof of identity.
