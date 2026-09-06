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
| `scripts/setup.sh` | One-shot idempotent setup. Links commands onto PATH and registers the guard |
| `scripts/safety-check.sh` | The doctor. Reports what is protected. `--doctor` heals gaps (tooling, guard, log opt-in) |
| `scripts/init-eventlog.sh` | Scaffold `.context/events.jsonl` and `EVENTLOG.md` in a repo |
| `scripts/append-event.sh` | The single sanctioned writer. Adds `seq` and `ts`, locks, caps event size |
| `scripts/eventlog-view.sh` | Human view of the log: colored, aligned, wrapped, filterable. `-f` follows in a pane |
| `scripts/check-claims.sh` | Read-only check that a worker changed only the files it claimed |
| `scripts/eventlog-guard.sh` | PreToolUse hook that blocks any rewrite or truncate of the log |
| `scripts/install-guard.sh` | Register or `--check` the guard in a `settings.json` |
| `scripts/protect-log.sh` | OS-level append-only (`chflags` on macOS, `chattr` on Linux) |
| `scripts/link-scripts.sh` | Symlink the commands onto PATH |
| `references/herdr-integration.md` | Mapping herdr controller and worker actions to events |
| `references/log-reactors.md` | Rules for a process that acts on events (a committer, a deployer) without repeating itself |
| `references/reactor-example.sh` | A working committer reactor: single instance, resumes from its own `ack` events, stages only named paths |

## Operating model

The agent runs every script. The user speaks in natural language. On first use
in a session the agent runs `safety-check.sh --doctor` once, then does not
mention scripts again unless asked. See `SKILL.md` for the full contract.

## Quick start

```bash
scripts/safety-check.sh --doctor      # once per session: link commands, register guard
cd /path/to/the/repo
init-eventlog.sh                      # creates .context/events.jsonl + EVENTLOG.md

append-event.sh spawn  agent=t2 model=claude pane=w1:p9 role=implement
append-event.sh claim  agent=t2 paths=Sources/App/SettingsView.swift,Sources/Theme
append-event.sh prompt agent=t2 ref=.context/handoffs/t2-brief.md
check-claims.sh t2 feature/base       # before accepting: did t2 stay inside its claim?
append-event.sh result agent=t2 verdict=ACCEPT commit=8eae01f ref=.context/handoffs/t2-brief.md
append-event.sh retire agent=t2 disposition=merged

eventlog-view.sh -f                   # watch it in a pane
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
5. Run `check-claims.sh` before accepting a `result`.
6. Append a `seam` the moment a report names a cross-worker dependency.
7. Workers are silent by design. Poll and append `progress` if you want a live signal.
8. Close every lifecycle.

## Reactors

A reactor tails the log and acts on events with no prompt, so a bug here is a
duplicate commit, not a stale note. Four rules, detailed in
`references/log-reactors.md`:

1. Resume from your own `ack` events in the log, never from a cursor file or a
   `git log --grep` marker.
2. One instance, held by a `mkdir` lock with a pid.
3. Foreground it in a dedicated terminal pane. A `nohup … &` from an agent's
   tool-call shell dies with the turn.
4. A committer stages only the `paths=` the decision names.

## Viewing the log

`eventlog-view.sh` renders one colored, aligned line per event:

```
seq   time      TYPE        agent           summary                        → ref
```

- `-f` follows. `--last N` limits or sets the follow start.
- `--type result,decision` and `--agent t2` filter.
- `--wrap` (default at a terminal) word-wraps long summaries under the summary
  column. `--truncate` cuts to one line with an ellipsis. `--width N` overrides
  the terminal width, which is read once at start.
- `--compact` drops the time column. `--no-color` for piping.

Colors are SGR parameter strings (`1` bold, `2` dim, `4` underline, `31` to `37`
colors, `1;32` bold green). Override per key with an env var or a config file.
Env wins:

```bash
EVENTLOG_COLOR_RESULT="1;35" eventlog-view.sh -f
```

```
# .context/eventlog-view.conf
result=1;36
agent=4
```

Keys are any event type plus `seq`, `ts`, `agent`, `ref`, `verdict_ok`, and
`verdict_bad`. Unknown event types render in the default color, so a new type
never breaks the view. A truncated or malformed line is skipped, not fatal.

## Install as a skill

Symlink or copy this directory into a skills directory your agent discovers.
For Claude Code that is `~/.claude/skills/event-log-coordination`. Start a new
session. Under Claude Code the guard activates on the next session start, since
hooks load at startup.

## Portability: Cursor, Codex, and other agents

| Piece | Portable? |
|-------|-----------|
| The pattern, the log, and every script except the guard | Yes. Any agent that runs a shell |
| `protect-log.sh` (OS-level append-only) | Yes. macOS and Linux, any agent |
| `eventlog-guard.sh` PreToolUse hook | Claude Code only. It reads Claude Code's hook payload |

Under Cursor, Codex, or aider, use the log tooling normally and get enforcement
from `protect-log.sh` instead of the hook.

OS protection is per-log and opt-in. Nothing is protected until someone runs
`protect-log.sh` on a file. `safety-check.sh` reports the real state and
`safety-check.sh --doctor` heals each gap. It prompts before it protects a log.
`init-eventlog.sh` runs the report right after creating a log, so no log is
silently assumed safe.

## Enforcement, honestly

- **Airtight** for `Edit` and `Write` under Claude Code. They can only overwrite or patch, so any hit is denied.
- **Best-effort** for `Bash` under Claude Code. A strong denylist covers truncating redirects, `sed -i`, `rm` and `mv`, non-append `tee`, and inline interpreter writes. It is not a proof.
- **Agent-independent** with `protect-log.sh`. The kernel refuses truncate, overwrite, and rm under any agent. Hash-chain entries if you need tamper evidence on top.
- **Single writer is a convention for herdr peers.** A Task subagent cannot reach the log. A peer with a shell can append, and the OS flag allows appends. The brief must forbid it. See the honest limit in `SKILL.md`.
