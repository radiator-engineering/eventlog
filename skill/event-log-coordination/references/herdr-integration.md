# herdr integration

`herdr-orchestration` tells the controller to "maintain a compact registry" of
its workers — identity, model, status, claimed files, disposition. That registry
is usually kept in the controller's head or a scratch note, which is exactly the
state that dies on compaction or a killed pane. Replace it with the append-only
log: same fields, but ordered, durable, and replayable.

Run every `herdr …` command as before. After each one that changes coordination
state, append one event. The controller is already the single writer, so the
single-writer rule holds by construction.

## Action → event mapping

| herdr action (controller)                          | event to append                                             |
|----------------------------------------------------|-------------------------------------------------------------|
| `tab create --label "opus: fix loops"`             | `spawn agent=fix-loops model=opus tab=<tab_id> pane=<pane_id> role=impl` |
| worker gets its file boundary (from the brief)     | `claim agent=fix-loops paths=Sources/Loops,Tests/LoopsTests` |
| `agent start … -- "<packet>"`                      | `prompt agent=fix-loops ref=<packet-file>`                  |
| `agent read` on a working agent (optional poll)    | `progress agent=fix-loops msg=<one line>`                   |
| worker asks a question (`agent wait --until blocked`) | `escalate agent=fix-loops subject=<q> ref=<screen-file>` |
| you answer the worker                              | `approval subject=<q> by=controller decision=<answer>`      |
| report names a cross-worker dependency             | `seam agents=fix-loops,fix-ui subject=<one line> ref=<report-file>` |
| `check-claims.sh fix-loops <base>` finds a gap     | `violation agent=fix-loops paths=<list>`                    |
| `agent wait --until done` + you read the report    | `result agent=fix-loops ref=<report-file> verdict=<v>`      |
| a choice others must follow lands in DECISIONS.md  | `decision key=<k> value=<v> ref=DECISIONS.md`               |
| `tab close <tab_id>`                                | `retire agent=fix-loops disposition=accepted`               |

Record both `tab` and `pane` on `spawn`. A peer started with `agent start` in a
split pane has no tab of its own; the pane ID is what `agent read` and
`check-claims.sh` need later.

Keep the worker's full report out of the log. herdr TUIs run on the alternate
screen, so long reports must be written to a temp Markdown file anyway (per the
orchestration skill's "read the file" fallback) — append the event with
`ref=<that file>`, not the text.

## Why append-only helps a herdr run specifically

- **Survives retirement.** `tab close` destroys the worker's scrollback. If the
  disposition was recorded as a `result`/`retire` event first, closing the tab
  loses nothing the controller needs.
- **Survives the controller's own compaction.** The controller lives longest and
  rots hardest. `tail`+`jq` over the log reconstructs the registry into a fresh
  controller with no transcript replay.
- **Answers "who approved that?"** `approval` events are timestamped and ordered
  next to the `escalate` that prompted them.

## Peer-agent (herdr-peer-agents) variant

The same mapping holds for peer agents in panes rather than tabs: `spawn` on
`agent start`, `prompt` on `agent send` + Enter, `message` when one peer messages
another, `result` after `agent read`, `retire` on `pane close`. Because peers can
message each other directly, `message from=… to=… subject=… ref=…` events are the
audit trail for that cross-talk — still written by the controller as it routes.

## Where a reactor lives

A reactor (see `log-reactors.md`) must outlive the turns of the agent that
owns it. A peer's tool-call shell does not: a `nohup bash watch.sh &` launched
from a Cursor or Claude Code turn is gone when the turn ends, and the peer's
next restart is a second instance racing the first. Give the reactor its own
pane, split off the owning peer's tab and labeled for it (`committer-watch`),
and run the script foregrounded there. Record `spawn` for the reactor with
that `pane=`, and put "the reactor is already running in pane X; do not start
another" in the owning peer's brief.

Peers differ from Task subagents in one way that matters: a peer has a shell in
the same repo, so it *can* run `append-event.sh` or `>>` the log, and
`protect-log.sh` does not stop appends. Single-writer holds for peers only
because the brief says "do not append to `.context/events.jsonl`". Put that line
in every peer brief. Silence from a working peer is therefore expected, not a
fault; if you want a live signal, poll with `agent read` and append `progress`
yourself.
