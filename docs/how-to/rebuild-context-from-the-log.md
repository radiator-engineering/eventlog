# Rebuild context from the log

The `eventlog-context` mod replaces Claude Code's compaction in the
controller's session. When the controller's context fills, the mod builds a
new context from the event log (decisions, agents, recent history, open
work) plus the last few turns word for word. No model summarizes anything,
so the rebuild takes milliseconds, and the packet holds only facts from the
log and the working tree.

This guide uses "Claude Code" for the engine that runs the controller's
session — never "the engine" — except where a literal value, such as
`reason=engine-fallback`, spells out the word.

This guide installs the mod, explains when it rebuilds, and shows how to
confirm that it worked.

## 1. Check the requirements

- Claude Code 2.1.278 or later.
- `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1` in the environment that starts
  Claude Code.
- An `eventlog` with the `context` command, first on `PATH`. The mod runs a
  bare `eventlog`. An older release on `PATH` has no `context` command, and
  the mod then falls back to Claude Code's compaction. Confirm with:

  ```sh
  eventlog context --help
  ```

- A trusted workspace. The mod lives under `.claude/skills/`, and Claude
  Code loads it only after you accept the workspace trust prompt for the
  repo.
- A log at `.context/events.jsonl` in the repo root.

## 2. Install

From anywhere in the repo:

```sh
eventlog context install
```

This writes the mod to `<git top level>/.claude/skills/eventlog-context/`
and prints a reminder about the two requirements above. Run it again after
you upgrade `eventlog`; it replaces the files and removes stale ones.

If function hooks are off, print the classic hook instead:

```sh
eventlog context install --classic
```

Add the printed `SessionStart` hook to `"hooks"` in `.claude/settings.json`.
The classic hook adds the packet after Claude Code's own compaction. It does
not replace the summary and does not rebuild on its own schedule. `install`
refuses to write the mod when `settings.json` or `settings.local.json`
already has the classic hook, because both would add the packet. Remove the
classic hook, or pass `--force` to write the mod anyway.

## 3. Know which sessions the mod acts in

The mod acts only in the controller's interactive session in a repo that has
the log. It does nothing:

- in headless sessions (`claude -p`, the SDK);
- when `EVENTLOG_AS` names a writer other than `controller`;
- when `LOG_DRIVEN_WORKER` is set, as in a worker a reactor launches;
- when `HERDR_PANE_ID` is set and `.context/layout.json` names a different
  pane as the controller's (the rule the controller stop hook uses);
- when `EVENTLOG_CONTEXT=off`;
- when the repo root has no `.context/events.jsonl`.

Without `.context/layout.json`, the mod cannot tell a worker's pane from the
controller's. Workers then stay safe only if each works in its own git
worktree, because the worktree has no installed copy of the mod. Keep
`.claude/skills/eventlog-context/` out of git, as this repo does, so a
worktree never inherits it. Or set `EVENTLOG_AS=<worker name>` when you spawn
a worker.

To turn it off for one session, start Claude Code with
`EVENTLOG_CONTEXT=off`. Subagent compactions always pass through to Claude
Code.

## 4. Know when it rebuilds

After each main-loop turn, the mod reads the context fill and runs:

```sh
eventlog context check --percent <fill> --growth <growth>
```

The fill is the context tokens as a percent of Claude Code's auto-compact
limit, not of the whole window. The status line's `CTX` figure uses a
different base, so the two do not agree. `--growth` is the expected rise in
fill before the next check, in percentage points. The mod sets it from the
average rise over the last five turns.

`check` applies these rules in order. The first match wins:

1. Fill below `floor_percent`: no rebuild.
2. Fill plus growth at or above `backstop_percent`: rebuild (`backstop`).
3. At a task boundary: rebuild (`boundary`). A task boundary is a new
   controller `result` since the last rebuild, no open controller intent,
   and no changed file outside `.context/` newer than that `result` — a
   file the controller itself claims counts too, unless another agent's
   open claim covers it.
4. Otherwise: no rebuild (`mid-task`).

Two things commonly block rule 3:

- **A tool cache that changes every session**, for example graft's `.ignore`.
  Commit the file so it stops changing, or add it to `.gitignore` so git
  stops reporting it.
- **The mod's own install.** `eventlog context install` writes untracked
  files under `.claude/skills/eventlog-context/`. Commit them and report
  them in a `result`, or add that path to `.gitignore`. Left untracked and
  unreported, the controller's Stop hook asks about them each time, and they
  block every boundary rebuild.

Two more ways start a rebuild:

- `/rebuild` queues a rebuild. It runs when the next turn ends, not at once.
  Send `/rebuild`, then one short prompt.
- `/compact` and Claude Code's own automatic compaction also rebuild from
  the log, at once.

If `eventlog` fails or times out, the mod lets Claude Code compact as usual
and records `reason=engine-fallback`.

If the newest turn alone is bigger than `tail_chars`, the mod keeps the packet,
your prompt for that turn and the model's final answer to it. It drops the
tool calls and their output in between, and records `reason=tail-trimmed`.
If the answer does not fit, or the turn has none yet, only the prompt stays.
The mod counts what the model reads: message text, tool inputs and tool
output, each once. If the prompt
alone is bigger than `tail_chars`, Claude Code's own compaction summarizes it
and the mod records `reason=tail-too-large`.

## 5. Tune the settings

Set any of these in `.context/eventlog.toml`. Leave out a key to keep its
default.

```toml
[context]
floor_percent = 25       # never rebuild below this fill
backstop_percent = 60    # always rebuild at this fill plus growth
keep_turns = 3           # turns kept word for word after the packet
tail_chars = 40000       # size cap for those turns; the newest turn is always kept
budget_chars = 12000     # size cap for the packet
```

`floor_percent` and `backstop_percent` must each be 0-100, and
`floor_percent` must be below `backstop_percent`. `eventlog context` and
`eventlog context check` refuse to run on a bad table; other commands keep
working, since they never read `[context]`.

## 6. Record intents so a rebuild knows the current task

The packet names the current task from the controller's newest open intent.
Record one when a task starts:

```sh
eventlog append intent msg="<task>" ref=<brief>
```

With a `ref`, the packet includes that file, capped at half of
`budget_chars`. Close the intent when the task is done:

```sh
eventlog append ack seq_done=<seq> outcome=done for=<seq>
```

`<seq>` is the intent's seq in both places. An open intent also blocks the
boundary rule, so an intent that is never closed leaves only the backstop.

## 7. Check that it worked

After a rebuild, the log has a `rebuild` event:

```sh
eventlog view --last 5
```

The row's summary column is empty. To read its fields (`trigger`, `as_of`,
`kept_turns`, `tokens_before`), use:

```sh
eventlog view --last 5 --json
```

To see the packet the controller received, run:

```sh
eventlog context
```

The output reflects the log now, so events after the rebuild also appear.

To check a reactor's own health, run `eventlog state --json` and read
`reactors`. A controller `ack` (for example closing an intent, step 6) makes
`controller` appear there too, even though the packet's own reactor section
leaves the controller out.

If no `rebuild` event appears, check the requirements in section 1. Start
Claude Code with `--debug-file <path>` and search that file for
`eventlog-context`. A mod that stays out of a session logs
`eventlog-context: inert in this session (<reason>)`.

## Related

- [Run a log-driven repo](run-a-log-driven-repo.md): the controller, the
  reactors and the log this mod reads.
- [`.context/EVENTLOG.md`](../../.context/EVENTLOG.md): the event vocabulary,
  including `rebuild`.
