#!/usr/bin/env bash
# append-event.sh — the ONLY sanctioned way to write the coordination log.
#
# Appends one JSON object (one line, JSONL) to an append-only event log. The
# controller/driver is the single writer: workers never call this. Each event
# is small — coordination facts only, never model output or file contents.
# Large artifacts are referenced by path: pass ref=<path>, not the bytes.
#
#   append-event.sh <type> [key=value ...]
#   append-event.sh spawn agent=reviewer model=opus tab=w1:t3
#   append-event.sh result agent=reviewer ref=.context/handoffs/review-auth.md verdict=CHANGES
#   append-event.sh --log path/to/events.jsonl decision key=auth-store value=sqlite
#
# Env: EVENTLOG_PATH overrides the default log path (.context/events.jsonl).
# Fields: seq (monotonic) and ts (UTC ISO-8601) are added automatically.
set -euo pipefail

LOG="${EVENTLOG_PATH:-.context/events.jsonl}"
MAX_VALUE_BYTES=2048    # one field this big is almost certainly a payload, not a fact
MAX_EVENT_BYTES=4096    # keep the whole event small so the log stays cheap to read

die() { echo "append-event: $*" >&2; exit 1; }

# --log <path> may lead the args
if [ "${1:-}" = "--log" ]; then
  [ $# -ge 2 ] || die "--log needs a path"
  LOG="$2"; shift 2
fi

[ $# -ge 1 ] || die "usage: append-event.sh [--log PATH] <type> [key=value ...]"
command -v jq >/dev/null || die "jq is required"

TYPE="$1"; shift
[[ "$TYPE" =~ ^[a-z][a-z0-9_-]*$ ]] || die "type must be a lowercase slug, got: $TYPE"

# Build the event object with jq, one key=value at a time. Reject fat values so
# nobody smuggles a transcript into the log — that is what ref=<path> is for.
filter=''
declare -a jqargs=()
i=0
for pair in "$@"; do
  case "$pair" in
    *=*) ;;
    *) die "field must be key=value, got: $pair" ;;
  esac
  k="${pair%%=*}"; v="${pair#*=}"
  [[ "$k" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]] || die "bad field name: $k"
  [ "$k" != seq ] && [ "$k" != ts ] || die "seq and ts are set automatically"
  if [ "${#v}" -gt "$MAX_VALUE_BYTES" ]; then
    die "field '$k' is ${#v} bytes (> $MAX_VALUE_BYTES). Write it to a file and pass ${k}=<path> or ref=<path> instead — the log holds facts, not payloads."
  fi
  jqargs+=(--arg "k$i" "$k" --arg "v$i" "$v")
  filter="$filter + {(\$k$i): \$v$i}"
  i=$((i+1))
done

mkdir -p "$(dirname "$LOG")"
[ -e "$LOG" ] || : > "$LOG"

# Portable lock (macOS has no flock): mkdir is atomic. Serializes seq+append so
# two controllers can't collide on a sequence number or interleave a line.
LOCK="$LOG.lock"
acquired=""
for _ in $(seq 1 100); do
  if mkdir "$LOCK" 2>/dev/null; then acquired=1; trap 'rmdir "$LOCK" 2>/dev/null || true' EXIT; break; fi
  sleep 0.05
done
[ -n "$acquired" ] || die "could not acquire lock on $LOG.lock (stale? remove it)"

last_seq="$(tail -n 1 "$LOG" 2>/dev/null | jq -r '.seq // 0' 2>/dev/null || echo 0)"
[[ "$last_seq" =~ ^[0-9]+$ ]] || last_seq=0
seq=$((last_seq + 1))
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

event="$(jq -cn \
  --argjson seq "$seq" --arg ts "$ts" --arg type "$TYPE" "${jqargs[@]}" \
  "{seq: \$seq, ts: \$ts, type: \$type} $filter")"

if [ "${#event}" -gt "$MAX_EVENT_BYTES" ]; then
  die "event is ${#event} bytes (> $MAX_EVENT_BYTES). Reference artifacts by path instead of inlining them."
fi

printf '%s\n' "$event" >> "$LOG"
echo "$event"
