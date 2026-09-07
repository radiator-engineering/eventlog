# Why a reclaimed lock is renamed aside before removal

`Lock::acquire` (reference) uses a lock
directory, not a lock file: `mkdir` is atomic, so exactly one racing caller
ever creates it. The holder's process ID goes in `dir/pid`. A second caller
that finds the directory already there has two cases to tell apart: the
holder is still alive (wait, or fail with `Busy`), or the holder is dead and
its lock is stale (reclaim it and try again).

## The problem this avoids

Several agents can call `acquire` on the same lock directory at once. If a
dead holder's lock were reclaimed by calling `remove_dir_all` directly,
several callers could all see the stale directory, all decide to reclaim it,
and all call `remove_dir_all` at roughly the same time. One of two bad things
can follow: one caller's `remove_dir_all` races another caller's fresh
`mkdir` and deletes a lock a third caller just legitimately acquired, or two
callers both believe they now hold the lock because both saw the directory
gone and both `mkdir`-ed successfully in sequence with no way to tell they
were not first.

## The fix

`reclaim` renames the lock directory aside, to a name with the caller's own
pid and a timestamp, before removing anything:

```rust
fn reclaim(dir: &Path) {
    // ... build a `dir.stale-<pid>-<nanos>` path ...
    if fs::rename(dir, &stale).is_ok() {
        let _ = fs::remove_dir_all(&stale);
    }
}
```

`rename` is atomic, the same way `mkdir` is: if several callers race to
rename the same source path, exactly one succeeds and the rest get an error
for a source that no longer exists. Only the winner proceeds to remove the
directory (now safely under its own private name) and loop back to `mkdir`
it fresh. Everyone else's `rename` fails, so they fall through to retrying
`acquire` from the top rather than deleting anything. At most one caller
ever removes a given stale lock, so a fresh, legitimately-held lock is never
mistaken for the stale one and swept up by a late reclaimer.

## Why "dead" errs toward "alive"

`pid_alive` checks a process with `kill -0` and treats any failure to tell —
not just a live process — as alive:

```rust
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(true)
}
```

Reclaiming a lock that is actually still held breaks mutual exclusion
outright: two processes would append to the log at once. Failing to reclaim
a lock that is actually dead only costs a caller the `wait` duration before
it gives up with `Busy`. The two mistakes are not equally costly, so the
function is written to make the cheaper one whenever it cannot be sure.
