# Context rebuild: automatic triggers, live

Engine: Claude Code 2.1.283 with `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.
Model: `--model sonnet` (Sonnet 5; the fill limit is 267k tokens).
Date: 2026-09-28. Binary: this worktree's `target/debug/eventlog`, first on
`PATH`. I changed no code.

Result: both automatic triggers fired with no `/rebuild`. The boundary
rebuild used the packet on the first try. The backstop fired three times.
The first two times the mod fell back to the engine's LLM summarizer
(`reason=tail-too-large`). The third time it used the packet. The probes
were correct after both packet rebuilds.

## Setup

- Scratch repo: `git clone` of the main checkout into the scratchpad
  (`auto-repo`), then `cp -R` of the real `.context/` into it. The copied
  log ended at seq 802. All appends went to the scratch log. I did not write
  the real log.
- `.gitignore` in the clone: added `/graft/`, `.ignore` and
  `.claude/skills/`. I committed this inside the clone.
- `.claude/settings.json` in the clone: I removed the `Stop` hook
  (`controller-stop-hook.sh`) and kept the `PreToolUse` guard.
- `.context/eventlog.toml` in the clone: I added
  `[context] floor_percent = 15, backstop_percent = 30`. `keep_turns` (3)
  and `tail_chars` (40,000) kept their defaults.
- `eventlog context install` in the clone, then a commit, so `git status`
  was clean.
- **Baseline `rebuild` event (seq 803).** With the copied log, `context
  check --percent 20` already returned `boundary`. The log has controller
  results and no `rebuild` event, so the first turn above the floor would
  have rebuilt before the scenario started. I appended
  `rebuild trigger=command as_of=0 reason=test-baseline` to the scratch log.
  After that, the check returned `mid-task`.
- Session: private tmux server (`tmux -L auto`), `claude --model sonnet
  --debug-file <scratchpad>/auto/session.debug --allowedTools
  "Bash,Read,Edit,Write,Glob,Grep"`, with `EVENTLOG_AS` and
  `EVENTLOG_CONTEXT` unset. A script accepted the trust prompt and typed the
  prompts. The first attempt worked.
- The mod loaded:
  `hooks module eventlog-context@skills-dir loaded ...; events: session.start,command.run,turn.complete,session.compact`
  and `$.command.register (eventlog-context): /rebuild listed`.

Start fill: turn 1 (read `AGENTS.md`, `EVENTLOG.md`, `src/query/mod.rs`,
ran `eventlog state`) ended at 54,286 tokens, 20.3%. The first API call of
the session was already 42,437 tokens (15.9%). A floor of 15 was therefore
crossed on turn 1. That was acceptable, because the baseline event kept the
verdict at `mid-task`.

## (A) Boundary: fired

Turn 2, as the controller, in one turn:

1. Wrote `.context/handoffs/agents-md-reports.md`.
2. `intent` seq 804 (`ref=` that brief). The model first tried `summary=`;
   `intent` rejected it, so it used `msg=`.
3. One-line edit to `AGENTS.md`.
4. `result` seq 805, `paths=AGENTS.md`.
5. `ack seq_done=804 outcome=done for=804`, seq 806.
6. `git status --short` and `eventlog state`; no open intent.

At `turn.complete` the check returned `boundary`. The mod compacted.

```
{"seq":807,"type":"rebuild","trigger":"boundary","as_of":"806","tokens_before":"61169","kept_turns":"1"}
```

- Fill: 61,429 tokens (23.0%) on the last call before; 47,988 (18.0%) on
  the first call after.
- Packet: 14,076 characters of `context --json`; 6,923 characters of
  markdown.
- `e.messages` present: yes. No `came without messages` line. Debug:
  `session.compact (plugin): a hook's 23 messages stand ...; core never ran`.
- Engine errors: none.
- `kept_turns` was 1, not 3. The session had only two turns, and turn 1
  (four file reads) alone was near `tail_chars`, so `selectTail` dropped it.
- `AGENTS.md` stayed modified and the brief stayed untracked. Neither
  blocked the boundary: `AGENTS.md` was older than the result, and
  `.context/` is outside the rule. I did not need a commit.

### Probes after (A)

Ground truth: `git status` showed ` M AGENTS.md` and
`?? .context/handoffs/agents-md-reports.md`. `eventlog state` showed no open
intents or escalations.

| # | Answer (short) | Score |
|---|---|---|
| 1 files and owners | `AGENTS.md` modified, the controller's (result 805, decision `agents-md-owner`); the brief untracked, the controller's, not named by a result. | correct |
| 2 `reactor-runtime` | `eventlog-react`, seq 341, from the rebuilt context. | correct |
| 3 open work | No open intents; the two files; `context-rebuild` waits on the merge decision (note seq 800, which I confirmed); both reactors stalled for 20 days. | correct |
| 4 next | Rerun `drove up`; merge decision on `context-rebuild`; add a result for the brief; retire stale claims. | reasonable |

The model did not re-read files and did not ask what it was doing. For
probes 1 and 3 it ran `git status` and `eventlog state` once each to
confirm.

## (B) Backstop: fired

Turn B1: the model wrote `.context/handoffs/guard-review.md` and opened
`intent` seq 808. I told it to keep the intent open. It read
`src/guard/mod.rs` and `src/guard/deny.rs` and summarized them. Intent 808
stayed open to the end.

### Attempt 1: seq 809, engine fallback

Fill after B1 was 74,085 (27.7%). Fill plus growth reached 30, and the
check returned `backstop`. The newest turn was larger than `tail_chars`, so
the mod appended the event and called `next(e)`:

```
{"seq":809,"type":"rebuild","trigger":"backstop","reason":"tail-too-large"}
```

The engine ran its summarizer: `[API REQUEST] /v1/messages ... source=compact`,
then `Forked agent [reactive-compact] finished`. It took 32 seconds. The
engine then re-attached the three files that had just been read. Fill after
the fallback was 62,281 (23.3%).

### Attempt 2: seq 810, engine fallback again

I sent one file per turn: `src/log/append.rs` (check: `mid-task`), then
`src/cmd/view.rs` (11,732 bytes). After the second turn, fill was 75,012
(28.1%). The check returned `backstop`, and the tail again counted as too
large:

```
{"seq":810,"type":"rebuild","trigger":"backstop","reason":"tail-too-large"}
```

The engine ran its summarizer a second time. Fill after it was 69,178
(25.9%).

### Attempt 3: seq 811, packet

I sent four small analysis turns: one `grep -rln` and three `grep -n` runs
over `src/guard/`. The first three returned `mid-task`, with intent 808
open. The fourth ended at 78,061 (29.2%):

```
{"seq":811,"type":"rebuild","trigger":"backstop","as_of":"810","tokens_before":"77702","kept_turns":"3"}
```

- Fill: 78,061 (29.2%) before; 50,498 (18.9%) on the first call after.
- Packet: 17,890 characters of `context --json`; 8,785 characters of
  markdown. The packet listed `intent seq 808 controller: review guard
  blocking rules`.
- `e.messages` present: yes. Debug: `a hook's 14 messages stand ...; core
  never ran`.
- Engine errors: none.

### Probes after (B)

Ground truth: ` M AGENTS.md`, `?? .context/handoffs/agents-md-reports.md`,
`?? .context/handoffs/guard-review.md`. Open intent: seq 808.

| # | Answer (short) | Score |
|---|---|---|
| 1 files and owners | The three files; `AGENTS.md` the controller's (result 805); both briefs written by the controller, no result behind them. | correct |
| 2 `eventlog-config` | `defaults-fsync-off`, seq 492, from the rebuilt context. | correct |
| 3 open work | Intent 808 (guard review) open, the three files, the merge decision, stalled reactors, and its guard findings so far. | correct |
| 4 next | Finish the guard review for intent 808: read the rest of `deny.rs` and `mod.rs`, test candidate payloads, write findings. | names intent 808: yes |

The model did not re-read files after the rebuild. It remembered its
finding from the kept tail (`/bin/rm` with a full path is not caught).

**Role drift after (B).** In probes 1, 3 and 4 the model called itself "a
read-only reviewer" and said that acking intent 808 is "the controller's
job, not mine". It is the controller. The likely cause is the brief it
wrote in B1, which reads like a worker brief. The packet puts that brief's
intent first under open work. I did not confirm the cause.

## Extra rebuilds, loops and LLM compactions

- Rebuild events in the session: 807 (boundary), 809 and 810 (backstop,
  engine fallback), 811 (backstop, packet). Seq 803 is my baseline.
- No loop. After each packet rebuild the next checks returned `mid-task`.
  After (A), boundary did not fire again, because result 805 is older than
  rebuild 807.
- LLM compactions: 2, both from the `tail-too-large` fallback. The debug log
  has two `source=compact` requests and two `reactive-compact` agents. There
  were no others.
- `$.session.compact (eventlog-context): compacting` appears 4 times; `core
  never ran` appears 2 times. The two counts match the events above.

## Findings

1. **`tail-too-large` blocks most backstop rebuilds in real reading work.**
   With `tail_chars = 40000`, a turn that reads one file of about 12k bytes
   was judged too large (seq 810). In the transcript that turn is about
   15.8k characters of message content, plus 12.6k of structured tool
   result. That totals about 28k, under 40k. The mod still counted more
   than 40k. A possible cause is text that other hooks inject into the
   prompt: `jev-skill-suggestion` injected a 6,559-character skill into
   that turn. I could not see `e.messages` from outside, so this cause is
   not proven. The effect is clear: the backstop replaced the conversation
   with an LLM summary twice before it used the packet.
2. **An engine fallback leaves fill high.** After each LLM summary the
   engine re-attached the files it had just read. Fill after the fallback
   was 23.3% and 25.9%, against 18-19% after a packet rebuild. The next
   backstop therefore came sooner.
3. **A fresh log rebuilds on the first turn above the floor.** A log with
   controller results and no `rebuild` event is at a boundary from the
   start. In a real repo, the first turn above the floor rebuilds at once.
   This may be what the design wants. It surprised me in the test, and I
   seeded seq 803 to avoid it.
4. **An older `eventlog` breaks on the `[context]` table.** The clone's
   `PreToolUse` guard runs `~/.cargo/bin/eventlog` (0.2.0). With a
   `[context]` table in `eventlog.toml`, every guard call failed:
   `eventlog: parsing .../.context/eventlog.toml`. The hook failed
   non-blocking, so the guard allowed every command for the whole session.
   Changing the hook to the worktree binary mid-session had no effect,
   because the session did not reload settings. Users who add `[context]`
   before they upgrade the guard binary lose the guard, and they see only a
   warning.
5. **The controller's `ack` makes it show as a reactor.** After ack 806,
   `eventlog state` listed `controller last_ack=804 unacked=1` under
   reactors. The packet's reactor health may show the same row.
6. `intent` takes `msg=`, not `summary=`. The model's first append failed.
   Minor.
7. Other mods wrap `session.compact`. The debug log says `hooked by
   jev-skill-suggestion+eventlog-context`. The packet result still stood.
8. Known from the earlier run: the load-time `userConfig` warning.

## Concerns

- Findings 1 and 2 mean the backstop, the rule meant to catch long
  sessions, will often give the engine summary, not the packet. Consider a
  larger default `tail_chars`, or a fallback that keeps the packet and
  trims the newest turn.
- The role drift after (B) needs a second look before merge. The packet
  may need to say who the reader is ("you are the controller").
- This is one session. The floor and backstop were lowered (15/30) so both
  paths fit in about 15 turns.

## Evidence

In `<scratchpad>/auto/`: `session.debug`, `transcript.jsonl`,
`tui-final.txt`, `packet-A.md`, `packet-B.md`, `gt-A-*.txt`, `gt-B-*.txt`,
`probeA*.txt`, `probeB*.txt`, `scratch-log-tail.jsonl` (seq 803-811),
`eventlog.toml.used`, `settings.orig.json`, `install.txt`, `launch.sh`,
`drive.sh`, `fill.sh`, `claude-json-before.json`. The guard blocked a
full copy of the scratch log, so I kept its last 9 events. The clone is
deleted and the tmux server is stopped.

Trust entry added to `~/.claude.json`:
`projects["<scratchpad>/auto-repo"].hasTrustDialogAccepted = true`. The
session also left a transcript folder under `~/.claude/projects/`
(`...-scratchpad-auto-repo`).

No append to the real log. No commit, stash or push in the worktree or the
main repo. The only worktree change is this file.
