# Brief: build-react-cmd

You are the worker **build-react-cmd**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: Claude (opus) via claudewho-radiator.

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

Frozen contracts: model (src/model), query (src/query/mod.rs), react (src/react/mod.rs, lock.rs, voter.rs, action.rs as landed; decision react-contract). Use them as they are. The current reactors to wrap are .context/bin/cursor-commit-reactor.sh and .context/bin/doc-sync-reactor.sh; read them and .context/bin/run-reactor.sh first. Do NOT start, stop or restart any reactor and do NOT edit layout.sh or run it: the controller switches the live reactors after your result. You may add fields to ReactArgs in src/cli.rs (targeted edit).

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-react-cmd` on every line (decision `log-writers`):

    append-event.sh progress by=build-react-cmd msg="<one line>" ref=<main file>
    append-event.sh result   by=build-react-cmd ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-react-cmd msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 17: `react` and `react test` commands; switch this repo's reactors

**Files:** Create `src/cmd/react.rs`, `tests/cmd_react.rs`, `.context/bin/commit-action.sh`, `.context/bin/doc-action.sh`; Modify `.context/handoffs/cursor-committer.md` (replace "never stage anything under .context/" with "never stage the log or lock dirs"), `.context/workspace.env` (add `REACTOR_RUNTIME=eventlog`).

Behavior: `eventlog react --as n --on t1,t2 [--filter k=v]... [--window 0s] [--git] -- cmd...` builds `ReactorConfig` and calls `supervise`; `eventlog react test <seq> --as n [--git] -- cmd...` calls `handle(dry=true)` and prints each would-be event. `commit-action.sh` is the `cursor-agent` invocation from `cursor-commit-reactor.sh` lines that build the prompt and run it, staging only `$EVENTLOG_PATHS`, with its HEAD-moved check, writing `outcome=committed ref=<shas>` or `outcome=skipped detail=...` to `$EVENTLOG_OUTCOME_FILE`. `doc-action.sh` likewise wraps the `claude -p` call and writes `outcome=updated files=N` or `outcome=skipped`. Neither script touches the log.

- [ ] **Step 1:** tests: `react test <seq>` against a fixture with a fake `cmd` prints an `intent` and an `ack` line and leaves the log unchanged; `react --as t --on result -- sh -c 'echo outcome=committed'` in a temp repo, then an `append result paths=a.md` from another process → within 3 s the log has `intent`, `ack seq_done=<n> outcome=committed`; a `veto for=<n>` appended before the window closes (use `--window 2s` in this test) → `outcome=vetoed`.
- [ ] **Steps 2–5.** Controller then updates `layout.sh`'s reactor commands to `eventlog react ...` in the two `maintenance` panes (its own `result`), records `claim agent=doc-worker paths=docs,README.md,AGENTS.md`, and runs `teardown.sh` + `layout.sh`.
