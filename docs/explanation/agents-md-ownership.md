# Why `AGENTS.md` belongs to the controller, not the doc worker

## The conflict

A `claim` is a path glob, and the reactor's veto rules give each path exactly
one live owner: `check` vetoes a reactor's action with `claimed-by-other` if
a path it would touch has a live claim held by a different agent (see
[`react-voter`](../reference/react-voter.md)). The doc worker's claim used to
include `AGENTS.md`, alongside `docs` and `README.md`. The controller also
maintains its own coordination block inside `AGENTS.md`. Both agents needed
the same file, so every controller `result` that named `AGENTS.md` was
vetoed `claimed-by-other`.

## The fix

`AGENTS.md` now has one owner: the controller (decision `agents-md-owner` in
[`.context/DECISIONS.md`](../../.context/DECISIONS.md)). The doc worker's
claim and its doc roots narrow to `docs,README.md`
(`.context/bin/doc-action.sh`'s `DOC_PATHS` default). If a shipped change
makes a line in `AGENTS.md` stale, the doc worker reports that in its result
instead of editing the file — see its brief,
[`.context/handoffs/doc-worker.md`](../../.context/handoffs/doc-worker.md).

This follows the same rule as the coordination survey behind this project:
a file two tasks both need gets exactly one owner, not a shared claim.

## See also

- [`AGENTS.md`](../../AGENTS.md) — the coordination rules the controller
  owns and maintains directly.
- [`react-voter`](../reference/react-voter.md) — `check` and the
  `claimed-by-other` veto rule this conflict triggered.
- [Why the controller Stop hook skips claimed paths](stop-hook-claims-and-memo.md)
  — another case where one agent's claim must not block another agent's
  report.
