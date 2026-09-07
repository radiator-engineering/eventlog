# Brief: build-scaffold-cmds

You are the worker **build-scaffold-cmds**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: cursor-agent (auto).

Read first: `docs/superpowers/plans/2026-09-06-eventlog-cli.md` sections
"Global constraints" and "File structure", and the spec sections your task cites
in `docs/superpowers/specs/2026-09-06-event-log-cli-design.md`.

## Rules

Do only this task. Touch only the files it lists (plus `Cargo.lock`, which
cargo maintains). Write the failing test first, run it, make it pass, then run
`cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt`.
Do not `git commit`. Do not edit `Cargo.toml`, `src/lib.rs`, `src/main.rs`
or `src/cli.rs` unless this task lists them; if you need a change there,
append `escalate` (below) and stop. Do not start subagents. Never edit,
truncate or `rm` `.context/events.jsonl`.

## Contract

Frozen contracts: model (src/model), query (src/query/mod.rs), react (src/react, decision react-contract seq 285), guard (src/guard as landed by result seq 295: guard::Agent, guard::install), skill (src/skill/mod.rs as landed by result seq 293: skill install and stamp). Use them as they are. Never run eventlog protect, eventlog init or eventlog doctor --fix against THIS repo; exercise them only in temp dirs under tests, and mark the protect test #[ignore] with a reason if it needs permissions. You may add fields to InitArgs, ProtectArgs and DoctorArgs in src/cli.rs (targeted edits only). Test only your own target: cargo test --test scaffold.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-scaffold-cmds` on every line (decision `log-writers`):

    append-event.sh progress by=build-scaffold-cmds msg="<one line>" ref=<main file>
    append-event.sh result   by=build-scaffold-cmds ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-scaffold-cmds msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 19: `init`, `protect`, `doctor`

Spec section 6 rows for these three.

**Files:** Create `src/scaffold/doctor.rs`, `tests/scaffold.rs`, `src/scaffold/templates/EVENTLOG.md`, `src/scaffold/templates/eventlog.toml`; Modify `src/scaffold/mod.rs`, `src/cmd/init.rs`, `src/cmd/doctor.rs`, `src/cmd/protect.rs` (stubs from Task 1).

Behavior: `init` creates `.context/events.jsonl` (empty), `.context/EVENTLOG.md` (template updated to subcommands and the `log-writers` grammar), `.context/eventlog.toml`, appends gitignore lines `.context/events.jsonl`, `.context/events.jsonl.lock`, `.context/*.reactor.lock/`, `.context/layout.json` and a `.gitattributes` line `.context/events.jsonl -text`; idempotent. `protect [--off] [--status]` runs `chflags uappnd|nouappnd` on macOS, `chattr +a|-a` on Linux; `--status` exit 0 protected / 1 not. `doctor [--fix] [--protect]` prints `[ OK ]`/`[WARN]`/`[FAIL]` rows: binary on PATH; guard installed per agent (reads the three hook files); protection; unsanctioned writers (fold as-of-seq); strict-rule violations over history; open lifecycles; stale reactor locks; skill stamp vs `--version`; old script symlinks on PATH. `--fix` installs guards for agents whose config dir exists, removes the old symlinks, installs the skill; `--protect` runs protect. Exit 1 on any `[FAIL]`.

- [ ] **Step 1:** tests in a temp repo: `init` twice leaves identical files; `doctor` on a log whose seq 3 is `result by=x` with no grant → `[FAIL] unsanctioned`; after appending `decision key=log-writers value=x:result` at seq 2 (rebuild the fixture) → OK; `protect --status` exit 1 before, 0 after `protect` (skip on CI without permission; mark `#[ignore]` with a reason).
- [ ] **Steps 2–5.**
