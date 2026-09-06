# Decisions

Append-only intent lives in `.context/events.jsonl`; this file holds the prose
each `decision` event references by `ref=`.

## log-writers = controller-plus-reactors
Sanctioned writers:
- the **controller** (may omit `by=`, or tag its own operational records
  `by=controller`);
- the **committer** reactor (`by=cursor-committer`: `ack`, `violation`,
  `escalate`, `note`);
- the **doc worker** reactor (`by=doc-worker`: `result` for its doc edits,
  plus its own `ack`/`escalate`).

- **research workers** (2026-09-06, user's request): a spawned research
  worker whose brief in `.context/handoffs/` grants it may append `progress`,
  `result` and `escalate` tagged `by=<its name>`, only through
  `append-event.sh`. The controller still records `spawn`, `prompt`, `claim`
  and `retire`, and the committer ignores `result by=<worker>`; the controller
  reviews the report and appends its own `result` to land it. Current grant:
  `logact-deep-read`, `survey-event-sourcing`, `survey-agent-coordination`.
- **build workers** (2026-09-06, eventlog CLI plan): the same grant applies to
  every worker spawned from `docs/superpowers/plans/2026-09-06-eventlog-cli.md`.
  Their names start with `build-`; each brief in `.context/handoffs/build-*.md`
  repeats the grant. Types allowed: `progress`, `result`, `escalate`.

Breach = any `by=` value outside {`controller`, `cursor-committer`,
`doc-worker`} plus the workers whose briefs grant it, or a `by=` worker line
of a type other than `progress`, `result`, `escalate`.

Breach check:
`jq -c 'select(.by != null and (.by|(IN("controller","cursor-committer","doc-worker","logact-deep-read","survey-event-sourcing","survey-agent-coordination") or startswith("build-"))|not))' .context/events.jsonl`

## commit-agent = cursor-commit-reactor + composer-2.5-fast (autonomous)
Commits are landed by an **autonomous reactor**, `.context/bin/cursor-commit-reactor.sh`,
running foregrounded in a dedicated herdr pane under the supervisor
`.context/bin/run-reactor.sh`. It `tail -F`s `.context/events.jsonl` itself —
**no human ping** — and on a completed-work event with a dirty tree invokes
**headless** `cursor-agent -p --force --trust --model composer-2.5-fast` to
author commits grounded in **both** the log (intent) and the diff (the change).

The **reactor**, not the model, records the `ack` (`by=cursor-committer`,
`seq_done`, `origin=<who triggered>`, `outcome=committed|skipped`), so resume is
correct even if the model forgets. Model brief: `.context/handoffs/cursor-committer.md`.

What it does and does not survive:
- Triggers only on controller `result`, `decision key=commit-message`, or
  `result by=doc-worker`. Other decisions never fire a pass. Dirty-gate
  ignores `.context/`.
- First start (no acks of its own) writes a baseline `ack outcome=skipped
  detail="baseline: …"` at the log's current tip and does NOT replay earlier
  events. Later restarts resume from the last real ack.
- Transient failures retry 3× before `ack skipped` + `escalate`.
- Files a commit touched outside the event's `paths=` are recorded as a
  `violation` (detection, not prevention — the commit stands).
- A stale `.git/index.lock` is detected and escalated, never auto-deleted.
- Supervisor respawns on crash, logs each restart as a `note`, gives up on a
  crash loop (`escalate`).
- It does NOT survive the pane, herdr session, or machine going away. After a
  reboot, run `.context/bin/run-reactor.sh` in the pane again.
- AI attribution: `.githooks/commit-msg` strips cursor-agent's auto
  `Co-authored-by: Cursor` trailer (`git config core.hooksPath .githooks` is
  local — re-run on a fresh clone).
- Editor churn files (none) are untracked + ignored so they cannot keep
  the tree permanently dirty.

## doc-agent = doc-sync-reactor + headless Claude (sonnet)
`.context/bin/doc-sync-reactor.sh` watches the log itself. On every
`ack by=cursor-committer outcome=committed` whose `origin != doc-worker` it runs
headless Claude (`claude -p --model sonnet`) loading three skills:
`documentation-writer` (Diátaxis structure; its interactive determinations are
pre-answered and its approval gate waived), `plain-technical-english`
(prose discipline) and `context-engineering` (for `AGENTS.md`, the rules file
every agent loads: the doc worker owns its upkeep — adds durable project-wide
facts a change introduced, deletes stale or task-specific lines, keeps it under
about 120 lines, never edits marker-delimited sections other tools own, never
touches `CLAUDE.md`, which only imports `AGENTS.md`). It edits only
docs,README.md,AGENTS.md, never commits, then reports as
a **real worker** — `result by=doc-worker paths=docs,README.md,AGENTS.md` — which the
committer lands with `origin=doc-worker`, which the doc worker ignores (loop
guard). Brief: `.context/handoffs/doc-worker.md`.

Auth: the reactor runs `claude -p` with `ANTHROPIC_API_KEY` removed from the
environment so headless Claude uses the claude.ai login (`DOC_USE_API_KEY=1`
keeps the key). The prompt is piped on stdin because `--allowedTools` is
variadic and swallows a trailing positional prompt.

## log = not OS-protected (opt-in pending)
`.context/events.jsonl` is guarded only by the Claude PreToolUse hook. It is
not `chflags uappnd` / `chattr +a` protected: `setup.sh --protect` has not been
run, so a non-Claude agent (the Cursor committer included) could still rewrite
or delete it. Opt in with `setup.sh --protect` or
`safety-check.sh --protect .context/events.jsonl`; `>>` appends keep working
afterwards, and `protect-log.sh --unprotect` lifts it.

## model-contract = src/model as landed by result seq 83
The public items in `src/model/{event,config,vocab,allow,paths}.rs` are the
contract every later eventlog task builds against (plan
`docs/superpowers/plans/2026-09-06-eventlog-cli.md`, Task 2). A worker that
needs a signature changed appends `escalate`; the controller makes the change
and records it. Later contracts (`query-contract`, `react-contract`) follow the
same rule.

## cli-args-ownership = each command task owns its own Args struct
Task 1 scaffolded `src/cli.rs` with empty `<Cmd>Args` structs. A command task
may add fields to its own `<Cmd>Args` struct only, with a targeted edit (never
a whole-file rewrite, since several workers share the file). The `Command`
enum, the global flags and every other struct stay frozen; changes there go
through `escalate`. Recorded after build-schema edited `SchemaArgs` (seq of the
violation event precedes this decision).

## append-strict-context = append takes a StrictContext trait
Plan Task 4 had `append` take a fold callback returning `query::State`, which
Task 9 builds in parallel; that import broke the build for every worker. The
`log` module must not depend on `query`. `src/log/append.rs` defines
`pub trait StrictContext` with exactly the queries its strict rules need
(allowlist at tip, agent open or not, claim owner of a path, open escalations),
takes `Option<&dyn StrictContext>`, and Task 6 (`build-append-cmd`) implements
the trait for `query::State` in `src/cmd/append.rs`.

## query-contract = src/query/mod.rs as landed by result seq 161
Same rule as `model-contract`: the public items in `src/query/mod.rs` are
frozen for Tasks 10-13. Change requests go through `escalate`.

## react-contract = src/react as landed by result seq 283
Same rule as `model-contract`: the public items in `src/react/{mod,lock,voter,action}.rs`
(`ReactorConfig`, `Steps`, `Reactor`, `ReactorLock`, `supervise`, the voter and
action functions) are frozen for Task 17 (`build-react-cmd`). Change requests go
through `escalate`.

## reactor-runtime = eventlog react (Phase 3 switch, 2026-09-06)
`run-reactor.sh` reads `REACTOR_RUNTIME` from `workspace.env`; with `eventlog`
(the default) it execs `eventlog react --as cursor-committer --on result --git
-- commit-action.sh` and `eventlog react --as doc-worker --on ack --filter
by=cursor-committer --filter outcome=committed -- doc-action.sh`. The Rust
runtime owns lock, resume, intent, voter, ack and supervision; the shell loops
stay as the `shell` fallback. The doc action appends its own
`result by=doc-worker` through `eventlog append` (spec section 7 allows a
reactor's own result; the committer cuts its scope to the doc-worker claim at
seq 337) and skips when the driving ack landed a doc-worker result (loop
guard, replacing the old `origin=` field). `layout.sh` is unchanged: it still
runs `run-reactor.sh`, which now dispatches.

## react-contract amendment: Authorized.subject (2026-09-06)
The first live doc pass was vetoed `claimed-by-other` because the voter
compared claim owners only against the reactor's own name. `Authorized`
gains `subject` (the driving event's `agent`, else its writer) and the rule
exempts a claim held by that subject: a worker's result on its claimed files,
or the controller's `result agent=<worker>`, is not "other". A controller
result with no `agent` naming a path an open agent claims is still vetoed.
The fold now keys escalations by subject and seq, defaults a worker
escalation's subject to its `by=`, and `approval for=<seq>` closes exactly one.
