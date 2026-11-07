# Herdr log panel

A [Herdr](https://herdr.dev) mod over an eventlog: the feed, agents, decisions and
health, with each event's `ref=` file (a handoff, a report) readable in place.

It runs only `eventlog --json` (and `ls` for a folder ref) in the project of the
focused terminal: the first folder at or above it with `.context/eventlog.toml`.
It never reads or writes the log file itself. When the focused terminal is
outside any project, it keeps the last log it showed.

## Install

```sh
cp -R mods/herdr-eventlog ~/.config/herdr-app/mods/eventlog
herdr-app check ~/.config/herdr-app/mods/eventlog
```

## Open it

- As a pane: `herdr-app do pane.split '{"direction":"right","mod":"eventlog"}'`
- As a side panel: add `{ "id": "eventlog", "type": "block", "content": { "mod": "eventlog" } }`
  to your layout's `panel-body` stack, and a button that runs
  `layout.select {stack: "panel-body", child: "eventlog"}` then `layout.show {id: "panel"}`.
- Props: `root` (a project folder), `machine`, `seq` (open one event), `file` (show one file).

## What it shows

- **Feed**: newest first, filtered by type, agent and text. Rebuild events are hidden
  until you turn them on. It loads the last 1,500 events and follows new ones every 3 s.
- **Agents**: phase, model, claims; click one to filter the feed to it.
- **Decisions**: every decision in force, newest first.
- **Health**: open escalations, reactors and their unacknowledged events, open intents and claims.
- **An event**: all its fields, its file (markdown rendered, a folder listed), and more
  events from the same agent. "Open beside" puts the file in a pane next to it.

Limit: a `host:path` ref is on another machine and is not fetched.
