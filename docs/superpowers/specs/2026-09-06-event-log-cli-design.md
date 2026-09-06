# eventlog CLI design

Date: 2026-09-06. Status: approved in brainstorm, awaiting spec review.

## 1. Purpose

`eventlog` is a Rust binary that replaces the shell toolkit in the
`event-log-coordination` skill. It writes, reads, queries and enforces the
append-only JSONL coordination log that multi-agent repos keep at
`.context/events.jsonl`. It also runs reactors against that log, guards the
log inside Claude Code, Cursor and Codex, and installs the skill that
explains all of this to agents.

Why a binary: Drove, Radiator and every log-driven repo depend on the
conventions, but today they only work where the skill's scripts are
symlinked onto PATH, and the PreToolUse guard only exists for Claude Code.
One versioned, installable binary makes enforcement the same on every
machine and for every agent.

Research behind the design: `research/README.md`. The two additions the
research argued for, and this spec adopts, are logging a reactor's intent
before it acts (LogAct, arXiv 2604.07988) and a per-type write allowlist
enforced by the writer.

## 2. Decisions

| Decision | Value | Source |
|---|---|---|
| Binary and crate name | `eventlog` | brainstorm |
| Structure | one crate, `lib.rs` plus `main.rs`; Drove links the crate when it needs to | brainstorm |
| Distribution | cargo-dist, shell installer, Homebrew tap `radiator-engineering/homebrew-tap`; copy Drove's `dist-workspace.toml` | brainstorm |
| Hash chain | written and verified from v1; older lines without `prev` stay valid | brainstorm |
| Config | `.context/eventlog.toml`, fallback `~/.config/eventlog/config.toml` | brainstorm |
| Configurable in v1 | log path and named logs, vocabulary, per-type write allowlist, view colors and columns, TUI keys | brainstorm |
| Guard targets | Claude Code, Cursor (`beforeShellExecution`), Codex (`PreToolUse`); all block on exit 2 | docs checked 2026-09-06 |
| Scope | the whole shell toolkit: writer, reader, TUI, scaffold, doctor, guard, reactor runtime, skill install | bootstrap brief |
| Not in v1 | snapshots, event upcasting, a query language, an LLM voter, multi-writer clocks, OpenTelemetry export | research surveys |

## 3. Log format

One JSON object per line. Fields, in this order:

| Field | Type | Rule |
|---|---|---|
| `seq` | integer | starts at 1, increases by 1 per line; the writer sets it |
| `ts` | string | RFC 3339 UTC, second precision; the writer sets it |
| `type` | string | lowercase slug from the vocabulary |
| `prev` | string | hex sha256 of the previous line's exact bytes, or `genesis` on the first chained line; the writer sets it |
| `by` | string | writer name on every line the controller did not write |
| other | string | flat, type-specific; each under 2 KB; whole line under 4 KB |

Compatibility: every existing log stays valid. A line without `prev` is a
pre-chain line. `eventlog verify` checks the chain from the first line that
carries `prev` and reports the last good `seq` when it breaks. A line that
does not parse is reported with its line number; views skip it, `verify`
fails on it.

The writer rejects `seq`, `ts` and `prev` as user-supplied fields.

## 4. Configuration

`.context/eventlog.toml` is optional. Missing tables use the defaults below.

```toml
[log]
path = ".context/events.jsonl"   # default log
fsync = false                    # true: fsync after every append
[log.named]                      # extra logs: eventlog --log <name> ...
# review = ".context/review.jsonl"

[vocabulary.result]              # one table per type; fields are required
fields = ["ref"]

[writers]                        # per-type allowlist, seed only (see below)
decision = ["controller"]
spawn    = ["controller"]
ack      = ["cursor-committer", "doc-worker"]

[view]
columns = ["seq", "type", "agent", "summary", "ref"]
[view.colors]
result = "1;36"

[keys]
filter = "/"
follow = "f"
open   = "o"
panes  = "tab"
```

Allowlist precedence, fixed: `decision key=log-writers` lines in the log win
over the file; the file wins over the built-in default. The built-in default
is: only `controller` writes `spawn`, `prompt`, `claim`, `decision`,
`retire`; a `by=`-tagged reactor writes `ack`, `note`, `escalate`,
`violation`, `intent`; `voter` writes `veto`; any name granted by a decision
writes what the decision grants. The log is the truth; the file is the seed.

The writer name comes from `--as <name>` or `EVENTLOG_AS`, default
`controller`. A name other than `controller` sets `by=`.

## 5. Modules

One crate. `main.rs` holds clap wiring only.

| Module | Does | Depends on |
|---|---|---|
| `model` | vocabulary, allowlist, config loading and precedence | serde, toml |
| `log` | append (lock, seq, ts, prev, caps, allowlist, fsync), read, verify | model, sha2 |
| `query` | fold events into `State`; state at seq N | log |
| `react` | reactor runtime: lock, baseline ack, resume, intent, voter, window, run, ack, supervise | log, query |
| `guard` | parse the three hook payloads; one denylist; exit 2 on deny | model |
| `scaffold` | `init`, `doctor`, `protect`, guard install | log, model |
| `tui` | ratatui views over `query` | query |
| `skill` | embedded `skill/` files; `skill install` | include_dir |

Each module has one public entry type and is tested on its own.

## 6. Commands

| Command | Replaces | Behavior |
|---|---|---|
| `eventlog append <type> k=v ...` | `append-event.sh` | takes the lock, validates, writes one line, prints it |
| `eventlog verify` | none | walks the hash chain; exit 1 on break with last good seq |
| `eventlog view [-f] [--type a,b] [--agent x] [--since ts] [--last N] [--grep s] [--json]` | `eventlog-view.sh` | colored aligned lines; `-f` follows with file watching, polling fallback |
| `eventlog agents [--at N] [--json]` | none | per-agent lifecycle table; flags open lifecycles and unreleased claims |
| `eventlog state [--at N] [--json]` | none | active agents, open claims, decisions in force, open escalations, open intents, unacked events per reactor |
| `eventlog check-claims <agent> <base> [head]` | `check-claims.sh` | changed files no claim covers; uses `git diff --name-only`; exit 1 on a gap |
| `eventlog open <seq> [--pager]` | none | opens the event's `ref` in `$EDITOR` or `$PAGER` |
| `eventlog tui` | `eventlog-view.sh -f` in the coordinator pane | see section 8 |
| `eventlog react --name n --on t1,t2 [--filter k=v] [--window 5s] -- cmd...` | `run-reactor.sh` plus each reactor's loop | see section 7 |
| `eventlog guard` | `eventlog-guard.sh` | reads a hook payload on stdin; exit 2 with reason on deny |
| `eventlog guard install [--agent claude\|cursor\|codex\|all]` | `install-guard.sh` | writes the hook entry idempotently |
| `eventlog init` | `init-eventlog.sh` | log, `EVENTLOG.md`, default `eventlog.toml`, gitignore lines |
| `eventlog doctor [--fix] [--protect]` | `safety-check.sh`, `setup.sh` | PATH, guard per agent, OS protection, unsanctioned writers by the real allowlist, open lifecycles, stale locks |
| `eventlog protect [--off] [--status]` | `protect-log.sh` | `chflags uappnd` or `chattr +a` |
| `eventlog skill install [--dir path]` | the GitHub skill repo | writes the embedded skill, stamped with the binary version; refuses to overwrite a newer stamp |

`--log <name|path>` on every command selects a named or explicit log.
`--json` prints the same rows as JSON lines for `jq`.

Exit codes: 0 success, 1 a check failed, 2 a guard deny or a lock conflict.
One line on stderr per failure. No panics on malformed input.

## 7. Reactor runtime

`eventlog react` owns everything the shell runtime and reactor scripts did
except the action itself.

1. Lock: `mkdir <log>.<name>.reactor.lock/` with the pid inside. Refuse to
   start while that pid is alive; reclaim a dead one.
2. Baseline: with no own `ack` in the log, append
   `ack by=<name> seq_done=<tip> outcome=skipped detail=baseline` and never
   replay older events.
3. Resume: process every matching event above the highest own `ack seq_done`.
4. Per event:
   1. Append `intent by=<name> for=<seq> action=<label> paths=<list>`; the
      paths are the driving event's `paths=` field.
   2. Run the rule voter against the fold at that moment. Rules: every path is
      in the driving event's own `paths=` or in a live `claim` held by the
      driving agent; no path is under `.context/`; no path is claimed by a
      different open agent; no open `escalate` names this reactor. On failure
      append `veto by=voter for=<intent seq> reason=<rule>` and
      `ack seq_done=<seq> outcome=vetoed`.
   3. Open the veto window (`--window`, default 5 s). Any writer allowed to
      write `veto` may stop the action. A veto in the window acks
      `outcome=vetoed`.
   4. Run the command with the event as JSON on stdin and `EVENTLOG_SEQ`,
      `EVENTLOG_PATHS`, `EVENTLOG_REF` in the environment. Time out at
      `PASS_TIMEOUT`. Retry transient failures as today.
   5. Append `ack seq_done=<seq> outcome=<o>` where `<o>` is the command's
      last stdout line of the form `outcome=...` if present, else
      `committed` on exit 0, `failed` otherwise.
5. Supervise: respawn the loop on a crash, append `note` per restart, append
   `escalate` and exit on a crash loop.

The existing reactor scripts shrink to their action: the committer becomes
the `cursor-agent` call plus the staging check, the doc worker the
`claude -p` call. Both keep working under the shell runtime until switched,
because the log format is unchanged.

Known limit: silence in the window is consent. If nothing is watching, the
rule voter is the only gate. Keep the window short and the voter in-process.

Open question for the plan: a reactor's intent whose `paths=` overlap a file
the controller is editing at that moment. Claims cover workers, not the
controller's own uncommitted edits. First answer: the voter treats
`git status` dirty files outside the driving event's `paths=` as a veto
reason `controller-dirty`.

## 8. Query engine and TUI

`query::State` is one fold over the events. It holds agents (name, model,
pane, phase from `spawn` through `prompt`, `claim`, `progress`, `result`,
`retire`), open claims by path, decisions in force by key, open escalations,
open intents, and the highest `ack seq_done` per reactor. Folding to seq N
is the same fold stopped early; every `--at N` uses it.

The TUI renders that fold live: a top pane with the filtered follow view, a
bottom pane that switches between the agents table and the state summary,
keys from `[keys]` for filter, follow toggle, open-ref and pane switch. It
replaces `eventlog-view.sh -f` in the coordinator pane.

## 9. Guard

`eventlog guard` reads stdin, detects the agent from the payload shape
(Claude Code: `tool_name` plus `tool_input`; Cursor: `command` plus `cwd`
and `sandbox`; Codex: `tool_name`, `tool_input` and `turn_id`), and applies
one denylist: an Edit or Write that targets the log, and any shell command
that names the log path together with a mutation. Deny is exit 2 with the
reason on stderr. Unparseable input fails open. The denylist has a fixture of
adaptive cases (commands crafted to slip past it) that grows with every
bypass found; the fixture is the regression test.

## 10. Testing

- Unit tests per module.
- Fixtures: the 283-event Drove sample at
  `.context/research/samples/drove-events.jsonl` and this repo's own log,
  for the reader, fold and verify.
- `assert_cmd` end-to-end tests: append under contention, react with a fake
  command, guard against captured payloads from all three agents.
- Mutation testing on `query` and the voter, since they carry the safety
  claims.

## 11. Phases

1. Crate, `model`, `log`, `append`, `verify`, `view`. Replaces
   `append-event.sh` and `eventlog-view.sh`.
2. `query`, `agents`, `state`, `check-claims`, `open`, `tui`.
3. `react` with intent and veto; switch the two reactors in this repo.
4. `guard` for three agents, `init`, `doctor`, `protect`, `skill install`,
   cargo-dist release.

Each phase ships a binary this repo runs on itself.
