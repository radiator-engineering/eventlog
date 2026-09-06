# Reactor loop: `src/react/mod.rs`

Status: `Reactor`, `ReactorConfig`, the `Steps` trait, and `supervise` are
implemented. Spec section 7, the loop around steps 1–7. This page covers the
loop itself — the lock (step 1), baseline and resume (steps 2–3), and one
pass over a driving event (step 4). The pass delegates its four steps to
[`Steps`](#the-steps-trait), implemented by [the rule voter](react-voter.md)
and [the action runner](react-action.md), not covered here. The lock itself
is covered in [Reactor lock](react-lock.md).

## `ReactorConfig` and `Reactor`

```rust
pub struct ReactorConfig {
    pub name: String,               // the `by=` on every line this runtime writes
    pub on: Vec<String>,            // event types to react to (`--on`)
    pub filter: Vec<(String, String)>, // extra `k=v` conditions (`--filter`)
    pub window: Duration,           // the veto window
    pub git: bool,                  // snapshot git before/after the action
    pub command: Vec<String>,
    pub pass_timeout: Duration,
}

impl Reactor {
    pub fn new(cfg: ReactorConfig, log: Log, config: Config, root: PathBuf) -> Reactor;
    pub fn with_steps(self, steps: Box<dyn Steps>) -> Reactor;
    pub fn config(&self) -> &ReactorConfig;
    pub fn root(&self) -> &Path;
    pub fn lock_dir(&self) -> PathBuf; // `<log>.<name>.reactor.lock`
    pub fn run(&mut self) -> anyhow::Result<()>;
    pub fn catch_up(&mut self) -> anyhow::Result<Vec<Event>>;
    pub fn baseline(&mut self, events: &[Event]) -> anyhow::Result<Vec<Event>>;
    pub fn handle(&mut self, driving: &Event, dry: bool) -> anyhow::Result<Vec<Event>>;
}
```

`Reactor::new` fits a placeholder `Steps` that errors on every call. A
caller that forgets `with_steps` fails on the first event instead of
silently acking everything.

`matches` decides whether an event drives this reactor: its type must be
in `cfg.on`, and every `cfg.filter` pair must match a field on the event —
`type` reads the event's type, `by` its writer, `agent` its subject, and
any other key a loose field.

## `run` and `catch_up`

`run` takes the [reactor lock](react-lock.md) at `lock_dir()`, then loops:
`catch_up`, sleep 200ms, `catch_up`, forever. It returns only on an error
acquiring the lock; the poll loop itself does not return.

`catch_up` is one pass over the whole log:

1. Fold the log to a `State` ([`query::fold`](query-module.md)).
2. If this reactor has never acked (`state.reactors[name].last_ack_seq` is
   `None`), run `baseline` and stop — see below.
3. Close every interrupted intent (below), collecting each one's driving
   seq to skip.
4. Collect every event above the reactor's last ack whose type is in
   `on`, skip the ones just closed as interrupted, and run `handle` on
   each in `seq` order.

`catch_up` returns every event the pass appended, which is how the
integration tests observe a pass without polling the log file.

## `baseline` — step 2, first start

A reactor with no `ack` of its own does not replay history: `baseline`
appends one `ack seq_done=<tip> outcome=skipped detail=baseline` naming
the log's current tip, and returns. The next `catch_up` finds
`last_ack_seq` set and proceeds normally from there.

## Interrupted intents — step 3, resume

An `intent` this reactor wrote with no `ack for=` behind it means the
runtime died between declaring intent and acking — the action may or may
not have run. `catch_up` closes each one before touching new events:

- `ack seq_done=<driving seq> for=<intent seq> outcome=interrupted`
- `escalate subject=<name> msg="interrupted after intent ... not re-run, the effect may have happened"`

The driving event is never re-run automatically; the escalation is what a
human or a later pass acts on.

## `handle` — step 4, one pass over one event

`handle(driving, dry)` runs the full pass for one driving event. With
`dry = true` (`react test`), it returns the events it would append and
writes none of them.

1. **Authorize** (`Steps::authorize`). On failure, `handle` emits a `veto`
   plus the closing `ack outcome=vetoed`, and returns.
2. **Intent**: `intent for=<driving seq> action=<name> paths=<authorized>`,
   appended before anything else so a crash after this point is visible
   as an interrupted intent on the next pass.
3. **Check** (`Steps::check`), the rule voter. `Some(reason)` vetoes the
   same way as a failed authorize.
4. **Veto window**: if `cfg.window` is non-zero, `handle` polls the log
   until a `veto` naming the driving seq appears or the window elapses.
   The veto can come from any writer, not only this reactor, and binds
   whichever intent it names — so a restart mid-window cannot lose it.
5. **Run** (`Steps::run`), bracketed by `Steps::snapshot` before and after
   when `cfg.git` is set. `outcome=retryable` gets exactly one retry; any
   other failure to run the command is captured as `outcome=failed
   detail=<error>` rather than propagated.
6. **Violation check**: paths dirty after the run that were not dirty
   before and that no authorized path covers become a
   `violation for=<driving seq> paths=<...>` line — detection, not
   prevention; the action already ran.
7. **Ack**: `ack seq_done=<driving seq> for=<intent seq> outcome=<outcome>`,
   plus every other field the outcome carried. The fields `seq`, `ts`,
   `prev`, `by`, `seq_done`, `for`, and `outcome` are dropped from the
   outcome's own fields first — the command cannot override what the loop
   already owns for this line.

## The `Steps` trait

```rust
pub trait Steps {
    fn authorize(&mut self, state: &State, driving: &Event) -> Result<Vec<String>, String>;
    fn check(&mut self, state: &State, driving: &Event, authorized: &[String]) -> Option<String>;
    fn run(&mut self, cfg: &ReactorConfig, driving: &Event, authorized: &[String], resume: u64)
        -> anyhow::Result<Outcome>;
    fn snapshot(&mut self) -> anyhow::Result<GitSnapshot>;
}
```

`authorize` and `check` map to [`voter::authorize` and `voter::check`](react-voter.md);
`run` and `snapshot` map to [`action::run` and `action::snapshot`](react-action.md).
Splitting them into a trait lets the loop's own tests
(`tests/react_loop.rs`) drive a fake `Steps` with no real command and no
git repository.

## `supervise`

```rust
pub fn supervise(
    cfg: ReactorConfig,
    log: Log,
    config: Config,
    root: PathBuf,
    steps: impl Fn() -> Box<dyn Steps>,
) -> !;
```

`supervise` builds a fresh `Reactor` and calls `run`, catching a panic as
well as an `Err` return. Each restart is recorded as
`note msg="reactor <name> restarting: <detail>"`. If 5 restarts happen
inside a 10-minute window, `supervise` appends
`escalate subject=<name> msg="reactor <name> restarted N times in M minutes; stopping"`
and exits the process with status 1 instead of restarting again.

## Tests

`tests/react_loop.rs` (7 tests) drives the loop against a fake `Steps` and
a hand-written, hash-chained log fixture: a dry run that writes nothing; a
veto that closes the event without running the action; ordering across a
resume (events already acked are not replayed); and lock races, where two
`Reactor`s pointed at the same `lock_dir` cannot both `run` at once. Run
them with:

```sh
cargo test
```

## See also

- [Reactor lock](react-lock.md) — the lock `run` takes before the poll loop starts.
- [Rule voter](react-voter.md) — `authorize` and `check`, the first `Steps` implementation.
- [Action runner](react-action.md) — `run` and `snapshot`, the second `Steps` implementation.
- [Query module](query-module.md) — `State`, `fold`, and `reactors`, which the loop reads for `last_ack_seq`.
