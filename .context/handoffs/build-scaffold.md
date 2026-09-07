# Brief: build-scaffold

You are the worker **build-scaffold**, spawned by the controller of this repo (see
AGENTS.md, "Spawned worker"). Runtime: Claude (sonnet) via claudewho-radiator.

Read first: `docs/superpowers/plans/2026-09-06-eventlog-cli.md` sections
"Global constraints" and "File structure", and the spec sections your task cites
in `docs/superpowers/specs/2026-09-06-event-log-cli-design.md`.

## Rules

Do only this task. Touch only the files it lists (plus `Cargo.lock`, which
cargo maintains). Write the failing test first, run it, make it pass, then run
`cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt`.
Do not `git commit`. Do not edit `Cargo.toml`, `src/lib.rs`, `src/main.rs`
or `src/cli.rs` unless this task lists them (exception: a command task may add
fields to its own `<Cmd>Args` struct in `src/cli.rs`, with a targeted edit only); if you need a change there,
append `escalate` (below) and stop. Do not start subagents. Never edit,
truncate or `rm` `.context/events.jsonl`.

## Contract

You define the CLI surface; spec section 6 is the frozen command list.

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-scaffold` on every line (decision `log-writers`):

    append-event.sh progress by=build-scaffold msg="<one line>" ref=<main file>
    append-event.sh result   by=build-scaffold ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-scaffold msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 1: Crate scaffold and frozen CLI surface **[radiator: sonnet]**

**Files:**
- Create: `Cargo.toml`, `rustfmt.toml`, `.gitattributes`, `src/main.rs`, `src/lib.rs`, `src/cli.rs`, `src/cmd/mod.rs`, one stub per `src/cmd/*.rs` and per module `mod.rs` listed in File structure, `tests/fixtures/drove-events.jsonl` (copy of `.context/research/samples/drove-events.jsonl`), `tests/cli_surface.rs`
- Modify: `.gitignore` (add `target/`)

**Interfaces produced:**
- `pub fn eventlog::cli::run() -> i32` (parses argv, dispatches, returns exit code).
- `pub enum Command` in `cli.rs` with exactly these subcommands: `Append`, `Vocab`, `Verify`, `View`, `Agents`, `State`, `Why`, `Claims` (alias `check-claims`, hidden), `Open`, `Tui`, `React` (subcommand `Test`), `Guard` (subcommand `Install`), `Init`, `Doctor`, `Protect`, `Schema`, `Skill` (subcommand `Install`), `Completions`.
- Global flags on every command: `--log <name|path>`, `--json`.
- Every `cmd::<x>::run` stub returns `Ok(1)` after printing `eventlog <x>: not implemented` to stderr.

- [ ] **Step 1: write the failing test** in `tests/cli_surface.rs`:

```rust
use assert_cmd::Command;
#[test]
fn every_frozen_command_is_recognized() {
    for c in ["append","vocab","verify","view","agents","state","why","claims","open","tui","react","guard","init","doctor","protect","schema","skill","completions"] {
        Command::cargo_bin("eventlog").unwrap().arg(c).arg("--help").assert().success();
    }
}
#[test]
fn check_claims_is_a_hidden_alias() {
    Command::cargo_bin("eventlog").unwrap().args(["check-claims","--help"]).assert().success();
    let help = Command::cargo_bin("eventlog").unwrap().arg("--help").output().unwrap();
    assert!(!String::from_utf8_lossy(&help.stdout).contains("check-claims"));
}
```

- [ ] **Step 2:** `cargo test` fails (no crate).
- [ ] **Step 3:** write `Cargo.toml` with the dependencies in Tech Stack, `[[bin]] name = "eventlog"`, `[lib]`; write `cli.rs` with the clap derive; write every stub file; `lib.rs` declares `pub mod cli; pub mod cmd; pub mod model; pub mod log; pub mod query; pub mod react; pub mod guard; pub mod scaffold; pub mod tui; pub mod skill;`.
- [ ] **Step 4:** `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt` pass.
- [ ] **Step 5:** report `result` naming every created file.
