# Run a log-driven repo

This guide sets up `eventlog` on a project so that one controller agent coordinates workers through the log and a reactor commits what the log says. It assumes `eventlog` is installed (`cargo install eventlog-cli --locked`, or `cargo install --path . --locked` from a local checkout) and the project is a git repository.

## 1. Create the log

```sh
cd /path/to/project
eventlog init
eventlog doctor --fix
```

`init` creates `.context/events.jsonl`, `.context/EVENTLOG.md` (the event vocabulary), and `.context/eventlog.toml`, and adds the log and its locks to `.gitignore`. `doctor --fix` installs the tool-call guard for Claude Code, Cursor, and Codex and reports anything still unprotected. Restart the agent session so the hook loads.

Optional, for agents whose hooks you do not control:

```sh
eventlog protect          # chflags uappnd / chattr +a: appends work, everything else is refused
eventlog protect --off    # lift it before an intentional removal
```

## 2. Decide who writes

By default only the controller appends. Any reactor that must write its own `ack` lines is a recorded exception:

```sh
eventlog append decision key=log-writers value=controller-plus-reactors ref=.context/DECISIONS.md
```

Write the decision into `.context/DECISIONS.md` as well. The log points at that file; it does not replace it.

## 3. Start a commit reactor

Run `eventlog setup preview` to see what a reusable setup would create, then `eventlog setup apply`. This writes `.context/eventlog-setup.toml` (reactor identities, models, timeouts, documentation roots) and `.context/eventlog-reactors.star`, a Drove helper you load into an existing `Drovefile`:

```python
load(".context/eventlog-reactors.star", "eventlog_reactors")
main = workspace("main", panes = eventlog_reactors())
```

That helper wires `eventlog lifecycle start`/`stop` to each pane's `on_start`/`on_stop`, so a supervisor spawns and retires the reactor and its claims without you hand-appending `spawn`, `claim`, and `retire`, and wires each pane's `serve` command to `eventlog react --as committer --on result --git -- eventlog action commit`, the packaged action that stages and commits exactly `$EVENTLOG_PATHS` and leaves unrelated staged work alone (see [Action](../reference/action.md)).

Writing your own action script is still supported: it must stage `$EVENTLOG_PATHS`, commit, and write `outcome=committed` to `$EVENTLOG_OUTCOME_FILE` (the skill's `references/reactor-example.md` is a 15-line version), and you spawn it by hand:

```sh
eventlog react --as committer --on result --git -- bash .context/bin/commit-action.sh
eventlog append spawn agent=committer role=commit-reactor runtime=eventlog-react
eventlog append claim agent=committer paths=.context/DECISIONS.md
```

Either way, the runtime holds a lock so a second instance refuses to start, resumes from its own `ack` lines after a restart, and dry-runs one event with `eventlog react test <seq> --as committer -- ...`.

## 4. Spawn a worker

1. Write a brief to `.context/handoffs/<name>.md`: the task, the paths it owns, and the line "do not append to the log".
2. Give it its own worktree: `git worktree add .worktrees/<name> -b <name> main`.
3. Record it:

```sh
eventlog append spawn agent=<name> model=<model> role=<role>
eventlog append prompt agent=<name> ref=.context/handoffs/<name>.md
eventlog append claim agent=<name> paths=src/foo,tests/foo.rs
```

Every other worker's brief lists those paths as off-limits. A file two workers need gets one owner; the other asks the controller for the change.

## 5. Accept a worker's result

In the worker's worktree, check that it stayed inside its claim:

```sh
eventlog claims <name> main
```

It lists changed files no claim covers and exits 1 if there are any. Record a hit as `violation agent=<name> paths=<list>` and decide whether to accept. Then:

```sh
eventlog append result agent=<name> ref=.context/handoffs/<name>.md paths=<files> summary="<one line>"
eventlog append retire agent=<name> disposition=accepted
```

The commit reactor lands the named files. Never run `git commit` yourself.

## 6. Read the log

```sh
eventlog view --last 20        # recent rows
eventlog view -f               # follow
eventlog state                 # active agents, open claims, decisions
eventlog why <seq>             # what caused an event and what a reactor did with it
eventlog tui                   # all of the above, live
```

Run any read of the log as its own command. The guard blocks a compound shell command that names the log file.

## What the reactor's lines mean

- `intent`: the reactor is about to act on event `for=` with these `paths=`.
- `veto ... reason=<rule>`: the voter refused. `unclaimed-paths` means the writer named files outside its claim; `claimed-by-other` means another open agent owns one of them. A controller event may cross a reactor's claim while that reactor is idle (no open intent); a worker's claim always binds.
- `ack seq_done=<seq> outcome=<o>`: the action ran and this is its result. Resume uses these lines.
- `violation paths=<list>`: the action committed files outside what the event authorized. The commit stands; this is detection.
- `observed paths=<list>`: unclaimed files turned dirty while the action ran. Someone's work in progress, no blame.

## Restarting a reactor

With `eventlog lifecycle stop <name>` and `start <name> --paths <...>` (see [Lifecycle](../reference/lifecycle.md)), this is one idempotent pair: `stop` retires the agent, and `start` re-spawns it and restores exactly the claims you pass, leaving every other agent's claims untouched. `eventlog-reactors.star` already wires these into each pane's `on_start`/`on_stop`.

By hand, the same steps are: append `retire agent=<name>` first, stop the process with SIGTERM or Ctrl-C (the runtime releases its lock on the way out), start it again, then append `spawn` and a fresh `claim`. A retire closes the reactor's claims. Strict append rejects a later result with `open-spawn` until a fresh spawn; a non-strict repair result can instead reach the reactor's `unclaimed-paths` veto. Never delete a `*.reactor.lock` directory; the runtime reclaims a stale one on start.
