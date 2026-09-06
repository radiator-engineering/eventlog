# event-log

Append-only coordination event log: Rust CLI + TUI, with the `event-log-coordination` skill versioned alongside.

## What is in this repo

- `skill/event-log-coordination/` — the skill source: the append-only log helpers, the PreToolUse guard, and the reactor references.
- `.context/` — this repo's own coordination log, decisions, worker briefs and reactor scripts. The repo runs on the tool it ships.
- `Drovefile` and `drove/reactors.star` — the herdr layout that places the controller, the log view and the two reactors.
- `research/` — prior-art reports that back the coordination design (event sourcing, multi-agent coordination, the LogAct paper), indexed in `research/README.md`.

The Rust CLI and TUI are not in the tree yet.
