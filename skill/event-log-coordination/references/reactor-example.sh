#!/usr/bin/env bash
# reactor-example.sh — a log reactor that commits on `decision key=commit-message`.
#
# A reactor is a long-lived process that tails the coordination log and acts on
# matching events without a prompt. This one is the reference shape; copy it and
# change ACT. The three rules it exists to demonstrate (see log-reactors.md):
#
#   1. It runs ONCE. A mkdir lock refuses a second instance (stale locks from a
#      killed process are detected by pid and reclaimed).
#   2. It resumes FROM THE LOG. Every action it takes is recorded back as an
#      `ack` event, and the resume point is the highest acked seq. There is no
#      side cursor file to go stale, get hand-reset, or race.
#   3. It stages only what the decision names (`paths=`). Without `paths=` it
#      falls back to everything dirty and says so in the ack, because a
#      whole-tree `git add -A` will sweep in other workers' unfinished files.
#
# Launch it in a real terminal (a dedicated herdr pane), foregrounded:
#   bash reactor-example.sh
# Never `nohup … &` it from an agent's tool-call shell: that shell is torn
# down at the end of the turn and takes the reactor with it.
#
# Env: EVENTLOG_PATH (default .context/events.jsonl), REACTOR_NAME (default committer).
set -uo pipefail

LOG="${EVENTLOG_PATH:-.context/events.jsonl}"
ME="${REACTOR_NAME:-committer}"
LOCK="$LOG.$ME.reactor.lock"

command -v jq >/dev/null || { echo "reactor: jq is required" >&2; exit 1; }
command -v append-event.sh >/dev/null || { echo "reactor: append-event.sh not on PATH (run safety-check.sh --doctor)" >&2; exit 1; }
[ -e "$LOG" ] || { echo "reactor: no log at $LOG" >&2; exit 1; }

# --- 1. single instance -----------------------------------------------------
if ! mkdir "$LOCK" 2>/dev/null; then
  other="$(cat "$LOCK/pid" 2>/dev/null || true)"
  if [ -n "$other" ] && kill -0 "$other" 2>/dev/null; then
    echo "reactor: another $ME reactor is running (pid $other, lock $LOCK)" >&2
    exit 1
  fi
  echo "reactor: reclaiming stale lock $LOCK (pid ${other:-?} is gone)" >&2
  rm -rf "$LOCK"; mkdir "$LOCK" || exit 1
fi
echo "$$" > "$LOCK/pid"
trap 'rm -rf "$LOCK"' EXIT

# --- 2. resume point from the log, not a side file --------------------------
acked_through() {
  jq -r --arg me "$ME" 'select(.type=="ack" and .by==$me) | .seq_done' "$LOG" 2>/dev/null \
    | sort -n | tail -1
}

# --- the action: one commit per decision -------------------------------------
ACT() {   # $1=seq $2=event json
  local seq="$1" line="$2" msg paths sha staged
  msg="$(jq -r '.value' <<<"$line")"
  paths="$(jq -r '.paths // empty' <<<"$line")"

  if [ -n "$paths" ]; then
    # shellcheck disable=SC2086
    git add -- ${paths//,/ }; staged="paths=$paths"
  else
    git add -A; staged="paths=ALL-DIRTY"      # sweep risk: say so in the ack
  fi

  if git diff --cached --quiet; then
    append-event.sh ack agent="$ME" by="$ME" seq_done="$seq" outcome=skipped \
      detail="seq $seq ($msg): nothing staged" >/dev/null
    return
  fi
  if git commit -q -m "$msg" -m "Covers events up to seq $seq."; then
    sha="$(git rev-parse --short HEAD)"
    append-event.sh ack agent="$ME" by="$ME" seq_done="$seq" outcome=committed \
      ref="$sha" "$staged" summary="$msg" >/dev/null
    echo "reactor: committed $sha  $msg"
  else
    append-event.sh violation agent="$ME" by="$ME" seq_done="$seq" \
      detail="seq $seq ($msg): git commit failed, left staged for review" >/dev/null
  fi
}

# --- main loop ---------------------------------------------------------------
resume="$(acked_through)"; resume="${resume:-0}"
echo "reactor($ME): acked through seq $resume; watching $LOG"

tail -n +1 -F "$LOG" 2>/dev/null | while IFS= read -r line; do
  seq="$(jq -r '.seq // empty' <<<"$line")"; [ -z "$seq" ] && continue
  jq -e '.type=="decision" and .key=="commit-message"' <<<"$line" >/dev/null 2>&1 || continue
  done_through="$(acked_through)"; done_through="${done_through:-0}"
  if [ "$seq" -le "$done_through" ]; then
    echo "reactor: seq $seq already acked (through $done_through), skipping"; continue
  fi
  ACT "$seq" "$line"
done
