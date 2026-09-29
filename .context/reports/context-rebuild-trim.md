# Context rebuild: tail-trimmed backstop, live

Engine: Claude Code 2.1.284 with `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.
Model: `--model sonnet` (Sonnet 5.5). Date: 2026-09-29. Binary: the main
checkout's `target/debug/eventlog` (fresh `cargo build`), first on `PATH`.
I changed no code.

Result: the backstop fired twice with no `/rebuild`. Both times the mod used
the packet and appended `reason=tail-trimmed`. There was no engine fallback
and no LLM compaction. The role answer and all three probes were correct.
After each trim, however, the model did the trimmed turn again: it re-read
every file of that turn. The mod's turn size is about 3.6 times the real size,
because it counts each tool output four times.

## Setup

- Scratch clone `auto2-repo` in the scratchpad: `git clone` of main, then
  `rsync` of main's working tree (excluding `.git`, `target`, `.worktrees`,
  `.context`, `graft`), then `cp -R` of main's real `.context/`. The copied
  log ended at seq 805.
- Clone `.gitignore`: added `/graft/`, `.ignore`, `.claude/skills/`.
- Clone `.claude/settings.json`: I removed the `Stop` hook and pointed the
  `PreToolUse` guard at the main debug binary.
- Clone `.context/eventlog.toml`: `[context] floor_percent = 15,
  backstop_percent = 30, tail_chars = 12000`. `keep_turns` kept its default.
- `eventlog context install`, then one commit in the clone. `git status` was
  clean.
- Baseline: `rebuild trigger=command as_of=805 reason=test-baseline`
  (seq 806). Before it, `context check --percent 20` returned `boundary`;
  after it, `mid-task`.
- The packet now opens with "You are the controller of this repo. Claude Code
  rebuilt your context from the event log: keep working as the controller,
  not as a reviewer or worker." Packet size: 8,760 characters.
- Session: private tmux server (`tmux -L auto2`), the same `launch.sh`,
  `drive.sh` and `fill.sh` method as the previous test. The first attempt
  worked. The mod loaded and listed `/rebuild`.

Fill below is tokens over 267,000, from the transcript. The mod measures
against the engine's auto-compact threshold, so its percent can differ.

## Scenario

1. Turn 1: the model wrote `.context/handoffs/context-query-audit.md` and
   appended `intent` seq 807 (`msg=`, `ref=` the brief). Fill 16.0%. Intent
   807 stayed open to the end.
2. Turn 2: read `src/query/mod.rs`, `src/query/why.rs`, `src/cmd/state.rs`,
   `src/cmd/why.rs` and listed the query API. Fill rose from 42,665 to
   63,109 (23.6%). Growth was about 7.6%, so fill plus growth passed 30.

## Rebuild 1: seq 808, tail-trimmed

```
{"seq":808,"type":"rebuild","trigger":"backstop","as_of":"807","tokens_before":"61134","kept_turns":"1","reason":"tail-trimmed"}
```

- Debug: `eventlog-context: newest turn 121309 chars, tail_chars 12000, 15 messages`,
  then `a hook's 2 messages stand ...; core never ran`. The two messages are
  the packet and the turn-2 prompt.
- Fill: 63,109 (23.6%) on the last call before; 46,064 (17.3%) on the first
  call after.
- Engine errors: none from the mod.

**Behavior after rebuild 1.** I sent turn 3: "read `src/context/mod.rs`,
`age.rs`, `check.rs`, `install.rs` and `src/cmd/context.rs`". The model did
not do turn 3. It re-read the four turn-2 files (same result sizes: 14,821,
3,753, 7,124, 2,513) and gave the turn-2 answer again. It ended with "Next
step: read src/context/mod.rs, ...". It did not ask what it was doing, and it
kept the intent open. The cause is clear from the tail: the rebuild kept the
turn-2 prompt and dropped its answer, so the model saw an unanswered prompt
and answered it first.

**Role question** ("What are you working on and what is your role?"): "I'm
the controller of this repo ... Current task: a read-only audit for the open
intent at seq 807, from `.context/handoffs/context-query-audit.md`." It listed
what it had read and what came next. Correct. No role drift.

### Probes after rebuild 1

Ground truth: `?? .context/handoffs/context-query-audit.md`; open intent
seq 807; `log-protected=chflags-uappnd` at seq 409.

| # | Answer (short) | Score |
|---|---|---|
| 1 files and owners | One untracked file, the brief; the controller's; no claim covers it; needs a result under `handoff-briefs=committed`. It ran `git status` once. | correct |
| 2 `log-protected` | `chflags-uappnd`, seq 409, ref `.context/DECISIONS.md`. No tools. | correct |
| 3 open work | Intent 807 and its progress; the untracked brief; the two unretired reactor agents; stale reactors (21 days); merge decision at seq 800. | correct |

## Rebuild 2: seq 809, tail-trimmed

Turn 4: read all six files under `src/context/`, `src/cmd/context.rs` and
`src/cmd/agents.rs`, then list duplications. The model answered in full.
At turn end the backstop fired again.

```
{"seq":809,"type":"rebuild","trigger":"backstop","as_of":"808","tokens_before":"89521","kept_turns":"1","reason":"tail-trimmed"}
```

- Debug: `newest turn 167155 chars, tail_chars 12000, 46 messages`; `a hook's
  2 messages stand ...; core never ran`.
- Fill: 92,700 (34.7%) on the last call before; 39,464 (14.8%) on the first
  call after. Fill overshot 30%, because the check runs only at turn end and
  turn 4 alone added about 20k tokens.

**Behavior after rebuild 2.** I sent "Go on with the open task." The model
again re-read the turn-4 files (nine reads, including `src/query/mod.rs`) and
wrote the duplication findings a second time. It did not ask what it was
doing. Its first findings list was lost from the context, so the rework cost
one full turn of reads.

## Overcount check

I measured each trimmed turn in the transcript: prompt text, assistant text,
`tool_use` JSON (name and input) and `tool_result` text, each counted once.

| Rebuild | Mod's N | Real size | Ratio |
|---|---|---|---|
| 808 (turn 2) | 121,309 | 34,080 | 3.6x |
| 809 (turn 4) | 167,155 | 44,712 | 3.7x |

N is not about double. It is about 3.6 times the real size. I rebuilt the
mod's `size()` in Python over the transcript and got 121,305 and 167,151,
within 4 characters of N. That confirms the cause.

`size()` sums `text`, `JSON.stringify(toolUses)` and
`JSON.stringify(toolResults)`. Per the `claude-code.d.ts` types, each tool
output appears four times in those fields:

1. `toolUses[].text` on the assistant message (the result as the model read it);
2. `toolUses[].result` on the assistant message (the tool's stored record; for
   `Read` it holds the file content again);
3. `toolResults[].text` on the user message;
4. `toolResults[].result` on the user message.

JSON escaping of quotes and newlines in source code adds about 10% more. To
count the real size, count each tool output once: for example `toolResults[].text`
only, and `toolUses[].input` only.

Effect in this test: with `tail_chars = 12000` both turns were over the budget
either way, so the overcount did not change the outcome. With the default
40,000, the overcount makes a turn of about 11k real characters count as
over budget. Such turns are common.

## Extra rebuilds, loops and LLM compactions

- Rebuild events: 808 and 809, both `backstop`, both `tail-trimmed`. Seq 806
  is my baseline. No `tail-too-large`, no `engine-fallback`.
- No loop. The turns after each rebuild did not rebuild again.
- LLM compactions: none. The debug log has 0 `source=compact` requests and 0
  `reactive-compact` agents. `compacting,` appears 2 times and `core never ran`
  2 times.
- Other errors: the user-level `PreToolUse` hook in `~/.claude/settings.json`
  runs `~/.cargo/bin/eventlog` (0.2.0). It failed 4 times with `eventlog:
  parsing .../.context/eventlog.toml`, non-blocking. The clone's own guard (the
  debug binary) ran. This is finding 4 of the previous test, from a second
  source.

## Findings

1. **The trim path works.** Both backstops kept the packet and the prompt and
   appended `reason=tail-trimmed`. Fill fell to 14.8-17.3%, with no LLM
   summary. The previous test needed two engine fallbacks before a packet
   rebuild.
2. **The kept prompt makes the model redo the turn.** The tail keeps the
   prompt and drops the answer. The model then treats the prompt as open and
   answers it again, with the same reads. After rebuild 1 it also skipped my
   newer prompt (turn 3) and did turn 2 again. The rework costs one turn of
   tokens and can hide a newer instruction. Options: keep the assistant's
   final text of the trimmed turn (small here: 4.9k and 4.2k characters), or
   add a line to the packet that says the last prompt was already answered.
3. **The size count is about 3.6 times the real size** (see Overcount check).
   It makes the trim, and at the default budget the whole-turn drop, happen
   much sooner than `tail_chars` implies.
4. **The packet's role line works.** The model called itself the controller
   in every answer and named intent 807. The previous test's role drift did
   not appear. The brief in this test was less worker-like, so this is not a
   full check of that fix.
5. **The fill can overshoot the backstop.** The check runs at turn end, so one
   large turn took fill from 27.3% to 34.7% before the rebuild. This is by
   design, but a backstop near the engine's own limit could let the engine
   compact first.

## Evidence

In `<scratchpad>/auto2/`: `session.debug`, `transcript.jsonl`,
`tui-final.txt`, `tui-after-turn3.txt`, `after2.txt`, `role.txt`,
`probe1.txt`-`probe3.txt`, `gt-status.txt`, `gt-state.txt`,
`packet-before.md`, `packet-89.md`, `packet-261.md`,
`scratch-log-tail.jsonl` (seq 806-809), `eventlog.toml.used`,
`settings.orig.json`, `install.txt`, `launch.sh`, `drive.sh`, `fill.sh`,
`claude-json-before.json`. The clone is deleted and the tmux server is
stopped.

Trust entry added to `~/.claude.json`:
`projects["<scratchpad>/auto2-repo"].hasTrustDialogAccepted = true`. The
session also left a transcript folder under `~/.claude/projects/`
(`...-scratchpad-auto2-repo`).

The real log still ends at seq 805. No commit, stash or push in main or the
worktree. The only file I wrote outside the scratchpad is this report.
