# Reusable infrastructure review — restart handoff

Status: PAUSED FOR FRESH CONTEXT

## Pinned inputs

- Baseline: `5679da46ee3f3a42f18f40ade1035513d64f382a`
- Contract: `/Users/jjmartin/Development/event-log/.context/handoffs/infra-contract.md`
- Setup tree: `/Users/jjmartin/Development/event-log/.worktrees/infra-setup`
- Runtime tree: `/Users/jjmartin/Development/event-log/.worktrees/infra-runtime`
- Latest disposable combined copy: `/tmp/eventlog-infra-final.kFCQLF/combined`
- Setup tracked-diff SHA-256: `043784be89e3e252d5aea57d109ba1754c990c711f3fc17ec066f99bba751db5`
- Runtime tracked-diff SHA-256: `36980d238481133bb936f21334068b2d35ef2dd86913d872794e59d624c9b082`
- Setup untracked files included in the combined copy: `src/cmd/action.rs`, `src/cmd/lifecycle.rs`, `src/cmd/setup.rs`, `src/scaffold/templates/eventlog-setup.toml`.
- No product source was edited by this reviewer. The only reviewer-owned change is this report.

## Verified findings

Verdict is not yet finalized, but the pinned combined tree currently requires changes.

1. **P1 — The native result → commit → docs → commit chain stops after docs run.** `action commit` reports `paths` but no commit SHA in `ref`; `append_docs_result` requires non-empty `EVENTLOG_REF`. In `/tmp/eventlog-final-loop.88B6Yc`, a real committer and doc reactor produced one source commit and one docs invocation, but `doc_results=0`, the committer ack ref was `<missing>`, and `docs/page.md` remained dirty. The log evidence is saved at `/tmp/eventlog-final-loop-events.out`.

2. **P1 — Glob-scoped commits consume unrelated pre-staged work.** The precheck compares staged file names against literal/directory authorized strings, but does not expand glob semantics. In `/tmp/eventlog-final-staging.zGWS85`, `EVENTLOG_PATHS='src/*.rs'` with only `src/b.rs` pre-staged committed both `src/a.rs` and `src/b.rs` and left no staged entry.

3. **P1 — Docs edits outside configured roots are silently accepted.** The docs action snapshots only configured roots and never compares the rest of the repository. In `/tmp/eventlog-final-loop.88B6Yc`, a command configured with roots `docs` modified `source.txt`; the action exited 0 with `outcome=updated paths=`.

4. **P1 — Lifecycle writes invalid history by bypassing strict validation.** `src/cmd/lifecycle.rs` calls `append` with `strict: false` and no strict context. In `/tmp/eventlog-final-life.BQBoN5`, `lifecycle start worker --paths does-not-exist` exited 0; an immediate `doctor` exited 1 with `claim-path-missing`.

5. **P1 — Loop suppression and recovery-ref handling do not meet the contract.** The docs loop guard compares `EVENTLOG_AGENT` directly with the docs identity, but a committer ack has no `agent`; it must resolve `ack.seq_done` to the originating result and also support legacy origin evidence. Comma-separated refs are not validated. In `/tmp/eventlog-final-refs.eRhRZn`, `deadbeef,not-a-commit` was accepted and copied into a docs result with exit 0.

6. **P1 — Most declared setup settings are inert and Drove integration is incomplete.** The generated config declares commit/docs identity, model, timeout, roots, command, and executable, but the actions deserialize only docs identity/roots/command. Commit model/identity/timeout and docs model/timeout are unused; the printed/skill examples hard-code identities and omit configured timeouts. The skill starts the doc reactor on `result` instead of the required committer `ack`, and provides fragments rather than an exact runnable Drove declaration. `setup upgrade` preserves any valid TOML but has no versioned upgrade or previewed migration to apply.

7. **P2 — Native action stderr diagnostics are drained then discarded.** In `/tmp/eventlog-final-diag.EdUlJB`, an action wrote `unique-diagnostic` to stderr and exited 7; neither stdout nor stderr retained it, and the ack detail was only `exit 7`.

## Positive evidence

- Fresh setup in `/tmp/eventlog-final-fresh.xFtmoU`: created init assets/log, repeated as `setup: no changes`, and preserved an existing Drovefile.
- Disposable Drove setup in `/tmp/eventlog-final-drove.u83vQx/repo`: setup preserved Drovefile and `drove render` exited 0.
- Valid setup customization is preserved; malformed TOML is rejected before setup writes.
- Exact-path commit test preserves unrelated staging. The failure is specifically directory/glob expansion coverage.
- Docs hashing detects additions, deletions, and a second modification to an already-dirty doc path.
- Valid lifecycle stop/start restores its claim and preserves other agents' claims.
- Runtime large-output, blocked-stdin, timeout, outcome-file, and unusual Git-name regression tests passed.
- Native descendant probe `/tmp/eventlog-final-desc.CIxIXX`: returned in one second, descendant was not alive, and ack was `failed` with `timed out after 1s`.
- Native resume probe did not duplicate an accepted commit and released reactor lock directories.

## Checks already run

- `rustfmt --edition 2024 --check` on every changed Rust file: PASS.
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS on the latest combined copy.
- `git diff --check`: PASS.
- `cargo fmt --all -- --check`: FAILS only on unchanged baseline `tests/cmd_append.rs:148` formatting.
- `cargo test --all-targets --all-features`: all tests reached the inherited `tests/react_loop.rs::exactly_one_of_two_racing_reclaimers_takes_a_dead_lock`, which failed with two winners. It had passed in the earlier combined run and passed once on the unchanged baseline, then failed repeatedly in the final disposable copy; treat it as a flaky integration limitation, not a worker-diff attribution. One protection test remains intentionally ignored.
- Contract probes listed above were run only in disposable repositories; no production reactor, log, Drove session, or checkpoint was touched.

## Remaining work / restart commands

1. Confirm the worker diffs have not changed since the hashes above:
   `git -C /Users/jjmartin/Development/event-log/.worktrees/infra-setup diff --binary | /usr/bin/shasum -a 256`
   and the equivalent command for `infra-runtime`.
2. If changed, rebuild a fresh disposable combined copy from baseline and include all four setup untracked files before reviewing.
3. Reinspect the findings above against the refreshed code. Do not edit product sources; route exact fixes through the controller/workers.
4. After fixes, rerun changed-file rustfmt, strict Clippy, full tests, fresh/Drove setup, customization/upgrade, glob and directory staged isolation, invalid lifecycle claims, out-of-root docs rejection, cumulative-ref validation, native two-reactor closed loop, model failure, timeout descendant cleanup, and resume.
5. The closed-loop gate must prove: one implementation result commit; committer ack contains the actual commit ref; exactly one docs model call; exact docs result; docs commit; final committer ack; zero additional docs calls; unrelated staging preserved; no dirty docs or live locks.
6. Final report must state `APPROVE` or `CHANGES_REQUESTED`, the refreshed diff hashes/tree evidence, commands/results, limitations, and exact controller integration instructions. With current evidence the likely verdict is `CHANGES_REQUESTED`.

HANDOFF READY
