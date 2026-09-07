# Brief: build-skill-docs

You are the worker **build-skill-docs**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: cursor-agent, auto model.

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

The command surface is spec section 6 of docs/superpowers/specs/2026-09-06-event-log-cli-design.md; the reactor runtime is spec section 7. Write for a reader who has the eventlog binary installed and has never seen the shell scripts.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-skill-docs` on every line (decision `log-writers`):

    append-event.sh progress by=build-skill-docs msg="<one line>" ref=<main file>
    append-event.sh result   by=build-skill-docs ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-skill-docs msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 21: Rewrite the skill to subcommands

**Files:** Modify `skill/event-log-coordination/SKILL.md`, `skill/event-log-coordination/README.md`, `skill/event-log-coordination/references/*.md`; Delete `skill/event-log-coordination/scripts/` (its `.gitignore` too).

Behavior: every script name becomes the subcommand from spec section 6; the "Setup" section becomes `eventlog init` + `eventlog doctor --fix`; the reactor section describes `eventlog react` with intent, voter, veto, violation and the outcome file; the "honest limit" section gains the unauthenticated-`--as` statement from spec section 3; `references/reactor-example.sh` becomes `references/reactor-example.md` showing a 15-line action script. Keep the section order. Run the `plain-technical-english` final gate.

- [ ] **Step 1:** test: `grep -rn '\.sh' skill/` returns nothing except inside a fenced block that shows the migration table.
- [ ] **Steps 2–5.**
