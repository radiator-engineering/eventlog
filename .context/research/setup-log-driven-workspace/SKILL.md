---
name: setup-log-driven-workspace
description: Use when starting a new project (or re-arming an existing one) that should have an append-only event log with an autonomous Cursor commit reactor, a headless-Claude documentation worker, and a herdr layout with a monitor workspace — "set up my standard workspace for this project", "give me the commit worker and doc worker", "event log + herdr in minutes", or when a reactor is missing, dead, replaying old events, or someone is pinging the commit watcher.
metadata:
  author: jjm@radiator.live
---

# setup-log-driven-workspace

Turns a bare git repo into a verified coordination setup: one append-only log,
two **self-watching
reactors** (nobody pings them), and a herdr layout whose monitor workspace
opens on the formatted log. Three commands, under five minutes.

**REQUIRED BACKGROUND:** event-log-coordination (the log, the guard, the
reactor rules) and herdr-layouts (naming, tabs-vs-panes). This skill composes
them; it does not replace them. The agent runs every script here.

## The three commands

```bash
S=~/.claude/skills/setup-log-driven-workspace/scripts
cd /path/to/repo
$S/setup.sh  --commit [--protect] [--churn .obsidian/workspace.json] \
             [--committer-model composer-2.5-fast] [--doc-model sonnet] [--doc-paths docs,README.md,AGENTS.md]
$S/layout.sh [--no-monitor] [--no-files]                 # from INSIDE your herdr pane
$S/status.sh                                             # exit 0 = work will flow
```

Defaults, so you can state them to the user before running: committer
`composer-2.5-fast` (cursor-agent), doc worker `sonnet` via `claude -p` with a
`$2` budget per pass (`DOC_BUDGET_USD`), doc roots `docs,README.md,AGENTS.md`,
pass timeout 300 s. Change them on the command line or later in
`.context/workspace.env`.

`setup.sh` lays down files only (log, reactors, briefs, DECISIONS, hook,
gitignore, decision events); `--commit` commits that scaffold so the tree is
clean before the reactors start — use it. It also writes the controller's
**standing orders**: one marker-delimited block in `AGENTS.md` (commands,
boundaries, pointers; "every task that changes files ends with
`append-event.sh result …`"), a `CLAUDE.md` that imports `AGENTS.md` with an
`@AGENTS.md` line so every agent tool reads one source of truth, and a Stop
hook in `.claude/settings.json` that hands the turn back once, with the file
list, when changed files have no `result` behind them. Without these the
controller edits and stops silently, and the reactors never fire; the hook
loads in a **new** Claude session, so restart the controller after setup.

The block is role-aware: it tells a **spawned worker** (a brief in
`.context/handoffs/` names it) to do only its brief, stay inside its claimed
paths, never append to the log and never commit, and it tells the reactors
their own briefs win. The Stop hook holds only the controller's pane (from
`layout.json`) and reactor workers (`LOG_DRIVEN_WORKER`) to the `result` rule.
On a live repo with running reactors, refresh just these with
`setup.sh --orders`; it never touches reactors or `workspace.env`.

`AGENTS.md` is the project's rules file and the **doc worker owns its
upkeep**: `AGENTS.md` is a doc root by default, and each pass loads
`context-engineering` to add the durable facts a change introduced, delete
stale or task-specific lines, and keep the file under about 120 lines. It never
edits inside marker-delimited sections (this skill's block, or sections other
skills upsert) and never touches `CLAUDE.md`. `--churn` names files an editor
rewrites by itself (Obsidian rewrites `.obsidian/workspace.json` on every file
open); they are untracked and ignored so the clean-tree gate can hold.
`--protect` OS-protects the log; it is an opt-in, so ask the user once — for a
Cursor committer recommend **yes**, because the Claude guard does not cover
cursor-agent (undo with `protect-log.sh --unprotect`).

`layout.sh` shapes the herdr session around **the Claude that runs it** — that
pane is the controller and is never renamed or moved. There is no separate
"event-log controller" tab. The shape:

| Workspace | Tab | Panes |
|---|---|---|
| `control` (this one, renamed) | `coordinator` | top: this Claude; bottom: `eventlog` running `eventlog-view.sh -f` (50/50 split) |
| | `monitor: system + agents` | `agentmon` running `agentmon --since <cutoff>` |
| `maintenance` | `lazygit` (first) | `gitlog` running `lazygit` |
| | `<committer model>: commit reactor` | `commit-reactor` — `run-reactor.sh` |
| | `<doc model>: doc sync` | `doc-sync` — `run-reactor.sh doc-sync-reactor.sh` |
| `files` | (default) | one pane running `spiceedit` |

The `agentmon` cutoff comes from `agent-since.sh`: the start time of the
earliest agent process in any pane of the current herdr session, minus 60 s
(`AGENT_SINCE_MARGIN`), so every agent session in this herdr session is
captured; `launch` when no pane hosts an agent. `agentmon` and `spiceedit` are
the user's own tools (`agentmon`: private repo `radiator-engineering/agentmon`,
`cargo build --release` → `~/.local/bin`); without them the monitor tab falls
back to `htop` and the files workspace stays a shell. `--no-monitor` /
`--no-files` skip those parts. It also records a real `spawn` + `prompt` for
both workers. It needs `HERDR_ENV=1` (it refuses otherwise;
`--dry-run` prints the commands from anywhere). `teardown.sh [--close]` stops
the reactors by pid and records `retire`.

Reading the log: the append-only guard blocks any **compound** Bash command
that names the log path alongside a mutation (`git commit`, `>`, `rm`, …) —
even when the log part is just `cat`. Read it with the Read tool, or with a
standalone `jq`/`tail` command, never inside a command that also changes
something.

**Both scripts are idempotent — run them again whenever in doubt.** `setup.sh`
keeps every existing file (`--force` refreshes reactors, briefs, hooks and the
`AGENTS.md` block from the templates; `DECISIONS.md` and `workspace.env` are
never regenerated, edit them directly) and records each decision once. After `--force` restart the
reactors (`teardown.sh` then `layout.sh`) so they run the new scripts. `layout.sh` discovers before it creates: a reactor tab is
reused when a tab in this workspace has the same label *and* its pane's cwd is
this repo; a reactor whose lock pid is alive is left alone and a dead one is
restarted in its existing pane; the `maintenance` and `files` workspaces, the
monitor tab and the `eventlog` pane are matched by label + cwd;
`spawn`/`prompt` are appended only when
the agent has no open (un-retired) spawn. A second run prints `reuse` on every
line and changes nothing. Never create a tab or start a reactor by hand to
"fix" a layout — rerun `layout.sh`.

Before `layout.sh`: commit or stash anything dirty outside `.context/`, or the
first `result` hands it all to the committer. Ask the user before `--protect`
(it is a deliberate, reversible opt-in — the same rule as the doctor).

## How work flows once it is up

1. You (the controller) finish a change and append
   `append-event.sh result ref=<file> paths=<a,b> summary="…"`.
2. The **committer** (`cursor-agent -p`, headless, per pass) stages only
   `paths=`, writes a message grounded in the log + diff, and the reactor acks
   with `origin=controller`.
3. The **doc worker** sees that ack, runs `claude -p` with
   `documentation-writer` + `plain-technical-english`, edits only the doc
   roots, and reports `result by=doc-worker`.
4. The committer lands the docs and acks with `origin=doc-worker`, which the
   doc worker ignores. Loop closed.

Never ping a reactor. Never invent a worker name in a `result` — every agent
in the log has a `spawn` and a `retire` (layout.sh/teardown.sh write them).
Never `rm` a `*.reactor.lock` dir: the guard blocks any command naming the log
path, and a restart reclaims a dead lock on its own.

## What the baseline got wrong (and the scripts now fix)

An agent without this skill, asked for the same setup, produced these. Each
one broke live before it was fixed in the templates.

| Baseline choice | What happened live | Template |
|---|---|---|
| Interactive `cursor-agent` looping on a blocking wait; the model writes its own ack; no supervisor | Stalled after one cycle, forgot acks, needed a human "check" — the exact failure the user refused | Headless per pass; the **reactor** writes the ack; `run-reactor.sh` respawns and stops on crash loops |
| Resume from seq 0 when a reactor has no acks | Cold start "documented" a sha an amend had already replaced | First start writes a baseline ack at the log tip; nothing older is replayed |
| `claude -p --allowedTools … "<prompt>"` | `--allowedTools` is variadic and swallowed the prompt: "Input must be provided…" | Prompt on stdin |
| `claude -p` with the shell's `ANTHROPIC_API_KEY` | Billed an empty API account: "Credit balance is too low" | `env -u ANTHROPIC_API_KEY` (login auth); `DOC_USE_API_KEY=1` opts back in |
| No `--trust` on `cursor-agent` | Workspace-trust dialog ate the start of the prompt | `--force --trust` |
| Editor churn not handled | `.obsidian/workspace.json` kept the tree dirty; the clean gate never held | `--churn` untracks + ignores |
| Strip every AI trailer, including Claude's | Repo policy may require the controller's trailer | Hook strips only `Co-authored-by: (Cursor\|Composer)` |
| A separate "event-log controller" tab, log tab created after htop | The controller lived apart from its log; herdr has no reorder | The invoking Claude *is* the controller; the log pane sits under it; tabs are created in the order they must appear |
| Controller told once, in chat, to append `result` | Next task: edited files, stopped, nothing committed until the user nudged it | `CLAUDE.md`/`AGENTS.md` block + Stop hook, both from `setup.sh` |
| "Would ask the user" six setup questions | Minutes lost per repo | Decided: trigger = `result`/`decision key=commit-message`; doc worker never commits; reactors in `maintenance`, monitor beside the controller; claims optional |

## Parameters and where they live

`.context/workspace.env` (written by setup.sh, sourced by the reactors; shell
env wins): `MODEL`, `DOC_MODEL`, `DOC_PATHS`, `PASS_TIMEOUT`. Reactor-specific:
`RETRIES`, `RETRY_SLEEP`, `DOC_BUDGET_USD` (2), `DOC_USE_API_KEY`.
`.context/layout.json` holds the herdr ids for status/teardown.

## Verify before you hand it over

1. `status.sh` exits 0 (both reactors running, hook active, no breach).
   In a fresh controller session, edit one file and stop without appending:
   the Stop hook must hand the turn back naming that file.
2. Smoke test: make one small real change, append a `result … paths=`, and
   watch the monitor's log tab until you see `ack by=cursor-committer` →
   `result by=doc-worker` → `ack … origin=doc-worker`. Then `git log -3` shows
   no `Co-authored-by: Cursor`.
3. `git status` clean outside `.context/`.

## Honest limits

Reactors die with the pane, herdr session, or machine; rerun `layout.sh`
after `teardown.sh`, or the one `run-reactor.sh` line in the pane — they resume
from their last ack. Staging scope is detected (`violation`), not prevented.
One shared worktree can race parallel workers; give each parallel worker its
own worktree and `claim`. `core.hooksPath` is local config: re-run `setup.sh`
on a fresh clone.

## Scripts

- `scripts/setup.sh` — files, hooks, gitignore, decisions, standing orders. Idempotent; `--force` refreshes templates, `--orders` refreshes only the `AGENTS.md` block, `CLAUDE.md` import and Stop hook.
- `scripts/layout.sh` — `control` / `maintenance` / `files` workspaces around the invoking Claude, reactors, `spawn`/`prompt` events. `--dry-run` prints the commands.
- `scripts/agent-since.sh` — RFC 3339 cutoff for `agentmon --since` covering every agent session in the current herdr session (`launch` if none).
- `scripts/status.sh` — health, exit 1 when work cannot flow.
- `scripts/teardown.sh` — stop by pid, `retire` events, `--close` removes the `maintenance`/`files` workspaces, monitor tab and eventlog pane (never the controller pane).
- `templates/` — the verified reactors, supervisor, briefs, commit-msg hook, DECISIONS, `CONTROLLER.md` (the standing-orders block), `controller-stop-hook.sh`.
