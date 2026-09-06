#!/usr/bin/env bash
# status.sh — one-screen health check for a log-driven workspace. Read-only.
# Exit 1 if anything that stops work from flowing is wrong.
set -uo pipefail
REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "status: run inside the repo" >&2; exit 1; }
cd "$REPO"
LOG=.context/events.jsonl; bad=0
ok()   { printf '[ OK ] %s\n' "$*"; }
warn() { printf '[WARN] %s\n' "$*"; }
err()  { printf '[FAIL] %s\n' "$*"; bad=1; }

[ -e "$LOG" ] && ok "log present ($(wc -l <"$LOG" | tr -d ' ') events)" || err "no $LOG — run setup.sh"
command -v append-event.sh >/dev/null && ok "append-event.sh on PATH" || err "append-event.sh not on PATH — run setup.sh"
[ "$(git config core.hooksPath)" = ".githooks" ] && ok "commit-msg hook active (core.hooksPath)" || err "core.hooksPath not .githooks — AI trailers will land; run setup.sh"
P="${EVENTLOG_SKILL:-$HOME/.claude/skills/event-log-coordination}/scripts/protect-log.sh"
[ -x "$P" ] && { "$P" --status 2>/dev/null | grep -q '^protected' && ok "log OS-protected" || warn "log not OS-protected (setup.sh --protect to opt in)"; }

for r in cursor-committer:cursor-commit-reactor doc-worker:doc-sync-reactor; do
  by="${r%%:*}"; name="${r##*:}"
  lock="$LOG.$by.reactor.lock"; pid="$(cat "$lock/pid" 2>/dev/null || true)"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    last="$(jq -r --arg me "$by" 'select(.type=="ack" and .by==$me) | "\(.seq_done) \(.outcome)"' "$LOG" 2>/dev/null | tail -1)"
    ok "$name running (pid $pid), last ack: ${last:-none}"
  else
    arg=""; [ "$name" = doc-sync-reactor ] && arg=" doc-sync-reactor.sh"
    err "$name NOT running — in its pane: bash .context/bin/run-reactor.sh$arg"
  fi
done

n="$(jq -c 'select(.by != null and (.by|IN("controller","cursor-committer","doc-worker")|not))' "$LOG" 2>/dev/null | wc -l | tr -d ' ')"
[ "$n" = 0 ] && ok "no unsanctioned log writers" || err "$n line(s) from unsanctioned writers: jq -c 'select(.by != null and (.by|IN(\"controller\",\"cursor-committer\",\"doc-worker\")|not))' $LOG"
e="$(jq -c 'select(.type=="escalate")' "$LOG" 2>/dev/null | tail -1)"; [ -z "$e" ] && ok "no escalations" || warn "last escalation: $e"
v="$(jq -c 'select(.type=="violation")' "$LOG" 2>/dev/null | wc -l | tr -d ' ')"; [ "$v" = 0 ] && ok "no scope violations" || warn "$v scope violation(s) recorded"
[ -e .git/index.lock ] && ! pgrep -x git >/dev/null && err "stale .git/index.lock — commits will fail until removed by hand"
d="$(git status --porcelain | grep -vE '^.. \.context/' | wc -l | tr -d ' ')"
[ "$d" = 0 ] && ok "tree clean outside .context/" || warn "$d path(s) dirty outside .context/ — the next result event will hand them to the committer"

if [ -f .context/layout.json ]; then
  for p in $(jq -r '.committer.pane, .doc_worker.pane, .controller.eventlog_pane // empty' .context/layout.json); do
    herdr pane get "$p" >/dev/null 2>&1 && ok "herdr pane $p alive" || warn "herdr pane $p gone (layout.json is stale)"
  done
else warn "no .context/layout.json — layout.sh has not run"; fi

# monitor: agentmon only shows what agentsview has ingested; a failing sync pass hides new agents
if command -v agentmon >/dev/null; then
  AV_URL="${AGENTSVIEW_URL:-http://127.0.0.1:8080}"
  if s="$(curl -s --max-time 3 "$AV_URL/api/v1/sync/status" 2>/dev/null)" && [ -n "$s" ]; then
    recent_err="$(grep 'sync error' "$HOME/.agentsview/debug.log" 2>/dev/null | tail -1)"
    ts="$(sed -E 's/^([0-9\/]+ [0-9:]+).*/\1/' <<<"$recent_err")"
    if [ -n "$recent_err" ]; then
      e_epoch="$(date -j -f '%Y/%m/%d %H:%M:%S' "$ts" +%s 2>/dev/null || date -d "$(tr / - <<<"$ts")" +%s 2>/dev/null || echo 0)"
      if [ $(( $(date +%s) - e_epoch )) -lt 300 ]; then
        warn "agentsview sync is failing (agentmon will miss new agent sessions): ${recent_err#*sync error: }"
      else ok "agentsview sync healthy (last sync $(jq -r .last_sync <<<"$s"))"; fi
    else ok "agentsview sync healthy (last sync $(jq -r .last_sync <<<"$s"))"; fi
  else warn "agentsview not reachable at $AV_URL — agentmon shows system only"; fi
fi
exit $bad
