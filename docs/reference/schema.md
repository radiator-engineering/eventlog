# Schema: `src/cmd/schema.rs` and `eventlog schema`

Status: implemented. Prints a [JSON Schema](https://json-schema.org/) for one
of the two JSON shapes this repo produces: a raw log line, or a `--json` view
row.

```sh
eventlog schema [--events | --output]
```

| Flag | Schema for |
|---|---|
| `--events` (default) | One line as stored in `.context/events.jsonl`. |
| `--output` | One row of `eventlog view --json` (adds `v`, the row format version). |

The two flags are mutually exclusive; passing both is a CLI error.

## Fields

Both schemas share the same core fields, generated from Rust structs with
[`schemars`](https://docs.rs/schemars):

| Field | Present | Meaning |
|---|---|---|
| `v` | `--output` only | Row format version, currently `1`. |
| `seq` | always | Sequence number. |
| `ts` | always | Timestamp, schema format `date-time`. |
| `type` | always | Event type (`result`, `decision`, `spawn`, …). |
| `prev` | optional | Hash of the previous line, once the chain has started. See [why the chain can start partway through a log](../explanation/hash-chain-verification.md). |
| `by` | optional | Agent that wrote the line, if not the controller. |
| `agent` | optional | Agent the event is about (for `spawn`, `claim`, `retire`, …). |

An event's other fields (`ref`, `paths`, `summary`, `key`, `value`, and so on)
vary by event type and are not fixed columns. Both schemas allow them through
`additionalProperties: {"type": "string"}` rather than listing them, so the
schema stays valid across event types. `.context/EVENTLOG.md` lists which
fields each event type expects.

## Example

```sh
eventlog schema --events | jq '.properties | keys'
```

```json
["agent", "by", "prev", "seq", "ts", "type"]
```

## Tests

`tests/cmd_schema.rs` checks that `--events` and the flag-less default agree,
that `--events` has an integer `seq` and a string `additionalProperties`
type, and that `--output` adds an integer `v`. Run them with:

```sh
cargo test
```

## See also

- [`eventlog` command list](eventlog-cli-surface.md)
- [Log module](log-module.md) — the on-disk line format `--events` describes.
