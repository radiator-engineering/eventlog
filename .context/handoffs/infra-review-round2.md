# infra-review: round-2 gate

Controller: the Claude in the event-log `control › coordinator` pane took over
this lifecycle from the Drove orchestrator. Your round-1 report at
`.context/reports/infra-review.md` (in your worktree) is accepted as the
authoritative CHANGES_REQUESTED verdict; findings 1–6 were routed to
infra-setup (`.context/handoffs/infra-setup-fixes.md`) and finding 7 to
infra-runtime (`.context/handoffs/infra-runtime-fixes.md`). Same rules as
before: read-only on product sources, edit only your report, no log append,
commit, production action, reactor or agent operation.

Wait for the controller's "both workers reported" prompt. Then:

1. Take the refreshed hashes from the workers' reports. Confirm them against
   the worktrees (`git diff --binary | /usr/bin/shasum -a 256`, plus every
   untracked file).
2. Rebuild a fresh disposable combined copy from baseline
   `5679da46ee3f3a42f18f40ade1035513d64f382a`, including every untracked
   worker file.
3. Rerun changed-file rustfmt, strict clippy, full tests, and the focused
   acceptance probes named in each finding.
4. Mandatory gate: the native two-reactor closed loop. Source result →
   commit ack carrying the actual OID → exactly one docs invocation → docs
   result → docs commit → final ack. Zero extra docs calls, unrelated staging
   preserved, no dirty docs, no live locks.
5. Update your report in place: verdict `APPROVE` or `CHANGES_REQUESTED`,
   the hashes you approved, commands and results, limitations, and exact
   controller integration instructions (branch order, conflicts, files).

Report in your Herdr response with the verdict on the first line.
