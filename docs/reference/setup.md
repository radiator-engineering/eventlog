# Setup: `eventlog setup`

Status: implemented. `src/scaffold/mod.rs` holds `setup_changes` and
`apply_setup`; `src/cmd/setup.rs` exposes them as `eventlog setup`. `setup`
scaffolds the project-owned reactor policy that `eventlog lifecycle` and
`eventlog action` read: identities, models, timeouts, documentation roots,
and the invocation used in Drove hook argv.

```sh
eventlog setup preview
eventlog setup apply
eventlog setup upgrade
```

| Subcommand | Effect |
|---|---|
| `preview` | Print the files `apply` or `upgrade` would create or change. Writes nothing. |
| `apply` | Run `eventlog init` (non-destructive), then create `.context/eventlog-setup.toml` and `.context/eventlog-reactors.star` if they do not already exist. |
| `upgrade` | Validate the existing `.context/eventlog-setup.toml`, then regenerate `.context/eventlog-reactors.star` from it. Fails before writing anything if the config does not parse. |

None of the three touches `.context/events.jsonl`, restarts a running
reactor, or advances a checkpoint.

## `.context/eventlog-setup.toml`

Project-managed policy, separate from `.context/eventlog.toml` (the log
vocabulary):

```toml
version = 2

[commit]
identity = "committer"
model = "composer-2.5-fast"
timeout = "600s"

[docs]
identity = "doc-worker"
model = "claude-sonnet"
timeout = "600s"
roots = ["docs", "README.md"]
command = []

[invocation]
eventlog = "eventlog"
```

`docs.roots` are single, relative, on-disk paths — no glob, no comma list,
no `..` or absolute path (`invalid_docs_roots_fail_setup_without_mutation_or_panic`
in `tests/scaffold.rs` checks all four). `docs.command` is an argv list run
directly, never through a shell; leave it empty and set a local stub for
tests. `upgrade` keeps every field you edit by hand — see
`setup_is_repeatable_preserves_customization_and_rejects_malformed_config` in
`tests/scaffold.rs` — and rejects the file only when it fails to parse as
TOML against the schema above.

## `.context/eventlog-reactors.star`

A generated Drove helper, carrying a `# eventlog-reactors managed
body-sha256=<hash>` header over its body. `upgrade` regenerates the file
when the hash still matches the last generated body, or when the file holds
the older, unhashed template; it refuses to overwrite a file whose body was
hand-edited, so a customized helper is never silently discarded. Load it
into an existing Drovefile:

```python
load(".context/eventlog-reactors.star", "eventlog_reactors")
main = workspace("main", panes = eventlog_reactors())
```

`eventlog_reactors()` returns a commit-reactor tab and a docs-reactor tab,
each with `on_start` and `on_stop` panes wired to `eventlog lifecycle start`
and `stop` (see [Lifecycle](lifecycle.md)) and a `serve` pane wired to
`eventlog react ... -- eventlog action ...` (see [Action](action.md)).

## Tests

`tests/scaffold.rs` covers: repeated `apply` is a no-op; `upgrade` preserves
hand-edited settings and regenerates the Drove helper from them; `upgrade`
refuses a malformed config without changing it; a config with an invalid
`docs.roots` entry fails every `setup` subcommand without writing a file or
panicking; and a rendered `Drovefile` referencing `eventlog_reactors()`
passes `drove render`. Run them with:

```sh
cargo test
```

## See also

- [Lifecycle](lifecycle.md) — the `eventlog lifecycle start`/`stop` commands `setup`'s generated Drove hooks call.
- [Action](action.md) — the `eventlog action commit`/`docs` commands `setup`'s generated reactors run.
- [Scaffold and setup: `init`, `doctor`, `protect`](scaffold.md) — the non-destructive bootstrap `setup apply` runs first.
- [`eventlog` command list](eventlog-cli-surface.md)
