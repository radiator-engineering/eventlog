# AGENTS.md

Guidance for AI coding agents working in this repo. Keep it short and current.
Claude Code reads it through CLAUDE.md, which imports it.

<!-- log-driven-workspace:start — managed by setup-log-driven-workspace; refresh with setup.sh --orders -->
## Coordination: this repo is log-driven

`.context/events.jsonl` is the append-only coordination log. Two reactors watch it, and nobody pings them: the **commit reactor** commits the files a `result` names; the **doc worker** then documents what was committed, editing only `docs,README.md,AGENTS.md`. Until a `result` is appended, nothing is committed and nothing is documented.

### Which agent are you?

- **Controller**: the Claude in the `control › coordinator` pane, or any agent with no brief naming it. The rest of this block is for you.
- **Spawned worker**: a brief in `.context/handoffs/` names you, or your prompt says you were spawned by the controller. Do only the task in your brief, touch only the paths it claims, and report back to the controller in your final message. Never run `eventlog append`, never `git commit`; the controller records your work. If you hit anything outside your brief, stop and report; do not widen scope.
- **Reactor** (commit reactor, doc worker): your own brief in `.context/handoffs/` wins over this block.

### Controller commands

- Report a finished change (the last step of every task that touches files, before you answer the user):
  `eventlog append result ref=<main file> paths=<comma-separated changed files> summary="<one line>"`
- Record a decision: `eventlog append decision key=<k> value=<v> ref=.context/DECISIONS.md`, and add the entry to `.context/DECISIONS.md`.
- Spawn a worker: write its brief to `.context/handoffs/<name>.md` (task, claimed paths, the pinned contract it builds against, "do not append to the log"), then `eventlog append spawn agent=<name> …`, `prompt agent=<name> ref=<brief>`, `claim agent=<name> paths=<globs>`. When it reports: `result agent=<name> ref=<brief> …`, then `retire agent=<name>`. Freeze anything two workers share (a contract, a model) as a `decision` before they start.
- Read the log: `eventlog view --last 20`, `eventlog state`, or `jq -c . .context/events.jsonl`. Run any read of the log as its own command; the guard blocks compound commands that name the log.

### Boundaries (everyone)

- Only `eventlog append` writes the log, and only the controller and the reactors run it. Never edit, truncate or `rm` the log; never `rm` a `*.reactor.lock` dir. The log is OS-protected (`eventlog protect`); `eventlog protect --off` before any intentional removal.
- Never `git commit`; the commit reactor does that from a `result`.
- Never message, ping or restart a reactor. If one looks dead, say so; the user reruns `drove up` (the `Drovefile` owns the layout and the reactor panes).
- Keep this block and its markers; other sections of this file are the project's to maintain.

A Stop hook enforces the controller's first command in the controller's pane only: if changed files have no `result` behind them, it hands the turn back once with the file list. Append the event, then finish.

### Pointers

- Event vocabulary: `.context/EVENTLOG.md`. Decisions so far: `.context/DECISIONS.md`.
- Worker briefs: `.context/handoffs/`. Reactor settings: `.context/workspace.env`.
<!-- log-driven-workspace:end -->
