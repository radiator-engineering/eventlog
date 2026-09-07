# Why the controller Stop hook skips claimed paths and blocks once per set

`.context/bin/controller-stop-hook.sh` is the Claude Code Stop hook installed
in the controller's pane. It runs when the controller tries to end its turn.
If files outside `.context/` changed since the controller's last `result`
event, it blocks the stop and hands back the file list, so the controller
appends the event the commit reactor needs (see [AGENTS.md](../../AGENTS.md)).

Two gaps in that check caused false blocks.

## A worker's claimed files are not the controller's debt

The controller often spawns a worker (or the doc worker reacts on its own)
that edits files under a path it claimed with `eventlog append claim`. Those
edits are real, uncommitted changes — but they are the claiming agent's
work, not the controller's. The claiming agent reports its own `result` when
it finishes. Before this fix, the hook saw the uncommitted files and blocked
the controller anyway, asking it to report changes it did not make and had
no context for.

The hook now reads the folded state and drops any changed path that falls
under another agent's open claim before deciding whether to block:

```sh
claimed="$(eventlog state --json 2>/dev/null \
  | jq -r '.open_claims[]? | select(.agent != "controller") | .path')"
```

A path matches a claim if it equals the claimed path or sits under it
(`"$f" in "$p"|"$p"/*`). If every changed file is covered by some other
agent's claim, the hook exits clean. Only files nobody has claimed — the
controller's own unreported work — can still trigger a block.

## Blocking twice for the same file set is noise

The hook re-runs on every Stop attempt. Before this fix, an unchanged set of
unreported files blocked every single time, even after the controller had
already appended the `result` for a different change, explained the
situation to the user, or was simply waiting on a worker to finish. Claude
Code's own `stop_hook_active` guard only prevents a hook from blocking twice
*inside the same turn*; it does nothing for the next turn.

The hook now remembers the last file list it blocked on, in a memo file
under `.git/` (never inside the repo tree, so it is not itself an
unreported change):

```sh
memo="$REPO/.git/eventlog-stop-hook-last"
if [ -f "$memo" ] && [ "$(cat "$memo" 2>/dev/null)" = "$list" ]; then exit 0; fi
printf '%s' "$list" > "$memo"
```

If the next turn produces the identical, comma-joined file list, the hook
exits without blocking — the controller already knows about those files. If
the list changes (a file is added, removed, or reported), the hook blocks
again with the new list.

## See also

- [AGENTS.md](../../AGENTS.md) — the coordination rules the hook enforces.
- [docs/reference/claims.md](../reference/claims.md) — `eventlog claims`, the
  command-line equivalent of checking an agent's changed files against its
  live claims.
- [Why the reactor panes hand their loop to `eventlog react`](reactor-runtime-switch.md)
  — another case where a headless agent's own commit must not trip a guard
  meant for someone else's unreported work.
