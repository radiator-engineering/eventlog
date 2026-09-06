#!/usr/bin/env bash
# agent-since.sh — print an RFC 3339 cutoff for `agentmon --since` that captures
# EVERY agent session in the current herdr session: the start time of the
# earliest-started agent process hosted in any herdr pane, minus a small margin.
# Prints `launch` when no pane hosts an agent (agentmon's own default).
#
#   agentmon --since "$(agent-since.sh)"
#
# Env: AGENT_SINCE_MARGIN seconds subtracted from the earliest start (default 60).
set -uo pipefail
MARGIN="${AGENT_SINCE_MARGIN:-60}"
command -v herdr >/dev/null && command -v jq >/dev/null || { echo launch; exit 0; }

earliest=""
for pane in $(herdr pane list 2>/dev/null | jq -r '.result.panes[]? | select(.agent != null) | .pane_id'); do
  for pid in $(herdr pane process-info --pane "$pane" 2>/dev/null | jq -r '.result.process_info.foreground_processes[]?.pid'); do
    # ps prints e.g. "Fri Sep  5 13:01:59 2026"; normalise runs of spaces before parsing
    l="$(ps -o lstart= -p "$pid" 2>/dev/null | tr -s ' ' | sed 's/^ //')"; [ -n "$l" ] || continue
    if date -j >/dev/null 2>&1; then e="$(date -j -f '%a %b %d %H:%M:%S %Y' "$l" +%s 2>/dev/null)"   # macOS
    else e="$(date -d "$l" +%s 2>/dev/null)"; fi                                                    # GNU
    [ -n "$e" ] || continue
    [ -z "$earliest" ] || [ "$e" -lt "$earliest" ] && earliest="$e"
  done
done
[ -n "$earliest" ] || { echo launch; exit 0; }
cut=$((earliest - MARGIN))
if date -u -r "$cut" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null; then :; else date -u -d "@$cut" +%Y-%m-%dT%H:%M:%SZ; fi
