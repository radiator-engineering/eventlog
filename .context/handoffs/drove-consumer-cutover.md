# Drove consumer migration and recovery — 2026-09-07

The user asked the event-log controller to reinstall the tested tool and
mediate with the Drove controller to finish the original setup/reset repair
in `/Users/jjmartin/Development/Drove` (`../Drove`). This is coordination
between the existing controllers, not a new worker lifecycle.

## Installed upstream

`cargo install --path . --locked --force` completed from event-log source
`e685252f7dd81bf9deabb364dc3f905b8584016d`. Installed executable:
`/Users/jjmartin/.cargo/bin/eventlog`. SHA-256 matches target/release/eventlog:
`14c65b1764eae1882581417e549588bb90be0de50aba4d1951676ce13a56bc45`.
The version string remains 0.1.0; the verified help now includes setup,
action and lifecycle. The Codex and Claude embedded skill copies were refreshed.

Upstream implementation is accepted and committed. Read
`../event-log/.context/reports/infra-review.md`,
`../event-log/.context/handoffs/infra-integration.md`, the installed CLI help,
and the distributed skill. Main checks: 204 passed, 0 failed, 1 intentional
OS-protection skip; changed-file formatting and strict Clippy pass.

## Drove controller's work

Own Drove repo changes and its log; the event-log controller will not write
that log concurrently. Resume from your `.context/reports/eventlog-handoff.md`
and `.context/reports/eventlog-cutover.md`, respecting current repo/PR rules
and the existing user authorization for audited cutover. Preserve inherited
dirty coordination artifacts. Do not revive archived migration drafts.

Use upstream setup preview/apply/upgrade, generated helper, lifecycle and
native reactor capabilities. Update active removed-shell-helper references,
current instructions and consumer wiring. Keep project-specific model
invocation thin; do not engineer another local runtime, lock manager,
checkpoint store or action framework. Preserve the intended Composer 2.5
Fast / Claude Sonnet behavior and report any unsupported upstream seam.

The first consumer assessment found that packaged `action commit` invoked
Git directly and ignored the model label. The upstream consumer repair adds
optional `[commit].command` argv: run the configured author in a disposable
clone with exact authorized files overlaid and staged, inherit driving-event
stdin, and expose `EVENTLOG_MODEL`, `EVENTLOG_PATHS` and
`EVENTLOG_COMMIT_MESSAGE`. Validate every produced commit before publication,
preserve unrelated staging, and report actual commit OIDs even when the
command exits nonzero after committing. No command retains direct Git mode.
Drove's thin Python adapter only launches its Composer/Sonnet CLI and passes
the driving event into its prompt; it does not implement another reactor.

Reconcile the verified historical commit prefix through result 471 and the
missed documentation trigger from saved evidence, preserving exact pending
coordination artifacts as fresh results. Do not erase log history or lock
directories, blindly replay broad historical scopes, or baseline away pending
work. Setup/reset here means repeatable configuration and lifecycle recovery,
not resetting the event history. Run action and recovery probes only on
disposable copies before any authorized production cutover.

Proceed with the authorized consumer repair. Report a brief initial assessment
and concrete upstream blockers promptly, then implement and verify what is
supported. Surface material cutover decisions with evidence. The event-log
controller will inspect progress and handle upstream changes if needed.

Communication: Drove controller is `herdr --session drove` pane `w1:p1`;
event-log controller is `herdr --session event-log` pane `w6:p6`.

## Consumer findings and upstream regressions

Drove's independent disposable configured probe verified bootstrap additions,
Composer model selection and stdin intent, and byte-identical unrelated
staged work. Two docs failures then blocked cutover: concurrent native commit
intent/ack appends were counted as outside-root content edits, and a no-edit
docs pass attempted to append an invalid empty result. Evidence was saved at
`/var/folders/x9/rz97sz4s75z45ym82kdfz87r0000gn/T/drove-consumer-nit8dild/`.

Upstream docs snapshots now exclude only the active log and its reserved
sidecars using the existing log-path policy. Other `.context` and outside-root
files remain checked. A no-change docs pass returns `skipped` and appends no
result. `tests/docs_snapshot.rs` reproduced the original failures before the
fix; `tests/commit_command.rs` covers configured author execution, scope,
bootstrap, concurrency, multiple commits, deletion and truthful late failure.

The Drove controller also found that `executor::up` never ran pane `on_start`.
Its bounded worker's fix was integrated and retired after 324 tests, strict
Clippy and a live disposable Herdr hook-before-command / zero-action-repeat
proof. The accepted hook patch is Drove result 528. The audited recovery
replacement boundary is now 528, with a fresh exact-path bootstrap result
above it and the separate truthful recovered committed ack through 471.
The controller owns the verified three-workspace/six-tab/seven-pane ownership
import and all production activation; upstream does not write the Drove log.

Upstream verification: `cargo test --all-targets` passed 220 tests with zero
failures and one intentional OS-protection skip. Strict all-target/all-feature
Clippy, changed-file rustfmt and `git diff --check` passed. Test and Clippy
output: `/tmp/eventlog-consumer-tests.out`, `/tmp/eventlog-consumer-clippy.out`.
The independent `configured-probe2` rerun produced docs results 533 and 537
without outside-log errors. Commit acknowledgment 539 contains both docs
updates; acknowledgment 543 correctly skips the later clean scope. Its
initial oracle required two commits. The corrected fresh probes 3 and 4
passed content, exact committed-or-clean-skipped accounting, unrelated
staging, own-doc suppression and lock cleanup. Probe 4 used the installed
accepted binary and also passed no-edit docs with a concurrent CLI append:
`skipped`, no result, unrelated staging intact.

## Accepted installation and live recovery

Upstream result 719 was acknowledged committed at 721, with commits
`3da83ed` (configured author), `1647934` (docs fixes), `e14c2ec` (skill), and
`c582632` (handoff). Reference docs followed in `9c3a1bc`. The tested release
was reinstalled and both Codex and Claude skills refreshed. Installed
`/Users/jjmartin/.cargo/bin/eventlog` and `target/release/eventlog` have SHA-256
`7bc864c0654b6a2dd3f35f888dbde90b4e49a9062cd5d665eae43b967aa7baca`.

Drove production result 530 was committed by real Composer and acknowledged
at 535 with full OIDs:

- `7213e44ec6029764840cbc14bc9f9ae21ad302cf`
- `f73659f04d995c424d213d93640711ee20d0afa5`
- `7170bcb247eff480cdf7d2f26ff9939d4f271a02`

The changed-path union exactly equals the 56 authorized paths; the source
tree and index were clean afterward. Recovered acknowledgment 531 maps to
471 with the 16 audited historical refs, and replacement acknowledgment 532
maps to 528. Only the audited existing layout IDs were imported. Native
Composer was verified before the remaining three existing panes were
reconciled, with zero creates/failures/focus changes.

Drove's final backend repair is committed as `10dde86`, with decision/report
commit `3170fb1`. It creates restartable shell panes, uses atomic input,
interrupts existing jobs and waits for a verified shell before replacement,
refuses unsupported direct-program roots, and selects the foreground group
leader instead of transient child processes. All 331 tests and strict Clippy
passed. Live disposable tests proved a new viewer PID in the same pane,
exact hook counts, zero-action repeat, and refusal to replace an INT-ignoring
job whose root PID remains unchanged and whose restart stays retryable.

Creating the restart worktree inside Drove during a live docs snapshot
correctly failed the outside-root guard (ack 538). The controller moved it
outside the repository after docs settled and reissued the unchanged audited
historical refs at 546. Native docs acknowledged that retry at 548 as
`skipped`, no documentation changes. No history was rewritten or policy
exemption introduced. Bootstrap docs were committed as `8022a9b`.

The final Drove release is installed with SHA-256
`9246868482f2a2418369e9b411d758ee6af002cc0f0a21f0eb37896dcc63b6f6`.
Production idle stop/resume released native locks, retired identities at
552/553, then restored them through Drove hooks at 554–557 in the same panes.
Checkpoints 540/546 were preserved through that proof without replay.
Repeat `up` returned `already_running`; setup reported no changes.

Final result 558 was acknowledged at 560 with exact backend/DECISIONS/report
paths. Sonnet result 562 was committed as
`9b48d3adf8a4a09ebf07e71529e4e3a19d4f055e` at acknowledgment 565, and its own
docs trigger was suppressed at 567. Independent final verification found:

- Drove root source and index clean at `9b48d3a`.
- `drove plan --json`: `in_sync`, no actions, controller adopted.
- Exactly the original three workspaces, six tabs and seven panes preserved.
- Committer checkpoint 562 and docs checkpoint 565, both with zero unacked
  events; no open intents or escalations.

The requested upstream reinstall and Drove setup/recovery repair are complete.
