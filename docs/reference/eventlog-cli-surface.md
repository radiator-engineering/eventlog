# eventlog CLI surface

Status: scaffolded. Every command parses; `verify` ([reference](verify.md))
and `schema` ([reference](schema.md)) are implemented, the rest still print
`eventlog <name>: not implemented` to stderr and exit 1. This page documents
the frozen command set so later tasks can fill in behavior without changing
names or flags.

## Global flags

Every command accepts:

| Flag | Meaning |
|---|---|
| `--log <name\|path>` | Named or explicit log to operate on. |
| `--json` | Emit machine-readable JSON rows instead of formatted text. |

## Commands

| Command | Purpose |
|---|---|
| `append` | Validate and append one event. |
| `vocab` | Show required and optional fields per event type. |
| `verify` | Walk the hash chain and report the first break. |
| `view` | Print log rows, optionally following new ones. |
| `agents` | Per-agent lifecycle table. |
| `state` | Folded state as of a sequence number. |
| `why` | Explain how one event was acted on. |
| `claims` | Report files a worker's claim does not cover. Hidden alias: `check-claims`. |
| `open` | Open an event's ref in `$EDITOR` or `$PAGER`. |
| `tui` | Interactive terminal UI over the log. |
| `react` | Reactor runtime. Subcommand: `test` (dry-run one reaction against a real sequence number). |
| `guard` | Hook guard for agent tool calls. Subcommand: `install`. |
| `init` | Create a new coordination log and scaffold. |
| `doctor` | Diagnose common setup problems. |
| `protect` | Toggle or report OS-level append-only protection. |
| `schema` | Print the JSON Schema for log types ([reference](schema.md)). |
| `skill` | Manage the embedded coordination skill. Subcommand: `install`. |
| `completions` | Generate shell completions. |

`check-claims` runs the same code as `claims` but is hidden from `--help` and
top-level command listings; use `eventlog claims --help` to see it, or run
`eventlog check-claims` directly.

## Building and checking the surface

```sh
cargo build
cargo test        # tests/cli_surface.rs asserts every command above is recognized
```

Source: `src/cli.rs` defines the command enum and dispatch table; each
`src/cmd/<name>.rs` holds that command (a stub, except `verify.rs`).

## See also

- [Why the CLI surface shipped before any command works](../explanation/frozen-cli-surface.md)
- [Verify](verify.md) — the first implemented command.
- [Schema](schema.md) — JSON Schema for event lines and `--json` view rows.
