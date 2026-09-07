# Claims: `src/cmd/claims.rs` and `eventlog claims`

Status: implemented. Compares an agent's changed files against its live
claims and reports every file the claims don't cover.

```sh
eventlog claims <agent> <base> [head]
eventlog check-claims <agent> <base> [head]   # hidden alias, same behavior
```

| Argument | Meaning |
|---|---|
| `<agent>` | Agent whose live claims to check against. |
| `<base>` | Git ref to diff from. |
| `[head]` | Git ref to diff to. Defaults to `HEAD`. |

`check-claims` runs the same code as `claims`. It's hidden from `--help` and
top-level command listings, so use `eventlog claims --help` to see its flags,
or run `eventlog check-claims` directly.

## What counts as changed

The changed-file set is the union of two git queries, run in the current
directory:

- `git diff --name-only --find-renames <base> <head>`
- `git status --porcelain`, filtered to untracked (`??`) entries

Both must run inside a git repository; `claims` fails otherwise. The command
does not stage, commit, or otherwise modify the repository.

## What counts as claimed

`claims` folds the log with `query::fold` to get the current `State`, then
reads `state.claims_for(<agent>)` — every live claim glob owned by that
agent, in claim order. A changed path is covered if any claim glob equals
the path, is a directory prefix of it (`src/` covers `src/a.rs`), or matches
it as a glob pattern (`src/**`).

If the agent has no live claims, `claims` still runs the diff and reports
every changed file as unclaimed; it prints a note to stderr rather than
treating this as an error.

## Output and exit code

Each unclaimed path prints to stdout as:

```
unclaimed <path>
```

- Exit `0`: no unclaimed files. If the agent has live claims, a summary line
  goes to stderr: `eventlog claims: <agent> stayed inside its claims
  (<base>..<head>)`.
- Exit `1`: at least one unclaimed file. A summary goes to stderr, including
  a ready-to-run `append-event.sh violation` command listing the agent and
  the comma-separated unclaimed paths.

## Tests

`tests/cmd_claims.rs` builds a temporary git repository, writes a `claim`
event for `src/**` directly to a log file, commits a change that touches
`src/a.rs` and `docs/b.md`, and leaves `docs/c.md` untracked. It asserts the
command exits `1` and lists `docs/b.md` and `docs/c.md` but not `src/a.rs`.
Run it with:

```sh
cargo test
```

## See also

- Query module — `State::claims_for`, which `claims` reads.
- [`eventlog` command list](eventlog-cli-surface.md)
