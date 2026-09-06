# Bootstrap brief: event-log controller

You are the **controller** for this repo (see AGENTS.md). This brief transfers
context from the Claude session in `~/Development/Drove` that created this
repo on 2026-09-06. Read it fully, then pick up the conversation with the user
where it left off: mid-brainstorm, on the architectural path.

## What this repo is

A Rust CLI with a TUI that replaces the shell toolkit in the
`event-log-coordination` skill (`~/.claude/skills/event-log-coordination/`).
The skill itself will be versioned in this repo alongside the binary.

Why: Drove (`~/Development/Drove`) and Radiator both depend on the event-log
conventions, but today they only work on a machine where the skill's shell
scripts are symlinked onto PATH (`examples/log-driven/Drovefile` in Drove
calls `append-event.sh`, `eventlog-view.sh`, `protect-log.sh` by bare name).
The user wants enforcement that is consistent across repos and works for any
agent (Claude Code, Cursor, Codex). A versioned, installable CLI is the fix;
the skill becomes thin and points at the CLI for the *how*.

The TUI replaces `eventlog-view.sh -f`, which runs in the lower pane of the
`coordinator` tab in every log-driven workspace (including this one).

## Decisions the user has already made

1. **Scope of v1: absorb the whole shell toolkit.** All four groups:
   - writer + reader + TUI (`append-event`, `eventlog-view`, new query commands)
   - scaffold + doctor (`init-eventlog`, `safety-check`, `protect-log`, `install-guard`)
   - the PreToolUse guard (`eventlog-guard`) as a subcommand the hook calls
   - the reactor runtime (the ack/lock/resume loop from
     `references/reactor-example.sh` and
     `~/.claude/skills/setup-log-driven-workspace/templates/run-reactor.sh`),
     so a reactor is `<cli> react --on <types> -- <cmd>` rather than a copied script
   The previous controller proposed building these in phases (core first, then
   scaffold/doctor, guard, reactor runtime) and the user did not object.
2. **What the TUI and query commands must answer (all four):**
   - filter + follow: live tail with filters by type, agent, time range, free text
   - agent lifecycle view: per-agent spawn -> prompt -> claim -> progress ->
     result -> retire, flagging open lifecycles and unclosed claims
   - state at seq N / replay: active agents, open claims, decisions in force,
     unacked events per reactor
   - open the ref: from any event, open its `ref=` file in a pager or `$EDITOR`
3. The tool must **read and query** logs, not only write them (user's words).
4. It must be **configurable** (user's word; not yet defined, see below).
5. **This repo is the tool; the skill ships with it.** `skill/` holds the
   skill as plain files (SKILL.md, references; no scripts, the CLI replaces
   them). The binary embeds them at build time and installs them with a
   `skill install` subcommand, stamped with the binary version, so skill and
   CLI cannot drift. `SKILL.md` names subcommands, never script paths. The
   GitHub repo `radiator-engineering/event-log-coordination` stops being the
   source: it mirrors `skill/` or is archived.

## Open questions (ask these next, one at a time)

The previous controller was about to ask these when the user redirected it to
create this repo. Do not re-ask what is settled above.

- **Name** of the binary and crate (the repo is `event-log`).
- **Distribution**: same as Drove (cargo-dist, Homebrew tap, crates.io)?
- **What "configurable" covers**: colors per type (today via
  `EVENTLOG_COLOR_<TYPE>` / `.context/eventlog-view.conf`), columns, keybindings,
  event vocabulary, log path, multiple logs?
- **Format compatibility**: the JSONL must stay readable by existing logs
  (`seq`, `ts`, `type`, flat string fields, `by=` for non-controller writers;
  see `.context/EVENTLOG.md` in any log-driven repo). Confirm whether a hash
  chain (`prev` = sha256 of the previous line) is in scope for v1.

## Then

Follow the `superpowers:brainstorming` architectural path from where it stands:
remaining questions, 2-3 approaches with a recommendation, sectioned design,
spec at `docs/superpowers/specs/2026-09-06-event-log-cli-design.md`, user
review, then `superpowers:writing-plans`. This project is one repo and one
binary; spec it whole, plan it in phases.

## Pointers

Everything below is inside this repo; start with `.context/research/README.md`.

- The skill to replace and version: `skill/event-log-coordination/` (copied
  from `github.com/radiator-engineering/event-log-coordination` at `6d9dac4`).
  `scripts/append-event.sh` is the format authority: seq, ts, mkdir lock, 2 KB
  field and 4 KB event caps, `--log PATH`, `EVENTLOG_PATH`.
- Reactor rules: `skill/event-log-coordination/references/log-reactors.md`; the
  reactors in real use: `.context/research/setup-log-driven-workspace/templates/`.
- Research reports and findings: `.context/research/`.
- A real 283-event log to test against: `.context/research/samples/drove-events.jsonl`.
- Drove's Drovefile reference: `~/Development/Drove/docs/drovefile.md`. This
  repo's own `Drovefile` starts the workspace you are in.
- Drove's release setup to copy for distribution: `~/Development/Drove/dist-workspace.toml`
  and commit `048adfb`.

## First: shape this herdr session

You are running in a dedicated herdr session for this repo. `drove up` cannot
create workspaces or panes yet (its `cli.rs` only runs tasks; `apply_plan` in
`executor.rs` is not wired in), so the `Drovefile` here is the declaration, not
the launcher. Before anything else, run from this pane:

    ~/.claude/skills/setup-log-driven-workspace/scripts/layout.sh --no-files

It shapes the session around this pane (control/coordinator, eventlog pane
below you, monitor tab, maintenance workspace with both reactors), writes
`.context/layout.json`, and records `spawn`/`prompt` for the reactors. Then run
`status.sh` from the same directory and confirm exit 0.

The log entries seq 4-11 are from an earlier attempt that built this layout in
the wrong herdr session; every agent in them is retired. Ignore them.

## Workspace notes

- The log is not OS-protected. `drove run protect-log --yes` opts in; ask the
  user first (the committer is cursor-agent, which the Claude guard does not cover,
  so the skill recommends yes).
- Every task that changes files ends with `append-event.sh result ...` (AGENTS.md).
