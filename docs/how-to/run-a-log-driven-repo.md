# Run a log-driven repo

This guide sets up `eventlog` on a project so that one controller agent coordinates workers through the log and a reactor commits what the log says. It assumes `eventlog` is installed (`cargo install --path .` from this repo) and the project is a git repository.

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

Write an action script that stages `$EVENTLOG_PATHS`, commits, and writes `outcome=committed` to `$EVENTLOG_OUTCOME_FILE` (the skill's `references/reactor-example.md` is a 15-line version). Run it in a terminal pane that outlives any agent's turn:

```sh
eventlog react --as committer --on result --git -- bash .context/bin/commit-action.sh
```

Then tell the log the reactor exists and what it may touch:

```sh
eventlog append spawn agent=committer role=commit-reactor runtime=eventlog-react
eventlog append claim agent=committer paths=.context/DECISIONS.md
```

The runtime holds a lock so a second instance refuses to start, resumes from its own `ack` lines after a restart, and dry-runs one event with `eventlog react test <seq> --as committer -- ...`.

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
- `veto ... reason=<rule>`: the voter refused. `unclaimed-paths` means the writer named files outside its claim; `claimed-by-other` means another open agent owns one of them.
- `ack seq_done=<seq> outcome=<o>`: the action ran and this is its result. Resume uses these lines.
- `violation paths=<list>`: the action committed files outside what the event authorized. The commit stands; this is detection.
- `observed paths=<list>`: unclaimed files turned dirty while the action ran. Someone's work in progress, no blame.

## Restarting a reactor

Append `retire agent=<name>` first, stop the process, start it again, then append `spawn` and a fresh `claim`. A retire closes the reactor's claims, and its next result is vetoed `unclaimed-paths` without a new one. Never delete a `*.reactor.lock` directory; the runtime reclaims a stale one on start.
