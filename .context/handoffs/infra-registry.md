# Upstream infrastructure work registry

Controller: Drove orchestrator, Herdr session drove, w1:p1.
Contract: .context/handoffs/infra-contract.md. Baseline: 5679da4.

| Agent | Model | Workspace / tab / pane | Worktree / branch | Status |
| --- | --- | --- | --- | --- |
| infra-setup | gpt-5.6-terra high | w7G / w7G:t1 / w7G:p1 | .worktrees/infra-setup / feat/reusable-setup | running |
| infra-runtime | gpt-5.6-terra high | w7H / w7H:t1 / w7H:p1 | .worktrees/infra-runtime / fix/action-runtime | running |
| infra-review | gpt-5.6-sol high | w7J / w7J:t1 / w7J:p1 | .worktrees/infra-review / review/reusable-infra | running |

All three interactive Codex TUIs were visually verified on the assigned model and worktree, with their initial assignment submitted and work underway. Initial Herdr start calls returned agent_not_ready due to repository/hook trust menus; the controller accepted trust for this authorized repository and the Herdr hooks just installed, then verified that all agents began working. No model downgrade or headless fallback.

The user requested dispatch in other workspaces. Tabs remain open because their assignments are in progress, not because their results were accepted. No implementation is yet accepted, committed or deployed. Read agents by their pane IDs; inspect reports and route seams, then integrate/verify before recording final results and retiring them. Worker diffs are uncommitted by policy. Independent reviewer report lives in infra-review/.context/reports/infra-review.md (under the worktree).

Controller installed missing Herdr Codex/Claude status integrations as required for native worker tracking. No live reactor was restarted or contacted. Drove production files and its coordination log were untouched by this dispatch; upstream event-log owns the new lifecycle events and briefs.
