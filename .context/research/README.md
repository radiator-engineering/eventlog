# Research for the event-log CLI

Material gathered on 2026-09-06 in the Drove session that created this repo.
Read `.context/handoffs/bootstrap.md` first; this directory is what it points at.

## What is here

| Path | What it is | Why it matters |
|---|---|---|
| `../../skill/event-log-coordination/` | The current skill, copied from `~/.claudewho-radiator/skills/event-log-coordination` (its `.git` removed). Upstream: `git@github.com:radiator-engineering/event-log-coordination.git`, HEAD `6d9dac4`. | The scripts this CLI replaces and the SKILL.md that will become thin. `~/.claude/skills/event-log-coordination` is a symlink to that checkout. |
| `setup-log-driven-workspace/` | The composing skill (`~/.claude/skills/setup-log-driven-workspace`), scripts and templates. | `templates/run-reactor.sh`, `cursor-commit-reactor.sh`, `doc-sync-reactor.sh` are the reactors in production use; the reactor runtime subcommand must be able to run them. `scripts/layout.sh` is the herdr layout the TUI pane lives in. |
| `recon-setup-skill-report.md` | Capability inventory of both skills, written for Drove. | Lists every script, flag, and env var; the checklist for parity. |
| `recon-herdr-api-report.md` | Herdr 0.8.2 socket API (protocol 20) vs Drove. | If the TUI opens panes or agents (open-the-ref in a pane, jump to an agent's pane), this is the API. |
| `samples/drove-events.jsonl` | A real 283-event log from Drove's v2/v3 build (reactors, parallel PR workers, claims, acks, violations). | Test fixture for the reader, lifecycle view, and replay. |
| `samples/EVENTLOG.md` | The vocabulary file `init-eventlog.sh` writes. | The event types and fields the reader must know. |
| `samples/drove-log-driven.Drovefile` | Drove's log-driven example. | Shows how the shell commands are invoked by bare name from a Drovefile; the CLI's names must work the same way. |

## Findings from the session

- The log format: one JSON object per line; `seq` (monotonic int), `ts`
  (UTC RFC 3339, second precision), `type` (lowercase slug), then flat string
  fields. `append-event.sh` rejects any field over 2 KB and any event over
  4 KB, takes a `mkdir` lock (`<log>.lock/`), and refuses `seq`/`ts` as user
  fields. Non-controller writers set `by=`.
- Reactors (`run-reactor.sh` + a reactor script) tail the log, act, and append
  `ack by=<name> seq_done=<seq> outcome=…`. Resume is from the last own `ack`;
  first start writes a baseline ack at the tip. Lock dir
  `<log>.<name>.reactor.lock/` holds the pid.
- The PreToolUse guard (`eventlog-guard.sh`) is Claude Code only. It blocks any
  Bash command that names the log path together with a mutation, even `git
  commit … && tail events.jsonl`. `protect-log.sh` (`chflags uappnd`) is the
  cross-agent enforcement and is opt-in per log.
- `.gitignore` entries `init-eventlog.sh` writes: `.context/events.jsonl`,
  `.context/events.jsonl.lock`. The setup skill adds
  `.context/*.reactor.lock/`, `.context/layout.json`, `.DS_Store`.
- Drove (`~/Development/Drove`) cannot yet apply workspaces/panes: `drove up`
  runs only tasks (`src/cli.rs:198-218`; `apply_plan` in `src/executor.rs:536`
  is unwired). That is a Drove spec, not this repo's. Until it lands,
  `layout.sh` builds the herdr layout.
- The user's stated goal: enforce the event-log conventions consistently across
  Drove, Radiator, and future repos, for any agent (Claude Code, Cursor,
  Codex). A versioned, installable CLI is the chosen fix. Longer term the
  Radiator hub could be the single writer (agents send events over its
  protocol; nothing else has a writer), which is stronger than hook or
  `chflags`.
