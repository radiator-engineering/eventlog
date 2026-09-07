# View: `src/cmd/view.rs` and `eventlog view`

Status: implemented. Prints log events as formatted text or `--json` rows,
filtered by type, agent, writer, time, or a text search, and can follow the
log for new lines.

```sh
eventlog view [--log <name|path>] [--json] [--follow]
              [--type <t1,t2,...>] [--agent <name>] [--by <name>]
              [--since <rfc3339>] [--last <n>] [--grep <text>]
              [--color auto|always|never]
```

| Flag | Meaning |
|---|---|
| `-f`, `--follow` | Print existing events, then keep watching the log and print new ones as they arrive. |
| `--type <t1,t2,...>` | Only events whose `type` is in this comma-separated list. |
| `--agent <name>` | Only events where `name` matches the event's `agent`, `by`, `from`, or `to` field. |
| `--by <name>` | Only events whose writer (`by`, or `"controller"` when absent) equals `name`. |
| `--since <rfc3339>` | Only events with `ts` at or after this RFC 3339 timestamp. |
| `--last <n>` | Keep only the last `n` events after the other filters are applied. |
| `--grep <text>` | Only events where the formatted line, or the raw JSON line, contains `text`. |
| `--color auto\|always\|never` | Color the type and agent columns. `auto` (default) colors only when stdout is a terminal. |

Filters combine with AND. `--json` (the global flag) switches the output
format; it does not filter anything.

## Text format

One line per event:

```
<seq>  <TYPE>  <agent>  <summary>  → <ref>
```

`seq` is zero-padded to 4 columns, `TYPE` is the event type in upper case,
and `agent` is `Event::subject()` (the event's `agent` field, or its writer).
`summary` is the first of: the `msg` field; for a `decision`, `key=value`;
the first present of `summary`, `verdict`, `subject`, `outcome`, `detail`,
`disposition`, `task`; `paths: <paths>`; `→ <to>`; or an empty string. The
`→ <ref>` suffix is appended whenever the event has a `ref` field.

Colors come from a built-in palette keyed by field name (`seq`, `agent`,
`ref`) and by event type (`result`, `decision`, `spawn`, …), overridable per
key by a `[view.colors]` table in config. Each value is an SGR code applied
as `\x1b[<code>m...\x1b[0m`.

## `--json` rows

Each row is one JSON object per line: `v: 1`, then `seq`, `ts`, `type`, and
`prev`/`by`/`agent` if present, plus every other field on the event. This is
the `--output` shape documented in [Schema](schema.md).

## Follow mode

`-f` first reads and prints the log as it stands, then watches the log file
with the `notify` crate (falling back to a 500&nbsp;ms poll) and prints each
complete new line that passes the filters as it is appended. A new line that
fails to parse as an event is skipped, with a reason printed to stderr; it
does not stop the follow.

## Tests

`tests/cmd_view.rs` runs the built `eventlog` binary against a copy of
`tests/fixtures/drove-events.jsonl` and checks: `--last 3` prints exactly 3
lines; `--type ack --json` rows all have `"v":1` and `"type":"ack"`;
`--agent doc-worker` matches a line where `doc-worker` only appears in `by`;
and `-f`, given a line appended to the log file after the process starts,
prints that line within 2 seconds. Run them with:

```sh
cargo test
```

## See also

- Log module — `Log::read`, which `view` reads the log with.
- [Schema](schema.md) — the `--json` row shape.
- [`eventlog` command list](eventlog-cli-surface.md)
