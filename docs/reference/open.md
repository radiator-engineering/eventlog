# Open: `src/cmd/open.rs` and `eventlog open`

Status: implemented. Opens the file an event's `ref` field points to, in
`$EDITOR` or `$PAGER`.

```sh
eventlog open <seq> [--pager]
```

| Argument or flag | Meaning |
|---|---|
| `<seq>` | Sequence number of the event to open. |
| `--pager` | Open with `$PAGER` (default `less`) instead of `$EDITOR`. |

## Resolving the target

`open` reads the log, finds the event with the given `seq`, and reads its
`ref` field. A relative `ref` resolves against the current directory; an
absolute `ref` is used as-is.

- No event with that `seq`: `open` fails with an error.
- The event has no `ref`, or `ref` is empty: `open` prints
  `eventlog open: seq <seq> has no ref` to stderr and exits `1`.

## Launching the program

Without `--pager`, `open` reads `$EDITOR` and runs it with the resolved path
as its only argument; if `$EDITOR` is unset, `open` fails with an error.
With `--pager`, it runs `$PAGER`, or `less` if `$PAGER` is unset. Either way,
`open`'s exit code is the child process's exit code (or `1` if the child
didn't report one).

## See also

- [`eventlog` command list](eventlog-cli-surface.md)
