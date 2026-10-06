# Run a log-driven repo

This guide sets up `eventlog` on a project so that one controller agent coordinates workers through the log. It assumes the latest stable `eventlog` is installed (`cargo install eventlog-cli --locked`, or `cargo install --path . --locked` from a local checkout) and the project is a git repository.

Sections 1 to 5 use the log alone. Section 6 adds reactors, which are optional: agents that act on log events, for example to commit the files a `result` names.

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

Then commit the files the `result` names, as you would without the log. A repo that runs the optional commit reactor (section 6) lets the reactor commit them instead; in that repo, never run `git commit` yourself.

## 5. Read the log

```sh
eventlog view --last 20        # recent rows
eventlog view -f               # follow
eventlog state                 # active agents, open claims, decisions
eventlog why <seq>             # what caused an event and what followed from it
eventlog tui                   # all of the above, live
```

Run any read of the log as its own command. The guard blocks a compound shell command that names the log file.

## 6. Optional: add reactors

A reactor is a long-running agent that waits for an event type and acts on it. The `eventlog-reactors` binary ships two: a commit reactor that commits the paths a `result` names, and a docs reactor that documents what was committed. Skip this section if you commit by hand. See [Reactors](../../reactors/README.md) for the overview.

### Install and set up

```sh
cargo install eventlog-reactors --locked   # or: cargo install --path reactors --locked
eventlog-reactors setup preview
eventlog-reactors setup apply
```

`setup apply` writes `.context/eventlog-setup.toml` (reactor identities, models, timeouts, documentation roots) and `.context/eventlog-reactors.star`, a Drove helper you load into an existing `Drovefile`, and adds `.context/*.reactor.lock/` to `.gitignore` (see [Reactor setup](../reactors/setup.md)):

```python
load(".context/eventlog-reactors.star", "eventlog_reactors")
main = workspace("main", panes = eventlog_reactors())
```

The helper wires `eventlog lifecycle start`/`stop` to each pane's `on_start`/`on_stop`, so a supervisor spawns and retires the reactor and its claims without you hand-appending `spawn`, `claim`, and `retire`. It wires each pane's `serve` command to `eventlog-reactors react --as committer --on result --git -- eventlog-reactors action commit`, the packaged action that stages and commits exactly `$EVENTLOG_PATHS` and leaves unrelated staged work alone (see [Action](../reactors/action.md)).

Reactors write their own `ack` lines, so record them as writers:

```sh
eventlog append decision key=log-writers value=controller-plus-reactors ref=.context/DECISIONS.md
```

### Write your own action

A custom action script must keep exact `$EVENTLOG_PATHS` boundaries and unrelated staging, commit, and write a truthful outcome with actual commit refs to `$EVENTLOG_OUTCOME_FILE`. The skill's `references/reactor-example.md` shows the packaged alternative. To launch a custom action, start its lifecycle before the blocking reactor command:

```sh
eventlog lifecycle start committer --role commit-reactor
eventlog-reactors react --as committer --on result --git -- bash .context/bin/commit-action.sh
```

Either way, the runtime holds a lock so a second instance refuses to start, and it resumes from its own `ack` lines after a restart. Test one event with `eventlog-reactors react test <seq> --as committer --git -- ...`. Use a disposable repository for this test: the action runs normally, while the runtime prints its coordination events instead of appending them.

### Check the reactors

```sh
eventlog-reactors doctor   # reactor setup, and each reactor lock: live or stale
eventlog state             # adds a reactors: section once a reactor has written
```

### What the reactor's lines mean

- `intent`: the reactor is about to act on event `for=` with these `paths=`.
- `veto ... reason=<rule>`: the voter refused. `unclaimed-paths` means the writer named files outside its claim; `claimed-by-other` means another open agent owns one of them. A controller event may cross a reactor's claim while that reactor is idle (no open intent); a worker's claim always binds.
- `ack seq_done=<seq> outcome=<o>`: the action ran and this is its result. Resume uses these lines.
- `violation paths=<list>`: the action committed files outside what the event authorized. The commit stands; this is detection.
- `observed paths=<list>`: unclaimed files turned dirty while the action ran. Someone's work in progress, no blame.

### Restart a reactor

Agents never restart a reactor. If one looks dead, the agent says so and the person reruns `drove up`; the Drovefile owns the layout and the reactor panes. `eventlog-reactors.star` wires `eventlog lifecycle stop <name>` and `start <name> --paths <...>` (see [Lifecycle](../reference/lifecycle.md)) into each pane's `on_stop`/`on_start`. The two are one idempotent pair: `stop` retires the agent, and `start` re-spawns it and restores exactly the claims you pass, leaving every other agent's claims untouched.

Without Drove, the person restarts a reactor by hand: append `retire agent=<name>` first, stop the process with SIGTERM or Ctrl-C (the runtime releases its lock on the way out), start it again, then append `spawn` and a fresh `claim`. A retire closes the reactor's claims. Strict append rejects a later result with `open-spawn` until a fresh spawn; a non-strict repair result can instead reach the reactor's `unclaimed-paths` veto. Never delete a `*.reactor.lock` directory; the runtime reclaims a stale one on start.
