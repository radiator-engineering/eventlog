# Context rebuilt from the log: design

Date: 2026-09-28. Status: draft for review.

## Problem

A long controller session fills its context window. Claude Code then
compacts it: a model writes a summary, and the summary replaces the
conversation. This has three costs:

- The controller waits while the summary is written.
- The summary is lossy. It can drop decisions, paraphrase file paths and
  identifiers, and state facts that never happened.
- Each later compaction summarizes the previous summary, so the loss adds up.

This repo already records the facts that matter in `.context/events.jsonl`:
decisions, claims, spawned agents, results, intents. The conversation does
not need to carry them, and a summary does not need to remember them.

## Theory: context is a projection of the log

`eventlog state` folds the log into current state. A model's context can be
one more fold of the same log: a view rendered for one reader. If context is
derived from the log, it never needs to be summarized. The system discards it
and renders it again. Rendering is a local fold with no model call, so it
takes milliseconds.

The limit: the log holds only what was appended. The user's latest request
and work since the last event are not in it. Phase 1 keeps the last few
conversation turns word for word to cover that gap. Later phases make
recording an `intent` at the start of each task routine, so less depends on
the kept turns.

## Vision (future work, not built in phase 1)

1. **Context projection.** `eventlog context` renders a context packet from
   the log. (Phase 1.)
2. **Artifact events.** A new `artifact` event type (path, kind, one-line
   summary). The projection then knows which files matter and when they
   changed, without inferring it from `ref=` fields.
3. **Rebuild mod.** A Claude Code mod replaces compaction with the projection
   and decides when to rebuild. (Phase 1, minimal form.)
4. **Seed every agent from the log.** `eventlog context --for <worker>`
   renders a worker's brief, claims, and pinned decisions. A `prompt.context`
   hook in each worker session loads it. An `agent.spawn` hook does the same
   for in-process subagents and refuses a spawn that has no brief or claim.
5. **Intent discipline.** Controller rules and hooks that make an `intent`
   event the first step of every task, so a mid-task rebuild knows the task
   without relying on kept turns.

6. **Rebuild when the prompt cache is cold.** After the prompt cache
   expires, the next request rewrites it anyway, so a rebuild then costs
   almost nothing extra. This needs a spike first: compaction from
   `prompt.submit` is unproven. (From a study of the cache-tax mod.)

Each item gets its own spec and plan after phase 1 has run in real sessions.

## Phase 1 scope

In scope:

- `eventlog context`: renders the controller's packet.
- `eventlog context check`: decides whether to rebuild now.
- A new `rebuild` event type.
- A mod, `eventlog-context`, that runs the rebuild inside Claude Code.
- A classic `SessionStart` hook that adds the packet when function hooks are
  off.

Out of scope: worker packets (`--for`), artifact events, `agent.spawn`
seeding, and any change to how `CLAUDE.md` or `AGENTS.md` load.

## Grounding

The design follows the user's context engineering skills:

- `context-compression`: summaries score lowest on the artifact trail (2.2 to
  2.5 of 5). Its advice: track files in the scaffolding, not in the
  summarizer. It also recommends a sliding window of recent turns for coding
  agents, and task-boundary triggers when work has clear phases.
- `context-sharing`: "A fresh agent seeded from a good handoff beats a
  compacted survivor."
- `context-degradation`: the start and end of the context get the most
  attention, and recall in the middle falls by 10 to 40%. Quality drops
  sharply at a threshold rather than slowly, so act at about 70% of the
  point where the drop starts. Keep content that is not needed now behind a
  tool call.
- `memory-systems` and `filesystem-context`: mark each fact with when it was
  true, and check that a cached path still exists before trusting it.

## Component 1: `eventlog context`

```
eventlog context [--budget <chars>] [--json]
```

The command prints one Markdown packet for the controller. It builds the
packet from the existing `query::fold`, from `git status --porcelain` and
`git diff --stat`, and from the files that events name in `ref=`. The same
log and working tree always produce the same bytes. To keep that true, every
age in the packet is measured back from the `ts` of the newest event in the
log, not from the wall clock.

### Sections, in order

The order puts stable facts first, which also keeps the prompt cache valid,
and puts the current task last, where the model attends most.

1. **Header.** "Context rebuilt from `.context/events.jsonl` as of seq N. The
   log is the source of truth. Read an event's artifact with
   `eventlog open <seq>`; read recent events with `eventlog view --last 20`."
2. **Decisions in force.** One line each: `key=value (seq N, ref)`.
3. **Agents.** Each active agent with its phase, claims, and brief path.
4. **Recent history.** The last 15 `result`, `decision`, `violation`,
   `observed`, and `note` events, oldest first. Each line shows seq, age,
   type, agent, and the `summary` or `msg` field. A `result` line also shows
   its `paths` verbatim.
5. **Artifact index.** Each distinct path named by `ref=` in the recent
   history, with the seq of the event that named it. Paths only. The command
   marks a path that no longer exists `(missing)`.
6. **Reactor health.** Present only when a reactor has unacked events or its
   last ack is older than one day.
7. **Open work.** Open intents (msg, paths, ref), open escalations, and
   uncommitted changes outside `.context/` from `git diff --stat` and
   untracked files. Each changed file under another agent's open claim is
   marked with that agent's name.
8. **Current task.** The text of the file named by the controller's newest
   open intent's `ref`, when that intent has one. Reactor and worker intents
   do not count. This is the only file the packet inlines. The command
   inlines at most half of `budget_chars` characters of it, so history and
   the artifact index keep room, and then adds
   "(truncated; read `<ref>` for the rest)".

### Budget

The default budget is 12,000 characters. When the packet is over budget, the
command removes the oldest history lines first, then the artifact index.
It never removes decisions, open work, or the current task. If those alone
exceed the budget, the command prints them in full and writes a warning to
stderr.

### `--json`

Prints one JSON object: `v`, `as_of`, `markdown` (the packet text),
`sections` (one key per section, each a list of lines), and `settings`
(`keep_turns`, `tail_chars`). The mod reads everything it needs from this one
call.

## Component 2: `eventlog context check`

```
eventlog context check --percent <n> [--growth <g>]
```

`<n>` is the context fill, 0 to 100, measured against the engine's
auto-compaction limit when the engine reports one (see Component 4).
`<g>` is the expected growth of the next turn in percentage points
(default 0). The command prints
`{"rebuild": true|false, "reason": "<text>"}` and exits 0. The rules, first
match wins:

1. `percent` is below `floor_percent`: no, reason `below-floor`.
2. `percent + growth` is at or above `backstop_percent`: yes, reason
   `backstop`. The growth term lets the rebuild happen before the next turn
   crosses the backstop, or reaches the engine's own auto-compaction.
3. The log is at a task boundary: yes, reason `boundary`. A task boundary
   means all of these are true:
   - A controller `result` has been appended since the last `rebuild` event.
   - No controller `intent` is open.
   - No changed file outside `.context/` is newer than the controller's last
     `result`, excluding files under another agent's open claim. This is the
     rule `controller-stop-hook.sh` applies.
4. Otherwise: no, reason `mid-task`.

### Settings

A `[context]` table in `.context/eventlog.toml`:

| Key | Default | Meaning |
|---|---|---|
| `floor_percent` | 25 | Never rebuild below this fill. |
| `backstop_percent` | 60 | Always rebuild at or above this fill. |
| `keep_turns` | 3 | Conversation turns the rebuild keeps word for word. |
| `tail_chars` | 40000 | Maximum size of the kept turns. |
| `budget_chars` | 12000 | Packet budget. |

## Component 3: the `rebuild` event

```
rebuild: required=[trigger] optional=[as_of, reason, tokens_before, tokens_after, kept_turns]
```

- `as_of`: the log seq the packet was rendered at. Absent when the mod fell
  back to the engine's compaction and had no packet.
- `trigger`: `boundary`, `backstop`, `auto`, `manual`, `command`, or
  `plugin` (another plugin asked for the compaction).
- `reason`: `engine-fallback` when the mod fell back. The mod appends the
  event on that path too. Without it, `check` would find the same boundary
  after every turn and compact again each time.

The mod appends one `rebuild` event after each rebuild. `check` reads the
newest one to find "since the last rebuild". The event also shows in the log
when the controller's context was reset and from which seq.

The mod runs in the controller's session and appends with
`eventlog append`, without `by=`, so the line's writer is `controller`. The
allowlist already lets the controller append every type, so `rebuild` needs
only a built-in vocabulary entry. Phase 1 records
`decision key=context-rebuild value=eventlog-context-mod` so the log shows
when rebuilds started.

## Component 4: the `eventlog-context` mod

A Claude Code plugin with a hooks module, under `mods/eventlog-context/` in
this repo:

```
mods/eventlog-context/
  .claude-plugin/plugin.json
  hooks/hooks.json
  hooks/register.ts
  tests/register.test.ts
```

It needs Claude Code 2.1.278 or later and
`CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`. The `eventlog` binary embeds the mod's
files. `eventlog context install` writes them to
`.claude/skills/eventlog-context/`, where Claude Code loads the mod in a
trusted project. The first plan task confirms that load path.

The mod holds no log logic. It calls `eventlog` through `$.process.run` and
acts on the output.

### Which sessions the mod acts in

The mod acts only in the controller's interactive session. It is inert, and
every hook passes straight to `next`, when any of these holds:

- The session is headless (`claude -p` or the SDK). The doc worker runs
  headless in the main checkout and must keep its own context.
- `EVENTLOG_AS` is set to a writer other than `controller`.
- `LOG_DRIVEN_WORKER` is set (a worker launched by a reactor).
- `HERDR_PANE_ID` is set, `.context/layout.json` records a controller pane,
  and the two differ. This is the controller stop hook's rule. With no
  layout file the check is skipped.
- `EVENTLOG_CONTEXT=off` is set.
- The repo root the mod was installed into has no `.context/events.jsonl`.

The mod runs every `eventlog` call with that repo root as its working
directory. It also refuses a packet with no events (`as_of` of 0 or less)
and falls back to the engine without appending, so it never writes a log
that did not exist.

### Hooks

**`turn.complete`** (main loop only: `agentId` absent; `reason` is
`answer`):

1. Read `$.session.usage({ breakdown: 'summary' })`. The fill is context
   tokens divided by `autoCompactThreshold` when both are present; else
   `context.percent`; else the mod skips the check for this turn. It never
   treats a missing reading as 0.
2. Keep the per-turn token increases of the last 5 turns. Clear them when a
   compaction happens. The growth is their average, as a percentage of the
   same limit.
3. Run `eventlog context check --percent <n> --growth <g>`.
4. On `rebuild: true`, call `$.session.compact({ instructions:
   "eventlog-rebuild:<reason>" })` directly inside the hook. A deferred call
   (for example through `$.clock.after`) skips the mod's own
   `session.compact` hook and runs the engine's LLM summary instead. Catch
   the rejection that headless sessions raise.

**`session.compact`** (main loop only; every trigger except `precompute`):

1. Run `eventlog context`.
2. Select the tail: the last `keep_turns` turns of `e.messages`, where a turn
   starts at a user message that has no `toolResults`. Keep each message with
   its `handle`. Drop whole turns from the oldest end until the tail fits
   `tail_chars`. Always keep the newest turn. If the newest turn alone is
   larger than `tail_chars`, keep its first message (the user's prompt) and,
   when both fit, its final answer (the last message, assistant text with no
   tool calls), and append `rebuild reason=tail-trimmed`; the packet carries
   the state. Without the answer the model redoes the turn. Sizes count what
   the model reads: text, tool name and input, and tool result text, once.
   If that prompt alone is larger than `tail_chars`, fall back to the
   engine's compaction and append `rebuild reason=tail-too-large`.
3. Return `{ messages: [packet, ...tail] }`, where `packet` is a user message
   built from the packet text. Do not call `next(e)`, so the engine's
   summarizer does not run.
4. Append `eventlog append rebuild as_of=<seq> trigger=<t> reason=<r>
   tokens_before=<n> kept_turns=<k>`. The engine's trigger `auto` or
   `manual` becomes the same `trigger` value. The engine's trigger `plugin`
   comes from this mod, so `trigger` takes the reason after
   `eventlog-rebuild:` in `e.instructions` (`boundary`, `backstop`, or
   `command`).

`precompute` is skipped with `{ skip: "eventlog-context rebuilds on demand" }`
because the rebuild takes milliseconds and has nothing to precompute.

**`command.register`**: `/rebuild` queues a rebuild. The engine refuses
`$.session.compact` inside `command.run`, so the mod sets a flag, and the
next main-loop `turn.complete` calls `$.session.compact({ instructions:
"eventlog-rebuild:command" })` without asking `check`.

After a rebuild, the engine re-runs `prompt.context`. `CLAUDE.md` and
`AGENTS.md` load again as they do after any compaction.

### Failure handling

The mod fails open. If `eventlog` is not on `PATH`, exits non-zero, takes
more than 5 seconds, or prints an empty packet, the `session.compact` hook
returns `next(e)` and appends `rebuild trigger=<t> reason=engine-fallback`.
The engine then compacts as it does today. The mod never
removes the conversation without a replacement. If the `rebuild` append
fails, the mod reports it with `$.ui.log` and keeps the rebuild.

## Component 5: fallback without function hooks

`eventlog context install --classic` prints this classic hook for
`.claude/settings.json`. It does not edit the file, because the
`setup-log-driven-workspace` script owns it:

```json
"SessionStart": [
  { "matcher": "compact",
    "hooks": [{ "type": "command", "command": "eventlog context" }] }
]
```

The engine's summary stays, and the hook adds the packet after it. If the
mod were also loaded, this hook would fire after the mod's rebuild and
repeat the packet. So a project uses the mod when function hooks are
enabled, and this hook when they are not, never both. `eventlog context
install` refuses to write the mod when `.claude/settings.json` already
contains `eventlog context` in a `SessionStart` hook.

## Testing

**Rust, `eventlog context`.** Golden-file tests with `assert_cmd` over
fixture logs in `tests/fixtures/context/`. Cases: empty log; decisions only;
an open intent with a `ref`; a `ref` to a missing file; a packet over budget
(check the drop order); two runs over the same input give identical bytes.

**Rust, `eventlog context check`.** One test per rule, plus the boundary
cases: a result before the last rebuild, an open intent, a changed file under
another agent's claim, and a changed file newer than the last result.

**Mod.** `claude plugin test mods/eventlog-context`, with `process.run`
answered by the test. Cases:

- `session.compact` returns the packet plus the tail and does not call
  `next`.
- The tail never separates a tool call from its result, and keeps the newest
  turn when that turn is larger than `tail_chars`.
- `eventlog` fails, times out, or prints nothing: the hook returns `next(e)`.
- `precompute` is skipped; a subagent's compaction passes through.
- `turn.complete` calls `$.session.compact` only when `check` says yes.

**Probe evaluation (manual, phase 1).** Take three real controller sessions
that ran past 50% of the window. Rebuild each. Ask the controller four
probes: which files changed, what the decision on a named key is, what the
open work is, and what comes next. Score each answer against the log. Write
the results to `.context/reports/context-rebuild-probes.md`. Phase 1 is done
when every answer about files and decisions matches the log.

## Risks and checks for the first plan task

1. **Compaction from `turn.complete`.** The API says `$.session.compact`
   rejects while a turn runs. If the turn still counts as running inside
   `turn.complete`, defer the call with `$.clock.after`, or rebuild at the
   next `prompt.submit`. Test this on the real engine before other work.
2. **Messages without a handle.** The API says the engine builds a message
   without a `handle` from its `role` and `text`. Confirm the packet message
   is accepted as the first message of the new conversation.
3. **Early-access API.** Anthropic says the mods API may change between
   releases without notice. The mod stays small so a change costs little,
   and the classic hook keeps working without it.
4. **Files that never get a result.** The boundary rule and the Stop hook
   share a rule. A generated file that changes every session, such as the
   untracked `graft/` cache, blocks every boundary until it is in
   `.gitignore`. The backstop still fires.
