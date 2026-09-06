# Rule voter: `src/react/voter.rs`

Status: `authorize`, `check`, and `veto_binds` are implemented. Spec section
7, step 4. `src/react/mod.rs` calls these three functions as steps 4.1, 4.3,
and 4.4 of a reactor's pass over one driving event — see the `Steps` trait in
[`src/react/mod.rs`](../../src/react/mod.rs), not covered here.

## `authorize` — step 4.1, the authorized set

```rust
pub struct Authorized {
    pub paths: Vec<RelPath>,
    pub excess: Vec<RelPath>,
    pub subject: String,
}

pub fn authorize(driving: &Event, state: &State) -> Authorized;
```

`authorize` splits a driving event's `paths=` into what the reactor may
touch and what its writer named but never claimed. `subject` is whose work
the driving event is about: its `agent` field, or if that is absent, its
writer.

- A driving event with no `by` is the controller's own: every named path is
  authorized, and `excess` is empty. The controller's `result` is its own
  declaration that the path is done.
- A driving event with a `by` is intersected with `state.claims_for(driving.writer())`.
  A path covered by one of those claims goes to `paths`; any other path goes
  to `excess`. This keeps a reactor's own `result` from widening its scope
  beyond what the controller claimed for it at spawn.

A path glob covers a target path on literal equality, on a directory prefix
(`docs` covers `docs/reference/append.md`), or on a glob match.

## `check` — step 4.3, the veto rules

```rust
pub enum Veto {
    UnclaimedPaths(Vec<RelPath>),
    LogOrLock(RelPath),
    ClaimedByOther { path: RelPath, owner: String },
    OpenEscalation(u64),
}

pub fn check(reactor: &str, auth: &Authorized, state: &State, cfg: &Config) -> Result<(), Veto>;
```

`check` runs four rules, in this order, against the fold at the moment of
the check:

| Rule | Vetoes when | `reason=` on the `veto` line |
|---|---|---|
| Unclaimed paths | `auth.excess` is not empty | `unclaimed-paths` |
| Log or lock | a path in `auth.paths` is the log file or one of its lock dirs | `log-or-lock` |
| Claimed by another | a path in `auth.paths` has a live claim owned by an agent other than `reactor` or `auth.subject` | `claimed-by-other` |
| Open escalation | an open `escalate` names `reactor`, as either its subject or its writer | `open-escalation` |

The first rule that vetoes wins; `check` does not collect more than one
`Veto`. `Veto::reason()` returns the `reason=` word, and `Display` formats
the full detail (for example `claimed-by-other: docs/x.md is claimed by
doc-worker`).

A claim held by `auth.subject` never triggers "claimed by another": a
worker's own `result` on the paths it claimed, or the controller's `result
agent=<worker>` recording that same result, is not "other" just because the
reactor checking it is a different agent (for example the commit reactor).
A controller `result` with no `agent` that names a path some other agent
still has claimed is still vetoed.

## `veto_binds` — step 4.4, the veto window

```rust
pub fn veto_binds(events: &[Event], driving_seq: u64, since_seq: u64) -> Option<&Event>;
```

During the veto window, a reactor waits to see whether any agent appends a
`veto` for the event it is about to act on. `veto_binds` finds the first
`veto` line with `seq > since_seq` whose `for=` names `driving_seq`.
`since_seq` is normally the reactor's own `intent` for this pass, but
`veto_binds` does not check who wrote the veto or which intent the vetoer
saw — so a restart between declaring intent and acting cannot lose a veto
that arrived in between.

## Tests

`tests/react_voter.rs` (21 tests) covers spec section 7 steps 4.1, 4.3, and
4.4: a controller `result` authorizing all its named paths; a reactor
`result` naming a path outside its claims, producing `UnclaimedPaths` from
both `authorize` and `check`; a path on the log file or a lock dir producing
`LogOrLock`; a path claimed by another live agent producing
`ClaimedByOther`; a worker's own `result` on its claimed paths, or the
controller's `result agent=<worker>` recording it, passing `check` because
the claim owner matches `auth.subject`; an open `escalate` naming the
reactor producing `OpenEscalation`; and a `veto_binds` match even when the
veto's `for=` targets an intent older than `since_seq`. Run them with:

```sh
cargo test
```

## See also

- [Query module](query-module.md) — `State`, `claims_for`, and `claim_owner`, which `authorize` and `check` read.
- [Model contract](model-contract.md) — `Event`, `Config`, and `RelPath`.
