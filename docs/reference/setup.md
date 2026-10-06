# Setup: `eventlog setup`

Status: implemented. `src/scaffold/mod.rs` holds `setup_plan` and `init`;
`src/cmd/setup.rs` exposes them as `eventlog setup`. `setup` sets up the
log and nothing else. It writes no reactor files and prints nothing about
reactors.

```sh
eventlog setup preview
eventlog setup apply
eventlog setup upgrade
```

| Subcommand | Effect |
|---|---|
| `preview` | Print what `apply` would add. Writes nothing. |
| `apply` | Run `eventlog init`: create the missing files and lines below. Never overwrites a file that exists. |
| `upgrade` | The same as `apply`. A later eventlog may add a template or a line; `upgrade` adds it and keeps your edits. |

Each subcommand prints one line per missing item, or `setup: no changes`
when nothing is missing:

| Line | Item |
|---|---|
| `create .context/events.jsonl` | The empty log. |
| `create .context/EVENTLOG.md` | The event vocabulary, from a template. |
| `create .context/eventlog.toml` | The log configuration, from a template. |
| `ignore <line>` | A missing `.gitignore` line: `.context/events.jsonl`, `.context/events.jsonl.lock`, or `.context/layout.json`. |
| `attribute .context/events.jsonl -text` | The missing `.gitattributes` line. |

`apply` and `upgrade` print the same lines, for the items they added. A
second run prints `setup: no changes`.

## Reactors are separate

Up to eventlog 0.5, `eventlog setup` also wrote the reactor policy
(`.context/eventlog-setup.toml`) and a Drove helper
(`.context/eventlog-reactors.star`). Those files now belong to the optional
`eventlog-reactors` binary: see [Reactor setup](../reactors/setup.md).
`eventlog setup` ignores both files when they exist, and never changes or
deletes them.

## Tests

`tests/no_reactors.rs` runs `setup preview`, `apply`, and `upgrade` in a
fresh git repository. It checks that no reactor file appears, that
`.gitignore` holds no reactor line, and that no command prints the word
"reactor". `tests/scaffold.rs` checks that a second `init` leaves every file
byte-for-byte the same.

```sh
cargo test
```

## See also

- [Scaffold and setup: `init`, `doctor`, `protect`](scaffold.md) — the bootstrap `setup apply` runs.
- [Reactor setup](../reactors/setup.md) — `eventlog-reactors setup`, the optional reactor policy.
- [Lifecycle](lifecycle.md) — `eventlog lifecycle start`/`stop` for a supervisor's hooks.
- [`eventlog` command list](eventlog-cli-surface.md)
