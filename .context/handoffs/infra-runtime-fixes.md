# infra-runtime: fix requested by review (round 2)

Controller: the Claude in the event-log `control › coordinator` pane took over
this lifecycle from the Drove orchestrator. Same rules as your first brief
(`.context/handoffs/infra-runtime.md`): do not append to the log, do not
commit, push, publish, operate live reactors, or spawn agents. Stay in
`/Users/jjmartin/Development/event-log/.worktrees/infra-runtime` and your
claimed paths. Contract: `.context/handoffs/infra-contract.md` (pinned).

Verdict on the combined patch: CHANGES_REQUESTED. Report:
`/Users/jjmartin/Development/event-log/.worktrees/infra-review/.context/reports/infra-review.md`.
Finding 7 is yours; findings 1–6 belong to infra-setup. Do not touch
`src/cmd/` or `src/scaffold/`.

Reviewed patch (not accepted): tracked diff SHA-256
`36980d238481133bb936f21334068b2d35ef2dd86913d872794e59d624c9b082`.

## Required fix

7. **Keep stderr diagnostics on failure.** `src/react/action.rs` drains
   stderr and discards it. Retain a bounded, useful tail of stderr and
   include it in the failed ack `detail` (for example `exit 7: <tail>`),
   without treating stderr as outcome protocol. Preserve the existing
   outcome normalization and resume semantics. Tests: nonzero exit with a
   stderr marker, large stderr (bounded, no deadlock), and timeout with a
   descendant holding the pipe.

Also: the reviewer saw
`tests/react_loop.rs::exactly_one_of_two_racing_reclaimers_takes_a_dead_lock`
fail with two winners in a disposable copy, and pass elsewhere. It is
inherited, not attributed to you. If you can see the race in `src/react/`
lock reclaim, fix it and say so; otherwise report it as out of scope.

## Report back (in your Herdr response)

- Refreshed tracked-diff SHA-256 (`git diff --binary | /usr/bin/shasum -a 256`)
  and any untracked files with SHA-256.
- What changed, which test proves it, the commands you ran and their results
  (changed-file rustfmt, strict clippy, focused tests).
- Any dependency you added to Cargo.toml, with the reason.

Do not announce completion with untested work. Do not commit.
