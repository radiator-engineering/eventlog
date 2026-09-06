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
- `docs/reference/verify.md` — `verify` and `eventlog verify`: walking the hash chain and reporting the first break.
- `docs/explanation/hash-chain-verification.md` — why the chain can start partway through a log, but never stop once started.
- `docs/reference/schema.md` — `eventlog schema`: JSON Schema for event lines (`--events`) and `--json` view rows (`--output`).
- `docs/reference/query-module.md` — `src/query`: `State`, `fold`, and `fold_at`, projecting agents, live claims, decisions, reactors, and the allowlist as of a `seq`.
- `docs/reference/query-commands.md` — `eventlog agents`, `state`, and `why`: the lifecycle table, the folded-state snapshot, and the causes/effects/verdict explanation for one seq.
- `docs/reference/view.md` — `eventlog view`: filtered, colored log display with type/agent/by/since/last/grep filters and follow mode.
- `docs/reference/claims.md` — `eventlog claims` (alias `check-claims`): reports changed files an agent's live claims don't cover.
- `docs/reference/open.md` — `eventlog open`: opens an event's `ref` in `$EDITOR` or `$PAGER`.
- `docs/reference/append.md` — `append` in `src/log/append.rs`: field and allowlist validation, strict rules, and hash-chained writes; `eventlog append` and `eventlog vocab`, the CLI commands that expose it.
- `docs/reference/react-voter.md` — `src/react/voter.rs`: `authorize`, `check`, and `veto_binds`, the reactor runtime's rule voter — the authorized set, the four veto rules, and the veto window.

The `eventlog` crate is scaffolded (`cargo build` and `cargo test` pass). The model layer (`src/model`) is implemented and frozen as a contract for later tasks. `src/log` can now read a log file (whole-file parse, cheap tail check), serialize writer access with a directory lock, and append a validated event with a hash-chained `prev`. `eventlog verify` walks the hash chain and reports the first break, `eventlog schema` prints the JSON Schema for event lines and `--json` view rows, `eventlog view` prints filtered log rows with optional follow mode, `eventlog claims` reports files outside an agent's live claims, `eventlog open` opens an event's ref, `src/query` folds a read log into one `State` as of any `seq`, and `eventlog agents`, `state`, and `why` print that folded state as a lifecycle table, a full snapshot, and a causes/effects/verdict explanation of one event. `eventlog append` validates and writes one event, folding the log for strict checks, and `eventlog vocab` shows the required and optional fields per event type from the loaded config. `src/react` runs a reactor's loop over one driving event at a time, computing its authorized set (`voter::authorize`), running the veto rules (`voter::check`) and the veto window (`voter::veto_binds`) before acting; every other command still prints `not implemented` and exits 1.
