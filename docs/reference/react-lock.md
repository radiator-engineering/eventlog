# Reactor lock: `src/react/lock.rs`

Status: `Token` and `ReactorLock` are implemented. Spec section 7, step 1.
[`Reactor::run`](react-loop.md) takes this lock, at `<log>.<name>.reactor.lock`,
before it starts polling. See [Why the reactor lock checks more than a pid](../explanation/reactor-lock-liveness.md)
for the reasoning behind `Token`; this page covers the API.

## `Token`

```rust
pub struct Token {
    pub pid: u32,
    pub start_time: String, // as the OS reports it; empty if unavailable
    pub hostname: String,
    pub boot_id: String,    // empty if the OS does not expose one
}

impl Token {
    pub fn current() -> Token;
    pub fn is_live(&self, me: &Token) -> bool;
}
```

`Token::current` reads this process's pid, start time (`ps -o lstart=`),
hostname, and boot id (`/proc/sys/kernel/random/boot_id` on Linux,
`sysctl -n kern.boottime` on macOS).

`is_live(me)` is `false` whenever `hostname` or `boot_id` differ from
`me`'s — a token from another machine, or from before this machine
rebooted, never reads as live. Otherwise: a matching pid is live only if
its start time still matches (a pid that outlived the token's process was
reused); a non-matching pid is live if `kill -0` succeeds against it.

## `ReactorLock`

```rust
pub struct ReactorLock { /* private */ }

impl ReactorLock {
    pub fn acquire(dir: &Path) -> Result<ReactorLock, LockError>;
    pub fn dir(&self) -> &Path;
}
impl Drop for ReactorLock {
    fn drop(&mut self) { /* removes `dir` */ }
}
```

`acquire` creates `dir` and writes `dir/token` as this process's
`Token::current()`, JSON-encoded. If `dir` already exists:

- No `token` file yet: another process is mid-write. Wait and retry.
- A token that `is_live` against this process: `Err(LockError::Busy)`.
- A dead token: reclaim it (rename `dir` aside to `dir.stale.<pid>-<nanos>`
  and remove it), then retry `mkdir`.

`acquire` retries up to 25 times, 20ms apart, before giving up with
`Busy`. After a fresh `mkdir` and token write, it sleeps 50ms and re-reads
`dir/token` to confirm the token is still its own — closing the one race
`mkdir` and rename cannot: a second process that read the same dead token
a moment earlier reclaiming the lock and creating its own directory in the
gap.

Reclaim itself re-reads the token before renaming anything: if the token
on disk no longer matches the one read as dead, or now reads as live, the
reclaim is skipped rather than tearing away a lock another process just
took legitimately.

Dropping a `ReactorLock` removes its directory.

## Tests

`tests/react_loop.rs` covers lock races as part of the loop's integration
tests (two `Reactor`s pointed at the same `lock_dir` cannot both `run` at
once); see [Reactor loop](react-loop.md#tests).

## See also

- [Why the reactor lock checks more than a pid](../explanation/reactor-lock-liveness.md)
- [Why a reclaimed lock is renamed aside before removal](../explanation/lock-reclaim.md) — the same rename-then-remove technique, for the plain pid-only `Lock` in `src/log`.
- [Reactor loop](react-loop.md) — `Reactor::run`, which takes this lock.
