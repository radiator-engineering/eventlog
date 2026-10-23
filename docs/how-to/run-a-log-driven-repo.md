# Run a log-driven repo

This guide sets up `eventlog` on a project so that one controller agent coordinates workers through the log. It assumes the latest stable `eventlog` is installed (`cargo install eventlog-cli --locked`, or `cargo install --path . --locked` from a local checkout) and the project is a git repository.

Agents that act on log events without a prompt (reactors) are a separate project: [eventlog-reactors](https://github.com/radiator-engineering/eventlog-reactors). This guide does not use them.

## 1. Create the log

```sh
cd /path/to/project
eventlog setup preview   # what setup would add; writes nothing
eventlog setup apply
eventlog doctor --fix
```

`setup apply` runs `eventlog init`. It creates `.context/events.jsonl`, `.context/EVENTLOG.md` (the event vocabulary), and `.context/eventlog.toml`, and adds the log, its lock, and `.context/layout.json` to `.gitignore`. It never overwrites a file that exists. `doctor --fix` installs the tool-call guard for Claude Code, Cursor, and Codex and reports anything still unprotected. Restart the agent session so the hook loads.

Optional, for agents whose hooks you do not control:

```sh
eventlog protect          # chflags uappnd / chattr +a: appends work, everything else is refused
eventlog protect --off    # lift it before an intentional removal
```

## 2. Decide who writes

By default only the controller appends. Record any other writer as a decision, and write the same decision into `.context/DECISIONS.md`. The log points at that file; it does not replace it. For example, this lets `build-worker` append its own `result` and `progress` lines:

```sh
eventlog append decision key=log-writers "value=build-worker:result|progress" ref=.context/DECISIONS.md
```

## 3. Spawn a worker

1. Write a brief to `.context/handoffs/<name>.md`: the task, the paths it owns, and the line "do not append to the log".
2. Give it its own worktree: `git worktree add .worktrees/<name> -b <name> main`.
3. Record it:

```sh
eventlog append spawn agent=<name> model=<model> role=<role>
eventlog append prompt agent=<name> ref=.context/handoffs/<name>.md
eventlog append claim agent=<name> paths=src/foo,tests/foo.rs
```

Every other worker's brief lists those paths as off-limits. A file two workers need gets one owner; the other asks the controller for the change.

## 4. Accept a worker's result

In the worker's worktree, check that it stayed inside its claim:

```sh
eventlog claims <name> main
```

It lists changed files no claim covers and exits 1 if there are any. Record a hit as `violation agent=<name> paths=<list>` and decide whether to accept. Then:

```sh
eventlog append result agent=<name> ref=.context/handoffs/<name>.md paths=<files> summary="<one line>"
eventlog append retire agent=<name> disposition=accepted
```

Then commit the files the `result` names, as you would without the log.

## 5. Read the log

```sh
eventlog view --last 20        # recent rows
eventlog view -f               # follow
eventlog state                 # active agents, open claims, decisions
eventlog why <seq>             # what caused an event and what followed from it
eventlog tui                   # all of the above, live
```

Run any read of the log as its own command. The guard blocks a compound shell command that names the log file.
