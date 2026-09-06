# Why the CLI surface shipped before any command works

The `eventlog` crate exists as a scaffold: a full set of subcommands that
parse correctly but each print `not implemented` and exit 1
([full list](../reference/eventlog-cli-surface.md)).

## The problem this avoids

The [implementation plan](../superpowers/plans/2026-09-06-eventlog-cli.md)
splits the CLI into 21 tasks across four phases, each done by a separate
worker. If every worker were free to add or rename commands and flags as it
went, two workers could each add a conflicting `--format` flag, or one could
rename `claims` while another still depended on the old name.

## The fix

`Cargo.toml`, `src/lib.rs`, `src/main.rs`, and `src/cli.rs` are claimed by
Task 1 only, and frozen once it lands: they define every command name, every
subcommand, and the two global flags (`--log`, `--json`) up front. A later
task that needs a new dependency or module still has to ask for it — it
appends an `escalate` event and the controller makes the one-line change —
rather than editing the frozen files itself.

Each stub in `src/cmd/` compiles from day one, so `cargo build` and
`cargo test` pass after Task 1 even though no command does real work yet.
Later tasks replace one stub at a time without touching the surface those
stubs sit behind.
