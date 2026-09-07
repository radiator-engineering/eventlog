# infra-setup: round-3 fix

Same rules as `.context/handoffs/infra-setup-fixes.md`: no log append, no
commit, no push, no reactors, no agents; stay in your worktree and claimed
paths. Round-2 verdict: CHANGES_REQUESTED. Report (round 2, in place):
`/Users/jjmartin/Development/event-log/.worktrees/infra-review/.context/reports/infra-review.md`.
Findings 1–6 from round 1 now pass, including the native closed loop. One
finding remains for you.

Reviewed patch (not accepted): tracked diff SHA-256
`d2fcc9381e5807d3f8b935b6a68b703c1c0211bff3cb5edcf8f02bb5b6930b49` plus the
five untracked files in the report.

## Required fix: customized setup values must drive the generated Drove reactors

`eventlog-setup.toml` declares commit/docs identity, model, timeout and the
eventlog executable, but `eventlog-reactors.star` hard-codes the shipped
defaults, and neither `setup apply` nor `setup upgrade` reconciles the helper
after the TOML changes. The reviewer customized every value, ran
`setup upgrade` (printed `setup: no changes`), and `drove render --json`
still emitted the defaults.

Pick one source of truth:
- regenerate or reconcile the helper from validated TOML, with previewed
  conflict handling when the user has edited the helper, or
- make the helper read its parameters from the TOML (or from arguments the
  Drovefile passes) and delete every inert TOML setting.

Required test: customize every declared value, run the supported
setup/upgrade workflow, render a host Drovefile, and assert reactor argv,
filters, labels and model metadata, timeout, executable, and lifecycle
identity all reflect the customization. Shipped defaults stay Composer 2.5
Fast and Claude Sonnet.

## Report back

Refreshed tracked-diff SHA-256, untracked files with SHA-256, what changed,
the proving test, and results of changed-file rustfmt, strict clippy and the
focused tests. Do not commit.
