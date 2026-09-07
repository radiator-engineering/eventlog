# infra-review: round-4 acceptance gate

Both implementation workers have reported. Resume now in your existing
worktree and model/session. Read the contract and your round-three report.
Do not append to the log, commit, edit product files, operate production
reactors, or start agents. Your only claimed output remains
`.context/reports/infra-review.md` in your worktree.

Controller independently verified:
- Setup tracked diff SHA-256:
  `715b62d368fec03c243b1f954a3af1f05c6a35ce9d6a1885512b05083c008f96`.
- Runtime tracked diff SHA-256:
  `9c9b03f3c04aaa3b4cb2995cc50d508e1f01ecd9937b53e80acb9081a9cf34e8`.
- All five setup untracked files match the hashes in your round-three report.
  Runtime has no new files. Runtime scope exception is accepted at event 650.

Setup reports custom roots now render as `manual/**,GUIDE.md`, with a Drove
render test and authorization test; changed-file rustfmt, strict Clippy and
scaffold/CLI/skill tests passed. Independently assess the full changed behavior.
In particular, `docs_claim_paths` uses a filename extension to distinguish
files from directories: check extensionless files and dotted directory roots
against actual claim and action semantics before deciding this is correct.

Rebuild a fresh disposable combined copy from baseline
`5679da46ee3f3a42f18f40ade1035513d64f382a` with these exact patches and new
files. Follow your report's round-four gate: custom configuration rendering
and custom-root authorization, changed-file rustfmt, strict Clippy, full
tests, default native closed loop and resume. Preserve prior runtime proof
where the runtime patch is byte-identical. No production probes.

Update your report in place with APPROVE or CHANGES_REQUESTED, exact hashes,
commands/results, limitations and integration instructions. Return the verdict
and report path in your pane. The controller owns integration and results.
