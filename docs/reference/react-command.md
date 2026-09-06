# React command: `src/cmd/react.rs`

Status: `eventlog react` and `eventlog react test` are implemented. This page
covers the CLI wiring: turning flags into a `ReactorConfig` and running the
live loop or a single dry-run pass. The loop itself is documented in
[Reactor loop](react-loop.md); the two steps this command wires in are
[the rule voter](react-voter.md) and [the action runner](react-action.md).

```sh
eventlog react --as <name> --on <t1,t2> [--filter k=v]... [--window <dur>] \
               [--git] [--timeout <dur>] -- <command>...

eventlog react test <seq> --as <name> [--on <t1,t2>] [--filter k=v]... \
               [--window <dur>] [--git] [--timeout <dur>] -- <command>...
```

| Flag | Meaning |
|---|---|
| `--as <name>` | The reactor's name: the `by=` on every line it writes. Falls back to the `EVENTLOG_AS` environment variable if omitted. Required one way or the other. |
| `--on <t1,t2,...>` | Comma-separated event types to react to. Required for the live loop; ignored for `react test`, which already has its event from `<seq>`. |
| `--filter k=v` | An extra condition the driving event must meet, beyond its type. Repeatable. Each value must contain an `=`. |
| `--window <dur>` | How long to wait for a veto after declaring intent. Default `0s`. |
| `--git` | Snapshot git before and after the action, and report writes outside the authorized set as a `violation`. |
| `--timeout <dur>` | How long the action command may run. Default `600s`. |
| `-- <command>...` | The action command. Required; everything after `--` is passed through as argv. |

Durations accept bare digits (seconds, matching the old `PASS_TIMEOUT`
environment variable) or a number suffixed `s`, `m`, or `h` — for example
`30`, `30s`, `5m`, `1h`.

## `eventlog react` — the live loop

Builds a `ReactorConfig` from the flags above and calls `supervise` (see
[Reactor loop](react-loop.md)), which restarts the reactor on a panic or
an error and never returns on its own. It exits the process only after 5
restarts inside a 10-minute window.

## `eventlog react test <seq>` — the dry run

Reads the event at `<seq>` from the log, runs one pass of `Reactor::handle`
(see [Reactor loop](react-loop.md)) with `dry = true`, and prints each event
the pass would append — typically an `intent` line and the closing `ack` —
as JSON, one per line. It writes nothing to the log.

## Wiring `Steps` to the voter and the action runner

`RealSteps` is the one implementation of the `Steps` trait (see [Reactor
loop](react-loop.md)) both commands use:

- `authorize` and `check` call `voter::authorize` and `voter::check`
  ([reference](react-voter.md)) directly.
- `run` builds an `ActionEnv` from the driving event and the authorized
  paths and calls `action::run` ([reference](react-action.md)).
- `snapshot` calls `action::snapshot` ([reference](react-action.md)).

### Reading the action's outcome

`action::run` returns raw `key=value` fields from the outcome file (or the
fallback stdout line). `RealSteps::run` sorts those fields against the
`ack` event type's fields and optional fields, loaded from config:

- A key the `ack` vocabulary knows (for example `ref`, `model`) becomes a
  field on the `ack` line.
- Any other key is folded into `detail=`, appended to an existing `detail`
  with `; ` rather than dropped — so a command can report whatever extra
  `k=v` pairs it likes without risking an `ack` the log would reject for an
  unknown field.

If the command reports no `outcome=` at all, `RealSteps` fills one in: exit
0 is `committed`; a timeout or a non-zero exit is `failed`, with `timed out
after <n>s` or `exit <n>` added to `detail`.

## Tests

`tests/cmd_react.rs` (4 tests) runs the built `eventlog` binary against a
temporary repository and a real action command:

- `react test <seq>` against a fixture `result` event prints an `intent`
  and a `committed` `ack`, and leaves the log byte-for-byte unchanged.
- A live `react --on result -- sh -c 'echo outcome=committed'`, given an
  `append result` from another process, acks it within 3 seconds.
- A command that writes `outcome=updated`, `files=3`, and `ref=abc` to the
  outcome file produces an `ack` with `ref=abc` and `files=3` folded into
  `detail`, since `files` is not an `ack` field.
- A `veto for=<seq>` appended inside a `--window 2s` stops the action from
  running and closes the pass as `outcome=vetoed`.

Run them with:

```sh
cargo test
```

## See also

- [Reactor loop](react-loop.md) — `ReactorConfig`, `Reactor`, the `Steps` trait, and `supervise`.
- [Rule voter](react-voter.md) — `authorize` and `check`, the first half of `RealSteps`.
- [Action runner](react-action.md) — `run` and `snapshot`, the second half of `RealSteps`.
- [`eventlog` command list](eventlog-cli-surface.md)
