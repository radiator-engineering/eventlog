# event-log

Append-only coordination event log: Rust CLI + TUI, with the `event-log-coordination` skill versioned alongside.

## What is in this repo

- `skill/event-log-coordination/` — the skill source: the append-only log helpers, the PreToolUse guard, and the reactor references.
- `.context/` — this repo's own coordination log, decisions, worker briefs and reactor scripts. The repo runs on the tool it ships.
- `Drovefile` and `drove/reactors.star` — the herdr layout that places the controller, the log view and the two reactors.
- `research/` — prior-art reports that back the coordination design (event sourcing, multi-agent coordination, the LogAct paper), indexed in `research/README.md`.
- `docs/superpowers/specs/` — design specs for tools this repo will ship. `2026-09-06-event-log-cli-design.md` is the approved design for `eventlog`, the Rust binary that will replace the shell toolkit above: log format, config, module layout, commands, and the reactor runtime with intent and veto.

The Rust CLI and TUI are not in the tree yet.
