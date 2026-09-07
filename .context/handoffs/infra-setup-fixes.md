# infra-setup: fixes requested by review (round 2)

Controller: the Claude in the event-log `control › coordinator` pane took over
this lifecycle from the Drove orchestrator. Same rules as your first brief
(`.context/handoffs/infra-setup.md`): do not append to the log, do not commit,
push, publish, operate live reactors, or spawn agents. Stay in
`/Users/jjmartin/Development/event-log/.worktrees/infra-setup` and your claimed
paths. Contract: `.context/handoffs/infra-contract.md` (pinned, unchanged).

Verdict on your current patch: CHANGES_REQUESTED. Report:
`/Users/jjmartin/Development/event-log/.worktrees/infra-review/.context/reports/infra-review.md`.
Read it in full. Findings 1–6 are yours. Finding 7 belongs to infra-runtime;
do not touch `src/react/`.

Reviewed patch (do not treat as accepted): tracked diff SHA-256
`043784be89e3e252d5aea57d109ba1754c990c711f3fc17ec066f99bba751db5`, plus the
four untracked files listed in the report.

## Required fixes

1. **Commit ack carries the OID.** After `git commit`, resolve the new commit
   and emit it as the ack `ref`. The docs reactor consumes that committer ack
   and appends an exact docs result the committer can commit. Closed loop:
   source commit → ack with real OID → one docs invocation → docs result →
   docs commit → final ack. No second docs call, no dirty docs, no live locks.
2. **Glob scopes must not swallow pre-staged work.** Resolve every authorized
   pathspec to concrete paths before checking the index, or reject
   unsupported pathspec syntax. Refuse the action if any concrete target is
   already staged. Prove glob and directory scopes both preserve unrelated
   staged entries.
3. **Docs edits outside the configured roots fail.** Snapshot the whole
   repository as well as the doc roots; fail with the exact out-of-scope
   paths when the docs command changes anything else. Keep detection of
   additions, deletions and repeat modifications to already-dirty in-scope
   files.
4. **Lifecycle writes go through strict validation.** Route lifecycle appends
   through the strict path with the controller context; an invalid claim
   fails before any event is appended. Reprove idempotent stop/start claim
   renewal and preservation of other agents' claims.
5. **Loop suppression and ref validation.** Derive origin from
   `ack.seq_done` → driving result (with the documented legacy provenance
   fallback); suppress only a docs-originated commit ack. Validate every
   member of a comma-separated ref as a real commit before appending. Tests:
   current-origin, legacy-origin, malformed member, cumulative ref.
6. **Every declared setting drives behavior.** Either wire commit/docs
   identity, model, timeout, roots, command and executable into the generated
   invocation and runtime, or remove the setting. Ship one exact, composable,
   runnable Drove declaration using the configured values, with docs driven
   by the committer `ack`, not `result`. Implement versioned, previewable
   `setup upgrade` that preserves customization or reports a conflict before
   mutating. Reprove fresh setup, repeat no-op, malformed and customized
   config, existing-Drovefile preservation, `drove render`, and one real
   previewed upgrade.

## Report back (in your Herdr response)

- Refreshed tracked-diff SHA-256
  (`git diff --binary | /usr/bin/shasum -a 256`) and the complete list of
  untracked files with their SHA-256.
- Per finding: what changed, which test proves it, and the command you ran.
- `rustfmt --edition 2024 --check` on changed files, strict clippy, and the
  focused tests: commands and results.
- Any seam you need from infra-runtime or the controller.

Do not announce completion with untested work. Do not commit.
