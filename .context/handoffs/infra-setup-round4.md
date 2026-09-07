# infra-setup: round-4 fix

Same rules as before: no log append, no commit, no push, no reactors, no
agents; stay in your worktree and claimed paths. Round-3 verdict:
CHANGES_REQUESTED, one finding, yours. Report (round 3, in place):
`/Users/jjmartin/Development/event-log/.worktrees/infra-review/.context/reports/infra-review.md`.
Identities, models, timeouts and executable now propagate correctly, and the
body-hash conflict guard passes. Runtime is approved.

Reviewed patch (not accepted): tracked diff SHA-256
`796745cacf350fffc31c8c08af1d007a492dd27ed1d1213e565e70af791867a6`.

## Required fix: render configured docs roots into the docs lifecycle claim

`eventlog-setup.toml` declares `docs.roots`, but `src/scaffold/mod.rs`
deserializes commit and docs into the same `ReactorPolicy` with no `roots`
member, and `render_drove_reactors` hard-codes the docs lifecycle argument
as `--paths "docs/**,README.md"`. With `roots = ["manual", "GUIDE.md"]` the
rendered claim still says `docs/**,README.md`, so the docs action emits paths
the lifecycle never claimed and the commit reactor can reject a valid docs
result as unclaimed.

Deserialize and validate `docs.roots` as part of the effective policy and
render them into the docs lifecycle `--paths`. Match the exact path semantics
lifecycle/claim matching expects (a directory root must cover its files the
way `docs/**` does today); do not fall back to the shipped defaults silently.

Tests:
- setup/upgrade with roots such as `manual` and `GUIDE.md`, then
  `drove render`, asserting the rendered docs lifecycle claim contains those
  roots and not the defaults;
- a focused claim/authorization or closed-loop test proving a docs result
  under custom roots is accepted, not rejected as unclaimed.

## Report back

Refreshed tracked-diff SHA-256, untracked files with SHA-256, what changed,
the proving tests, and results of changed-file rustfmt, strict clippy and the
focused tests. Do not commit.
