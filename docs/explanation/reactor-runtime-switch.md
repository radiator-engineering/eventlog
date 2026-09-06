# Why the reactor panes hand their loop to `eventlog react`, and how the doc worker avoids looping on itself

This repo runs its own coordination log, so its two reactors —
[the commit reactor and the doc worker](../../AGENTS.md) — must run on
something. The herdr layout (`Drovefile`, via the `reactor()` function in
`drove/reactors.star`) starts each one as a pane that runs [`eventlog
react`](../reference/react-command.md), the Rust reactor runtime, directly:

```
eventlog react --as cursor-committer --on result --git \
  --timeout 300s -- bash .context/bin/commit-action.sh

eventlog react --as doc-worker --on ack --filter by=cursor-committer \
  --filter outcome=committed --timeout 300s -- bash .context/bin/doc-action.sh
```

The Rust runtime owns the lock, resume point, intent, veto window, and
acknowledgment for each reactor; see [Reactor loop](../reference/react-loop.md)
and [why its lock checks more than a pid](reactor-lock-liveness.md).

## `run-reactor.sh` is a standalone fallback, not part of the layout

Earlier, the herdr layout ran each reactor through `.context/bin/run-reactor.sh`,
a supervisor that read `REACTOR_RUNTIME` from `.context/workspace.env` and
either `exec`'d `eventlog react` (`REACTOR_RUNTIME=eventlog`, the default) or
fell back to a bash polling loop (`REACTOR_RUNTIME=shell`) with its own
crash-restart and escalate-after-5-crashes logic.

The Drovefile no longer calls `run-reactor.sh`: `reactor()` builds the
`eventlog react` command straight into the pane's `serve`. `run-reactor.sh`
still works if run by hand (`bash .context/bin/run-reactor.sh
cursor-commit-reactor.sh`) and still honors `REACTOR_RUNTIME=shell`, but that
switch no longer affects what the layout starts. To rule out the Rust
runtime as a cause, run a reactor through `run-reactor.sh` with
`REACTOR_RUNTIME=shell` outside the layout, or edit the reactor's `action`
command directly.

## Why the doc worker appends its own `result`

Under the Rust runtime, the doc worker reacts to the committer's `ack`
(`--on ack --filter by=cursor-committer --filter outcome=committed`) — the
signal that a commit just landed. `doc-action.sh` runs headless Claude
with the documentation-writer skill against the files that commit touched,
then must get its own edits committed in turn. It does that the same way
any other reactor result does: it calls `eventlog append --as doc-worker
result agent=doc-worker ref=<file> paths=<changed files>
summary=<summary>` directly, instead of only printing an outcome for
`eventlog react` to fold into an `ack`. That `result` is what gives the
commit reactor something to react to next — without it, the doc worker's
edits would sit uncommitted until some other event happened to wake the
committer.

`doc-action.sh` still never runs `git commit` itself. The controller
already claimed `docs`, `README.md`, and `AGENTS.md` for `doc-worker`, and
the `result` `doc-action.sh` appends stays inside that claim — see [the
sanctioned writers in DECISIONS.md](../../.context/DECISIONS.md).

## Why it also has to skip its own commit

The doc worker's `result` triggers a commit, and that commit produces
another committer `ack` — the same event type and filter the doc worker
reacts to. Without a check, that ack would trigger another documentation
pass over a commit that only contains documentation, which would produce
another `result`, and so on indefinitely.

`doc-action.sh` breaks the cycle before doing any work: it reads the
driving `ack` event from stdin, takes its `seq_done` field, and looks up
who wrote the `result` at that sequence number in the log. If that `by` is
`doc-worker`, the pass reports `outcome=skipped` and exits immediately
instead of running Claude. This replaces an earlier `origin=` field on the
`ack` with a direct lookup against the log, since `origin` only recorded
who triggered the *previous* commit, not who wrote the result the current
`ack` is closing out.

## See also

- [Reactor loop](../reference/react-loop.md) — the poll loop `eventlog react` runs.
- [React command](../reference/react-command.md) — the `eventlog react` CLI flags used above.
- [Why the reactor lock checks more than a pid](reactor-lock-liveness.md) — the liveness check the Rust runtime's lock relies on.
