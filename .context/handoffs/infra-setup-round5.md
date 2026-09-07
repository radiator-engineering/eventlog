# infra-setup: round-5 root-policy corrections

Continue in your existing worktree/session. Stay within your claimed paths.
Do not append to the log, commit, push, start agents, or operate production
reactors. Read the round-four reviewer report at
`/Users/jjmartin/Development/event-log/.worktrees/infra-review/.context/reports/infra-review.md`.

Two verified findings remain in your round-four patch `715b62d3...`:

1. `docs_claim_paths` must preserve validated literal roots, comma-joined.
   Literal claims cover both the root and descendants. Do not infer a file
   or directory from its extension and do not append `/**`. The current
   implementation fails strict lifecycle startup for `LICENSE` and existing
   empty directories. Add an end-to-end regression with an extensionless
   file, dotted directory, and empty directory: render the actual helper,
   execute its lifecycle claim successfully, run the stubbed docs action,
   and authorize the actual emitted paths. Replace synthetic assertions
   that merely repeat the rendered strings.
2. Propagate invalid docs-root policy as a contextual error through setup
   preview/apply/upgrade. Remove the `expect` on fallible user configuration.
   Test empty root arrays, absolute paths, escaping paths, and comma-containing
   roots: controlled nonzero exit, no panic text, no filesystem mutation.

Run changed-file rustfmt, strict Clippy and focused tests. Report tracked
binary-diff SHA-256, complete untracked file hashes, changed behavior and
check results. Runtime is already accepted and must remain unchanged.

The controller will collect your report and route the independent final gate;
do not contact another worker yourself.
