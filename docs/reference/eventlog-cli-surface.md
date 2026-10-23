# eventlog CLI surface

Status: implemented. Every command below runs; the table links each to its
reference page. Run `eventlog --help` for the same list.

The reactor commands (`react`, `action`) are not part of `eventlog`. They live in [eventlog-reactors](https://github.com/radiator-engineering/eventlog-reactors).

## Global flags

Every command accepts:

| Flag | Meaning |
|---|---|
| `--log <name\|path>` | Named or explicit log to operate on. |
| `--json` | Emit machine-readable JSON rows instead of formatted text. |

## Commands

| Command | Purpose |
|---|---|
| `append` | Validate and append one event ([reference](append.md)). |
| `vocab` | Show required and optional fields per event type ([reference](append.md)). |
| `verify` | Walk the hash chain and report the first break. |
| `view` | Print log rows, optionally following new ones ([reference](view.md)). |
| `agents` | Per-agent lifecycle table ([reference](query-commands.md)). |
| `state` | Folded state as of a sequence number ([reference](query-commands.md)). |
| `why` | Explain how one event was acted on ([reference](query-commands.md)). |
| `worktree-facts` | For each worktree path on stdin, say whether the log holds it or is done with it ([reference](worktree-facts.md)). |
| `claims` | Report files a worker's claim does not cover ([reference](claims.md)). Hidden alias: `check-claims`. |
| `open` | Open an event's ref in `$EDITOR` or `$PAGER` ([reference](open.md)). |
| `tui` | Interactive terminal UI over the log ([reference](tui.md)). |
| `guard` | Hook guard for agent tool calls ([reference](guard.md)). Subcommand: `install`. |
| `init` | Create a new coordination log and scaffold ([reference](scaffold.md)). |
| `doctor` | Diagnose common setup problems ([reference](scaffold.md)). |
| `context` | Render the controller's context from the log, decide when to rebuild it, install the `eventlog-context` mod ([reference](context.md)). |
| `protect` | Toggle or report OS-level append-only protection ([reference](scaffold.md)). |
| `setup` | Preview, apply, or upgrade the log's project setup ([reference](setup.md)). Subcommands: `preview`, `apply`, `upgrade`. |
| `lifecycle` | Idempotently spawn, claim, and retire a supervised agent ([reference](lifecycle.md)). Subcommands: `start`, `stop`. |
| `schema` | Print the JSON Schema for log types ([reference](schema.md)). |
| `skill` | Manage the embedded coordination skill ([reference](skill.md)). Subcommand: `install`. |
| `completions` | Generate shell completions ([reference](skill.md)). |

`check-claims` runs the same code as `claims` but is hidden from `--help` and
top-level command listings; use `eventlog claims --help` to see it, or run
`eventlog check-claims` directly.

## Building and checking the surface

```sh
cargo build
cargo test        # tests/cli_surface.rs asserts every eventlog command above is recognized,
```

Source: `src/cli.rs` defines the command enum and dispatch table; each
`src/cmd/<name>.rs` implements that command.

## See also

- Why the CLI surface shipped before any command works
- [Verify](verify.md) — the first implemented command.
- [Schema](schema.md) — JSON Schema for event lines and `--json` view rows.
- [View](view.md) — filtered, colored log display with follow mode.
- [Claims](claims.md) — compare changed files against an agent's live claims.
- [Open](open.md) — open an event's ref in `$EDITOR` or `$PAGER`.
- [Query commands](query-commands.md) — `agents`, `state`, and `why`.
- [Append](append.md) — the `append` library function and the `append`/`vocab` commands.
- [TUI](tui.md) — the live terminal UI over the folded log.
- [Guard](guard.md) — parsing agent hook payloads and the shared denylist.
- [Scaffold and setup](scaffold.md) — `init`, `doctor`, and `protect`.
- [Setup](setup.md) — `eventlog setup`, the log's project setup.
- [Worktree facts](worktree-facts.md) — `eventlog worktree-facts`, the provider that `offcut` reads.
- [Context](context.md) — `eventlog context`, `context check`, `context install`, and the `[context]` settings.
- [Lifecycle](lifecycle.md) — `eventlog lifecycle`, idempotent spawn/claim/retire for a supervised agent.
- [Skill and completions](skill.md) — installing the embedded skill and generating shell completions.
