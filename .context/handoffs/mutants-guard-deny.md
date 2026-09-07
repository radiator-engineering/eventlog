# Brief: mutants-guard-deny

You are a spawned worker named `mutants-guard-deny`. Do only this task. Do not
append to `.context/events.jsonl`, do not run `git commit`, do not start
subagents. Report back in your final message: which tests you added, the
`cargo test` result, and any mutant you believe is a real bug rather than a
test gap.

## Task

`cargo mutants` on `src/guard/deny.rs` left these mutants alive. Add tests to
`tests/guard.rs` so each one is caught. Do not change `src/`; if a mutant
survives because the code is wrong, say so in your report instead of fixing it.

```
src/guard/deny.rs:34:12: delete ! in protected_basename
src/guard/deny.rs:90:22: replace match guard c.is_whitespace() with false in words
src/guard/deny.rs:110:55: replace || with && in is_simple_sanctioned_writer
src/guard/deny.rs:110:30: replace || with && in is_simple_sanctioned_writer
src/guard/deny.rs:135:17: delete match arm '\'' | '"' in truncating_redirect_into_log
src/guard/deny.rs:168:22: replace + with * in opens_for_writing
src/guard/deny.rs:196:68: replace == with != in mutation_reason
src/guard/deny.rs:205:12: delete ! in mutation_reason
src/guard/deny.rs:207:32: replace || with && in mutation_reason
src/guard/deny.rs:207:24: replace == with != in mutation_reason
src/guard/deny.rs:207:74: replace && with || in mutation_reason
```

The quote-handling arm at line 135 matters most: it is the fix from commit
cf58b91 ("ignore shell operators inside quoted arguments") and no test pins
it. Read `src/guard/deny.rs` around each line, read the existing tests in
`tests/guard.rs` for the payload helpers, and write one focused test per
behavior: a quoted `>` that must pass, an unquoted one that must be denied,
each sanctioned-writer form, each branch of `mutation_reason`. Load the
`testing-bar` skill for what a good mutation-killing test looks like.

Verify with:

```
cargo test --test guard
cargo mutants -f src/guard/deny.rs --in-place -j 2
```

## Claimed paths

- `tests/guard.rs`

Everything else is off-limits.
