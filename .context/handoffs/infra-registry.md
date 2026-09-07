# Upstream infrastructure work registry

Controller: handed to the event-log coordinator (Claude, Herdr session event-log, w6:p6) on 2026-09-07 after the round-1 review. Workers stay in Herdr session drove.
Contract: .context/handoffs/infra-contract.md. Baseline: 5679da4.

| Agent | Model | Workspace / tab / pane | Worktree / branch | Status |
| --- | --- | --- | --- | --- |
| infra-setup | gpt-5.6-terra high | w7G / w7G:t1 / w7G:p1 | .worktrees/infra-setup / feat/reusable-setup | running |
| infra-runtime | gpt-5.6-terra high | w7H / w7H:t1 / w7H:p1 | .worktrees/infra-runtime / fix/action-runtime | running |
| infra-review | gpt-5.6-sol high | w7J / w7J:t1 / w7J:p1 | .worktrees/infra-review / review/reusable-infra | running |

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
