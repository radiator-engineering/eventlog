# event-log

Append-only coordination event log: Rust CLI + TUI, with the `event-log-coordination` skill versioned alongside.

## What is in this repo

- `skill/event-log-coordination/` — the skill source: the append-only log helpers, the PreToolUse guard, and the reactor references.
- `.context/` — this repo's own coordination log, decisions, worker briefs and reactor scripts. The repo runs on the tool it ships.
- `Drovefile` and `drove/reactors.star` — the herdr layout that places the controller, the log view and the two reactors.
- `research/` — prior-art reports that back the coordination design (event sourcing, multi-agent coordination, the LogAct paper), indexed in `research/README.md`.
- `docs/superpowers/specs/` — design specs for tools this repo will ship. `2026-09-06-event-log-cli-design.md` is the approved design for `eventlog`, the Rust binary that will replace the shell toolkit above: log format, config, module layout, commands, and the reactor runtime with intent and veto.
- `docs/superpowers/plans/` — implementation plans built from those specs. `2026-09-06-eventlog-cli.md` breaks the `eventlog` build into 21 tasks across four phases, each scoped as a herdr worker brief with claimed paths and the controller's spawn/claim/result/retire protocol.
- `docs/reference/eventlog-cli-surface.md` — the frozen `eventlog` command list and global flags.
- `docs/explanation/frozen-cli-surface.md` — why the CLI surface was locked down before any command works.
- `docs/reference/model-contract.md` — the frozen `src/model` contract: `Event`, `Config`, `Vocabulary`, `Allowlist`, and path rules.
- `docs/explanation/model-contract-precedence.md` — why the model layer is frozen, and how the write allowlist combines defaults, config file, and log decisions.
- `docs/reference/log-module.md` — `src/log`: `Log::open/read/tail/hash_line` and the mkdir-based `Lock`.
- `docs/explanation/lock-reclaim.md` — why a dead holder's lock is renamed aside before removal, not deleted directly.

The `eventlog` crate is scaffolded (`cargo build` and `cargo test` pass). The model layer (`src/model`) is implemented and frozen as a contract for later tasks. `src/log` can now read a log file (whole-file parse, cheap tail check) and serialize writer access with a directory lock, but no command works yet: each one prints `not implemented` and exits 1.
