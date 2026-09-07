# Reusable infrastructure review — round 5 final gate

## Verdict: APPROVE

The exact round-five combined candidate satisfies the pinned infrastructure contract. Both round-four root-policy defects are fixed: generated lifecycle claims preserve validated literal roots, and invalid root policy returns controlled failures before mutation. Static checks, strict Clippy, the full suite, independent custom-root probes, and a fresh default native loop/resume all pass. No blocking standards or contract findings remain.

## Exact approved candidate

- Contract: `/Users/jjmartin/Development/event-log/.context/handoffs/infra-contract.md`
- Gate: `/Users/jjmartin/Development/event-log/.context/handoffs/infra-review-round5.md`
- Baseline and both worker HEADs: `5679da46ee3f3a42f18f40ade1035513d64f382a`
- Fresh disposable combined tree: `/tmp/eventlog-infra-round5.ndrURe/combined`
- infra-setup tracked binary diff SHA-256: `aa90811ac005bf32a3017c2a59b9c6773650885f4a28e1abf87d43612645fd6d`
- infra-runtime tracked binary diff SHA-256: `9c9b03f3c04aaa3b4cb2995cc50d508e1f01ecd9937b53e80acb9081a9cf34e8`
- infra-runtime has no untracked files. Its `src/cmd/react.rs` scope exception was accepted by the controller at event 650.
- infra-setup untracked files:
  - `src/cmd/action.rs`: `3c0ce133d664972bb02a8942f905c339e306abf80739f95e2cb89443b432cf8c`
  - `src/cmd/lifecycle.rs`: `fa91fe9ada97475a6be4ab61f18c9d02613f70f2187147ee4613d720ae34c4cc`
  - `src/cmd/setup.rs`: `e1a29311d0b662c871fa29989cfde7caf1fb20723790c774847fddfa887282af`
  - `src/scaffold/templates/eventlog-reactors.star`: `c6c55183edda8162922ec777dd7cfe151c1cad5e745f89e62b998ad9459c769d`
  - `src/scaffold/templates/eventlog-setup.toml`: `709377a431436ea118939badedc3531d0c65e799e5c1be43f952e41613e55a9b`

All hashes and both complete worker status sets were independently recomputed. The fresh combined tree was checked out at the pinned baseline, then received the exact two binary diffs and five declared setup files. Its status contains only the approved candidate paths.

## Contract/spec review

No findings.

- `src/scaffold/mod.rs:190-204` validates each configured root and preserves the literal path in the generated claim. This matches docs-action exact-or-prefix scope and voter literal-or-directory-prefix authorization semantics.
- `src/scaffold/mod.rs:126-159` now returns and propagates root-policy errors through setup planning instead of asserting a user-input invariant.
- The new regression tests exercise an extensionless file, dotted directory, empty directory, actual lifecycle startup, docs-action paths, authorization, and invalid preview/apply/upgrade nonmutation.
- The rest of the combined implementation remains aligned with the frozen setup, lifecycle, action, loop-prevention, process-supervision, Git-accounting, and resume requirements established in prior rounds.

## Standards review

No findings. The changed files follow repository formatting and lint rules, and the round-five delta removes the prior fallible-input `expect` smell. No material Fowler-baseline smell remains in the targeted changes.

## Round-five acceptance evidence

### Static and automated checks

- PASS — `rustfmt --edition 2024 --check` on all 14 changed Rust files.
- PASS — `git diff --check`.
- PASS — `cargo clippy --all-targets --all-features -- -D warnings`.
- PASS — `cargo test --all-targets --all-features`; all executed tests passed, with one intentional OS-protection test ignored.
- PASS — focused tests `generated_literal_roots_start_lifecycle_and_authorize_docs_action_paths` and `invalid_docs_roots_fail_setup_without_mutation_or_panic` as part of the full suite.
- PASS — customized identities, models, timeouts, executable, filters, lifecycle identities, and literal `manual,GUIDE.md` roots rendered through Drove.

### Independent literal-root lifecycle/action/authorization

Disposable repository: `/tmp/eventlog-round5-roots.WVxb6D`.

- Customized roots were `LICENSE`, `docs.v2`, and an existing empty directory `empty`.
- `drove render --json` emitted the actual docs `on_start` argv with `--paths LICENSE,docs.v2,empty`.
- Executing that lifecycle command succeeded and appended the exact literal claim.
- The direct docs action modified the configured extensionless file and dotted directory file, reporting exactly `LICENSE,docs.v2/page.md`.
- A docs-worker result carrying those paths appended successfully, and `eventlog react test` authorized both paths, emitted an intent with the same set, and completed without a veto.

### Independent invalid-policy nonmutation

Disposable repository: `/tmp/eventlog-round5-invalid.0I8nsl`.

- With syntactically valid `docs.roots = []`, `setup preview`, `setup apply`, and `setup upgrade` each exited 1 without panic output.
- Before/after non-Git snapshot SHA-256 remained exactly `07fbc231e5d374f517aa4a07f3878e193aaa87ee4075e0591ccaff96dcbeafa7`.
- No helper, event log, init asset, or other file was created; the customized config remained byte-for-byte unchanged.
- The full suite additionally exercised absolute, escaping, and comma-containing roots across all three setup commands with the same no-panic/no-mutation requirements.

### Fresh default native closed loop and resume

The literal-root change altered default generated lifecycle argv, so the prior loop evidence was not reused. A new loop ran in `/tmp/eventlog-round5-loop.ivuwuQ` with the generated default claim `docs,README.md`.

- Source result seq 9 produced source commit `376ee8c968674c67557cc30e9dddbf043f4e012b`; committer ack seq 11 carried that exact OID.
- Docs ran once and emitted result seq 13 with exactly `docs/invocations.log,docs/page.md`.
- Docs commit `84268be287ce05a30fed3c049c254a9e197d529f` produced committer ack seq 16.
- The docs reactor emitted the expected origin-suppressed skipped ack at seq 18; history contained exactly baseline commit `210d16ef79315845b6a6fe1942a5fc2c2f43f72a` plus the two reactor commits.
- `unrelated.txt` remained staged, docs had no residual dirty changes, and both reactor locks disappeared after shutdown.
- Restarting both reactors left the log at 18 events, history at three commits, docs invocations at one, and unrelated staging intact; shutdown again removed both locks.

### Preserved runtime evidence

The runtime binary diff is byte-identical to rounds three and four. The prior invalid-UTF-8 stderr repro remains accepted: 1024 invalid bytes plus a diagnostic tail produced a bounded failed ack retaining the tail and exit status rather than a field-size error. The round-five full suite reran invalid-UTF-8, timeout/descendant, Git-accounting, and lock-race regressions successfully. The lock implementation also retains the prior 30/30 consecutive race-repetition evidence.

## Limitations and non-blocking notes

- `cargo fmt --all -- --check` has a known unchanged baseline failure at `tests/cmd_append.rs:148`; every changed Rust file passes standalone rustfmt.
- One OS-protection test remains intentionally ignored because it requires `chflags`/`chattr` permission.
- Semantic root failures currently print the outer setup-policy context and config path; the inner cause is retained in the error chain but not displayed by the CLI's global `{err}` formatting. Exit behavior and nonmutation are correct; richer chain formatting is a non-blocking CLI UX follow-up.
- All setup, Drove, action, lifecycle, and reactor probes ran only in disposable repositories. No production log, Drove session, reactor, lock, checkpoint, commit, or publish operation was touched.
- The code-review skill's standards/spec axes were performed directly because the final-gate handoff explicitly prohibited starting agents.

## Exact integration instructions

1. Accept infra-runtime at `9c9b03f3c04aaa3b4cb2995cc50d508e1f01ecd9937b53e80acb9081a9cf34e8` first, including the event-650 `src/cmd/react.rs` exception:
   - `src/cmd/react.rs`
   - `src/react/action.rs`
   - `src/react/lock.rs`
   - `tests/cmd_react.rs`
   - `tests/react_action.rs`
2. Accept infra-setup at `aa90811ac005bf32a3017c2a59b9c6773650885f4a28e1abf87d43612645fd6d` second:
   - `skill/event-log-coordination/SKILL.md`
   - `src/cli.rs`
   - `src/cmd/append.rs`
   - `src/cmd/mod.rs`
   - `src/scaffold/mod.rs`
   - `tests/cli_surface.rs`
   - `tests/scaffold.rs`
   - new `src/cmd/action.rs`
   - new `src/cmd/lifecycle.rs`
   - new `src/cmd/setup.rs`
   - new `src/scaffold/templates/eventlog-reactors.star`
   - new `src/scaffold/templates/eventlog-setup.toml`
3. The two accepted path sets do not overlap after the recorded runtime exception, and applying runtime before setup produced the reviewed combined tree without conflicts.
4. The controller owns result recording, integration, commits, and any production Drove recovery. Do not substitute later worker state without recomputing hashes and re-reviewing its delta.

## Reviewer-owned change

- `.context/reports/infra-review.md` only. Product sources remained read-only.
