# Brief: build-claims

You are the worker **build-claims**, spawned by the controller of this repo (see
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

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are. src/log/mod.rs provides Log::open/read/tail/hash_line. Other workers are editing src/log/append.rs and src/cmd/view.rs right now: never touch those, and if the crate fails to build because of them, append escalate and wait rather than editing their files. The fixture tests/fixtures/self-log-2026-09-06.jsonl is this repo's log frozen at seq 125; seq 39 is a controller result acked by seq 42, seq 33 is a worker result with no ack. You may add fields to ClaimsArgs and OpenArgs in src/cli.rs (targeted edits).

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-claims` on every line (decision `log-writers`):

    append-event.sh progress by=build-claims msg="<one line>" ref=<main file>
    append-event.sh result   by=build-claims ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-claims msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 12: `claims` and `open`

**Files:** Create `src/cmd/claims.rs`, `src/cmd/open.rs`, `tests/cmd_claims.rs`.

Behavior: `claims <agent> <base> [head]` runs `git diff --name-only --find-renames <base> <head|HEAD>` plus untracked from `git status --porcelain`, canonicalizes, and lists paths no live claim of `<agent>` covers; exit 1 if any. `check-claims` hidden alias. `open <seq> [--pager]` resolves the event's `ref`, runs `$EDITOR` (or `$PAGER` with `--pager`, default `less`), exit 1 if no ref.

- [ ] **Step 1:** tests in a temp git repo: a claim on `src/**`, a commit touching `src/a.rs` and `docs/b.md` → output lists `docs/b.md` only, exit 1; an untracked `docs/c.md` also listed.
- [ ] **Steps 2–5.**
