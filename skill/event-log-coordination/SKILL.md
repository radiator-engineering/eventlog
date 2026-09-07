---
name: event-log-coordination
description: >-
  Share context between subagents/peer agents through an append-only JSONL event
  log that a single controller writes and everyone else reads. Use this whenever
  several agents coordinate — spawning workers, routing messages, delegating
  slices, waiting on human approvals, orchestrating herdr panes/tabs — and you
  need one ordered, replayable, auditable record of who knew what and when
  instead of pasting transcripts between windows. Trigger it on requests like
  "share context between subagents", "coordinate multiple agents", "event log
  for agents", "single source of truth for the orchestration", "who approved
  that", "make the agent hand-off auditable/replayable", "a watcher/committer
  that acts on log events" (duplicate commits, a watcher that keeps dying), or
  when wiring an append-only log the model cannot rewrite. Pairs with context-sharing (files +
  handoffs) and herdr / herdr-peer-agents / herdr-orchestration (the panes the
  events describe).
metadata:
  author: jjm@radiator.live
---

# Event-log coordination

**Operating model: the agent runs every `eventlog` command in this skill. The user
never runs a command by hand.** The user speaks in natural language ("scaffold X",
"spawn the workers", "who approved that?"); you translate that into the commands
below and run them. Every subcommand here is something *you* execute, not
something you hand back to the user. **On first use in a session, run
`eventlog doctor --fix`** (it is idempotent): it registers guards where they are
missing, removes obsolete PATH symlinks, and reports what is not protected. When
it names a gap you cannot heal silently — a log that should be OS-protected —
tell the user in plain words and get their yes before running
`eventlog protect`. Then do not mention commands again unless they ask.

When several agents work together, the failure mode is context bleeding between
them: one pane's transcript pasted into another, a decision that survives only
in one window, an approval nobody can later point to. The fix is a **shared,
append-only event log**: one ordered JSONL file that a single controller writes
and every agent (and every human) can read. Walk the log to event N and you know
exactly what the system knew at event N — which is what makes it debuggable,
auditable, and replayable.

This skill is the enforcement layer under that idea. It gives you the log, the
one sanctioned writer, and a PreToolUse hook that makes "append-only" a rule the
model *cannot* break, not just one it is asked to follow.

**What's actually non-obvious here.** A capable agent, unprompted, will already
reach for append-only JSONL, `chflags uappnd`, and referencing artifacts by
path — that part is table stakes. The two things worth taking from this skill
are the parts that are easy to miss: (1) a **PreToolUse hook that *prevents*
rewrites before they happen**, not just a hash chain that *detects* them after,
and (2) making the **controller the single writer** so worker context never
bleeds across panes. Prevention and detection are complementary — see
"Enforcement".

## How it relates to the skills around it

- **`context-sharing`** owns the durable working state: `progress.md`, curated
  handoff files, `DECISIONS.md`. Use it for the *contents* an agent produces.
  This skill is the thin ordered index over that state — the log's `result` and
  `decision` events carry `ref=` paths pointing into those files. Log = spine;
  handoffs = the flesh. Keep payloads in the handoff, keep the fact in the log.
- **`herdr` / `herdr-peer-agents` / `herdr-orchestration`** are the substrate the
  events *describe*. A herdr controller already keeps "a compact registry" of
  workers (tab, agent name, model, status, disposition). This log **is** that
  registry, made append-only and replayable: `spawn` when you launch a worker
  tab, `prompt` when you send the packet, `result` when it reports, `retire`
  when you close the tab. See `references/herdr-integration.md`.

## The core idea (from the append-only event-log pattern)

1. **Single writer.** Only the controller/driver appends. Workers report back;
   the controller records. This is why it holds in Claude Code for free: spawned
   subagents run isolated and structurally cannot reach the controller's log —
   they return a result, and the controller writes one line.
2. **Append-only.** Lines are never edited or deleted. The sequence is the
   truth. No schema migrations, no database — just a file you can `tail -f`.
3. **Small events.** No model output, no file bodies, no giant payloads.
   Reference big artifacts by path (`ref=...`). The log stays cheap for the
   longest-lived, most context-starved agent — the controller — to re-read.

## Setup (agent runs the doctor once)

First use in a session: run the doctor. It is idempotent — it registers guards
where they are missing, removes obsolete PATH symlinks, and reports what is not
protected:

```bash
eventlog doctor --fix    # heal tooling + guard, report protection
eventlog doctor          # report only, change nothing
```

After the doctor, every command below is a bare `eventlog` subcommand you run
from the user's repo. Do not surface this plumbing unless they ask — but *do*
surface any protection gap the doctor names. Then, per repo:

```bash
cd /path/to/the/repo
eventlog init   # creates .context/events.jsonl + EVENTLOG.md + eventlog.toml, gitignores the log
```

`init` is safe to rerun: it never overwrites an `EVENTLOG.md` or `eventlog.toml`
that already exists, edited or not.

`eventlog init` creates the log and an `EVENTLOG.md` cheat-sheet, but not the
handoff/`DECISIONS.md` files your events will `ref=` — write those yourself (use
`context-sharing` for their shape). The log points at them; it does not make them.

Append only through the writer — never hand-edit the file (the guard blocks it):

```bash
eventlog append spawn   agent=reviewer model=opus tab=w1:t3 role=review
eventlog append prompt  agent=reviewer ref=.context/handoffs/review-auth.md
eventlog append result  agent=reviewer ref=.context/handoffs/review-auth.md verdict=CHANGES
eventlog append decision key=auth-store value=sqlite ref=DECISIONS.md
eventlog append retire  agent=reviewer disposition=accepted
```

`seq` (monotonic) and `ts` (UTC) are added for you. The writer takes a portable
lock (works on macOS — no `flock`), so parallel controllers cannot collide on a
sequence number or interleave a half-written line. It rejects any field over 2KB
and any event over 4KB, so a transcript cannot sneak in — that is what `ref=` is
for. Read the log with `eventlog view`, or plain tools: `tail -f`,
`jq -c 'select(.type=="decision")'`.

The default event vocabulary (`spawn`, `prompt`, `message`, `drain`, `result`,
`decision`, `escalate`, `approval`, `retire`, plus the parallel-work set
`claim`, `progress`, `seam`, `violation`, and the reactor-written `observed`) lives in `.context/EVENTLOG.md`
after init. Add new `type`s freely; keep fields flat and small. Run
`eventlog vocab` to list required and optional fields per type.

## Keeping parallel workers apart

The log records coordination; it does not create it. What actually stops two
workers from doing the same work or editing the same file is decided *before*
they spawn, and the log's job is to make those decisions replayable. Do all of
these, and record each one:

1. **Serialize the shared foundation.** Anything more than one worker will
   import (a model, a shared view, a helper) is one task that lands first. Fan
   out only after its `result` and `retire`. Briefs then say "reuse X, do not
   recreate it" and point at the commit.
2. **One worktree per worker.** Record it once as
   `decision key=agent-topology value=parallel-worktrees`.
3. **Claim files at spawn.** Right after `spawn`, append
   `claim agent=<name> paths=<glob>,<glob>` with the repo-relative paths the
   worker owns. Every other worker's brief lists those paths as off-limits. A
   file that two tasks both need gets exactly one owner; the other worker
   requests changes through the controller (`message`), never by editing.
   Ownership that lives only in brief prose cannot be replayed; a `claim` can.
4. **Scope the brief.** "Do ONLY task Tn. Do not start subagents. Do not append
   to the log." The last one matters more than it looks: see the honest limit
   below.
5. **Verify at `result` time.** Before accepting, run
   `eventlog claims <agent> <base-ref> [head-ref]` in the worker's worktree. It
   lists every changed file no claim covers. Record a hit as
   `violation agent=<name> paths=<list>` and decide whether to accept, then
   integrate. This is detection, not prevention, and it is cheap.
6. **Name seams as soon as they appear.** When a worker's report says "this must
   be reconciled with Tn at merge", append
   `seam agents=t3,t4 subject=<one line> ref=<report>` right then. Integration
   then starts from a list instead of a note buried in a `result` field.
7. **Poll and record `progress`.** Workers do not write to the log, so a running
   worker is silent by design. If you want a live signal, the controller polls
   (`herdr agent read`, a subagent's partial output) and appends
   `progress agent=<name> msg=<short> ref=<file>`. Keep it to one line; the
   worker's transcript is not the log's business.
8. **Close every lifecycle.** Every agent that appears in a `result` must also
   have a `spawn`, a `prompt`, and a `retire`, including one-off reviewers and
   Task subagents. A `result` with no `spawn` cannot be replayed.

## Reactors: when a process acts on the log

A **reactor** tails the log and acts on matching events with no prompt: a
committer that lands a commit on `decision key=commit-message`, a deployer that
ships on `approval`. This is where the log drives effects, so the failure is a
duplicate commit or a double deploy, not a stale note.

`eventlog react` owns the reactor runtime: lock, baseline ack, resume, intent,
voter, veto window, command execution, violation detection, and ack. You supply
only the action script after `--`. Four rules for the operator, each one
learned from a committer that recommitted the same cutoff three times:

1. **Resume from the log, never from a side file.** No cursor file, no
   `git log --grep` for a marker string. The runtime appends an `ack` event
   (`by=<name> seq_done=<seq> outcome=<what>`) for every action, and skips
   any event at or below its highest acked seq. A cold start replays the whole
   log safely.
2. **One instance, enforced by a lock.** The runtime holds
   `<log>.<name>.reactor.lock/` with pid, start time, hostname, and boot id.
   Refuse to start while that lock is live.
3. **Run it foregrounded in a real terminal**, a dedicated herdr pane. A
   reactor started with `nohup … &` from an agent's tool-call shell dies when
   the turn ends, and the agent's next "restart" is the second instance that
   races the first.
4. **A committer stages only the authorized paths.** The runtime computes an
   authorized set from the driving event's `paths=` and the writer's live
   claims. `git add -A` sweeps other workers' unreviewed files under the wrong
   message.

**Per event, the runtime:**

1. Computes the **authorized set**. Controller-written events use `paths=` as-is.
   Reactor-written events intersect `paths=` with the writer's live claims; excess
   paths trigger a `veto` with reason `unclaimed-paths`.
2. Appends `intent by=<name> for=<seq> action=<label> paths=<authorized>`.
3. Runs the **voter** against the fold at that moment. Rules: no path is the log
   file or a lock dir; no path is claimed by a different open agent; no open
   `escalate` names this reactor. On failure append `veto by=<name> role=voter
   for=<seq> intent=<intent seq> reason=<rule>` and `ack seq_done=<seq>
   outcome=vetoed`.
4. Opens the **veto window** (`--window`, default 0). A `veto` whose `for=`
   names the driving seq binds regardless of which intent it saw.
5. Runs your command with the event as JSON on stdin and env vars:
   `EVENTLOG_LOG`, `EVENTLOG_SEQ`, `EVENTLOG_TYPE`, `EVENTLOG_AGENT`,
   `EVENTLOG_BY`, `EVENTLOG_PATHS`, `EVENTLOG_REF`, `EVENTLOG_RESUME`,
   `EVENTLOG_OUTCOME_FILE`. Write `outcome=<o>` and other fields as `k=v` lines
   to the outcome file; the runtime reads them for the ack.
6. With `--git`, snapshots `HEAD` and `git status --porcelain` before and after.
   Files the action **committed** outside the authorized set (`git diff
   --name-only` between the two `HEAD`s) are appended as
   `violation by=<name> for=<seq> paths=<outside>`. Files that merely became
   dirty while the action ran are never a violation: the tree is shared, so
   they are another agent's work in progress. Under an open claim they are
   silent; unclaimed, they are appended as `observed by=<name> for=<seq>
   paths=<list>` so the controller sees them, with no blame attached.
7. Appends `ack seq_done=<seq> outcome=<o> ...` from the outcome file.

Launch a committer:

```bash
eventlog react --as committer --on decision --filter key=commit-message --git -- \
  bash path/to/commit-action
```

Dry-run one event without writing:

```bash
eventlog react test 42 --as committer --git -- bash path/to/commit-action
```

A reactor is a recorded exception to single-writer: append
`decision key=log-writers value=controller-plus-reactors` once, and the reactor
tags every line `by=<name>`. `references/log-reactors.md` has the reasoning and
a pre-trust checklist; `references/reactor-example.md` is a 15-line committer
action script. Copy it and change the git commands.

## Enforcement — this is the point

A convention the model is asked to honor is not enforcement. Two complementary
mechanisms; **install the hook — it is the piece an agent will not build on its own.**

**Prevention — the PreToolUse hook (install this).** `eventlog guard` reads a
hook payload on stdin and **blocks** (exit 2) anything that would rewrite or
truncate the log — a `Write` over it, an `Edit` into it, a truncating `>`/`>|`,
`sed -i`, `rm`/`mv`, a non-append `tee`, even an inline
`python -c "open(log,'w')"`. Appends via `eventlog append` and all reads
(including read-mode opens) pass through. `eventlog doctor --fix` registers the
guard; to check or (re)install standalone:

```bash
eventlog guard install --agent claude   # idempotent; doctor --fix does this
```

It adds the guard as an **additional** `PreToolUse` entry (matcher
`Edit|Write|Bash`) without touching your other hooks, refuses to write invalid
JSON, and is idempotent. **The hook only loads in a new session** — restart or
run `/hooks` after installing.

Override the protected basename with `EVENTLOG_GUARD_BASENAME` (ERE) if your log
is not `events.jsonl`. The guard fails **open** on an unparseable payload, so it
can never brick your tools — it only ever *adds* a deny for a clearly-mutating op.

Honest scope: airtight for `Edit`/`Write` (they can only overwrite/patch — any
hit is denied). For `Bash` it is a strong but **best-effort denylist**: it covers
the shapes an agent actually reaches for, but a determined rewrite through an
exotic tool can still slip past. That gap is exactly what detection closes.

**Agent-independent immutability (this is also the cross-agent path).** The hook
governs the *controller's own tool calls* and only fires inside Claude Code. For
anything out-of-band — a stray process, a bug, a tool the denylist misses, or
**an agent that is not Claude Code (Cursor, Codex, aider)** — make the file itself
immutable at the OS level:

```bash
eventlog protect              # chflags uappnd (macOS) / chattr +a (Linux); >> still works
eventlog protect --off        # lift it before an intentional rm/checkout
```

`>>` appends still succeed, so the writer keeps working; truncate/overwrite/rm
are refused by the kernel regardless of which agent tried. This fights routine
`rm -rf`/checkout, so scope it to logs that must be tamper-proof. If you also
want to *prove* no past line changed, hash-chain the entries (each line carries
`prev` = sha256 of the previous line); `eventlog verify` flags any edit. The
hook prevents (Claude Code); `eventlog protect`/hash-chaining make tampering
impossible or evident (any agent).

## Portability (Cursor, Codex, other agents)

The pattern, the log, and the commands are agent-agnostic — anything that runs a
shell uses them identically. Only the *auto-blocking hook* is Claude Code-specific
(it reads Claude Code's PreToolUse payload). Under another agent: run the log
tooling as usual and enforce with `eventlog protect` instead of the hook
(`eventlog protect` to enable, `--off` before an intentional rm/checkout,
`--status` to check). That is the whole difference — same log, same commands,
OS-level enforcement in place of the hook.

That OS protection is **per-log and opt-in** — nothing is protected until
someone runs `eventlog protect` on a specific file. So the user should know the
real state and opt in deliberately, not assume coverage. Run the posture check:

```bash
eventlog doctor              # report: guard per agent, THIS log's OS protection
eventlog doctor --fix        # check AND heal each gap (interactive on a terminal)
eventlog protect             # opt in now: OS-protect .context/events.jsonl
```

The doctor heals guard registration on its own, but protecting a log is a
deliberate opt-in: it only protects when you pass `--protect`/`--yes` or answer
its terminal prompt — never silently. `eventlog init` runs the report right
after creating a log, so the moment a log exists you (and the user) see whether
it is covered. Surface that to the user for any log that must be tamper-proof,
and protect it only when they say so.

### The honest limit

Claude Code does not hand you the writer process the original pattern assumes —
you steer a model that *chooses* to append. So the guarantee is: workers cannot
reach the log (isolation), and the controller's own writes are held to
append-only by the hook. Without the hook you have the architecture but not the
guarantee. With it, the only way for the controller to change history is to be
denied trying — and `chflags`/hash-chaining cover the rest.

**Not authenticated.** No field in the log proves who wrote it. `--as` is a
declaration. The allowlist catches mistakes; OS protection and the guard resist
rewriting; nothing in v1 resists a process that lies about its name. That is the
same trust model as before, stated plainly.

"Workers cannot reach the log" is true only for Task subagents, which run
isolated. A herdr peer agent has a shell, and the OS protection still allows
`>>` appends, so nothing physically stops a peer from appending. Single-writer
holds for peers only because the brief forbids it. Say so in every brief ("do
not append to `.context/events.jsonl`"), and detect a breach with a field
check: the controller never writes a `by=` field, so any line carrying one, or
any `type` outside the vocabulary you use, came from somewhere else:

```bash
jq -c 'select(.by != null)' .context/events.jsonl
```

Single-writer can be relaxed on purpose, and sometimes should be: a reactor
must record its own acks, and a peer that reports through the log saves a
polling round-trip. Do it as a recorded decision
(`decision key=log-writers value=<who> ref=DECISIONS.md`), require `by=<agent>`
on every non-controller line, and keep the controller the only writer that
omits `by=`. The check above then lists the sanctioned writers; a `by=` you
did not sanction, or a line with no `by=` that the controller did not write,
is still a breach.

## Before you rely on it

1. Run `eventlog doctor --fix` — it heals guard registration, then reports
   whether this log is OS-protected (restart the session / run `/hooks` to make
   the guard live). For a log that must be tamper-proof under Cursor/Codex/other
   agents, opt in with `eventlog protect`.
2. Are events small and pointing at artifacts by `ref=`, not carrying bodies?
3. Is the controller the only thing appending — workers report, controller
   records?
4. Does each worker's lifecycle close the loop: `spawn` → … → `result` →
   `retire`?
5. For parallel workers: does every worker have a `claim`, and did
   `eventlog claims` run before each `result` was accepted?
6. For any reactor: does it resume from its own `ack` events, hold a lock,
   and run in a terminal that outlives the owning agent's turn? (checklist in
   `references/log-reactors.md`)

## Commands (the agent runs these, not the user)

Install the skill with `eventlog skill install` (writes this directory, stamped
with the binary version). Then run subcommands from any repo:

- `eventlog append <type> k=v ... [--as n] [--dry-run]` — the single sanctioned
  writer (seq, ts, lock, size caps, strict validation).
- `eventlog vocab [<type>] [--json]` — required and optional fields per type.
- `eventlog verify` — walk the hash chain; exit 1 on break.
- `eventlog view [-f] [--type a,b] [--agent x] [--by x] [--last N] [--json]` —
  colored aligned lines; `-f` follows with file watching.
- `eventlog agents [--at N] [--json]` — per-agent lifecycle table.
- `eventlog state [--at N] [--json]` — active agents, claims, decisions, intents.
- `eventlog why <seq> [--json]` — event chain and reactor match.
- `eventlog claims <agent> <base> [head]` — changed files no claim covers; exit 1
  on a gap. Hidden alias: `check-claims`.
- `eventlog open <seq> [--pager]` — open the event's `ref` in `$EDITOR`.
- `eventlog tui` — ratatui views over the log (follow, agents, state).
- `eventlog react --as n --on t1,t2 [--filter k=v] [--window 0s] [--git] -- cmd...`
  — reactor runtime (lock, intent, voter, veto, action, violation, ack).
- `eventlog react test <seq> --as n [--git] -- cmd...` — dry-run one event.
- `eventlog guard [--agent claude|cursor|codex]` — hook payload on stdin; exit 2
  on deny.
- `eventlog guard install [--agent claude|cursor|codex|all]` — register the hook.
- `eventlog init` — scaffold log, `EVENTLOG.md`, default config, gitignore lines.
- `eventlog doctor [--fix] [--protect]` — posture check; `--fix` heals guards and
  removes obsolete PATH symlinks.
- `eventlog protect [--off] [--status]` — OS-level append-only.
- `eventlog schema [--events|--output]` — JSON Schema from the model types.
- `eventlog skill install [--dir path]` — write the embedded skill directory.
- `eventlog completions <shell>` — shell completions.

Global flags on every command: `--log <name|path>`, `--json`.

## Reference files

- `references/herdr-integration.md` — mapping herdr controller/worker actions to
  events, and where the log replaces the ad-hoc registry.
- `references/log-reactors.md` — processes that act on events (committer,
  deployer): resume from `ack` events, single-instance lock, where to run
  them, what a committer may stage. Pre-trust checklist.
- `references/reactor-example.md` — a 15-line committer action script for
  `eventlog react`. Copy it and change the git commands.
