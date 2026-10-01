# Worktree facts: `eventlog worktree-facts`

Status: implemented. Tells a cleanup tool which worktrees the log still needs
and which it is finished with. [offcut](https://github.com/radiator-engineering/offcut)
calls it on its own when a repo has a log and `eventlog` is on PATH. It reads
the log and writes nothing.

```sh
git worktree list --porcelain | awk '/^worktree /{print $2}' | eventlog worktree-facts
```

## Input

Absolute worktree paths on stdin, one per line. Run it from the repo root.

## Output

One JSON line per path the log can place:

```json
{"path": "/abs/.worktrees/api", "verdict": "done", "reason": "agent api retired after a result"}
```

A path the log cannot place gets no line. That means no opinion.

| Verdict | When |
|---|---|
| `done` | The agent is retired, with or without a `result`. No one will work in its worktree again. |
| `hold` | The agent is not retired. |

## Which agent owns a path

1. A `result` that names the path in `worktree=`. A relative path resolves
   against the repo root. The newest `result` wins.
2. Otherwise the path `<repo>/.worktrees/<agent>`, when `<agent>` has a
   `spawn` on the log. This is the layout the controller's briefs use.

An agent that was retired and then spawned again is not retired, so its
worktree is held.

## What the verdict means

`done` is a hint, not an instruction. The tool that reads it still refuses to
remove a worktree with uncommitted changes, unpushed commits, a lock, or open
files. `done` only lets it skip the idle-time wait.
