# eventlog

`eventlog` is a Rust CLI for coordinating several coding agents in one repo through an append-only event log. One controller writes small events to `.context/events.jsonl` (`spawn`, `claim`, `result`, `decision`, `retire`); every agent and every human reads the same file. Walk the log to event N and you know exactly what the system knew at event N.

Agents that act on log events without a prompt (reactors) live in a separate repo, [eventlog-reactors](https://github.com/radiator-engineering/eventlog-reactors). The log needs none of them.

This repo runs on the tool it ships. Its own log, decisions, and worker briefs live in `.context/`; AGENTS.md says who may write what.

## Install

```sh
cargo install eventlog-cli --locked  # puts `eventlog` in ~/.cargo/bin
eventlog setup preview        # what setup would add; writes nothing
eventlog setup apply          # .context/events.jsonl, EVENTLOG.md, eventlog.toml, git ignore lines
eventlog doctor --fix         # installs the tool-call guard for Claude, Cursor, and Codex
eventlog protect              # optional: OS-level append-only on the log
```

To build a local checkout instead, run `cargo install --path . --locked`.
The crate is named `eventlog-cli`; the executable is `eventlog`.

`setup apply` does the same as `eventlog init`. It is safe to rerun: it never overwrites a file that already exists. The guard hook loads in a new agent session.

## Daily commands

| Command | What it does |
|---|---|
| `eventlog append <type> k=v ...` | Write one validated, hash-chained event. The only sanctioned writer. |
| `eventlog view [-f] [--last N]` | Print log rows; `-f` follows. |
| `eventlog state` | Active agents, open claims, and decisions as of now. |
| `eventlog why <seq>` | The chain of events behind one event, and what followed from it. |
| `eventlog claims <agent> <base>` | Changed files that an agent's claim does not cover. |
| `eventlog tui` | Live terminal view of the folded log. |

Run `eventlog --help` for the full list, and `eventlog vocab` for the fields each event type takes.

## Rebuild the controller's context from the log

A long controller session normally ends in an LLM summary of itself. The `eventlog-context` mod for Claude Code replaces that: it builds the new context from the log (decisions, agents, recent history, open work) plus the last turns word for word, in milliseconds, and it fires at task boundaries instead of at a token limit.

```sh
eventlog context install     # writes .claude/skills/eventlog-context/ (rerun after upgrading eventlog)
echo '/.claude/skills/eventlog-context/' >> .gitignore   # generated copy; keep it out of git
CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1 claude               # Claude Code 2.1.278+, in a trusted workspace
```

Then work as usual. `eventlog view --last 20` shows each `rebuild` event with its trigger. `/rebuild` forces one when the next turn ends; `EVENTLOG_CONTEXT=off` turns the mod off for a session. `eventlog context` prints the packet the mod would use. See [Rebuild context from the log](docs/how-to/rebuild-context-from-the-log.md) and the [reference](docs/reference/context.md).

## How the log drives work

1. The controller appends `spawn`, `prompt`, and `claim` for a worker. The worker edits only the paths it claimed.
2. When the worker reports back, the controller runs `eventlog claims <worker> main`. It lists changed files that the claim does not cover.
3. The controller appends `result ... paths=<files>`, then `retire`.
4. The controller commits the files the `result` names.

Nothing edits or truncates the log: a hook guard blocks the agents' tool calls that would, and `eventlog protect` makes the kernel refuse everything else.

## Documentation

- [Run a log-driven repo](docs/how-to/run-a-log-driven-repo.md): the how-to for setting this up on a project.
- [Rebuild context from the log](docs/how-to/rebuild-context-from-the-log.md): install and use the `eventlog-context` mod.
- [Cut a release](docs/how-to/cut-a-release.md): generate the changelog with git-cliff, tag, and publish.
- [Command reference](docs/reference/eventlog-cli-surface.md): every subcommand, with a page per command in `docs/reference/`.
- [Design notes](docs/explanation/): the invariants worth understanding before you change the code.
- [Decisions](.context/DECISIONS.md): every design decision this repo has taken, dated, with the log seq that recorded it.
- [Design spec](docs/superpowers/specs/2026-09-06-event-log-cli-design.md) and [build plan](docs/superpowers/plans/2026-09-06-eventlog-cli.md): how the tool was designed and built.

The `event-log-coordination` skill for Claude Code lives in `skill/` and ships inside the binary. `eventlog skill install` writes it to `~/.claude/skills`.

## Layout

- `src/` the `eventlog` crate: `model` (event, config, vocabulary), `log` (read, lock, append), `query` (fold to state), `guard` (hook guard), `context` (the context packet), `scaffold`, `skill`, `tui`, `cmd`.
- `skill/` the coordination skill, embedded at build time.
- `mods/eventlog-context/` the Claude Code mod, embedded at build time and written by `eventlog context install`.
- `.context/` this repo's own log, decisions, and briefs.
