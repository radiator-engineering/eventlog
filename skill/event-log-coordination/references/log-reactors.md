# Log reactors

A reactor is a long-lived `eventlog react` process that acts on matching
events without a prompt: a committer on `result`, a deployer on `approval`.
A bug here is a duplicate commit or a double deploy, not a stale note. The
runtime owns everything except the action script.

## What the runtime does per event

1. **Authorized set.** A controller-written event's `paths=` is used as-is.
   A reactor-written event's `paths=` is intersected with the writer's live
   claims; excess paths end the pass with `veto reason=unclaimed-paths`.
   Without `paths=` the set is empty and `EVENTLOG_PATHS` is blank. `result`
   carries `paths=`; `decision` does not, so drive a committer on `result`.
2. **Intent.** Appends `intent by=<name> for=<seq> paths=<authorized>`.
3. **Voter.** Vetoes with one of `log-or-lock` (a path is the log or a lock
   dir), `claimed-by-other` (a path another open agent claims), or
   `open-escalation` (an open `escalate` names this reactor). Two exemptions:
   a claim held by the driving event's own agent never vetoes, and a
   controller-written event passes over a reactor's claim while that reactor
   has acked at least once and has no open intent. A worker's claim always
   binds. A veto closes the pass with `ack outcome=vetoed`.
4. **Veto window.** `--window` (default `0s`). A `veto for=<seq>` appended
   in the window stops the action.
5. **Action.** Runs the command after `--` with the event as JSON on stdin and
   `EVENTLOG_LOG`, `EVENTLOG_SEQ`, `EVENTLOG_TYPE`, `EVENTLOG_AGENT`,
   `EVENTLOG_BY`, `EVENTLOG_PATHS`, `EVENTLOG_REF`, `EVENTLOG_RESUME`,
   `EVENTLOG_OUTCOME_FILE` in the environment. The command has `--timeout`
   (default `600s`). It writes `k=v` lines to the outcome file: `outcome=`
   plus any `ack` field (`ref`, `detail`). Other keys are folded into
   `detail=`. No `outcome=` means `committed` on exit 0, else `failed`.
6. **Git check** (`--git`). Files the action **committed** outside the
   authorized set become `violation by=<name> for=<seq> paths=<list>`.
   Files that only turned dirty are another agent's work: silent under an
   open claim, otherwise `observed by=<name> for=<seq> paths=<list>` with
   no blame.
7. **Ack.** `ack by=<name> seq_done=<seq> outcome=<o> [ref detail]`.

## Start, resume, stop

- **Baseline.** A reactor with no `ack` of its own in the log writes
  `ack seq_done=<tip> outcome=skipped detail=baseline` and never replays
  what came before it. It does not act on history.
- **Resume.** On every pass it skips events at or below its highest
  `seq_done`. There is no cursor file and no `git log --grep` marker.
- **Lock.** `<log>.<name>.reactor.lock/` holds pid, start time, hostname and
  boot id. A second instance refuses to start. Never delete the dir; a stale
  one is reclaimed on the next start.
- **Stop.** SIGINT, SIGTERM or SIGHUP ends the loop, releases the lock and
  exits 0. Restarting: append `retire agent=<name>`, stop it, start it, then
  append `spawn` and a fresh `claim` (a retire closes claims).
- **Where.** A terminal pane that outlives any agent's turn. A reactor
  started with `nohup … &` from a tool-call shell dies when the turn ends,
  and an agent that restarts it from a live turn starts a second instance
  that races the first. Tell the owning agent in its brief that the reactor
  is already running.

## Sanctioning a writer

A reactor writes `ack`, `intent`, `veto`, `violation`, `observed`, `note`,
`escalate`, `result` and `progress` with `by=<name>`, which `--as <name>`
sets. The built-in allowlist already permits that for any `by=`-tagged
writer; record it once so readers know:

```sh
eventlog append decision key=log-writers value=controller-plus-reactors ref=.context/DECISIONS.md
```

Any other value is a full replacement in the form
`name:type1|type2;name2:type3`, for example
`value=build-worker:result|progress`. The controller always keeps `spawn`,
`prompt`, `claim`, `decision`, `retire` and `approval`.

## Before trusting a reactor

1. `eventlog react test <seq> --as <name> --git -- <cmd>` prints the intent
   and ack the pass would write, and writes nothing.
2. Start a second instance while the first runs. It must refuse.
3. Fire one event, then a second. `eventlog why <seq>` on each shows one
   pass per event.
4. For a committer, `git show --stat HEAD` after a pass names only files from
   the driving event's `paths=`.
