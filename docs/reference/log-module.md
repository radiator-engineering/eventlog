# Log module: `src/log`

Status: `open`, `read`, `tail`, and `hash_line` are implemented, plus a
directory-based `Lock`. `src/log/verify.rs` builds `verify` on top of this
module — see [Verify](verify.md), not covered here. `src/log/append.rs`
builds `append` on top of this module too — see [Append](append.md).

## `Log` — reading the log file

```rust
pub struct Log { pub path: PathBuf }

impl Log {
    pub fn open(path: impl Into<PathBuf>) -> Log;
    pub fn read(&self) -> anyhow::Result<ReadReport>;
    pub fn tail(&self) -> anyhow::Result<Tail>;
    pub fn hash_line(bytes: &[u8]) -> String;
}
```

`open` does not require the file to exist yet; a missing file reads as empty
rather than erroring.

| Function | Behavior |
|---|---|
| `read()` | Parses every non-blank line with `Event::parse_line`. Returns every event that parsed, plus a list of `(line number, reason)` for every line that did not — a malformed line is reported, not fatal. |
| `tail()` | Reads only the last line, without parsing the rest of the file. Cheap enough for a writer to call before every append. |
| `hash_line(bytes)` | Hex SHA-256 of `bytes` with trailing `\r` and `\n` stripped, so a line hashes the same whether it came from a `\n`- or `\r\n`-terminated file. |

## `ReadReport` and `Tail`

```rust
pub struct ReadReport {
    pub events: Vec<Event>,
    pub malformed: Vec<(usize, String)>, // one-indexed line number, reason
}

pub struct Tail {
    pub last_seq: u64,              // 0 if the file is empty or the last line does not parse
    pub last_line: Option<Vec<u8>>, // trailing newline stripped; None if the file is empty
    pub chained: bool,              // whether the last line carries a `prev` field
    pub torn: Option<usize>,        // one-indexed line number of the last line, if it did not parse
}
```

A `torn` tail is not an error: it is how a writer detects a previous append
that was cut off partway (for example, by a crash mid-write) and decides
whether to repair it before appending its own line.

## `Lock` — one writer at a time

```rust
pub struct Lock { dir: PathBuf }

impl Lock {
    pub fn acquire(dir: &Path, wait: Duration) -> Result<Lock, LockError>;
}
impl Drop for Lock {
    fn drop(&mut self) { /* removes `dir` */ }
}

pub enum LockError {
    Busy { dir: PathBuf },
    Io { dir: PathBuf, source: std::io::Error },
}
```

`acquire` creates `dir` with `mkdir`, which is atomic, so whichever caller
creates it holds the lock. It writes its own process ID into `dir/pid`. If
`dir` already exists, `acquire` reads the pid inside: a live process means
busy (retry until `wait` elapses, then return `LockError::Busy`); a dead
process means the lock is stale, so `acquire` reclaims it and retries
immediately. Dropping a `Lock` removes `dir`, releasing it.

See [why a stale lock is reclaimed by renaming it aside, not deleting it
directly](../explanation/lock-reclaim.md).

## Fixture

`tests/fixtures/drove-events.jsonl` is a real, non-synthetic log (287
events) used to test `read` and `tail` against actual event shapes rather
than hand-built lines.

## Tests

`tests/log_read.rs` covers `hash_line`, `tail` and `read` against the
fixture, and torn/malformed line reporting. `tests/log_lock.rs` covers a
same-process double acquire (busy) and reclaim from a dead pid. Run them
with:

```sh
cargo test
```

## See also

- [Model contract](model-contract.md) — `Event`, parsed by `Log::read` and `Log::tail`.
- [Why a reclaimed lock is renamed aside before removal](../explanation/lock-reclaim.md)
- [Verify](verify.md) — `verify`, the first caller of this module.
- [Append](append.md) — `append`, which uses `Log::tail`, `Log::hash_line`, and `Lock`.
