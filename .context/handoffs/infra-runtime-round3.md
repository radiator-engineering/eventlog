# infra-runtime: round-3 fix

Same rules as `.context/handoffs/infra-runtime-fixes.md`: no log append, no
commit, no push, no reactors, no agents; stay in your worktree and claimed
paths. Round-2 verdict: CHANGES_REQUESTED. Report (round 2, in place):
`/Users/jjmartin/Development/event-log/.worktrees/infra-review/.context/reports/infra-review.md`.
Your stderr fix and the lock-reclaim fix pass (30/30 on the formerly flaky
test). One finding remains for you.

Reviewed patch (not accepted): tracked diff SHA-256
`1999e944d01f79f8252bbdfee4e5745380f6e835f49517763a33e10adbb53050`.

## Required fix: bound the encoded ack detail, not the raw bytes

`src/react/action.rs` keeps 1024 raw stderr bytes and converts them with
`from_utf8_lossy`; each invalid byte can become a three-byte replacement
character, and the result plus the `exit N` prefix exceeds the 2 KiB `detail`
field limit. Reviewer repro, in a disposable repo:

```sh
eventlog react test 7 --as binary-diag --timeout 3s -- \
  python3 -c 'import os; os.write(2, bytes([255]) * 1024); raise SystemExit(7)'
```

Result: exit 1, `eventlog: field too large: detail`, no failed ack at all.

Bound the final encoded detail string: account for the `exit N` and spillover
prefixes and any action-supplied detail, truncate on a valid UTF-8 boundary,
and guarantee the vocabulary limit cannot be exceeded. Add a reactor-level
test with invalid UTF-8 stderr proving a failed ack is appended with a
useful bounded diagnostic. Keep the large-ASCII, timeout and descendant-pipe
tests passing.

## Report back

Refreshed tracked-diff SHA-256, what changed, the proving test, and results
of changed-file rustfmt, strict clippy and the focused tests. Do not commit.
