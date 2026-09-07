# herdr integration

A herdr controller keeps a registry of its workers: pane, model, status,
claimed files, disposition. Keep that registry in the log instead, so it
survives compaction and closed panes. Run each `herdr` command as usual,
then append one event. `eventlog vocab` accepts every line below.

| herdr action (controller) | event to append |
|---|---|
| `pane split` + `agent start fix-loops --kind claude` | `spawn agent=fix-loops model=opus pane=<pane_id> role=impl` |
| brief names the worker's files | `claim agent=fix-loops paths=Sources/Loops,Tests/LoopsTests` |
| `agent prompt fix-loops "<packet>"` | `prompt agent=fix-loops ref=<packet-file>` |
| `agent read` on a working agent (optional poll) | `progress agent=fix-loops msg=<one line>` |
| `agent wait --until blocked` shows a question | `escalate agent=fix-loops subject=<q> ref=<screen-file>` |
| you answer it | `approval subject=<q> decision=<answer>` |
| report names a cross-worker dependency | `seam agents=fix-loops,fix-ui subject=<one line> ref=<report-file>` |
| `eventlog claims fix-loops main` finds a gap | `violation agent=fix-loops paths=<list>` |
| `agent wait` returns done and you read the report | `result agent=fix-loops ref=<report-file> paths=<files> summary=<one line>` |
| a choice others must follow | `decision key=<k> value=<v> ref=.context/DECISIONS.md` |
| `pane close` | `retire agent=fix-loops disposition=accepted` |

Record `pane=` (and `tab=` when the worker has its own tab) on `spawn`. The
controller never writes `by=`; an `approval` is the controller's by
construction.

Long reports go to a file (herdr agents run on the alternate screen, so a
read may not recover them) and the event carries `ref=<that file>`.

## Peers can reach the log

A Task subagent cannot run `eventlog append`. A herdr peer has a shell in the
same repo, so it can, and `eventlog protect` still allows appends. Single
writer holds for peers only because the brief says "do not append to
`.context/events.jsonl`". Put that line in every brief. `eventlog doctor`
reports any line whose writer the allowlist does not sanction.

## Where a reactor lives

Give a reactor its own pane, split off the owning agent's tab and named for
it, and run `eventlog react` foregrounded there. Record `spawn agent=<name>
pane=<pane_id> runtime=eventlog-react`, and tell the owning agent in its
brief that the reactor is already running.
