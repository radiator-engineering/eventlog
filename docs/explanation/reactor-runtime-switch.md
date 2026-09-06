# Why the reactor scripts hand their loop to `eventlog react`, and how the doc worker avoids looping on itself

This repo runs its own coordination log, so its two reactors —
[the commit reactor and the doc worker](../../AGENTS.md) — must run on
something. Before this change, `.context/bin/run-reactor.sh` ran a plain
bash polling loop for each one. Now it runs [`eventlog
react`](../reference/react-command.md), the Rust reactor runtime, by
default, and keeps the bash loop only as a fallback.

## `REACTOR_RUNTIME` picks the loop

`run-reactor.sh` reads `REACTOR_RUNTIME` from `.context/workspace.env`:

- `eventlog` (the default) — `run-reactor.sh` `exec`s `eventlog react` with
  the flags each reactor needs: `--as cursor-committer --on result --git`
  for the committer, `--as doc-worker --on ack --filter by=cursor-committer
  --filter outcome=committed` for the doc worker. The Rust runtime then owns
  the lock, resume point, intent, veto window, and acknowledgment for that
  reactor; see [Reactor loop](../reference/react-loop.md) and [why its lock
  checks more than a pid](reactor-lock-liveness.md).
- `shell` — `run-reactor.sh` falls through to its original bash loop, which
  restarts the reactor script on a crash and escalates after 5 crashes in
  120 seconds.

Switching is a one-line config change, not a code change: set
`REACTOR_RUNTIME=shell` in `workspace.env` to fall back if the Rust runtime
ever needs to be ruled out as a cause.

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
