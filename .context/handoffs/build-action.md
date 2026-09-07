# Brief: build-action

You are the worker **build-action**, spawned by the controller of this repo (see
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

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are; src/log/mod.rs provides Log and Lock, src/log/append.rs provides append. Other workers are editing other files right now: never touch a file outside your list; if the crate fails to build because of someone else's file, append escalate and wait. Task 14 (src/react/mod.rs) is being built in parallel; do not edit it. action.rs imports only model and std; git is run through std::process::Command.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-action` on every line (decision `log-writers`):

    append-event.sh progress by=build-action msg="<one line>" ref=<main file>
    append-event.sh result   by=build-action ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-action msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 16: Action runner and violation snapshot

Spec section 7 steps 4.5–4.7.

**Files:** Create `src/react/action.rs`, `tests/react_action.rs`.

**Interfaces:**

```rust
pub struct ActionEnv { pub log: PathBuf, pub seq: u64, pub r#type: String, pub agent: String, pub by: String, pub paths: Vec<RelPath>, pub reference: Option<String>, pub resume: u64, pub outcome_file: PathBuf }
pub struct Outcome { pub exit: i32, pub fields: Vec<(String,String)>, pub timed_out: bool }  // fields from outcome file (k=v lines; seq/ts/prev/by dropped), fallback last stdout "outcome=..."
pub fn run(command: &[String], stdin_json: &str, env: &ActionEnv, timeout: Duration) -> anyhow::Result<Outcome>;
pub struct Snapshot { pub head: Option<String>, pub dirty: BTreeSet<String> }
pub fn snapshot(root: &Path) -> anyhow::Result<Snapshot>;        // git rev-parse HEAD, git status --porcelain
pub fn touched(before: &Snapshot, after: &Snapshot, root: &Path) -> BTreeSet<String>;  // files in commits before..after plus dirty delta
pub fn outside(touched: &BTreeSet<String>, authorized: &[RelPath]) -> Vec<String>;
```

Retry: `run` never retries; the loop (Task 14) retries once only if `fields` contains `outcome=retryable`.

- [ ] **Step 1:** tests: a shell script command that writes `outcome=committed\nref=abc` to `$EVENTLOG_OUTCOME_FILE` → `Outcome.fields == [("outcome","committed"),("ref","abc")]`; a command that prints `outcome=skipped` as last stdout line with no file → fallback works; `sleep 5` with 1 s timeout → `timed_out`; in a temp git repo, a command that commits `a.rs` and `b.rs` with authorized `[a.rs]` → `outside == ["b.rs"]`.
- [ ] **Steps 2–5.**
