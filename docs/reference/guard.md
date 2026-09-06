# Hook guard: `src/guard`, `eventlog guard`

Status: implemented. Spec section 9. `src/guard/mod.rs` reads one agent's
tool-call payload and reduces it to an `Action`; `src/guard/deny.rs` judges
that action against the denylist; `src/cmd/guard.rs` wires both into the
`eventlog guard` and `eventlog guard install` commands.

## `parse` — reading one payload

```rust
pub enum Agent { Claude, Cursor, Codex }

pub enum Action {
    Edit { path: String },
    Write { path: String },
    Shell { command: String },
    Other,
}

pub enum ParseFail { NotJson, UnknownShape }

pub fn parse(payload: &str, agent: Option<Agent>) -> Result<(Agent, Action), ParseFail>;
```

`parse` reads the JSON on stdin and reduces it to one `Action`. `agent`
comes from `--agent` when the hook install baked it in; without it, `parse`
detects the shape: a payload with `turn_id` and `tool_name` is Codex, one
with `tool_name` alone is Claude, and one with a string `command` field is
Cursor.

Claude and Codex both send `tool_name` plus `tool_input`. `parse` maps the
tool name to an action:

| `tool_name` | `Action` |
|---|---|
| `Bash`, `shell`, `local_shell`, `exec_command`, `run_terminal_cmd` | `Shell { command }` |
| `Write`, `create_file` | `Write { path }` |
| `Edit`, `MultiEdit`, `NotebookEdit`, `str_replace_editor` | `Edit { path }` |
| `apply_patch` | `Edit { path }`, the first file named in the patch envelope |
| anything else | `Other` |

Cursor payloads carry only a shell `command`, so they always become
`Action::Shell`.

`parse` fails two different ways, and the caller must treat them
differently: input that is not JSON returns `ParseFail::NotJson`, and
JSON in a shape `parse` cannot map returns `ParseFail::UnknownShape`. See
[why fail-open and fail-closed split this way](../explanation/guard-fail-modes.md).

## `decide` — the one denylist

```rust
pub enum Decision { Allow, Deny(String) }

pub fn decide(action: &Action, log_path: &Path) -> Decision;
```

`decide` judges an `Action` against `log_path`'s basename (or
`EVENTLOG_GUARD_BASENAME`, if set). It denies:

- An `Edit` or `Write` whose path names the log.
- A `Shell` command that names the log and is not the sanctioned writer,
  when that command is either compound (`;`, `&&`, `||`, `|`, `$(`,
  backtick, or a newline) or carries one of these mutating shapes:
  - `rm`, `mv`, `cp`, `truncate`, `shred`, or `dd` naming the log.
  - A truncating redirect (`>` or `>|`, not `>>`) into the log.
  - An in-place edit (`sed -i`, `perl -i`, `ruby -i`) of the log.
  - `tee` into the log without `-a`/`--append`.
  - An interpreter call that opens the log for writing (`open(...'w'...)`).
  - `git checkout` or `git restore` naming the log.

Everything else — reads, and any command that never names the log — is
`Decision::Allow`. `is_simple_sanctioned_writer` names the one exemption:
a single command with no operator, redirect, or `&`, whose argv[0] (after
stripping a leading path) is `eventlog` or `append-event.sh`. See
[why that exemption checks command shape, not a substring match](../explanation/guard-fail-modes.md).

## `eventlog guard` — the command

```sh
echo '<hook payload json>' | eventlog guard [--agent claude|cursor|codex] [--log <path>]
```

Reads one payload from stdin, calls `parse` then `decide`, and never
returns a parse or I/O error to its caller — a guard that fails loudly on
its own bugs would block every tool call in the session. Exit codes:

| Exit | Meaning |
|---|---|
| `0` | Allow. Nothing on stdout. |
| `2` | Deny. stderr carries the reason and the append-only reminder; stdout carries `{"permission":"deny","userMessage":"..."}` for Cursor's hook protocol. |

`ParseFail::NotJson` allows (exit 0): something other than a hook payload
is on stdin. `ParseFail::UnknownShape` denies (exit 2) with the reason
"unrecognized payload: no tool_name, command or tool_input field".

## `eventlog guard install` — wiring the hook

```sh
eventlog guard install [--agent claude|cursor|codex]
```

Writes (or updates) the hook entry that runs `eventlog guard --agent
<agent>` for one agent, or for all three when `--agent` is omitted. Each
agent keeps its hooks in a different file and shape:

| Agent | File | Event |
|---|---|---|
| Claude | `.claude/settings.json` | `PreToolUse`, matcher `Edit\|Write\|Bash` |
| Cursor | `.cursor/hooks.json` | `beforeShellExecution` |
| Codex | `.codex/hooks.json` | `PreToolUse`, matcher `Edit\|Write\|Bash` |

`install` creates the file (with the skeleton object above) if it does not
exist, and appends its entry to the existing array otherwise. It reads the
current install's own path (`std::env::current_exe`, falling back to
`eventlog` on `PATH`) as the binary to bake into the command. Running
`install` again for an agent that already has the entry is a no-op: it
returns `false` (printed as `unchanged <agent>`) instead of rewriting the
file. A first-time install prints `installed <agent>`.

## Tests

`tests/guard.rs` runs `eventlog guard` end to end against fixture payloads
under `tests/fixtures/hooks/`: a deny fixture per agent and action shape
(`claude_bash_deny.json`, `claude_edit_deny.json`, `cursor_shell_deny.json`,
`codex_bash_deny.json`, `codex_apply_patch_deny.json`), one allow fixture
for the sanctioned writer (`claude_append_allow.json`), a compound
append-then-truncate command, and a table of adaptive deny cases in
`adaptive_cases.txt`. Unit tests in `src/guard/deny.rs` cover the sanctioned
writer's shape check, log-name boundary matching, and truncating vs.
appending redirects. Run them with:

```sh
cargo test
```

## See also

- [Why the guard fails open on non-JSON and closed on an unknown shape, and why the sanctioned-writer check parses shape instead of matching a substring](../explanation/guard-fail-modes.md)
- [eventlog CLI surface](eventlog-cli-surface.md) — where `guard` sits among the other commands.
