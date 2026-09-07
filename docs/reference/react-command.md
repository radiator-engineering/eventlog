# React command: `src/cmd/react.rs`

Status: `eventlog react` and `eventlog react test` are implemented. This page
covers the CLI wiring: turning flags into a `ReactorConfig` and running the
live loop or a single dry-run pass. The loop itself is documented in
Reactor loop; the two steps this command wires in are
the rule voter and the action runner.

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
| `--git` | Snapshot git before and after the action; report files the action committed outside the authorized set as a `violation`, and unclaimed files that became dirty meanwhile as `observed`. `action::snapshot` parses status from NUL-delimited porcelain, so a path with a space, a quote, or `->` in it is tracked exactly. |
| `--timeout <dur>` | How long the action command may run. Default `600s`. On timeout, `action::run` kills the whole process tree (SIGTERM, then SIGKILL after a grace period on Unix; `taskkill /T` on Windows), not just the direct child, so a descendant cannot outlive the deadline or leave an optimistic outcome file behind. |
| `-- <command>...` | The action command. Required; everything after `--` is passed through as argv. |

Durations accept bare digits (seconds, matching the old `PASS_TIMEOUT`
environment variable) or a number suffixed `s`, `m`, or `h` — for example
`30`, `30s`, `5m`, `1h`.

## `eventlog react` — the live loop

Builds a `ReactorConfig` from the flags above and calls `supervise` (see
Reactor loop), which restarts the reactor on a panic or an
error. SIGINT, SIGTERM, and SIGHUP stop it cleanly instead: the loop
returns, the lock directory is released, and the process exits 0 without
counting as a restart. Absent a stop signal, it exits the process only
after 5 restarts inside a 10-minute window.

## `eventlog react test <seq>` — the dry run

Reads the event at `<seq>` from the log, runs one pass of `Reactor::handle`
(see Reactor loop) with `dry = true`, and prints each event
the pass would append — typically an `intent` line and the closing `ack` —
as JSON, one per line. It writes nothing to the log.

## Wiring `Steps` to the voter and the action runner

`RealSteps` is the one implementation of the `Steps` trait (see Reactor
loop) both commands use:

- `authorize` and `check` call `voter::authorize` and `voter::check`
  (reference) directly.
- `run` builds an `ActionEnv` from the driving event and the authorized
  paths and calls `action::run` (reference).
- `snapshot` calls `action::snapshot` (reference).

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

On a timeout or a non-zero exit, `action::run` folds a bounded tail of the
action's stderr (up to 1024 bytes) into `detail`, so a failure carries its
own diagnostics into the log instead of just an exit code. `action::run`
never inherits the action's stderr onto the reactor's own; it pipes and
drains it concurrently with stdout, so an action that writes to stderr
cannot deadlock the pipe. `bounded_detail` caps the assembled `detail` —
action-supplied detail, spillover fields, and the stderr tail — at 2048
bytes, the log's field limit (reference), keeping the end of the text: the
stderr tail is already a tail, and the failure reason comes last.

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

- Reactor loop — `ReactorConfig`, `Reactor`, the `Steps` trait, and `supervise`.
- Rule voter — `authorize` and `check`, the first half of `RealSteps`.
- Action runner — `run` and `snapshot`, the second half of `RealSteps`.
- [`eventlog` command list](eventlog-cli-surface.md)
