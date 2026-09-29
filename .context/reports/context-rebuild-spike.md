# Context rebuild: engine spike

Engine: Claude Code 2.1.283, with `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.
Date: 2026-09-28. Probed model: Haiku 4.5.

The spike mod is the Task 1 brief's `register.ts` with two additions for
evidence only. A top-level `mark()` helper writes each `$.ui.log` line to a
marker file too, and a `session.start` hook logs `spike: loaded`. The hook
logic under test is unchanged.

Interactive probes ran in a real TUI inside a private tmux server. A script
typed the prompts. Headless probes ran with `claude -p`.

## 1. A handle-less packet message is accepted as the first message

**Answer: yes.**

Log lines (headless, session `edf7ae51-dccf-41ab-ad87-74ea1bf510b4`):

```
spike: compact trigger=manual messages=3
```

After `/compact`, the question "What is the secret word?" returned:

```
The secret word is **heliotrope**.
```

The session transcript shows a `compact_boundary` entry followed by the
user message `SPIKE PACKET: the secret word is heliotrope.` and the two kept
assistant messages. The debug log for the same kind of compaction reads:

```
spike-rebuild (user) answered session.compact without next() in 67.1ms; nothing beneath it ran for this dispatch
session.compact (manual): a hook's 3 messages stand (hooked by spike-rebuild); core never ran
```

The engine made no summarizer request. The hook's messages replaced the
transcript.

## 2. `$.session.compact` from `turn.complete`

**Answer: direct works.** Deferred returns "ok" but does not run the mod's
own `session.compact` hook, so it does not serve this design.

Interactive, direct (`SPIKE-DIRECT`):

```
spike: percent=21
spike: compact trigger=plugin messages=3
spike: direct compact ok
```

Debug:

```
$.session.compact (spike-rebuild): compacting, 12 chars of instructions
hooks module spike-rebuild@inline: session.compact nested in spike-rebuild#2
session.compact (plugin): a hook's 3 messages stand (hooked by spike-rebuild); core never ran
```

Interactive, deferred with `$.clock.after(0, ...)` (`SPIKE-DEFER`):

```
spike: percent=21
spike: deferred compact ok
```

No `spike: compact` line appears. Debug:

```
hooks module spike-rebuild@inline session.compact skipped: re-entry (the plugin's own code raised it; origin spike-rebuild)
[API REQUEST] /v1/messages ... source=compact
Forked agent [reactive-compact] finished: 5 messages, ...
```

The engine skips the calling plugin's own `session.compact` hook when the
call comes from a timer, then runs its own LLM compaction. The direct call
sits inside the `turn.complete` dispatch, and the engine lets the mod's hook
answer.

Headless (`claude -p`), both forms reject:

```
spike: direct compact rejected: HooksError: spike-rebuild: $.session.compact: not available in a headless (-p / SDK) session yet: compaction here runs inside a turn (a /compact prompt); catch it and carry on
spike: deferred compact rejected: HooksError: spike-rebuild: $.session.compact is not available in this mode: no session is bound in this process (the REPL has not mounted and no headless session is built); catch it and carry on
```

In headless runs only a `/compact` prompt compacts. The mod's hook still
answers that (question 1).

## 3. `.claude/skills/<name>/` loads the mod

**Answer: yes, in a trusted workspace only.**

Scratch git repo with the mod at `.claude/skills/spike-rebuild/`, interactive
session after accepting the trust prompt, then a turn and `/compact`:

```
spike: loaded (session.start)
spike: percent=21
spike: compact trigger=manual messages=3
```

Debug:

```
hooks module spike-rebuild@skills-dir loaded (worker, environment 2, tier user); events: session.start,session.compact,turn.complete
session.compact (manual): a hook's 3 messages stand (hooked by spike-rebuild); core never ran
```

Before trust, a headless run in the same repo did not load the mod. The
debug log said `Loaded 1 skills-as-plugins` (a user-level plugin only), and
the marker file stayed empty. After trust, the same headless command loaded
it (`Loaded 2 skills-as-plugins`, `spike: loaded (session.start)`).

## Gate (Step 4)

- Question 1 is yes. The core mechanism works. Continue.
- Question 2 is "direct works", not "neither". Task 6 calls
  `$.session.compact` directly from `turn.complete`. Do not use
  `$.clock.after`: it bypasses the mod's own hook and runs the LLM summarizer.
- Question 3 is yes. Task 7 can install to `.claude/skills/<name>/`. Users
  must trust the workspace first. An untrusted workspace does not load it.

No gate branch that stops or reroutes the plan applies.

## Limits for later tasks

- `$.session.compact` rejects in headless (`-p`, SDK) sessions. A trigger in
  `turn.complete` does nothing there. Catch the rejection.
- `context.percent` read 14 to 21 on a nearly empty session. The status line
  showed `CTX 0%` at the same time. Task 6 should not assume the two figures
  agree.
