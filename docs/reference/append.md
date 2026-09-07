# Append: `src/log/append.rs`, `eventlog append`, `eventlog vocab`

Status: implemented. `append` validates one event and writes it to the log.
`eventlog append` (`src/cmd/append.rs`) and `eventlog vocab`
(`src/cmd/vocab.rs`) expose it and the vocabulary on the CLI.

```rust
pub struct AppendRequest {
    pub r#type: String,
    pub fields: Vec<(String, String)>,
    pub writer: String,
    pub strict: bool,
    pub dry_run: bool,
}

pub fn append(
    log: &Log,
    cfg: &Config,
    req: AppendRequest,
    ctx: Option<&dyn StrictContext>,
) -> Result<Event, AppendError>;
```

`writer` is the caller's identity (`"controller"` or an agent name, set by
the `--as` flag on `eventlog append`). `ctx` is folded coordination state, needed only for the
allowlist and for `strict` checks; pass `None` to skip both and fall back to
`cfg.writers` for the allowlist.

## `StrictContext`

```rust
pub trait StrictContext {
    fn allowlist(&self) -> &Allowlist;
    fn agent_is_open(&self, agent: &str) -> bool;
    fn claim_owner(&self, path: &str) -> Option<String>;
    fn has_open_escalation(&self, agent: &str) -> bool;
}
```

`append` reads folded state only through this trait, so `src/log` does not
depend on `src/query`. `src/cmd/append.rs` implements it as `FoldContext`, a
thin wrapper over a folded `query::State` (see Query
module).

## Validation order

`append` checks, in order, and returns the first `AppendError` it hits:

| Step | Failure |
|---|---|
| No reserved field (`seq`, `ts`, `prev`) among `req.fields` | `Reserved(field)` |
| A `by` field, if present, equals `writer`, and `writer` is not `controller` | `ByMismatch` |
| `req.r#type` is a known type in `cfg.vocabulary` | `UnknownType(type)` |
| Every required field for that type is present, and every field is either required or optional for it — the resolved `agent` satisfies a type's required `agent` field, so it does not also have to appear in `fields` | `MissingField(field)` |
| Any `paths` field parses under `model::paths::validate_paths` | `BadPath(reason)` |
| No field exceeds 2048 bytes | `FieldTooLarge(field)` |
| `writer` may write `type` under the allowlist (`ctx.allowlist()`, or `cfg.writers` when `ctx` is `None`) | `NotPermitted { writer, ty }` |
| When `req.strict` and `ctx` is given: no open escalation for `writer`; a `result`/`progress`/`claim`/`retire` names an agent `ctx.agent_is_open` reports open; a `claim`'s paths exist on disk (literal, or a glob match under the repo root) and are not already claimed by another agent | `Strict(rule)` |
| The log's tail is not torn (see Log module) | `TornTail(line)` |

`resolve_agent` moves an `agent` field out of `fields` and into the event's
top-level `agent`, so it is not written twice. If no `agent` field was given
and `writer` is `"controller"` and the type is `result`, it sets `agent` to
`"controller"` — matching how the controller's own `result` lines are
written today.

The controller always passes the allowlist check (see
`Allowlist::permits`), whatever type it appends, so a
`decision key=log-writers` line can restrict other writers without ever
locking the controller out of a type such as `result`.

## Sequencing and the hash chain

A successful, non-`dry_run` append acquires a `Lock` (see Log
module) on `<log path>.log.lock`, rereads the tail under the
lock, sets `seq = tail.last_seq + 1` and `ts` to the current UTC time, and
sets `prev` to `Log::hash_line` of the previous line's raw bytes, or
`"genesis"` if the log is empty or has no chain yet. It then writes the line
and a trailing `\n` in one `write_all`, and calls `fsync` if
`cfg.log.fsync` is set. Locking and rereading the tail after acquiring the
lock is what keeps concurrent writers from producing duplicate or
out-of-order `seq` values.

`dry_run: true` runs every check above, including the torn-tail check, and
returns the event `append` would have written — with its final `seq`, `ts`,
and `prev` — without acquiring the lock or writing anything.

## `eventlog append`

```sh
eventlog append <type> [field=value ...] [--as <writer>] [--dry-run] [--no-strict]
```

| Flag | Meaning |
|---|---|
| `--as <writer>` | Writer identity for `req.writer`. Defaults to `$EVENTLOG_AS`, or `"controller"` if that is unset. |
| `--dry-run` | Validate and print the event without writing it. |
| `--no-strict` | Skip the strict fold checks (for repair work). Sets `req.strict = false`. |

The command loads config, reads and folds the whole log into a `query::State`
via `FoldContext`, and calls `append` with that state as the `StrictContext`.
On success it prints the written (or, with `--dry-run`, the would-be-written)
event as one JSON line and exits `0`. On failure it prints the `AppendError`
to stderr and exits `2` for lock contention (`AppendError::Lock(LockError::
Busy { .. })`) or `1` for every other error.

`--help` on `append` prints a static vocabulary epilogue (required and
optional fields per type) built into the binary, not read from config — see
`eventlog vocab` below for the config-aware version.

## `eventlog vocab`

```sh
eventlog vocab [<type>] [--json]
```

Prints required and optional fields per event type from `cfg.vocabulary`,
which reflects the loaded config rather than the static list `append --help`
shows. With `<type>`, prints only that type; without it, prints every type in
`cfg.vocabulary`. `--json` prints `{"v":1,"type":...,"fields":[...],
"optional":[...]}` — one object for a single type, an array of them for all
types. Text output is `<type>: required=[...] optional=[...]` (optional
segment omitted when the type has no optional fields). Naming an unknown type
prints `eventlog vocab: unknown type: <type>` to stderr (JSON mode returns an
error instead of printing).

## Tests

`tests/log_append.rs` covers: genesis `prev` on an empty log; the second
append's `prev` hashing the first line; a reserved field; a `by` mismatch; a
writer not permitted to write a type; a field over the 2048-byte cap; the
first append after copying in a pre-chain fixture, hashing that fixture's
last line; a torn tail; and two threads appending 50 events each to the same
log with no gaps or duplicates in the resulting `seq` sequence. Run them
with:

```sh
cargo test
```

`tests/cmd_append.rs` covers the CLI layer: a `result` append prints a
`{"seq":1...}` line and creates the log file; `--dry-run` prints the same
shape without creating the file; a missing required field exits `1` with
`missing field` and the field name on stderr; `vocab result --json` lists
`"fields":["agent","ref"]`; and two processes appending 25 events each end
with 50 lines and `seq` `1..=50` with no gaps or duplicates.

## See also

- Log module — `Log`, `Tail`, and `Lock`, which `append` builds on.
- Model contract — `Event`, `Config`, and `Allowlist`.
- Query module — `query::State`, the `StrictContext` implementation `eventlog append` uses.
- [`eventlog` command list](eventlog-cli-surface.md)
