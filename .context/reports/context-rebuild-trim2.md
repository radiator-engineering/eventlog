# Context rebuild: tail-trimmed with the kept answer, live

Engine: Claude Code 2.1.284 with `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.
Model: `--model sonnet` (Sonnet 5.5). Date: 2026-09-29. Binary: the main
checkout's `target/debug/eventlog` (fresh `cargo build`), first on `PATH`.
I changed no code.

Result: pass. The backstop fired once with no `/rebuild` and appended
`reason=tail-trimmed`. The rebuild kept the packet, the prompt and the
turn's final answer. The model then answered the new prompt. It did not
re-read the trimmed turn's files or write the old summary again. The role
answer and all three probes were correct. The mod's turn size N equals the
real size exactly.

## Setup

Same as the previous test (`context-rebuild-trim.md`), with `auto3` names:

- Scratch clone `auto3-repo`: `git clone` of main, `rsync` of main's
  working tree (excluding `.git`, `target`, `.worktrees`, `.context`,
  `graft`), then `cp -R` of main's real `.context/`. The copied log ended
  at seq 805.
- Clone `.gitignore`: added `/graft/`, `.ignore`, `.claude/skills/`.
- Clone `.claude/settings.json`: removed the `Stop` hook; pointed the
  `PreToolUse` guard at the main debug binary. No reactors ran.
- Clone `.context/eventlog.toml`: `[context] floor_percent = 15,
  backstop_percent = 30, tail_chars = 12000`.
- `eventlog context install`, then one commit in the clone. `git status`
  was clean.
- Baseline: `rebuild trigger=command as_of=805 reason=test-baseline`
  (seq 806). Before it, `context check --percent 20` returned `boundary`;
  after it, `mid-task`.
- I checked that the installed `policy.ts` has the new `size()` and
  `trimTurn()`.
- Session: private tmux server (`tmux -L auto3`), the same `launch.sh`,
  `drive.sh` and `fill.sh`. The first attempt worked. The mod loaded
  (`hooks module eventlog-context@skills-dir loaded`).

Fill below is tokens over 267,000, from the transcript.

## Scenario

1. Turn 1: the model wrote `.context/handoffs/context-module-audit.md` and
   appended `intent` seq 807. Fill 17.5%. Intent 807 stayed open to the end.
2. Turn 2: "read `src/query/mod.rs`, `src/query/why.rs`, `src/cmd/state.rs`
   and `src/cmd/why.rs` in full, and then summarize them in a few
   paragraphs." Four `Read` calls, then a 3,533-character summary. Fill
   rose from 17.5% to 24.9%. Fill plus growth passed 30.

## Rebuild: seq 808, tail-trimmed

```
{"seq":808,"type":"rebuild","trigger":"backstop","as_of":"807","tokens_before":"65168","kept_turns":"1","reason":"tail-trimmed"}
```

- No `/rebuild` was typed.
- Debug: `eventlog-context: newest turn 32520 chars, tail_chars 12000, 18 messages`,
  then `a hook's 3 messages stand ...; core never ran`.
- Messages after the rebuild, from the transcript:
  1. the packet (`# Context rebuilt from the event log`, 8,998 characters);
  2. the turn-2 prompt;
  3. the turn-2 final answer (the summary, starting "**`src/query/mod.rs`**
     is the fold").

  The tool loop was dropped. Prompt plus answer is about 3.7k characters,
  under `tail_chars`.
- Fill: 66,519 (24.9%) on the last call before; 48,990 (18.3%) on the first
  call after.

## After the rebuild

**New prompt:** "Now list the public functions in `src/context/check.rs`."
The model ran one `Grep` and answered: `to_json` (line 18) and `decide`
(line 28). That matches the file. It made no `Read` call on any turn-2
file and did not repeat the summary. Tool calls after the rebuild, in the
whole session: that one `Grep`, one `git status`, one `eventlog state`.

**Role question** ("What are you working on and what is your role?"): "I'm
the controller of this repo ... I don't act as a reviewer or worker." It
named open intent seq 807 and its brief, and listed the four files it had
read. Correct.

### Probes

Ground truth: `?? .context/handoffs/context-module-audit.md`; open intent
seq 807; `log-scope=local-untracked` at seq 491.

| # | Answer (short) | Score |
|---|---|---|
| 1 files and owners | One untracked file, the brief; the controller's; no claim covers it; needs a result under `handoff-briefs=committed`. It ran `git status` and `eventlog state` once each. | correct |
| 2 `log-scope` | `local-untracked`, seq 491, ref `.context/DECISIONS.md`. No tools. | correct |
| 3 open work | Intent 807 and its brief; no escalations; next step is to read the six `src/context` files. | correct |

In probe 3 the model said it had "not started the audit itself" and called
the turn-2 reads a side request. That is a fair reading of my prompts. It
also said a `result` "would also close intent 807"; closing an intent takes
an `ack`, so this detail is wrong. It did not act on it.

## Overcount check

I counted turn 2 in the transcript, each item once: prompt text, assistant
text, tool name plus compact input JSON, and tool result text.

| Part | Characters |
|---|---|
| prompt | 150 |
| assistant text | 3,533 |
| tool calls (name + input) | 626 |
| tool results | 28,211 |
| total | 32,520 |

The mod's N was 32,520. The ratio is 1.00. The previous test measured 3.6x.

## Extra rebuilds, loops and LLM compactions

- Rebuild events: 808 only (`backstop`, `tail-trimmed`). Seq 806 is my
  baseline. No `tail-too-large`, no `engine-fallback`.
- No loop. No later turn rebuilt again.
- LLM compactions: none. The debug log has 0 `source=compact` requests and
  0 `reactive-compact` agents. `core never ran` appears once.
- Mod errors: none.
- The user-level guard in `~/.claude/settings.json` (`~/.cargo/bin/eventlog`)
  failed 4 times with `eventlog: parsing .../eventlog.toml`, non-blocking.
  This is known.

## Findings

1. **The kept answer fixes the rework.** With the final answer in the tail,
   the model treated turn 2 as done. It answered the new prompt at once. In
   the previous test it re-read every file and skipped the newer prompt.
2. **The size count is now exact.** N matched the real size to the
   character.
3. **The role line still works.** The model called itself the controller
   and named intent 807.

## Limits

- One session, one trim. The trimmed turn ended with a plain text answer,
  so this test does not cover a turn cut off mid-loop (prompt only).
- I did not test the case where prompt plus answer exceeds `tail_chars`.
- The floor and backstop were lowered (15/30) so the backstop fired on
  turn 2.

## Evidence

In `<scratchpad>/auto3/`: `session.debug`, `transcript.jsonl`,
`tui-final.txt`, `after-rebuild.txt`, `role.txt`, `probe1.txt`-`probe3.txt`,
`gt-status.txt`, `gt-state.txt`, `packet-before.json`,
`scratch-log-tail.jsonl` (seq 806-808), `eventlog.toml.used`,
`settings.orig.json`, `install.txt`, `launch.sh`, `drive.sh`, `fill.sh`,
`claude-json-before.json`. The clone is deleted and the tmux server is
stopped.

Trust entry added to `~/.claude.json`:
`projects["<scratchpad>/auto3-repo"].hasTrustDialogAccepted = true`. The
session also left a transcript folder under `~/.claude/projects/`
(`...-scratchpad-auto3-repo`).

The real log still ends at seq 805. No commit, stash or push in main or the
worktree. The only file I wrote outside the scratchpad is this report.
