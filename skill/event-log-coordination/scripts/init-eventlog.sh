#!/usr/bin/env bash
# init-eventlog.sh — scaffold the append-only coordination log for a repo.
# Idempotent: creates what is missing, never overwrites what exists.
#
#   .context/events.jsonl   the append-only log (single-writer: the controller)
#   .context/EVENTLOG.md     one-screen readme: schema + the single-writer rule
#   + gitignores .context/events.jsonl and its .lock
#
# Run from the repo root. Pair with the PreToolUse hook (see SKILL.md) so the
# append-only rule is enforced, not just documented.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG=".context/events.jsonl"
DOC=".context/EVENTLOG.md"
made=()

mkdir -p .context

if [ ! -e "$LOG" ]; then
  : > "$LOG"
  made+=("$LOG")
fi

if [ ! -e "$DOC" ]; then
  cat > "$DOC" <<'EOF'
# Coordination event log

`events.jsonl` is the append-only, ordered truth for this task's multi-agent
work. One JSON object per line. It is **not** a transcript — it holds small
coordination facts, and points at bigger artifacts by path.

## Rules
- **Single writer.** Only the controller/driver appends, and only via
  `append-event.sh`. Workers report back; the controller records.
- **Append-only.** Lines are never edited or deleted. "The log up to event N"
  is exactly what the system knew at event N — that is what makes it replayable
  and auditable.
- **Small events.** No model output, no file contents. Reference artifacts by
  path: `ref=.context/handoffs/foo.md`.

## Event shape
Every line has `seq` (monotonic), `ts` (UTC), `type`, plus type-specific fields:

| type       | fields (besides seq/ts/type)                    | meaning                          |
|------------|--------------------------------------------------|----------------------------------|
| `spawn`    | agent, model, tab, role                          | a worker was launched            |
| `prompt`   | agent, ref                                        | a delegation packet was sent     |
| `message`  | from, to, subject, ref                            | inter-agent routing              |
| `drain`    | agent                                             | inbox/turn processed             |
| `result`   | agent, ref, verdict                               | worker reported; artifact at ref |
| `decision` | key, value, ref                                   | a choice others must follow      |
| `escalate` | subject, ref                                      | queued for human approval        |
| `approval` | subject, by, decision                             | human decided                    |
| `retire`   | agent, disposition                                | worker tab closed                |

Parallel-work events (append these when workers run side by side):

| type        | fields (besides seq/ts/type)                     | meaning                                        |
|-------------|--------------------------------------------------|------------------------------------------------|
| `claim`     | agent, paths (comma-separated globs)             | the files this worker owns; nobody else edits  |
| `progress`  | agent, msg, ref                                  | controller polled a working agent; short note  |
| `seam`      | agents, subject, ref                             | a cross-worker dependency found mid-work       |
| `violation` | agent, paths                                     | worker changed files outside its claim         |
| `ack`       | by, seq_done, outcome, ref                       | a reactor acted on event seq_done              |

Any line the controller did not write carries `by=<agent>`; only writers named
in a `decision key=log-writers` may set it. A reactor (a process that acts on
events, e.g. a committer) resumes from its own `ack` lines, never a side file.

Extend with new `type`s freely; keep fields flat and small.

## Read it
    eventlog-view.sh -f                           # live, colored, aligned (for a pane)
    tail -f .context/events.jsonl                 # raw
    jq -c 'select(.type=="decision")' .context/events.jsonl
    jq -c 'select(.agent=="reviewer")' .context/events.jsonl
    jq -r 'select(.type=="claim") | "\(.agent)\t\(.paths)"' .context/events.jsonl   # who owns what
    check-claims.sh reviewer feature/base            # did the worker stay inside its claim?
EOF
  made+=("$DOC")
fi

# gitignore the volatile bits (log + lock). Keep EVENTLOG.md committed.
ensure_ignore() {
  local line="$1"
  if [ -f .gitignore ]; then
    grep -qxF "$line" .gitignore || printf '%s\n' "$line" >> .gitignore
  else
    printf '%s\n' "$line" > .gitignore
  fi
}
if [ ! -f .gitignore ] || ! grep -qxF '.context/events.jsonl' .gitignore; then
  ensure_ignore '# coordination event log (event-log-coordination)'
  ensure_ignore '.context/events.jsonl'
  ensure_ignore '.context/events.jsonl.lock'
  made+=(".gitignore (+events.jsonl)")
fi

if [ ${#made[@]} -eq 0 ]; then
  echo "event-log: already initialised, nothing to do"
else
  printf 'event-log: created %s\n' "${made[@]}"
fi
echo "next (agent runs, not the user): append events with 'append-event.sh <type> key=value ...'"
echo "  e.g. append-event.sh spawn agent=reviewer model=opus   — guard enforces append-only"

# Surface the protection posture at the moment a log exists — this is the point
# where OS-level protection becomes an opt-in the user should knowingly make.
# Report-only (never auto-protects); safety-check.sh prints the opt-in command.
if command -v safety-check.sh >/dev/null 2>&1; then
  echo
  safety-check.sh .context/events.jsonl || true
fi
