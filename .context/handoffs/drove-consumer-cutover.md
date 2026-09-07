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

Important actual product detail: packaged `action commit` currently invokes
Git directly with `--message`; it does not invoke Composer. Packaged docs
requires `[docs].command` argv and fails if empty. Do not assume model labels
cause a model invocation or silently replace the intended model behavior.
Read source/help and surface the needed upstream capability if this prevents
a faithful, reusable consumer setup.

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
