# Lifecycle: `eventlog lifecycle`

Status: implemented. `src/cmd/lifecycle.rs` exposes `start` and `stop`, so a
supervisor (a Drove `on_start`/`on_stop` hook, or any other process manager)
can spawn and retire a reactor or worker through argv, without hand-writing
`spawn`, `claim`, and `retire` events.

```sh
eventlog lifecycle start <agent> [--model <model>] [--role <role>] [--paths <p1,p2,...>]
eventlog lifecycle stop <agent>
```

| Flag | Meaning |
|---|---|
| `--model <model>` | Recorded on the `spawn` event, if one is written. |
| `--role <role>` | Recorded on the `spawn` event. Default `reactor`. |
| `--paths <p1,p2,...>` | Comma-separated paths or globs the agent claims. |

Call it with a literal argv list, not an interpolated shell string:

```python
on_start = ["eventlog", "lifecycle", "start", agent, "--model", model, "--paths", paths]
on_stop = ["eventlog", "lifecycle", "stop", agent]
```

## `eventlog lifecycle start` — idempotent spawn and claim

1. Validates every path in `--paths` before writing anything: a literal path
   must exist on disk, and a glob must match at least one file. An invalid
   path fails the whole command with no `spawn` or `claim` appended.
2. Appends `spawn agent=<agent> role=<role> [model=<model>]` only if `<agent>`
   has no currently open spawn (no prior `spawn` without a later `retire`).
3. Appends `claim agent=<agent> paths=<missing>` for whichever of `--paths`
   the agent does not already hold. Paths it already claims are left alone.

Restarting a reactor with `start` after a `stop` claims exactly the paths
provided in the new `--paths`; omitting it creates an unclaimed lifecycle.
Historical claims are not restored automatically. Other agents keep their claims —
`lifecycle_start_stop_start_restores_only_its_own_claim` in
`tests/scaffold.rs` covers this. A supervisor runs `start`/`stop`, not a
worker: a worker never appends to the log (see the coordination skill).

## `eventlog lifecycle stop` — retire

Appends `retire agent=<agent> disposition=stopped` if `<agent>` has an open
spawn; does nothing if it does not. Retirement needs no preceding result.
It closes the agent's claims. Strict append rejects a later result with
`open-spawn` until a fresh `start`; a non-strict repair result can reach the
reactor and be vetoed `unclaimed-paths`.

## Tests

`tests/scaffold.rs` covers: a claim naming a path that does not exist fails
before any event is appended, and every other agent's claims are unaffected;
and a `start`/`stop`/`start` cycle restores only the claims the restarted
agent passes again in `--paths`, leaving a second agent's claims untouched. Run them with:

```sh
cargo test
```

## See also

- [Setup](setup.md) — generates the Drove hooks that call `lifecycle start`/`stop`.
- [Action](action.md) — the reactor commands a lifecycle-managed agent runs.
- [`eventlog` command list](eventlog-cli-surface.md)
