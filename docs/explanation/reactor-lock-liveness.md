# Why the reactor lock checks more than a pid

The log's own writer lock (`src/log/lock.rs`, see
[Why a reclaimed lock is renamed aside before removal](lock-reclaim.md))
stores just a pid and treats any process with that pid as the live holder.
That is enough for a lock held for the length of one append. The reactor
lock in `src/react/lock.rs` ([reference](../reference/react-lock.md)) is
held for as long as the reactor runs — hours or days — so a pid alone is
not enough to tell a live holder from a dead one.

## What a bare pid gets wrong

Two things break a pid-only check over a long-lived lock:

- **Pid reuse.** Operating systems recycle pids. A reactor that held pid
  4821 and crashed can, an hour later, have pid 4821 reassigned to an
  unrelated process. A check that only asks "is a process with this pid
  running" answers yes, and a dead lock is never reclaimed.
- **Portable and rebooted state.** `.context/events.jsonl` and its lock
  directories are ordinary files: they survive a copy to another machine
  or a reboot of the same one. A lock directory copied to a second host,
  or left over from before a restart, names a pid that means nothing on
  the machine reading it now — it may coincidentally match a running
  process that has no relation to the reactor at all.

## The fix: a token, not a pid

`Token` (`src/react/lock.rs`) stores four fields: `pid`, `start_time`,
`hostname`, and `boot_id`. `is_live` checks all four before deciding a
holder is still running:

```rust
pub fn is_live(&self, me: &Token) -> bool {
    if self.hostname != me.hostname || self.boot_id != me.boot_id {
        return false;
    }
    if self.pid == me.pid {
        return self.start_time == me.start_time;
    }
    if !pid_alive(self.pid) {
        return false;
    }
    let now = start_time(self.pid);
    !now.is_empty() && now == self.start_time
}
```

`hostname` and `boot_id` rule out a token from another machine or from
before this one rebooted, before the pid is even considered — a portable
or post-reboot token is dead regardless of what its pid says now.
`start_time` rules out pid reuse: a live process at that pid must also
report the same start time the token recorded, or the pid was recycled
and the original holder is gone.

## Why the fields degrade instead of failing closed

Not every OS exposes every field. `boot_id` reads
`/proc/sys/kernel/random/boot_id` on Linux and falls back to
`sysctl -n kern.boottime` on macOS; either can come back empty. An empty
`boot_id` compares equal to another empty `boot_id`, so on a platform that
cannot report one, the check falls back to pid and start time alone — it
degrades, rather than refusing to ever reclaim a lock, on a platform the
authors could not fully verify.

`pid_alive` makes the opposite trade-off from an unreadable field: when
`kill -0` cannot say whether a pid is running, it answers "alive". A lock
that stays held a little longer than necessary costs a caller a wait; a
lock reclaimed while its holder is still writing breaks the one guarantee
the lock exists to provide, that only one reactor acts on the log at a
time. The same reasoning governs the plain pid lock's `pid_alive`,
documented in [Why "dead" errs toward "alive"](lock-reclaim.md#why-dead-errs-toward-alive).

## See also

- [Reactor lock](../reference/react-lock.md) — `Token` and `ReactorLock` reference.
- [Why a reclaimed lock is renamed aside before removal](lock-reclaim.md) — the rename-then-remove technique both locks share.
- [Reactor loop](../reference/react-loop.md) — `Reactor::run`, which takes this lock before polling.
