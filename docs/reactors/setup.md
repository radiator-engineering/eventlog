# Reactor setup: `eventlog-reactors setup`

Status: implemented. `reactors/src/setup.rs` holds `setup_changes` and
`apply_setup`; `reactors/src/cmd/setup.rs` exposes them as
`eventlog-reactors setup`. It writes the project-owned reactor policy that
`eventlog-reactors action` reads (identities, models, timeouts,
documentation roots) and a Drove helper that starts the two reactors.

Up to eventlog 0.5, `eventlog setup` wrote these files. It no longer does:
`eventlog setup` only sets up the log (see [Setup](../reference/setup.md)).

```sh
eventlog-reactors setup preview
eventlog-reactors setup apply
eventlog-reactors setup upgrade
```

| Subcommand | Effect |
|---|---|
| `preview` | Print the files `apply` or `upgrade` would create or change. Writes nothing. |
| `apply` | Run the log setup (`eventlog init`, non-destructive), create `.context/eventlog-setup.toml` and `.context/eventlog-reactors.star` if they do not exist, and add `.context/*.reactor.lock/` to `.gitignore`. |
| `upgrade` | Validate the existing `.context/eventlog-setup.toml`, then regenerate `.context/eventlog-reactors.star` from it. Fails before it writes anything if the config does not parse. |

None of the three touches `.context/events.jsonl`, restarts a running
reactor, or advances a reactor's acks.

## `.context/eventlog-setup.toml`

Project-managed policy, separate from `.context/eventlog.toml` (the log
vocabulary):

```toml
version = 2

[commit]
identity = "committer"
model = "composer-2.5-fast"
timeout = "600s"
command = []

[docs]
identity = "doc-worker"
model = "claude-sonnet"
timeout = "600s"
roots = ["docs", "README.md"]
exclude = []
command = []

[invocation]
eventlog = "eventlog"
reactors = "eventlog-reactors"
```

`docs.roots` are single, relative, on-disk paths: no glob, no comma list,
no `..` and no absolute path
(`invalid_docs_roots_fail_setup_without_mutation_or_panic` in
`reactors/tests/setup.rs` checks all four). `docs.exclude` lists paths the
docs action leaves out of its before/after snapshot, so another process
that writes there during a docs run does not fail the action. An entry that
holds `*`, `?`, `[` or `{` is a glob matched against the whole relative path
(`*` stops at `/`, `**` crosses it); any other entry is a path prefix, so
`"graft/"` covers everything under `graft`. Entries must be relative, with
no `.`, `..` or empty segment; the docs action fails on an invalid one. The
default is empty, which keeps every path outside the log checked.

`docs.command` is an argv list. The action runs it directly, never through a
shell. Leave it empty and set a local stub for tests. `commit.command` is
the same kind of argv list. An empty list (the default) keeps
`eventlog-reactors action commit` in direct `git commit` mode. Set it to run
a model-backed commit author in a disposable clone instead; see
[Action](action.md#configured-commit-author). An argv entry that is exactly
`{model}` is replaced with `commit.model`.

`[invocation]` names the executables the generated helper runs:
`eventlog` for the lifecycle hooks, and `reactors` for the reactor loop and
its actions. `reactors` is optional and defaults to `eventlog-reactors`, so a
policy written by eventlog 0.5 still parses.

`upgrade` keeps every field you edit by hand (see
`setup_is_repeatable_preserves_customization_and_rejects_malformed_config` in
`reactors/tests/setup.rs`). It rejects the file only when the file fails to
parse as TOML against the schema above. It replaces the file only when the
file is the untouched template eventlog 0.5 wrote, or that template's v1
form.

## `.context/eventlog-reactors.star`

A generated Drove helper. Its first line is a `# eventlog-reactors managed
body-sha256=<hash>` header over the body. `upgrade` regenerates the file
when the hash still matches the body, or when the file holds the older
template that had no hash. It refuses to overwrite a file whose body you
edited, so it never discards a customized helper. Load it into an existing
Drovefile:

```python
load(".context/eventlog-reactors.star", "eventlog_reactors")
main = workspace("main", panes = eventlog_reactors())
```

`eventlog_reactors()` returns a commit-reactor tab and a docs-reactor tab.
Each pane has:

- an `on_start` hook: `<eventlog> lifecycle start <identity> --role reactor
  --model <model> --paths <paths>` (see [Lifecycle](../reference/lifecycle.md));
- an `on_stop` hook: `<eventlog> lifecycle stop <identity>`;
- a `serve` command: `<reactors> react ... -- <reactors> action commit` or
  `... action docs` (see [React command](react-command.md) and
  [Action](action.md)).

A helper that eventlog 0.5 generated ran `eventlog react` and
`eventlog action`. Run `eventlog-reactors setup upgrade` to regenerate it
with `eventlog-reactors` (`upgrade_moves_a_0_5_helper_to_the_reactors_binary`
in `reactors/tests/setup.rs` covers this).

## Tests

`reactors/tests/setup.rs` covers these cases:

- A second `apply` changes nothing.
- `upgrade` keeps hand-edited settings and regenerates the Drove helper from
  them.
- `upgrade` refuses a malformed config and leaves it unchanged.
- A config with an invalid `docs.roots` entry fails every `setup` subcommand
  without writing a file or panicking.
- The untouched 0.5 template and its v1 form upgrade to the current template.
- A rendered `Drovefile` that loads `eventlog_reactors()` passes
  `drove render`. This test needs an installed Drove, so it is ignored by
  default.

```sh
cargo test
# With Drove installed:
cargo test -p eventlog-reactors --test setup setup_helper_renders_with_drove -- --ignored
```

## See also

- [Reactors](../../reactors/README.md) — what reactors are and how to opt in.
- [Lifecycle](../reference/lifecycle.md) — the `eventlog lifecycle start`/`stop` commands the generated Drove hooks call.
- [Action](action.md) — the `eventlog-reactors action commit`/`docs` commands the generated reactors run.
- [Setup](../reference/setup.md) — `eventlog setup`, the log setup that `eventlog-reactors setup apply` runs first.
