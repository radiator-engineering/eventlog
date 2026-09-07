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

## layout-owner = Drovefile via drove up (2026-09-06)
`layout.sh` is retired. The `Drovefile` (drove 0.1.0, v3 form) owns the herdr
layout: the controller is the `caller_pane`, the log view runs `eventlog view -f`,
the monitor tab adds `eventlog tui`, and each reactor pane runs `eventlog react`
directly against its `.context/bin/*-action.sh`. Drove does not yet run pane
`on_start` hooks, so the controller appends the reactors' `spawn` by hand after
`drove up` (seqs 400, 401).

## controller-writer = eventlog append (2026-09-06)
The controller's own commands moved from `append-event.sh` to `eventlog append`;
`eventlog doctor --fix` removed the `~/.local/bin` shell-script symlinks. Reads
use `eventlog view` / `eventlog state`.

## log-protected = chflags uappnd (2026-09-06)
`eventlog protect` ran on the coordination log. `>>` appends still work;
truncate, overwrite and `rm` are refused by the kernel. Lift with
`eventlog protect --off` before any intentional removal.

## handoff-briefs = committed (2026-09-06)
The `.context/handoffs/*.md` briefs are tracked. The log names them by `ref=`,
so a replay needs them in history.

## agent-topology = parallel-worktrees (2026-09-06, seq 473)
Every spawned worker gets its own `git worktree` under `.worktrees/<name>`,
branched from `main`; it never edits the controller's checkout. Before the
controller accepts a worker's report it runs
`eventlog claims <name> main` in that worktree, and only then appends the
`result` (which the commit reactor lands from the worker's branch). This is
the survey's recommendation (`research/survey-agent-coordination.md`) and the
open thread the neuroarxiv report left undecided: with one shared tree the
committer's git check cannot tell one agent's edits from another's.

## violation-scope = committed-files-only (2026-09-06, seq 474)
A reactor's `violation` names only the files its action *committed* outside
the authorized set (`git diff --name-only` between the two `HEAD`s). Files
that merely became dirty while the action ran are never a violation: the tree
is shared, so they are another agent's work in progress. Under an open claim
they are expected and silent; unclaimed, the reactor records them as
`observed by=<reactor> for=<seq> paths=<list>` so the controller can see
them, with no blame attached. The controller's own edits are therefore never
a violation. Grounded in Causal Agent Replay (post-hoc attribution from a
shared tree is weak) and LogAct (blame only what an agent declared and did).
This also fixed the porcelain parse that dropped each path's first letter.

## model-contract amendment: `observed` event type (2026-09-06)
The frozen model contract (seq 85) gains one reactor-written type:
`observed` (required `paths`; optional `for`, `ref`, `detail`), listed in
`REACTOR_TYPES` and the default vocabulary. No existing type changed.

## log-scope = local-untracked (2026-09-07)
The live log (the JSONL file under `.context/`), its writer lock, the reactor
lock dirs and `layout.json` stay out of git. Everything the log points at is
tracked, one file per artifact: `DECISIONS.md`, `EVENTLOG.md`, the briefs, the
action scripts, `eventlog.toml`. Branches therefore merge per file with no
log merge step; `DECISIONS.md` merges with `merge=union` since its sections
are independent. This is what the research recommends: one writer, no
branchable log (neuroarxiv report, "Avoid"), small `ref=` events (survey
rank 1), and archiving or snapshots deferred until a log outgrows full
replay (event-sourcing survey rows 11 and 17). Preserving history across
clones, if ever needed, is a frozen archive copy, never a merge.

## eventlog.toml = defaults, fsync off (2026-09-07)
`eventlog init` scaffolded `.context/eventlog.toml` (it also overwrote the
hand-maintained `EVENTLOG.md`, restored from git; init is not idempotent for
that file, follow-up). `fsync = false` is kept: the event-sourcing survey
(row 8) makes fsync an optional strict mode, and this log is local
coordination state, not the durable record.

## agents-md-owner = controller (2026-09-07)
A claim is a path glob, so one file has one owner. `AGENTS.md` was under the
doc worker's claim while its coordination block was controller-maintained;
every controller `result` naming it was vetoed `claimed-by-other` (seam seq
493). The doc worker's claim and doc roots narrow to `docs,README.md`; the
controller owns and reports `AGENTS.md`. The doc worker reports a stale
`AGENTS.md` line in its result instead of editing it. This follows the
coordination survey, "a file that two tasks both need gets exactly one owner".

## init-templates = write-if-missing (2026-09-07)
`eventlog init` rewrote `EVENTLOG.md` and `eventlog.toml` whenever they
differed from its templates, which is every hand-edited copy. It now writes a
template only when the file is absent.

## repo-name = radiator-engineering/eventlog (2026-09-07)
The GitHub repository is `radiator-engineering/eventlog`, private, matching the crate and binary name. `Cargo.toml` points there. The local folder is still `event-log`; renaming it means `drove down`, a move, and `drove up`, since Drove keys its state on the path.

## docs-shape = commands, invariants, one how-to (2026-09-07)
`docs/reference/` is one page per command. `docs/explanation/` holds only
invariants a maintainer must know before changing the code (lock reclaim,
reactor lock liveness, hash-chain start rule, guard fail modes, model-contract
precedence). Change rationale is a decision and lives here, not in a doc page.
`docs/how-to/run-a-log-driven-repo.md` is the setup guide. `README.md` has a
fixed shape (what it is, install, daily commands, how the log drives work,
documentation, layout) and is edited in place, never appended to. Removed on
this date: seven change-rationale explanation pages and seven module
reference pages that restated the source (`log-module`, `query-module`,
`model-contract`, `react-loop`, `react-voter`, `react-action`, `react-lock`);
git history keeps them. The doc worker's prompt carries these rules.

## stop-hook = claim-aware, one block per file set (2026-09-06, seq 447)
The controller's Stop hook ignores files under another agent's open claim and
blocks once per distinct set of unreported files (memo in
`.git/eventlog-stop-hook-last`), so work in flight is not nagged every turn.

## claimed-by-other-scope = binds unless the owner is an idle reactor (2026-09-07)
The voter's `claimed-by-other` rule vetoed every controller `result` that named
`README.md` or `docs/` while the doc worker held its claim, even when the doc
worker was idle (seqs 535, 536). The controller grants every claim, and with
workers in their own worktrees (`agent-topology`) the only claims left in the
main checkout belong to the reactors. So a controller-written event now passes
over a reactor's claim when that reactor has acked at least once and has no
open intent. While an intent is open, a pass is in flight and the claim binds
again. A worker's claim always binds; a worker's files reach the log through
`result agent=<worker>`, the existing subject exemption.

## reactor-stop = signal releases the lock (2026-09-07)
`eventlog react` handles SIGINT, SIGTERM and SIGHUP (`ctrlc` crate): the loop
returns, the lock directory is removed, the process exits 0 with no restart
note. Before this, a stopped reactor left its lock behind and the next start
had to reclaim it.

## view-since = seq or timestamp (2026-09-07)
`eventlog view --since` accepts a bare sequence number as well as an RFC 3339
timestamp. It used to reject the seq form that its own error message implied.

## commit-action-timeout = 900s (2026-09-07)
The commit action runs a Cursor agent, which stayed alive past 300s on a
28-file change (ack seq 540 `failed: timed out`, commit landed anyway). The
`reactor()` helper in `drove/reactors.star` now defaults `timeout` to 900s.

## old-skill-repo = deleted (2026-09-07)
`radiator-engineering/event-log-coordination` got a final pointer commit and
was archived; deletion is pending a `delete_repo` token scope. The skill's
only home is `skill/` in this repo. Drove bugs filed upstream as
radiator-engineering/Drove issues 20, 21, 22.

## repo-visibility = public (2026-09-07)
The GitHub repo went public so the cargo-dist installer script and the
Homebrew formula can download release assets without a token, matching
radiator-engineering/Drove. The tap is `radiator-engineering/homebrew-tap`;
its formula publish job needs a `HOMEBREW_TAP_TOKEN` secret (contents write
on the tap), which neither repo has yet.

## crate-name = eventlog-cli (2026-09-07)
crates.io already has an unrelated `eventlog` crate (a Windows Event Log
library, since 2020), so the package is `eventlog-cli`. The binary, the lib
crate and the Homebrew formula stay `eventlog` (`[package.metadata.dist]
formula = "eventlog"`); the cargo-dist archives and installer scripts take
the `eventlog-cli-` prefix. Install paths: `cargo install eventlog-cli`,
`brew install radiator-engineering/tap/eventlog`, or the release installer.

## changelog = git-cliff-generated (2026-09-07)
`CHANGELOG.md` is generated from conventional commit messages with git-cliff
(`cliff.toml`), never hand-edited. Parallel workers therefore never touch it
and it cannot conflict on merge. The commit reactor's message, derived from
the `result` summary, is the changelog entry, so summaries should read as
user-facing lines. Alternative kept in reserve: per-change fragment files in
`changelog.d/` if curated wording ever diverges from commit messages.

## 2026-09-07 reusable infrastructure across repositories

The user authorized the Drove orchestrator to coordinate upstream eventlog implementation in separate workspaces. Contract: `.context/handoffs/infra-contract.md`; baseline `5679da4`. infra-setup owns reusable setup, packaged actions and skill; infra-runtime owns native action supervision and Git accounting; infra-review independently validates disposable fresh and Drove consumer repositories. Existing react CLI/environment/public signatures remain the shared contract. Product edits stay in isolated worktrees, with no worker commits or live reactor cutover. Drove migration/recovery follows accepted upstream changes.
