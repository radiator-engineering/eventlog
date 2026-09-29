# Context rebuild: live probes

Engine: Claude Code 2.1.283, with `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.
Date: 2026-09-28. Probed model: `--model sonnet` (Sonnet 5, auto-compact
window 300k tokens, auto-compact buffer 33k tokens).
Binary: the context-rebuild worktree's `target/debug/eventlog`, first on
`PATH`.

This is one interactive session (Task 8a), not the three sessions of the
plan's Task 8. The session did not reach the 25% floor by normal work, so the
rebuild came from `/rebuild`, not from a boundary or the backstop.

## Setup

- Scratch repo: `git clone` of the main checkout into the scratchpad, then
  `cp -R` of the real `.context/` directory into it. The copied log ended at
  seq 798. All appends below went to the scratch log. The real log was not
  written.
- Scratch `.claude/settings.json`: I removed the `Stop` hook
  (`.context/bin/controller-stop-hook.sh`). It reads the scratch log and
  hands the turn back when a changed file has no `result`. I kept the
  `PreToolUse` guard (`~/.cargo/bin/eventlog guard --agent claude`).
- `eventlog context install`, run in the clone with the worktree binary,
  printed:

  ```
  wrote .claude/skills/eventlog-context
  Start Claude Code with CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1 (Claude Code 2.1.278 or later).
  The mod loads only in a trusted workspace (<scratchpad>/probe-repo), since it lives under .claude/skills/.
  ```

- I committed the setup inside the clone, so `git status` started clean.
- A private tmux server (`tmux -L probe`) ran `claude --model sonnet
  --debug-file <scratchpad>/probe-session.debug --allowedTools
  "Bash,Read,Edit,Write,Glob,Grep"`. `EVENTLOG_AS` and `EVENTLOG_CONTEXT`
  were unset. A script accepted the trust prompt and typed the prompts.
- The mod loaded and was not inert:

  ```
  hooks module eventlog-context@skills-dir loaded (worker, environment 2, tier user); events: session.start,command.run,turn.complete,session.compact
  $.command.register (eventlog-context): /rebuild listed
  ```

## Session script

1. Orientation: read `AGENTS.md` and `.context/EVENTLOG.md`, run
   `eventlog state`, summarize.
2. A user-level tool (graft) had written an untracked `graft/` folder. The
   model read `.gitignore` and two `DECISIONS.md` entries, then appended
   `intent` seq 799: "ignore graft/ in .gitignore".
3. The model added `graft/` to `.gitignore` and appended `result` seq 800
   (`paths=.gitignore`).
4. The model closed the intent: `ack seq_done=799 outcome=done for=799`
   (seq 801). `eventlog state` then showed no open intents.
5. `/rebuild`, then one short turn ("Run git status --short ...").
6. The four probes.

After every main-loop turn, the mod ran `eventlog context check`. It returned
41 characters each time, which is `{"reason":"below-floor","rebuild":false}`.

## The rebuild event

```
{"as_of":"801","kept_turns":"3","seq":802,"tokens_before":"62618","trigger":"command","ts":"2026-09-28T06:01:42Z","type":"rebuild","v":1}
```

- trigger `command`, as_of 801, kept_turns 3, tokens_before 62618.
- Debug: `session.compact (plugin): a hook's 15 messages stand (hooked by
  eventlog-context); core never ran`. The engine made no summarizer call.
- Messages after the rebuild: the packet, then three whole turns (turns 3
  and 4, and the `git status` turn). That is 1 + 14 messages.
- Packet size: 7,353 characters as installed. `eventlog context --json`
  printed 14,937 characters.
- Context tokens: 62,618 on the last call before the rebuild, 41,891 on the
  first call after it.

## Probes

Ground truth, from the clone after the probes: `git status --short` showed
` M .gitignore` and `?? .ignore`. `.ignore` is another file written by the
graft tool, after `result` 800. `eventlog state` showed no open intents or
escalations; `reactor-runtime=eventlog-react (seq 341)`; the
`context-rebuild` worker still claimed; both reactors 20 days since their
last ack.

| # | Question | Answer (short) | Score |
|---|---|---|---|
| 1 | Which files have changed since the last commit, and who owns each? | `.gitignore` modified, the controller's, reported in result seq 800. `.ignore` untracked, no owner, not created by the controller. | correct |
| 2 | What is the decision in force for `reactor-runtime`? | `reactor-runtime=eventlog-react`, seq 341, `.context/DECISIONS.md`. It said it took this from the rebuilt context. | correct |
| 3 | What open work is there? | No open intents or escalations; intent 799 closed. `.gitignore` waits on the commit reactor; `.ignore` has no owner; both reactors stalled for 20 days; `context-rebuild` still claimed with Tasks 1-7 done. | correct |
| 4 | What should happen next? | Rerun `drove up`; decide on `.ignore`; let the `context-rebuild` worker finish Task 8; merge, one `result`, `retire`; confirm `git status` is clean. | not scored; reasonable |

I chose `reactor-runtime` for probe 2 because the packet carries it and
`AGENTS.md` does not.

## Gate

Probes 1 and 2 are correct. **Gate PASSED** for this one session. The plan's
gate asks for three sessions that each pass 50% fill with a natural rebuild.
This run does not cover that.

## Checks

- **`$.plugin.root` resolves to the clone root: yes.** Every `eventlog` call
  from the mod ran there, for example:
  `$.process.run (eventlog-context): eventlog with 6 args in <scratchpad>/probe-repo`.
  The `rebuild` event landed in the scratch log.
- **Plugin compaction arrives with `e.messages`: yes.** No
  `session.compact came without messages` line appeared in the debug log or
  the transcript. `$.ui.log` lines from other mods do reach both, so the
  absence means something. The engine reported 15 messages from the hook.
- **Two user messages in a row cause no engine error: yes, no error.** The
  packet (user) is followed by the kept tail's first message (user). The four
  probe turns ran normally. The debug log after the rebuild has no `[ERROR]`
  or API error lines.
- **`/rebuild` counts as a turn start: no.** The command row is not in the
  messages the engine hands to `session.compact`. The kept tail holds three
  real turns, and `kept_turns` is 3.
- **Fill on the first turn after the rebuild (G4): 16%.** 41,891 tokens
  against the auto-compact limit of 267k (300k window minus the 33k
  buffer). That is 9 points below `floor_percent` 25, so it is not within 5
  points. I propose no new floor. Before the rebuild the fill was 23%
  (62,618 tokens). `/context` right after probe 1 read 49.1k/300k (16% of
  the window, 18% of the limit). The fixed part alone (system prompt, tools,
  skills, memory, MCP instructions) was about 34k tokens, 13% of the limit.
- **Headless `claude -p` leaves the mod inert: yes.** Debug:
  `[eventlog-context] $.ui.log: eventlog-context: inert in this session (headless)`.
  A `/compact` in the resumed headless session went to the engine (the mod's
  hook passed through), and the log gained no `rebuild` event. The last
  event stayed seq 802.

## Behavior after the rebuild

The model showed no sign of missing context. It did not re-read files it
had read before, and it did not ask what it was doing. It named intent 799,
result 800 and the `.gitignore` change without being told. For probes 1 and
3 it ran `git status` and `eventlog state` to confirm the packet. That is a
check, not a gap.

## Findings

1. **Tool-written untracked files block the boundary rule.** graft wrote
   `.ignore` 6 seconds after `result` 800. At 30% fill, `context check`
   returned `mid-task`, not `boundary`: an unowned change newer than the last
   result counts as work in progress. In a repo where a tool writes
   untracked files after each turn, the boundary rule may never fire, and
   only the backstop rebuilds. Ignoring those files in `.gitignore` fixes it
   for that repo.
2. **The floor is hard to reach on a large window.** With a 267k limit, 25%
   is about 67k tokens. This session reached 23% after four short turns,
   mostly from about 34k tokens of fixed overhead. The status line showed
   `CTX 6%` at the same time, so the status line and the mod's fill do not
   agree (the spike saw the same).
3. **The `/rebuild` reply repeats the mod name:**
   `eventlog-context: eventlog-context: rebuild queued; ...`. The engine
   adds its own prefix to command output. Cosmetic.
4. **`eventlog view` shows a `rebuild` row with an empty summary.** The row
   reads `802 REBUILD controller` and nothing else.
5. **The engine warns at load:** `plugin eventlog-context: options requested
   but its manifest declares no userConfig; every option reads as absent`.
   It did not change behavior.
6. `rebuild` fields are stored as strings (`"as_of":"801"`), like other
   `key=value` fields.

## Evidence

Kept in the scratchpad: `probe-session.debug`, `probe-transcript.jsonl`,
`probe-tui-final.txt`, `packet-installed.md`, `packet-before.md`,
`context-after-rebuild.txt`, `probe-log-tail.json`, `state-after.txt`,
`headless-a.*`, `headless-b.*`, `probe-install.txt`,
`probe-settings.orig.json`. The scratch clone is deleted.
