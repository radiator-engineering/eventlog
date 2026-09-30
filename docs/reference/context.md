# Context: `eventlog context`

Status: implemented. Renders a controller's context from the log, decides when
to rebuild it, and installs the `eventlog-context` mod that acts on both. The
task-oriented guide is [Rebuild context from the log](../how-to/rebuild-context-from-the-log.md).

```sh
eventlog context [--budget <chars>] [--json]
eventlog context check --percent <n> [--growth <g>] [--json]
eventlog context install [--classic] [--force] [--json]
```

## `eventlog context`

Prints the context packet as Markdown, or as JSON with `--json`. It reads the
log, the working tree, and the file behind the controller's newest open
intent. It writes nothing.

| Flag | Meaning |
|---|---|
| `--budget <chars>` | Packet size cap. Default: `[context] budget_chars` (12,000). |
| `--json` | One JSON object: `v`, `as_of`, `over_budget`, `markdown`, `sections`, `settings`. |
| `--log <log>` | The log to read. |

The packet has these sections, in order: a header that says the reader is the
controller, decisions in force, agents, recent history, the artifact index,
reactor health (only when a reactor has unacked events or a stale ack), open
work, and the current task. The same log and working tree always give the same
bytes. Ages count from the newest event, not from the clock.

Size rules:

- Every line is cut to 200 characters.
- Decisions and open work show their newest 20 lines; agents show 10. Older
  lines become "+N older not shown".
- A decision whose value is `retired` is not in force.
- More than three untracked files in one directory collapse to one line.
- Over budget, the command drops artifact lines, then the oldest decision and
  open-work lines (down to four each), then the oldest history (down to five),
  then agent lines. If the header and current task alone exceed the budget, it
  prints them whole and warns on stderr.
- The current task inlines at most half of the budget from the intent's `ref`.

## `eventlog context check`

Prints `{"rebuild": bool, "reason": text}` for a given context fill. It writes
nothing.

| Flag | Meaning |
|---|---|
| `--percent <n>` | Context fill, 0 to 100. Required. |
| `--growth <g>` | Expected growth by the next check, in points. Default 0. |

Rules, in order; the first that applies decides:

1. `below-floor`: `percent` is below `floor_percent`. No rebuild.
2. `backstop`: `percent + growth` is at least `backstop_percent`. Rebuild.
3. `boundary`: the controller has a `result` after the last `rebuild`, no
   controller intent is open, and no file outside `.context/` changed after
   that result unless another agent's open claim covers it. Rebuild.
4. `mid-task`. No rebuild.

## `eventlog context install`

Writes the mod to `<git top level>/.claude/skills/eventlog-context/`. It
replaces an earlier install of the mod, so rerun it after upgrading
`eventlog`.

| Flag | Meaning |
|---|---|
| `--classic` | Print a `SessionStart` hook for `settings.json` and write nothing. |
| `--force` | Install even when `settings.json` or `settings.local.json` already has the classic hook. |

Claude Code loads the mod only in a trusted workspace, started with
`CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`, version 2.1.278 or later.

## Settings

`[context]` in `.context/eventlog.toml`. Only the context commands read it.

| Key | Default | Meaning |
|---|---|---|
| `floor_percent` | 25 | No rebuild below this fill. |
| `backstop_percent` | 60 | Rebuild at this fill plus growth. |
| `keep_turns` | 3 | Turns kept word for word after the packet. |
| `tail_chars` | 40000 | Size cap for those turns. |
| `budget_chars` | 12000 | Packet size cap. |

`floor_percent` and `backstop_percent` must be 0-100, and `floor_percent` must
be below `backstop_percent`.

## The `rebuild` event

The mod appends `rebuild` after each rebuild: `trigger` (`boundary`,
`backstop`, `command`, `manual`, `auto`, `plugin`), `as_of`, `kept_turns`,
`tokens_before`, and a `reason` when something unusual happened
(`tail-trimmed`, `tail-too-large`, `engine-fallback`). Run `eventlog vocab`
for the fields.

## Related

- [Rebuild context from the log](../how-to/rebuild-context-from-the-log.md)
- [Design spec](../superpowers/specs/2026-09-28-context-from-log-design.md)
