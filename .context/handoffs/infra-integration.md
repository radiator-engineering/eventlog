# Reusable infrastructure integration — 2026-09-07

Status: accepted and committed on main. Independent verdict:
`.context/reports/infra-review.md` (APPROVE, committed at `c0e9b59`).

## Exact accepted patches

- Runtime binary diff SHA-256:
  `9c9b03f3c04aaa3b4cb2995cc50d508e1f01ecd9937b53e80acb9081a9cf34e8`.
- Setup binary diff SHA-256:
  `aa90811ac005bf32a3017c2a59b9c6773650885f4a28e1abf87d43612645fd6d`.
- All five new setup files match the hashes in the final review report.
- The controller reverified all hashes before applying runtime, then setup.
  All 17 resulting product files match the approved candidate byte-for-byte,
  including after the commit reactor landed them. No integration conflicts.

## Commits and lifecycle

- `446938c`: lock token verification after reclaim rename.
- `5424e9b`: bounded action execution and retained failure diagnostics.
- `4974600`: reusable setup preview, apply and upgrade.
- `861a389`: idempotent lifecycle registration and claims.
- `15b8596`: scoped commit and documentation actions.
- `1b30158`: CLI wiring and distributed coordination skill.

Runtime result seq 688 and setup result seq 693 drove these commits. The
controller did not commit directly. The reviewer retired at seq 679, setup
at seq 681, and runtime at seq 689. Setup's acceptance-only result seq 680
preceded its retirement, releasing its broad `src/cmd` claim before runtime
integration. The exception for runtime's `src/cmd/react.rs` was already
accepted at seq 650. Worker snapshots and terminal sessions remain preserved;
they do not represent unfinished assignments.

## Validation on main

- `git diff --check`: pass.
- `rustfmt --edition 2024 --check` on all 14 changed Rust files: pass.
- `cargo clippy --all-targets --all-features -- -D warnings`: pass.
- `cargo test --all-targets --all-features`: 204 passed, 0 failed, 1 ignored.
- Before setup integration, all 28 focused runtime integration tests passed.
- Independent custom-root lifecycle/action/authorization, invalid-policy
  nonmutation, fresh native source/docs loop and resume all passed; full
  evidence and disposable repository locations are in the final report.

The required `eventlog claims <agent> main` calls reported controller-only
handoff history because worker HEADs remain at the pinned baseline; that
command compares commits and misses uncommitted tracked edits. Its untracked
directory reporting also collapsed the review report to `.context/reports`.
The controller separately audited actual binary diffs and fully expanded
untracked file lists. There are no new ownership violations: setup and review
are in scope; runtime has only the accepted event-650 exception. Recorded at
seq 687. No controller history was copied back from the older worker trees.

## Limitations

- Repository-wide `cargo fmt --all -- --check` has an unchanged baseline
  failure in `tests/cmd_append.rs:148`; changed files pass.
- The OS-protection test remains intentionally ignored.
- Invalid-policy errors preserve the inner cause, but global CLI formatting
  currently displays only outer context; reviewer marked richer diagnostics
  as a non-blocking follow-up.
- No release, publish, production Drove cutover, installed-binary replacement,
  or production-reactor restart was performed by this integration.
