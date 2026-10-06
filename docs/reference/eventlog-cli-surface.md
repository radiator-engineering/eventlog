# eventlog CLI surface

Status: scaffolded. Every command parses; `verify` ([reference](verify.md)),
`schema` ([reference](schema.md)), `view` ([reference](view.md)), `claims`
([reference](claims.md)), `open` ([reference](open.md)), `agents`, `state`,
`why` ([reference](query-commands.md)), `append`, `vocab`
([reference](append.md)), `tui` ([reference](tui.md)), `guard`
([reference](guard.md)), `init`,
`doctor`, `protect` ([reference](scaffold.md)), `setup`
([reference](setup.md)), `lifecycle` ([reference](lifecycle.md)), `skill`,
`context` ([reference](context.md)), and `completions` ([reference](skill.md))
are implemented, the rest still print `eventlog <name>: not implemented` to
stderr and exit 1. This page documents the frozen command set so later
tasks can fill in behavior without changing names or flags.

The reactor commands are not part of `eventlog`. Up to eventlog 0.5 it had
`react` and `action`; they moved to the optional `eventlog-reactors` binary
(see [Reactor commands](#reactor-commands-eventlog-reactors) below).

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

## Reactor commands: `eventlog-reactors`

The optional `eventlog-reactors` binary (crate `eventlog-reactors`, in
`reactors/`) takes the same global flags, `--log` and `--json`:

| Command | Purpose |
|---|---|
| `react` | Reactor runtime: live loop, or `test <seq>` to dry-run one reaction against a real sequence number ([reference](../reactors/react-command.md)). |
| `action` | Run a packaged reactor action ([reference](../reactors/action.md)). Subcommands: `commit`, `docs`. |
| `setup` | Preview, apply, or upgrade the reactor policy and its Drove helper ([reference](../reactors/setup.md)). Subcommands: `preview`, `apply`, `upgrade`. |
| `doctor` | Check the reactor setup and each reactor lock ([reactors README](../../reactors/README.md)). |

## Building and checking the surface

```sh
cargo build
cargo test        # tests/cli_surface.rs asserts every eventlog command above is recognized,
                  # and reactors/tests/cli_surface.rs does the same for eventlog-reactors
```

Source: `src/cli.rs` defines the command enum and dispatch table; each
`src/cmd/<name>.rs` implements that command. `reactors/src/cli.rs` and
`reactors/src/cmd/` do the same for `eventlog-reactors`.

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
- [React command](../reactors/react-command.md) — `eventlog-reactors react` and `react test`, wiring the reactor loop to the rule voter and the action runner.
- [Action](../reactors/action.md) — `eventlog-reactors action`, packaged commit and docs reactor actions.
- [Reactor setup](../reactors/setup.md) — `eventlog-reactors setup`, the reactor policy and Drove helper.
- [Skill and completions](skill.md) — installing the embedded skill and generating shell completions.
