# Brief: build-tui

You are the worker **build-tui**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: Claude (sonnet) via claudewho-radiator.

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

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are. src/log/mod.rs provides Log::open/read/tail/hash_line. Other workers are editing src/log/append.rs and src/cmd/view.rs right now: never touch those, and if the crate fails to build because of them, append escalate and wait rather than editing their files. The fixture tests/fixtures/self-log-2026-09-06.jsonl is this repo's log frozen at seq 125; seq 39 is a controller result acked by seq 42, seq 33 is a worker result with no ack. ratatui 0.29 and crossterm 0.28 are in Cargo.toml. The filters in `view` are being built in parallel; implement the TUI's own filtering over Vec<Event> and do not import from src/cmd/view.rs. You may add fields to TuiArgs in src/cli.rs (targeted edit).

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-tui` on every line (decision `log-writers`):

    append-event.sh progress by=build-tui msg="<one line>" ref=<main file>
    append-event.sh result   by=build-tui ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-tui msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 13: TUI **[radiator: sonnet]**

Spec section 8.

**Files:** Create `src/tui/views.rs`, `tests/tui_render.rs`; Modify `src/tui/mod.rs`, `src/cmd/tui.rs`.

Behavior: ratatui app; top pane = follow view with the same filters as `view`; bottom pane toggles agents table / state summary; keys from `[keys]`: filter (opens an input line), follow toggle, open (runs `open` on the selected row, suspends the terminal), `w` why (bottom pane shows the `why` report), `tab` pane switch, `q` quit. Re-folds on every new line (fold is cheap at this scale; keep an `events: Vec<Event>` and refold).

- [ ] **Step 1:** render tests with `ratatui::backend::TestBackend`: the follow view shows the last N lines of the fixture; pressing the filter key then typing `ack` and Enter leaves only ack rows; the agents pane shows a retired agent with phase `retired`.
- [ ] **Steps 2–5.**
