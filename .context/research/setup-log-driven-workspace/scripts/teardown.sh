#!/usr/bin/env bash
# teardown.sh — stop both reactors cleanly, record their retirement in the log,
# and (with --close) close what layout.sh created: the maintenance and files
# workspaces, the monitor tab and the eventlog pane. The controller pane stays.
# Files in .context/ are kept; the log is never touched except by append.
#
#   teardown.sh [--close] [--dry-run]
#
# Ctrl+C in a pane does NOT stop a reactor mid-pass (the foreground `timeout
# claude -p` / cursor-agent swallows it), so this kills the supervisor, the
# reactor, and their children by pid. A killed reactor leaves its lock dir
# behind; the next start reclaims it (the pid inside is dead). Do not rm it by
# hand — the append-only guard blocks any command naming the log path.
set -uo pipefail
CLOSE=0; DRY=0
while [ $# -gt 0 ]; do case "$1" in
  --close) CLOSE=1; shift ;; --dry-run) DRY=1; shift ;;
  -h|--help) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  *) echo "teardown: unknown arg $1" >&2; exit 2 ;;
esac; done
say() { printf 'teardown %s\n' "$*"; }
run() { if [ "$DRY" = 1 ]; then { printf '+'; printf ' %q' "$@"; echo; } >&2; else "$@"; fi; }

REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "teardown: run inside the repo" >&2; exit 1; }
cd "$REPO"
BIN="$REPO/.context/bin"

# kill a process tree rooted at each pid matching PATTERN (children first)
kill_tree() {
  local pid
  for pid in $(pgrep -f "$1" || true); do
    for c in $(pgrep -P "$pid" || true); do run kill -TERM "$c" 2>/dev/null || true; done
    run kill -TERM "$pid" 2>/dev/null || true
  done
}
for name in cursor-commit-reactor doc-sync-reactor; do
  if pgrep -f "$BIN/run-reactor.sh.*$name|$BIN/$name.sh" >/dev/null; then
    kill_tree "$BIN/run-reactor.sh"          # supervisors first so they do not respawn
    kill_tree "$BIN/$name.sh"
    say "stopped $name"
  else say "$name was not running"; fi
done
sleep 1
for name in cursor-commit-reactor doc-sync-reactor; do
  pgrep -f "$BIN/$name.sh" >/dev/null && { run pkill -KILL -f "$BIN/$name.sh"; say "force-killed $name"; }
done
pgrep -f "$BIN/run-reactor.sh" >/dev/null && run pkill -KILL -f "$BIN/run-reactor.sh"

# retire the workers in the log (close the lifecycle layout.sh opened)
if command -v append-event.sh >/dev/null && [ -e .context/events.jsonl ]; then
  for a in cursor-committer doc-worker; do
    if jq -e --arg a "$a" 'select(.type=="spawn" and .agent==$a)' .context/events.jsonl >/dev/null 2>&1 \
       && ! jq -e --arg a "$a" 'select(.type=="retire" and .agent==$a)' .context/events.jsonl >/dev/null 2>&1; then
      run append-event.sh retire agent="$a" disposition=stopped detail="teardown.sh" >/dev/null; say "retired $a"
    fi
  done
fi

if [ "$CLOSE" = 1 ] && [ -f .context/layout.json ]; then
  # close what layout.sh created; the coordinator pane (the controller Claude) is never touched
  for w in $(jq -r '.maintenance.workspace // empty, .files.workspace // empty, .monitor.workspace // empty' .context/layout.json); do
    [ -n "$w" ] && [ "$w" != "<none>" ] && run herdr workspace close "$w" >/dev/null 2>&1 && say "closed workspace $w"
  done
  for t in $(jq -r '.controller.monitor_tab // empty' .context/layout.json); do
    [ -n "$t" ] && [ "$t" != "<none>" ] && run herdr tab close "$t" >/dev/null 2>&1 && say "closed tab $t"
  done
  p="$(jq -r '.controller.eventlog_pane // empty' .context/layout.json)"
  [ -n "$p" ] && run herdr pane close "$p" >/dev/null 2>&1 && say "closed eventlog pane $p"
  run rm -f .context/layout.json
fi
say "done"
