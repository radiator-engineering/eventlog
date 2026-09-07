# Log reactors: processes that act on events

A **reactor** is a long-lived process that tails `events.jsonl` and acts on
matching events without a prompt: a committer that lands a commit on every
`decision key=commit-message`, a deployer that ships on `approval`, a notifier
that pings a human on `escalate`. Reactors are the point where the log stops
being a record and starts driving effects, so a bug here is not a stale note,
it is a duplicate commit or a double deploy.

`references/reactor-example.md` in this directory is the reference action
script (a committer). Copy it and change the git commands. `eventlog react`
owns the loop. Everything below is the reasoning behind its four operator
rules, learned from a session where a committer reactor recommitted the same
cutoff three times.

## Rule 1: the reactor resumes from the log, never from a side file

The obvious design keeps a cursor file (`watch.seq`) holding the last seq
handled. It fails in every way a side file can fail:

- A restart with a stale, deleted, or hand-reset cursor replays old decisions
  and repeats their effects.
- Two instances share the cursor and race: one reads the cursor before the
  other writes it, and the same decision is acted on twice.
- The cursor is host-local and gitignored, so a fresh checkout starts at zero.

Grepping the effect for a marker (`git log --grep "Covers events up to seq
N"`) is the second obvious design. It fails the moment any effect was made by
hand or by an earlier version of the reactor with different phrasing. In the
session that produced this note, one commit said `Covers events 8-11.` and the
reactor searched for `Covers events up to seq 11.`, so seq 11 replayed.

The design that holds: **every action is recorded back into the log as its own
event, and the resume point is computed from those events.**

```
decision seq=11 key=commit-message value="Scaffold"
ack      seq=12 by=committer seq_done=11 outcome=committed ref=9a28f99
```

On start, and again before each action, the runtime reads the highest
`seq_done` among its own `ack` events and skips any decision at or below it.
The log is append-only and already the single source of truth, so this has no
state to drift. A cold start replays the whole log and every already-acked
decision is skipped.

Keep a second, effect-level guard anyway: for a committer, "nothing staged"
means skip. It covers the one case the log cannot — a log that was itself lost
while the effect persisted.

## Rule 2: one instance, enforced with a lock

Nothing in the log stops a second copy of the reactor from starting. A human,
the controller, and the agent that owns the reactor have all started one "to
be safe" in the same session. Two instances acting on the same event race even
with Rule 1, because both can read the acks before either writes one.

`eventlog react` takes a lock at startup:
`<log>.<name>.reactor.lock/` holds pid, process start time, hostname, and boot
id. The lock is live only when pid and start time match on this host. Stale
locks are reclaimed by atomic rename.

## Rule 3: run it in a real terminal, not an agent's tool-call shell

A coding agent's tool-call shell is torn down when the turn ends. A reactor
started from it with `nohup … &` dies with the shell, silently, and the agent
restarts it next turn, which is where the duplicate instances come from.
Claude Code's background Bash outlives a turn but not the session. Neither is
a supervisor.

Run the reactor foregrounded in its own dedicated terminal: a herdr pane
labeled for it, split off the owning agent's tab, is the right home. Tell the
owning agent in its brief that the reactor is already running and it must not
start another. Record the placement once:

```
progress agent=committer msg=reactor-placed detail="foreground in pane w1:p7" ref=references/reactor-example.md
```

## Rule 4: a committer stages what the decision names

`git add -A` at a cutoff commits the whole dirty tree, including files another
worker finished but nobody has reviewed. In the session above, a replayed
scaffold decision swept the AI module's files into a commit whose message
describes something else, and the "never amend" rule means it stays that way.

The runtime computes an **authorized set** from the driving event's `paths=` and
the writer's live claims. Put `paths=` on the decision (the same globs as the
worker's `claim`), and stage only those via `EVENTLOG_PATHS`. Without `paths=`
the authorized set is empty: `EVENTLOG_PATHS` is blank, and anything the action
commits is a `violation`.

## What `eventlog react` does per event

The runtime handles steps that every reactor duplicated in shell:

1. **Authorized set** — intersect `paths=` with live claims when the driving
   event carries `by=`.
2. **Intent** — append `intent by=<name> for=<seq> action=<label> paths=<authorized>`.
3. **Voter** — block if a path is the log, a lock dir, claimed by another open
   agent, or named in an open `escalate` for this reactor. On failure: `veto`
   and `ack outcome=vetoed`.
4. **Veto window** — `--window` (default 0). A `veto for=<seq>` binds even
   after restart between intent and action.
5. **Action** — run your command with JSON on stdin and `EVENTLOG_*` env vars.
   Write `outcome=<o>` and other fields to `EVENTLOG_OUTCOME_FILE`.
6. **Violation detection** — with `--git`, diff the two `HEAD`s; append
   `violation` for files the action committed outside the authorized set.
   Paths that only became dirty meanwhile are other agents' work in progress:
   silent under an open claim, otherwise appended as `observed` (no blame).
7. **Ack** — append `ack seq_done=<seq> outcome=<o> ...` from the outcome file.

Dry-run one seq without writing: `eventlog react test <seq> --as <name> -- cmd...`

## Reactors and the single-writer rule

A reactor writes to the log (its `ack`, `intent`, and `veto` events). That is a
deliberate exception to single-writer. Record it once as a decision, and have
the reactor tag every line with `by=<name>`:

```
decision key=log-writers value=controller-plus-reactors ref=DECISIONS.md
```

The `jq -c 'select(.by != null)'` check from SKILL.md then lists exactly the
reactor's lines instead of flagging a breach. A line with `by=` from an agent
that is not a recorded reactor is still a breach.

## Checklist before trusting a reactor

1. Kill it and restart it against the full log. It must act on nothing.
2. Start a second instance while the first runs. It must refuse.
3. Fire one event, then fire it again with a new seq. Only the first acts, or
   the second acts on new work only. Never the same work twice.
4. Confirm it is running in a terminal that outlives the owning agent's turn.
5. For a committer: `git show --stat HEAD` after a cutoff contains only the
   files the decision named.

The example action script passes 5 by construction when paired with
`eventlog react`; 1–3 are enforced by the runtime; 4 is where it runs.
