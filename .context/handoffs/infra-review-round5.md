# infra-review: final root-policy gate

Setup finished both corrections. Continue your existing review session and
scope: product sources read-only, only your claimed report is editable.
Do not append to the log, commit, start agents or operate production reactors.

Controller recomputed the complete candidate:
- Setup tracked binary diff SHA-256:
  `aa90811ac005bf32a3017c2a59b9c6773650885f4a28e1abf87d43612645fd6d`.
- Runtime unchanged:
  `9c9b03f3c04aaa3b4cb2995cc50d508e1f01ecd9937b53e80acb9081a9cf34e8`.
- The same five setup untracked files retain every hash in your round-four
  report. Runtime has no untracked files.

Setup now preserves validated literal roots and propagates policy errors.
Worker reports changed-file rustfmt, strict Clippy, scaffold/CLI/skill tests
and whitespace checks passing. New tests:
`generated_literal_roots_start_lifecycle_and_authorize_docs_action_paths`
and `invalid_docs_roots_fail_setup_without_mutation_or_panic`.

Follow the round-four report's final gate: independently verify hashes,
build the fresh combined baseline candidate, review the two fixes, run
changed-file rustfmt, strict Clippy, full tests, actual generated Drove
lifecycle/action/authorization with extensionless/dotted/empty roots, and
invalid policy preview/apply/upgrade nonmutation. Preserve previously proven
default-loop and runtime evidence if their paths remain byte-identical;
do not repeat unrelated probes without a new concern.

Update `.context/reports/infra-review.md` in place with APPROVE or
CHANGES_REQUESTED, exact candidate hashes, evidence, limitations and exact
integration instructions. Report the verdict and path in your pane.
