# eventlog-reactors

`eventlog-reactors` is an optional add-on for an [`eventlog`](https://github.com/radiator-engineering/eventlog/blob/main/README.md) coordination log. It runs reactors: long-running agents that wait for one type of log event, act on it, and record the outcome as an `ack` in the same log. The log works without it.

## What a reactor does

For each event that matches its `--on` type and `--filter` conditions, the runtime:

1. Takes the event's `paths=` as the authorized set. An event without `paths=` authorizes nothing. If a worker or reactor wrote the event, a path outside that writer's claim ends the pass with `veto reason=unclaimed-paths`.
2. Appends `intent` with those paths.
3. Runs the rule voter. It appends a `veto`, and the action does not run, when a path is the log or a lock, when another open agent claims a path, or when an open `escalate` names the reactor.
4. Runs your action: the command after `--`. The action gets the event as JSON on stdin, and `EVENTLOG_PATHS`, `EVENTLOG_SEQ`, `EVENTLOG_OUTCOME_FILE` and other `EVENTLOG_*` variables in its environment.
5. With `--git`, appends `violation` if the action committed files outside the authorized set, and `observed` if unclaimed files turned dirty while it ran.
6. Appends `ack seq_done=<seq> outcome=<outcome>`.

A reactor holds a lock, so a second instance with the same name refuses to start. After a restart it resumes from its own `ack` lines. On its first start it acks the current end of the log and does not act on older events. See [React command](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/react-command.md).

## When you want reactors

Use a reactor when the same step must follow every event of one type, and you want the log to record that step. For example:

- Commit exactly the files each accepted `result` names, so no agent runs `git commit`.
- Update the docs after each commit.
- Run your own action on any event type, for example a deploy on `approval`.

Skip reactors if you commit by hand. Also skip them if nothing can keep a process running for the whole session: a reactor needs a terminal pane that outlives every agent's turn. A reactor started from an agent's tool shell stops when that turn ends.

## Install

```sh
cargo install eventlog-cli --locked        # `eventlog`, if you do not have it yet
cargo install eventlog-reactors --locked   # `eventlog-reactors`
```

From a checkout, run `cargo install --path . --locked` and `cargo install --path reactors --locked`. Each release also carries archives and a Homebrew formula for both binaries. The two crates share one version; install the same version of each.

## Set up

```sh
eventlog-reactors setup preview   # what setup would add; writes nothing
eventlog-reactors setup apply
```

`setup apply` first runs the log setup, the same as `eventlog setup apply`. Then it creates two files if they are missing, and adds one `.gitignore` line:

- `.context/eventlog-setup.toml`: the reactor policy. It names each reactor's identity, model and timeout, the commit command, the documentation roots and command, and the executables the helper runs (`[invocation]`).
- `.context/eventlog-reactors.star`: a Drove helper that defines the commit and docs reactor panes.
- `.context/*.reactor.lock/` in `.gitignore`.

`setup` never overwrites a file that exists, never rewrites a `Drovefile`, never restarts a reactor, never removes a lock, and never advances an `ack`. After you edit the policy, run `eventlog-reactors setup upgrade`. It keeps your policy edits and regenerates the helper. It refuses a malformed policy, and it refuses to overwrite a helper you edited by hand. See [Reactor setup](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/setup.md).

`[docs].command` is empty in the template. Set it to the command that updates your docs before you start the docs reactor; an empty command is a failed action.

Reactors append their own lines, so record them as log writers once:

```sh
eventlog append decision key=log-writers value=controller-plus-reactors ref=.context/DECISIONS.md
```

## Load the Drove helper

Load the helper into your `Drovefile` and add its panes to a workspace. In an existing workspace, add the result of `eventlog_reactors()` to its panes list.

```python
load(".context/eventlog-reactors.star", "eventlog_reactors")
main = workspace("main", panes = eventlog_reactors())
```

`eventlog_reactors()` returns two tabs, one per reactor. In each pane:

- `serve` runs `eventlog-reactors react … -- eventlog-reactors action commit` (or `action docs`).
- `on_start` runs `eventlog lifecycle start <identity> --role reactor --model <model> --paths <paths>`, which records the reactor's `spawn` and `claim`.
- `on_stop` runs `eventlog lifecycle stop <identity>`, which records its `retire`.

Without Drove, run the same two commands in a terminal pane:

```sh
eventlog lifecycle start committer --role reactor --paths .
eventlog-reactors react --as committer --on result --git -- eventlog-reactors action commit
```

## The commit and docs reactors

The **commit reactor** runs on `result`:

```sh
eventlog-reactors react --as committer --on result --git -- eventlog-reactors action commit
```

`action commit` stages and commits only the files in `EVENTLOG_PATHS`. It rejects a target path that was already staged, and it leaves unrelated staged work alone. Without a `[commit].command`, it commits with a direct message. With one, the command writes the commit in a disposable clone. The action checks each commit the command makes against the authorized paths before the commit reaches your branch.

The **docs reactor** runs on the commit reactor's `ack`:

```sh
eventlog-reactors react --as doc-worker --on ack --filter by=committer --filter outcome=committed -- eventlog-reactors action docs
```

`action docs` runs `[docs].command` as argv, with no shell, and reports the files it changed below `[docs].roots`. If the command changed nothing, the outcome is `skipped`. Otherwise it appends one `result` as the docs identity, so the commit reactor commits the docs. The docs reactor skips the `ack` for that commit, so the two reactors do not loop.

To run your own action, put any command after `--`. It can write `outcome=<outcome>` and `ref=<commit>` lines to `$EVENTLOG_OUTCOME_FILE`. With no `outcome=` line, exit 0 counts as `committed` and any other exit as `failed`. Try it on one event in a disposable repository first:

```sh
eventlog-reactors react test <seq> --as <name> --git -- <command>
```

The action runs as normal; the runtime prints its `intent` and `ack` instead of appending them. See [Action](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/action.md).

## Check the reactors

```sh
eventlog-reactors doctor
```

`doctor` prints one line for the reactor setup: `[ OK ]`; `[WARN]` when the policy is missing or the setup is out of date (the line names the command to run); or `[FAIL]` when `upgrade` would refuse, for example on a malformed policy or a hand-edited helper. It prints one line per `<log>.<name>.reactor.lock`: `[ OK ]` while its process is live, `[WARN]` when the lock is stale. It exits 1 only on `[FAIL]`. Never delete a lock directory; the runtime reclaims a stale one on start. See [Reactor lock liveness](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/reactor-lock-liveness.md).

The log commands show reactor activity once a reactor has written to the log. `eventlog state` adds a `reactors:` section, and `eventlog why <seq>` adds a `verdict:` line. The reactor's lines mean:

- `intent`: the reactor is about to act on event `for=` with these `paths=`.
- `veto ... reason=<rule>`: the voter refused. `unclaimed-paths` means the writer named files outside its claim; `claimed-by-other` means another open agent owns one of them.
- `ack seq_done=<seq> outcome=<outcome>`: the action ran, and this is its outcome. Resume reads these lines.
- `violation paths=<list>`: the action committed files outside the authorized set. The commit stands; the line records it.
- `observed paths=<list>`: unclaimed files turned dirty while the action ran. This is another agent's work in progress, with no blame.

## Migrate from eventlog 0.5

Up to eventlog 0.5, the reactors were part of `eventlog`. The log format and the event types did not change, and `decision key=log-writers value=controller-plus-reactors` still works. Reactors resume from their own `ack` lines on the new binary. Do these four steps:

1. **Rename the commands.** Install `eventlog-reactors`. Then change `eventlog react` to `eventlog-reactors react`, and `eventlog action` to `eventlog-reactors action`, in your scripts, hooks and briefs. The flags and behavior are the same.
2. **Run `eventlog-reactors setup upgrade`.** An untouched 0.5 policy, or its v1 form, upgrades to the new template. `upgrade` keeps an edited policy as is; its missing `[invocation] reactors` key defaults to `eventlog-reactors`. `upgrade` regenerates an unedited helper to run `eventlog-reactors`. A helper you edited makes `upgrade` stop with an error; change it by hand as in step 3.
3. **Change the binary in hand-written Drovefile panes.** In each reactor pane's `serve` argv, change `"eventlog", "react"` to `"eventlog-reactors", "react"`, and `"eventlog", "action"` to `"eventlog-reactors", "action"`.
4. **Add `--role reactor` to hand-written `lifecycle start` hooks.** The default role is now `agent`; in 0.5 it was `reactor`. Without the flag, a restarted reactor's `spawn` records `role=agent`.

Then restart each reactor so it runs `eventlog-reactors`. `eventlog doctor` no longer checks reactor locks, and `eventlog setup` no longer writes reactor files; use `eventlog-reactors doctor` and `eventlog-reactors setup`.

## Reference

- [React command](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/react-command.md): `eventlog-reactors react` and `react test`.
- [Action](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/action.md): `eventlog-reactors action commit` and `action docs`.
- [Reactor setup](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/setup.md): the policy file, the Drove helper, and `upgrade`.
- [Reactor lock liveness](https://github.com/radiator-engineering/eventlog/blob/main/docs/reactors/reactor-lock-liveness.md): how the runtime detects and reclaims a stale lock.
- [Lifecycle](https://github.com/radiator-engineering/eventlog/blob/main/docs/reference/lifecycle.md): `eventlog lifecycle start` and `stop`.
- [Run a log-driven repo](https://github.com/radiator-engineering/eventlog/blob/main/docs/how-to/run-a-log-driven-repo.md#6-optional-add-reactors): the how-to, with reactors as its last section.
- [Command list](https://github.com/radiator-engineering/eventlog/blob/main/docs/reference/eventlog-cli-surface.md#reactor-commands-eventlog-reactors): every `eventlog` and `eventlog-reactors` command.
