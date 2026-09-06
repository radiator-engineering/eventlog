# eventlog CLI design

Date: 2026-09-06. Status: approved in brainstorm, revised after two
adversarial reviews (correctness, ergonomics), awaiting final spec review.

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
| Guard targets | Claude Code, Cursor (`beforeShellExecution`), Codex (`PreToolUse`); all honor exit 2 | docs checked 2026-09-06 |
| Command names | frozen at v1 (section 6); briefs and Drovefiles call them by name | ergonomics review |
| Skill content | the embedded skill names `eventlog` subcommands, never scripts | ergonomics review |
| Scope | the whole shell toolkit: writer, reader, TUI, scaffold, doctor, guard, reactor runtime, skill install | bootstrap brief |
| Not in v1 | snapshots, event upcasting, a query language, an LLM voter, multi-writer clocks, OpenTelemetry export, log import or merge | research surveys |

## 3. Log format

One JSON object per line. Fields, in this order:

| Field | Type | Rule |
|---|---|---|
| `seq` | integer | starts at 1, increases by 1 per line; the writer sets it |
| `ts` | string | RFC 3339 UTC, second precision; the writer sets it |
| `type` | string | lowercase slug from the vocabulary |
| `prev` | string | hash of the previous line (below); the writer sets it |
| `by` | string | writer identity, from `--as`; absent when the writer is `controller` |
| `agent` | string | subject of the event; required on `spawn`, `prompt`, `claim`, `progress`, `result`, `retire` |
| other | string | flat, type-specific; each under 2 KB; whole line under 4 KB |

**Writer and subject.** `by` says who wrote the line. `agent` says whose
work the line is about. They differ on a `result` the controller records
for a worker (`agent=worker`, no `by`) and coincide on a `result` a worker
writes itself (`by=worker agent=worker`). A `result` the controller writes
for its own work carries `agent=controller`; the writer fills that in when
`agent` is absent and `--as` is `controller`. Pre-existing lines with no
`agent` are read as `agent=controller`. The writer rejects a user-supplied
`by=` unless it equals `--as`.

**Reference fields.** `seq_done`, `for`, `for_ack` and `intent` name another
line's `seq`. On disk they stay JSON strings, so existing logs are
unchanged. The model parses them as integers, rejects non-numeric values at
append time, and compares them numerically everywhere. `max("9","51")` is
never computed on strings.

**Hash chain.** `prev` is the hex sha256 of the previous line's bytes with
trailing `\r` and `\n` stripped. On an empty log the first line writes
`prev=genesis`. On an existing log, the first chained line hashes the last
pre-chain line, so the pre-chain prefix is bound too. Once any line at seq K
carries `prev`, a later line without `prev` is a verify failure, not a
pre-chain line. `eventlog init` writes a `.gitattributes` line marking the
log `-text` so git never rewrites line endings. `eventlog verify` walks the
chain and reports the last good `seq` when it breaks.

**Malformed lines.** A line that does not parse is reported with its line
number; views skip it and `verify` fails on it. If the *last* line does not
parse or has no `seq`, `append` refuses with exit 1 and names the line: a
torn tail is repaired by a person, never guessed around.

**Paths.** A `paths=` field is comma-separated. Each entry is relative to
the repo root, may contain no comma, may not be absolute, and may not escape
the root after resolving `..`. The writer rejects entries that break these
rules; the voter canonicalizes the rest before any rule runs.

**Identity of `seq`.** `seq` is per log. A future import of a worktree log
re-sequences the imported lines and records `orig_seq=` and `orig_log=`.
Nothing in v1 treats `seq` as an identity across logs.

**Not authenticated.** No field in the log proves who wrote it. `--as` is a
declaration. The allowlist in section 4 catches mistakes; OS protection and
the guard resist rewriting; nothing in v1 resists a process that lies about
its name. That is the same trust model as today, stated.

## 4. Configuration

`.context/eventlog.toml` is optional. Missing tables use the defaults below.

```toml
[log]
path = ".context/events.jsonl"   # default log
fsync = false                    # true: fsync after every append
[log.named]                      # extra logs: eventlog --log <name> ...
# review = ".context/review.jsonl"

[vocabulary.result]              # one table per type
fields = ["agent", "ref"]        # required
optional = ["paths", "summary", "verdict"]

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

**Vocabulary.** The built-in vocabulary is the table in `EVENTLOG.md` plus
`ack`, `note`, `intent`, `veto`, `violation`, and the reference fields
`origin`, `for`, `for_ack`, `seq_done`, `intent`. The file adds types or
fields; it cannot remove a built-in type.

**Allowlist precedence**, fixed: `decision key=log-writers` lines in the log
win over the file; the file wins over the built-in default. A later
`log-writers` decision replaces the earlier one in full; revoking a writer
is a new decision that omits it. The allowlist is evaluated as of each
line's own `seq`, so a later revocation never turns history into a breach.

**Built-in default.** Only `controller` writes `spawn`, `prompt`, `claim`,
`decision`, `retire`, `approval`. A `by=`-tagged reactor writes `ack`,
`note`, `escalate`, `violation`, `intent`, `veto` and `result`. The
in-process voter writes `veto by=<reactor> role=voter`; there is no
separate `voter` identity to spoof. Any name granted by a decision writes
what the decision grants. The log is the truth; the file is the seed.

The writer name comes from `--as <name>` or `EVENTLOG_AS`, default
`controller`.

## 5. Modules

One crate. `main.rs` holds clap wiring only.

| Module | Does | Depends on |
|---|---|---|
| `model` | vocabulary, allowlist, config loading and precedence, typed fields, path rules | serde, toml |
| `log` | append (lock, seq, ts, prev, caps, validation, fsync), read, verify | model, sha2 |
| `query` | fold events into `State`; state at seq N; `why` chains | log |
| `react` | reactor runtime: lock, baseline ack, resume, intent, voter, window, run, violation, ack, supervise | log, query |
| `guard` | parse the three hook payloads; one denylist; exit 2 on deny | model |
| `scaffold` | `init`, `doctor`, `protect`, guard install | log, model |
| `tui` | ratatui views over `query` | query |
| `skill` | embedded `skill/` files; `skill install` | include_dir |

Each module has one public entry type and is tested on its own.

## 6. Commands

Names below are frozen at v1.

| Command | Replaces | Behavior |
|---|---|---|
| `eventlog append <type> k=v ... [--as n] [--dry-run] [--no-strict]` | `append-event.sh` | validates, takes the lock, writes one line, prints it; `--dry-run` validates and prints the line without writing, exit 1 on any error |
| `eventlog vocab [<type>] [--json]` | none | required and optional fields per type from the loaded config; `append --help` renders the same table |
| `eventlog verify` | none | walks the hash chain; exit 1 on break with last good seq |
| `eventlog view [-f] [--type a,b] [--agent x] [--by x] [--since ts] [--last N] [--grep s] [--json]` | `eventlog-view.sh` | colored aligned lines; `-f` follows with file watching, polling fallback |
| `eventlog agents [--at N] [--json]` | none | per-agent lifecycle table; flags open lifecycles and unreleased claims |
| `eventlog state [--at N] [--json]` | none | active agents, open claims, decisions in force, open escalations, open intents, unacked events and time since last ack per reactor |
| `eventlog why <seq> [--json]` | none | prints the event, walks `for`, `for_ack`, `seq_done`, `origin`, `intent` and `ref` in both directions, ends with "acted on by X at seq N" or "not matched by reactor X" |
| `eventlog claims <agent> <base> [head]` | `check-claims.sh` (kept as a hidden alias) | changed files no claim covers, from `git diff --name-only --find-renames` plus untracked files from `git status --porcelain`; exit 1 on a gap |
| `eventlog open <seq> [--pager]` | none | opens the event's `ref` in `$EDITOR` or `$PAGER` |
| `eventlog tui` | `eventlog-view.sh -f` in the coordinator pane | see section 8 |
| `eventlog react --as n --on t1,t2 [--filter k=v] [--window 0s] [--git] -- cmd...` | `run-reactor.sh` plus each reactor's loop | see section 7 |
| `eventlog react test <seq> --as n [--git] -- cmd...` | none | runs intent, voter and the command against a real seq; prints every event it would append; writes nothing |
| `eventlog guard [--agent claude\|cursor\|codex]` | `eventlog-guard.sh` | reads a hook payload on stdin; exit 2 with reason on deny |
| `eventlog guard install [--agent claude\|cursor\|codex\|all]` | `install-guard.sh` | writes the hook entry, with `--agent` baked in, idempotently |
| `eventlog init` | `init-eventlog.sh` | log, `EVENTLOG.md`, default `eventlog.toml`, gitignore and gitattributes lines |
| `eventlog doctor [--fix] [--protect]` | `safety-check.sh`, `setup.sh` | PATH, guard per agent, OS protection, unsanctioned writers by the as-of-seq allowlist, reference validation over history, open lifecycles, stale locks, skill stamp vs binary version; `--fix` also removes the old script symlinks from PATH |
| `eventlog protect [--off] [--status]` | `protect-log.sh` | `chflags uappnd` or `chattr +a` |
| `eventlog schema [--events\|--output]` | none | JSON Schema generated from the model types |
| `eventlog skill install [--dir path]` | the GitHub skill repo | writes the embedded skill, stamped with the binary version; refuses to overwrite a newer stamp |
| `eventlog completions <shell>` | none | shell completions |

**Strict append.** By default `append` validates against the current fold:
required fields per type are present; `agent=` on `result`, `progress`,
`claim`, `retire` names an agent with an open `spawn`; `retire` closes an
open lifecycle; `seq_done`, `for`, `for_ack`, `intent` name an existing seq;
each `claim` path exists on disk or is a glob that matches something. A
failure exits 1 with the rule name. `--no-strict` bypasses for repair work.
`doctor` runs the same rules over history.

**`--agent x`** matches any of `agent`, `by`, `from`, `to`. `--by x`
matches the writer only.

**`--log <name|path>`** on every command selects a named or explicit log.

**`--json`** prints one object per row with `"v":1`. `v` bumps on any
removal or rename of a field, never on an addition.

Exit codes: 0 success, 1 a check failed, 2 a guard deny or a lock conflict.
One line on stderr per failure. No panics on malformed input.

## 7. Reactor runtime

`eventlog react` owns everything the shell runtime and reactor scripts did
except the action itself.

1. **Lock.** `<log>.<name>.reactor.lock/` holds `{pid, process start time,
   hostname, boot id}`. The lock is live only when the pid and start time
   match on this host. Reclaim is by atomic rename to
   `<lock>.stale.<random>`; only the process whose rename succeeded creates
   the new lock, then re-reads it to confirm its own token.
2. **Baseline.** With no own `ack` in the log, append
   `ack by=<name> seq_done=<tip> outcome=skipped detail=baseline` and never
   replay older events.
3. **Resume.** Process every matching event above the highest own
   `ack seq_done`, compared as integers. If an own `intent` exists for a
   driving seq with no own `ack`, the runtime was interrupted after
   declaring intent: append `ack seq_done=<seq> outcome=interrupted` and
   `escalate`, and do not re-run, because the effect may have happened.
4. **Per event.**
   1. Compute the **authorized set**. If the driving event has no `by`
      (controller-written), the authorized set is its `paths=`. Otherwise
      it is the driving event's `paths=` intersected with the live `claim`s
      held by the driving writer; any excess is a veto with reason
      `unclaimed-paths`. A reactor's own `result` therefore never widens
      its scope beyond what the controller claimed for it at spawn.
   2. Append `intent by=<name> for=<seq> action=<label> paths=<authorized>`.
   3. Run the rule voter against the fold at that moment. Rules: no path is
      the log file or a lock dir (`<log>`, `<log>.*`); no path is claimed by
      a different open agent; no open `escalate` names this reactor. On
      failure append `veto by=<name> role=voter for=<seq> intent=<intent
      seq> reason=<rule>` and `ack seq_done=<seq> outcome=vetoed`.
   4. Open the veto window (`--window`, default 0). A `veto` whose `for=`
      names the driving seq binds regardless of which intent it saw, so a
      restart between intent and action cannot lose it. The window is
      opt-in for irreversible actions; commits do not need it.
   5. Run the command with the event as JSON on stdin and these variables:
      `EVENTLOG_LOG`, `EVENTLOG_SEQ`, `EVENTLOG_TYPE`, `EVENTLOG_AGENT`,
      `EVENTLOG_BY`, `EVENTLOG_PATHS` (the authorized set), `EVENTLOG_REF`,
      `EVENTLOG_RESUME` (the highest own ack), `EVENTLOG_OUTCOME_FILE`.
      Time out at `PASS_TIMEOUT`. **No retry by default.** The command
      asks for one retry by writing `outcome=retryable` to the outcome
      file; everything else acks once.
   6. With `--git`, snapshot `HEAD` and `git status --porcelain` before and
      after. Files the action touched outside the authorized set are
      appended as `violation by=<name> for=<seq> paths=<outside>`. This is
      detection after the fact; the commit stands, as today.
   7. Append `ack seq_done=<seq> outcome=<o> ...` where the fields come from
      the outcome file (`k=v` lines; `seq`, `ts`, `prev`, `by` rejected),
      `outcome` defaulting to `committed` on exit 0 and `failed` otherwise.
      The last stdout line of the form `outcome=...` is a fallback only
      when the file is absent.
5. **Supervise.** Respawn the loop on a crash, append `note` per restart,
   append `escalate` and exit on a crash loop.

The existing reactor scripts shrink to their action: the committer becomes
the `cursor-agent` call plus its HEAD-moved check, the doc worker the
`claude -p` call. Both keep working under the shell runtime until switched,
because the log format is unchanged. The doc worker needs a `claim` at spawn
covering its doc roots; `layout.sh` records one.

**Tracked files under `.context/`.** The voter excludes only the log and
its lock dirs, so `DECISIONS.md`, briefs and reactor scripts are committable
when a controller `result` names them. The committer's brief stops saying
"never stage anything under `.context/`" and says "never stage the log".

**Controller overlap, resolved.** The committer stages only the authorized
set, so other dirty files are inert. A path in a controller `result` is the
controller's own declaration that the path is done. No `controller-dirty`
rule.

## 8. Query engine and TUI

`query::State` is one fold over the events. It holds agents (name, model,
pane, phase from `spawn` through `prompt`, `claim`, `progress`, `result`,
`retire`), open claims by path, decisions in force by key (including the
allowlist as of that seq), open escalations, open intents (an `intent`
with no matching `ack`), and the highest `ack seq_done` per reactor with
its timestamp. Folding to seq N is the same fold stopped early; every
`--at N` uses it. `why` walks the reference fields over the same fold.

The TUI renders that fold live: a top pane with the filtered follow view, a
bottom pane that switches between the agents table and the state summary,
keys from `[keys]` for filter, follow toggle, open-ref, `why` on the
selected row, and pane switch. It replaces `eventlog-view.sh -f` in the
coordinator pane.

## 9. Guard

`eventlog guard` reads stdin. The agent comes from `--agent`, which
`guard install` bakes into the hook entry; shape detection (Claude Code:
`tool_name` plus `tool_input`; Cursor: `command` plus `cwd`; Codex:
`tool_name`, `tool_input` and `turn_id`) is used only when the flag is
absent. Input that is not JSON fails open. JSON that matches no known shape
fails closed with "unrecognized payload".

One denylist: an Edit or Write whose target is the log, and any shell
command that names the log path together with a mutation. **Passthrough is
never by substring.** A command is allowed as the sanctioned writer only
when it parses as a single simple command whose argv[0] is `eventlog` or
`append-event.sh`; any command containing `;`, `&&`, `||`, `|`, `$(`, a
backtick or a newline that also names the log is denied.

Deny is exit 2 with the reason on stderr, plus
`{"permission":"deny","userMessage":"..."}` on stdout for Cursor. An
end-to-end fixture per agent, from a captured payload, asserts the deny is
honored, not only that exit 2 was returned. The denylist has a fixture of
adaptive cases (commands crafted to slip past it) that grows with every
bypass found.

## 10. Testing

- Unit tests per module.
- Fixtures: the 283-event Drove sample at
  `.context/research/samples/drove-events.jsonl` and this repo's own log,
  for the reader, fold, `why` and verify. Add a line with a quoted
  `seq_done` and a torn last line.
- `assert_cmd` end-to-end tests: append under contention (two processes,
  one lock), lock reclaim with two starters, react with a fake command
  including an interrupted intent, guard against captured payloads from
  all three agents.
- Mutation testing on `query`, the voter and the guard parser, since they
  carry the safety claims.

## 11. Phases

1. Crate, `model`, `log`, `append` with strict validation and `--dry-run`,
   `vocab`, `verify`, `view`, `schema`. Replaces `append-event.sh` and
   `eventlog-view.sh`.
2. `query`, `agents`, `state`, `why`, `claims`, `open`, `tui`.
3. `react` and `react test` with intent, voter and violation; switch the
   two reactors in this repo; record the doc worker's claim.
4. `guard` for three agents, `init`, `doctor`, `protect`, `skill install`
   with the rewritten skill, `completions`, cargo-dist release.

Each phase ships a binary this repo runs on itself.
