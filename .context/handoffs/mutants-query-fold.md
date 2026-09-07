# Brief: mutants-query-fold

You are a spawned worker named `mutants-query-fold`. Do only this task. Do not
append to `.context/events.jsonl`, do not run `git commit`, do not start
subagents. Report back in your final message: which tests you added, the
`cargo test` result, and any mutant you believe is a real bug rather than a
test gap.

## Task

`cargo mutants` on `src/query/mod.rs` left these mutants alive. Add tests to
`tests/query_fold.rs` so each one is caught. Do not change `src/`; if a mutant
survives because the code is wrong, say so in your report instead of fixing it.

```
src/query/mod.rs:34:13: delete match arm "spawn" in Phase::of_type
src/query/mod.rs:36:13: delete match arm "claim" in Phase::of_type
src/query/mod.rs:37:13: delete match arm "progress" in Phase::of_type
src/query/mod.rs:38:13: delete match arm "result" in Phase::of_type
src/query/mod.rs:234:57: replace == with != in on_claim
src/query/mod.rs:270:50: replace > with >= in on_ack
src/query/mod.rs:312:29: replace || with && in note_orphan
src/query/mod.rs:315:34: replace == with != in note_orphan
```

Read `src/query/mod.rs` around each line, read the existing tests in
`tests/query_fold.rs` for the style and helpers, and write one focused test
per behavior (a phase per event type, claim ownership comparison, the ack
`last_ack_seq` boundary, orphan detection). Load the `testing-bar` skill for
what a good mutation-killing test looks like.

Verify with:

```
cargo test --test query_fold
cargo mutants -f src/query/mod.rs --in-place -j 2
```

(`--in-place` avoids copying the tree; the mutants output dir is gitignored.)

## Claimed paths

- `tests/query_fold.rs`

Everything else is off-limits.

## Contracts

`src/query/mod.rs` is frozen (decision `query-contract`, seq 163). Tests
only.
