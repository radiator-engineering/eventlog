# eventlog

`eventlog` is a Rust CLI for coordinating several coding agents in one repo through an append-only event log. One controller writes small events to `.context/events.jsonl` (`spawn`, `claim`, `result`, `decision`, `retire`); every agent and every human reads the same file. Reactors watch the log and act on it, so a commit or a doc pass happens because an event says so, not because someone pinged a pane. Walk the log to event N and you know exactly what the system knew at event N.

This repo runs on the tool it ships. Its own log, decisions, and worker briefs live in `.context/`; AGENTS.md says who may write what.

## Install

```sh
cargo install eventlog-cli --locked  # puts `eventlog` in ~/.cargo/bin
eventlog init                 # .context/events.jsonl, EVENTLOG.md, eventlog.toml, gitignore lines
eventlog doctor --fix         # installs the tool-call guard for Claude, Cursor, and Codex
eventlog protect              # optional: OS-level append-only on the log
```

To build a local checkout instead, run `cargo install --path . --locked`.
The crate is named `eventlog-cli`; the executable is `eventlog`.

`init` is safe to rerun. It never overwrites a file that already exists. The guard hook loads in a new agent session.

## Daily commands

| Command | What it does |
|---|---|
| `eventlog append <type> k=v ...` | Write one validated, hash-chained event. The only sanctioned writer. |
| `eventlog view [-f] [--last N]` | Print log rows; `-f` follows. |
| `eventlog state` | Active agents, open claims, decisions, and open intents as of now. |
| `eventlog why <seq>` | The chain of events behind one event, and how a reactor handled it. |
| `eventlog claims <agent> <base>` | Changed files that an agent's claim does not cover. |
| `eventlog react --as <name> --on <type> -- <cmd>` | Run a reactor: intent, voter, action, ack. |
| `eventlog tui` | Live terminal view of the folded log. |

Run `eventlog --help` for the full list, and `eventlog vocab` for the fields each event type takes.

## How the log drives work

1. The controller appends `spawn`, `prompt`, and `claim` for a worker, and the worker edits only the paths it claimed.
2. When the worker reports back, the controller appends `result ... paths=<files>`.
3. The commit reactor sees the `result`, declares `intent`, runs the rule voter, commits exactly those paths, and appends `ack`.
4. The doc reactor sees the `ack` and updates the docs the same way.

A reactor that commits a file outside the `paths=` it was given appends a `violation`. A file that another agent dirtied while the reactor ran is an `observed` line, with no blame. Nothing edits or truncates the log: a hook guard blocks the controller's own tool calls, and `eventlog protect` makes the kernel refuse everything else.

## Documentation

- [Run a log-driven repo](docs/how-to/run-a-log-driven-repo.md): the how-to for setting this up on a project.
- [Cut a release](docs/how-to/cut-a-release.md): generate the changelog with git-cliff, tag, and publish.
- [Command reference](docs/reference/eventlog-cli-surface.md): every subcommand, with a page per command in `docs/reference/`.
- [Design notes](docs/explanation/): the invariants worth understanding before you change the code.
- [Decisions](.context/DECISIONS.md): every design decision this repo has taken, dated, with the log seq that recorded it.
- [Design spec](docs/superpowers/specs/2026-09-06-event-log-cli-design.md) and [build plan](docs/superpowers/plans/2026-09-06-eventlog-cli.md): how the tool was designed and built.

The `event-log-coordination` skill for Claude Code lives in `skill/` and ships inside the binary. `eventlog skill install` writes it to `~/.claude/skills`.

## Layout

- `src/` the crate: `model` (event, config, vocabulary), `log` (read, lock, append), `query` (fold to state), `react` (reactor runtime), `guard` (hook guard), `scaffold`, `tui`, `cmd`.
- `skill/` the coordination skill, embedded at build time.
- `.context/` this repo's own log, decisions, briefs, and reactor action scripts.
- `Drovefile` the herdr layout: controller pane, log view, and the two reactor panes.
