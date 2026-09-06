# TUI: `src/tui` and `eventlog tui`

Status: implemented. A live terminal UI over [`query::State`](query-module.md):
a follow pane of log events on top, and a bottom pane that toggles between an
agents table, a full state snapshot, and a `why` explanation of the selected
event.

```sh
eventlog tui [--log <name|path>]
```

`tui` reads the log once, folds it with `query::fold`, then re-reads and
re-folds whenever the file grows. Folding the whole log on every new line is
cheap at this scale, so `App` keeps the raw `Vec<Event>` and refolds rather
than updating `State` incrementally.

## Layout

```
┌─ follow ──────────────────────────────┐
│ seq  TYPE       agent          summary│
│ ...                                   │
├─ agents / state / why ────────────────┤
│ ...                                   │
└────────────────────────────────────────┘
```

A third line appears at the bottom only while the filter input is open:
`filter: <text being typed>`.

### Follow pane

Lists events as `<seq> <type> <agent> <summary>`, `agent` being the event's
`agent` field or its writer. The title shows `[filter: <text>]` when a filter
is set and `[following]` while follow mode is on. The selected row is
highlighted; the pane shows only the events that fit, scrolled so the
selection stays visible.

### Bottom pane

Toggles between three views ([`BottomView`](../../src/tui/mod.rs)):

- **agents** (default) — a table of `agent`, `model`, `phase` for every agent
  in `state.agents`. `phase` is the same lifecycle value `eventlog agents`
  prints: `spawned`, `prompted`, `claimed`, `progressing`, `resulted`,
  `retired`.
- **state** — `state.at`, and counts of agents, open claims, escalations,
  intents, and open lifecycles, followed by one line per decision in force
  (`decision <key>=<value> (seq <seq>)`).
- **why** — the [`why`](query-commands.md#why) report for the selected row:
  the event itself, what it references (`for`, `for_ack`, `seq_done`,
  `intent`), and what references it back, in each case as `seq <seq> <type>
  by=<writer>`.

## Keys

Keys come from the `[keys]` table in config (see
[Model contract](model-contract.md)); the defaults are:

| Key | Action |
|---|---|
| `/` | Open the filter input line. |
| `f` | Toggle follow mode. |
| `tab` | Switch focus between the follow pane and the bottom pane. |
| `o` | Run `open` on the selected row's `ref` field, suspending the terminal for the duration. |
| `w` | Switch the bottom pane to `why` for the selected row. |
| `↑` / `↓` | Move the selection; turns off follow mode. |
| `q` | Quit. |

While the filter input is open, every key edits its buffer instead of
triggering a binding: `Enter` applies the typed text as the filter and closes
the input, `Esc` cancels, `Backspace` deletes the last character.

### Filtering

The filter is a case-insensitive substring match, over `type`, `by`, `agent`,
and `ref`. It is built directly over `Vec<Event>` inside `src/tui`, separate
from the filters `eventlog view` implements in `src/cmd/view.rs`.

## Tests

`tests/tui_render.rs` drives [`views::render`](../../src/tui/views.rs)
against a `ratatui::backend::TestBackend`, with no real terminal, over the
frozen fixture `tests/fixtures/self-log-2026-09-06.jsonl` (self-log at seq
125). It checks: the follow pane shows the fixture's last lines and scrolls
the first one out on a 30-row terminal; pressing the filter key, typing
`ack`, then `Enter` leaves only `ack` rows and shows `[filter: ack]` in the
title; and the agents pane shows a retired agent with phase `retired`. Run
them with:

```sh
cargo test
```

## See also

- [Query module](query-module.md) — `State`, `fold`, and the `Phase` values
  the agents pane prints.
- [Query commands](query-commands.md) — the non-interactive `agents`,
  `state`, and `why` commands this TUI mirrors.
- [View](view.md) — the filtered, non-interactive log display.
- [`eventlog` command list](eventlog-cli-surface.md)
