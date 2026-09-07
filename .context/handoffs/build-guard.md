# Brief: build-guard

You are the worker **build-guard**, spawned by the controller of this repo (see
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

Frozen contracts (decisions model-contract, query-contract): use the public items in src/model/*.rs and src/query/mod.rs as they are. Other workers are editing other files right now (react, tui, cmd/append); never touch a file outside your list, and if the crate fails to build because of someone else's file, append escalate and wait. The current shell guard is skill/event-log-coordination/scripts/eventlog-guard.sh; read it for the denylist shapes, then implement them in Rust with the stricter rules in the task. Hook payload shapes: Claude Code PreToolUse JSON has tool_name and tool_input (command / file_path); Cursor beforeShellExecution has command, cwd; Codex PreToolUse has tool_name, tool_input. You may add fields to GuardArgs in src/cli.rs (targeted edit).

## Log

You MAY append to `.context/events.jsonl`, only through `append-event.sh`
and only with `by=build-guard` on every line (decision `log-writers`):

    append-event.sh progress by=build-guard msg="<one line>" ref=<main file>
    append-event.sh result   by=build-guard ref=<main file> paths=<comma-separated files you changed> summary="<one line>"
    append-event.sh escalate by=build-guard msg="<what blocks you>"

Append one `progress` when the failing tests are written, and one `result`
when everything is green. End your final message with the word DONE.

## Task (from the plan)

### Task 18: Guard for three agents **[radiator: opus]**

Spec section 9.

**Files:** Create `src/guard/deny.rs`, `src/cmd/guard.rs`, `tests/guard.rs`, `tests/fixtures/hooks/claude_bash_deny.json`, `claude_edit_deny.json`, `cursor_shell_deny.json`, `codex_bash_deny.json`, `codex_apply_patch_deny.json`, `claude_append_allow.json`, `adaptive_cases.txt`; Modify `src/guard/mod.rs`.

**Interfaces:**

```rust
pub enum Agent { Claude, Cursor, Codex }
pub enum Action { Edit{path: String}, Write{path: String}, Shell{command: String}, Other }
pub fn parse(payload: &str, agent: Option<Agent>) -> Result<(Agent, Action), ParseFail>;  // ParseFail::NotJson -> fail open; ParseFail::UnknownShape -> fail closed
pub fn decide(action: &Action, log_path: &Path) -> Decision;   // Decision::Allow | Decision::Deny(reason)
pub fn is_simple_sanctioned_writer(command: &str) -> bool;      // single simple command, argv[0] in {eventlog, append-event.sh}; false on ; && || | $( ` newline
pub fn install(agent: Agent, root: &Path, bin: &Path) -> anyhow::Result<bool>;  // idempotent; returns true if changed
```

`decide` for Shell: allow if `is_simple_sanctioned_writer`; deny if the command names the log basename (or `EVENTLOG_GUARD_BASENAME` regex) together with any of: `>`/`>|`/`>>`-less redirect into it, `sed -i`, `rm`, `mv`, `truncate`, `tee` without `-a`, `cp` onto it, `perl -i`, `python … open(… 'w')`, `git checkout -- <log>`, `git restore <log>`; deny compound commands naming the log. Deny output: stderr reason, stdout `{"permission":"deny","userMessage":"<reason>"}`, exit 2. `install` writes `.claude/settings.json` PreToolUse entry (matcher `Edit|Write|Bash`), `.cursor/hooks.json` `beforeShellExecution` and `beforeReadFile`-less entry, `.codex/hooks.json` `PreToolUse`, each with `eventlog guard --agent <a>`.

- [ ] **Step 1:** tests: each deny fixture → exit 2 and `permission: deny`; `claude_append_allow.json` (command `append-event.sh result ref=x`) → exit 0; `eventlog append note x=1; : > .context/events.jsonl` → deny; every line of `adaptive_cases.txt` (start with 12: `cat .context/events.jsonl | tee .context/events.jsonl`, `python3 -c "open('.context/events.jsonl','w')"`, `env X=1 rm .context/events.jsonl`, `bash -c 'rm .context/events.jsonl'`, `git checkout -- .context/events.jsonl`, `cp /dev/null .context/events.jsonl`, `truncate -s0 .context/events.jsonl`, `perl -i -pe s/a/b/ .context/events.jsonl`, `sed -i '' d .context/events.jsonl`, `mv .context/events.jsonl x`, `: >| .context/events.jsonl`, `eventlog append a b=c && rm .context/events.jsonl`) → deny; a JSON payload with no known fields → exit 2 "unrecognized payload"; non-JSON stdin → exit 0; `install` twice → second returns false and file unchanged.
- [ ] **Steps 2–5.**
