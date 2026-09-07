# Upstream infrastructure work registry

Controller: handed to the event-log coordinator (Claude, Herdr session event-log, w6:p6) on 2026-09-07 after the round-1 review. Workers moved into Herdr session event-log at the user's request; current addresses are below.
Contract: .context/handoffs/infra-contract.md. Baseline: 5679da4.

| Agent | Model | Workspace / tab / pane | Worktree / branch | Status |
| --- | --- | --- | --- | --- |
| infra-setup | gpt-5.6-terra high | w9 / w9:t1 / w9:p1 | .worktrees/infra-setup / feat/reusable-setup | round-4 reported; awaiting review |
| infra-runtime | gpt-5.6-terra high | wA / wA:t1 / wA:p1 | .worktrees/infra-runtime / fix/action-runtime | approved; awaiting integration |
| infra-review | gpt-5.6-sol high | wB / wB:t1 / wB:p1 | .worktrees/infra-review / review/reusable-infra | round-4 acceptance gate |

All three interactive Codex TUIs were visually verified on the assigned model and worktree, with their initial assignment submitted and work underway. Initial Herdr start calls returned agent_not_ready due to repository/hook trust menus; the controller accepted trust for this authorized repository and the Herdr hooks just installed, then verified that all agents began working. No model downgrade or headless fallback.

The user requested dispatch in other workspaces. Tabs remain open because their assignments are in progress, not because their results were accepted. No implementation is yet accepted, committed or deployed. Read agents by their pane IDs; inspect reports and route seams, then integrate/verify before recording final results and retiring them. Worker diffs are uncommitted by policy. Independent reviewer report lives in infra-review/.context/reports/infra-review.md (under the worktree).

Controller installed missing Herdr Codex/Claude status integrations as required for native worker tracking. No live reactor was restarted or contacted. Drove production files and its coordination log were untouched by this dispatch; upstream event-log owns the new lifecycle events and briefs.

User-requested Sol context refresh (2026-09-07): reviewer saved its findings, pinned diffs, disposable reproduction paths, completed checks, and next steps in its claimed report. Controller archived the handoff as `.context/handoffs/infra-review-refresh.md`, issued `/new` in the same w7J:p1 pane, and verified gpt-5.6-sol high with Context 0% used. Original review scope and worktree persist; fresh session resumes from the handoff. Open tab remains justified for ongoing review. Findings remain worker-reported and implementation is not accepted.

## Round 2 (2026-09-07)

Round-1 verdict: CHANGES_REQUESTED (`.worktrees/infra-review/.context/reports/infra-review.md`).
Reviewed hashes, not accepted: setup `043784be…51db5`, runtime `36980d23…9b082`.
Routed: findings 1–6 → infra-setup (`infra-setup-fixes.md`, seq 617); finding 7 → infra-runtime (`infra-runtime-fixes.md`, seq 618); re-review gate → infra-review (`infra-review-round2.md`, seq 619).
Integration waits for an `APPROVE` against refreshed hashes after the mandatory closed-loop gate.

## Round 3 (2026-09-07)

Round-2 verdict: CHANGES_REQUESTED. Round-1 findings 1–7 all pass, including the mandatory native closed loop (source commit → ack with OID → one docs call → docs result → docs commit → final ack; unrelated staging preserved; no locks left) and the lock-reclaim race fix (30/30).
Reviewed hashes, not accepted: setup `d2fcc938…30b49`, runtime `1999e944…53050`.
Two findings remain. Routed: customized setup values must drive the generated Drove reactors → infra-setup (`infra-setup-round3.md`, seq 628); invalid UTF-8 stderr overflows the ack detail limit → infra-runtime (`infra-runtime-round3.md`, seq 629). Seam seq 625 is now round-2 finding 1.
Integration order on APPROVE: infra-runtime first (4 files), then infra-setup (12 files); path sets do not overlap.

Round-3 reports (2026-09-07): setup `796745ca…67a6` (untracked set unchanged; helper rendered from TOML in `src/scaffold/mod.rs` with body-hash conflict detection). Runtime `9c9b03f3…34e8` (detail capped at 2 KiB post-decoding, UTF-8-safe front truncation; new live-reactor test). Runtime also edits `src/cmd/react.rs`, outside its claim: violation seq 636, no textual conflict with setup. Reviewer released for the round-3 gate.

## Workspace relocation (2026-09-07)

User explicitly requested moving all three workers from drove to event-log. Herdr has no cross-session workspace-transfer API, so the Drove orchestrator gracefully exited each Codex TUI, recreated its workspace in event-log, and resumed the identical Codex session in the unchanged worktree. Destination screens verified preserved transcripts/models and review continuation. Old drove workspaces w7G, w7H, w7J were closed after verification.

Current targets (all in `herdr --session event-log`): infra-setup w9:p1, infra-runtime wA:p1, infra-review wB:p1. Coordinator remains w6:p6. Update pane-status monitors to these addresses; old drove addresses are retired. Setup/runtime await review follow-up; reviewer continues the interrupted round-three gate. No product diffs were changed by relocation.

Preserved Codex session IDs: setup `01a07c4b-bd41-7623-b0fd-973cd1eda368`; runtime `01a07c4b-cf5a-7513-bdbc-19d5fa66dc02`; review `01a07c67-cc9a-7b91-a602-4acad44ebceb`.

## Round 4 (2026-09-07)

Round-3 verdict: CHANGES_REQUESTED, one finding. Runtime `9c9b03f3…34e8` passes every gate (invalid UTF-8 ack, closed loop, resume, clippy, fmt, full tests); the `src/cmd/react.rs` placement is accepted as an explicit scope exception (note after violation seq 636). Setup `796745ca…67a6` propagates identities, models, timeouts and executable, and the body-hash guard works, but `docs.roots` is not rendered into the docs lifecycle `--paths` (hard-coded `docs/**,README.md`), so a docs result under custom roots could be rejected as unclaimed.
Routed: → infra-setup (`infra-setup-round4.md`). infra-runtime idle, no further work requested. Integration on the combined APPROVE: runtime first (5 files incl. `src/cmd/react.rs`), then setup.
Panes now in `herdr --session event-log`: setup w9:p1, runtime wA:p1, review wB:p1.

Round-4 report collected by the successor Codex controller in w6:p6:
setup `715b62d368fec03c243b1f954a3af1f05c6a35ce9d6a1885512b05083c008f96`
independently verified; all five untracked hashes match round three. Runtime
remains `9c9b03f3c04aaa3b4cb2995cc50d508e1f01ecd9937b53e80acb9081a9cf34e8`.
Both implementation workers finished; review had been idle awaiting routing.
Reviewer released for the combined gate via `infra-review-round4.md`, including
verification of extensionless file and dotted directory root semantics.
