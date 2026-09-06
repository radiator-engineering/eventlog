# Append: `src/log/append.rs`

Status: implemented at the library level. `append` validates one event and
writes it to the log. The `eventlog append` command (`src/cmd/append.rs`)
that will expose it on the CLI is still a stub.

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

`writer` is the caller's identity (`"controller"` or an agent name — the
future `--as` flag). `ctx` is folded coordination state, needed only for the
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
depend on `src/query`. `src/cmd/append.rs` will implement `StrictContext` for
`query::State` (see [Query module](query-module.md)) when the CLI command is
built.

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
| The log's tail is not torn (see [Log module](log-module.md)) | `TornTail(line)` |

`resolve_agent` moves an `agent` field out of `fields` and into the event's
top-level `agent`, so it is not written twice. If no `agent` field was given
and `writer` is `"controller"` and the type is `result`, it sets `agent` to
`"controller"` — matching how the controller's own `result` lines are
written today.

The controller always passes the allowlist check (see
[`Allowlist::permits`](model-contract.md)), whatever type it appends, so a
`decision key=log-writers` line can restrict other writers without ever
locking the controller out of a type such as `result`.

## Sequencing and the hash chain

A successful, non-`dry_run` append acquires a `Lock` (see [Log
module](log-module.md)) on `<log path>.log.lock`, rereads the tail under the
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

## See also

- [Log module](log-module.md) — `Log`, `Tail`, and `Lock`, which `append` builds on.
- [Model contract](model-contract.md) — `Event`, `Config`, and `Allowlist`.
- [Query module](query-module.md) — `query::State`, the intended `StrictContext` implementation.
- [`eventlog` command list](eventlog-cli-surface.md)
