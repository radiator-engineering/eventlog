#!/usr/bin/env bash
# check-claims.sh — did a worker stay inside the files it claimed?
#
# Reads the `claim` events for one agent from the log, lists the files that
# agent's branch actually changed, and prints every changed file that no claim
# covers. Read-only: it never appends. The controller decides what to record
# (usually `append-event.sh violation agent=... paths=...`, or nothing).
#
#   check-claims.sh <agent> <base-ref> [head-ref]        # head defaults to HEAD
#   check-claims.sh --log PATH --repo DIR <agent> <base> [head]
#
# Claims are `claim` events with `agent=<name>` and `paths=<glob>[,<glob>...]`.
# A changed file is covered when it equals a glob, matches it as a shell glob,
# or sits under it as a directory (`Sources/Editor` covers `Sources/Editor/X.swift`).
# Globs are matched against repo-relative paths, so claim repo-relative globs.
#
# Exit 0 when every changed file is claimed, 1 when at least one is not,
# 2 on usage/setup errors. No claims for the agent is reported as a gap (exit 1):
# a worker with no claims has no boundary the log can vouch for.
set -euo pipefail

LOG="${EVENTLOG_PATH:-.context/events.jsonl}"
REPO="."

die() { echo "check-claims: $*" >&2; exit 2; }

while [ $# -gt 0 ]; do
  case "$1" in
    --log)  [ $# -ge 2 ] || die "--log needs a path"; LOG="$2"; shift 2 ;;
    --repo) [ $# -ge 2 ] || die "--repo needs a dir";  REPO="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    --*) die "unknown flag: $1" ;;
    *) break ;;
  esac
done

[ $# -ge 2 ] || die "usage: check-claims.sh [--log PATH] [--repo DIR] <agent> <base-ref> [head-ref]"
AGENT="$1"; BASE="$2"; HEAD="${3:-HEAD}"
command -v jq >/dev/null || die "jq is required"
[ -f "$LOG" ] || die "no log at $LOG"
git -C "$REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "$REPO is not a git repo"

# Every claimed glob for this agent, one per line. Later claims add to earlier ones.
claims="$(jq -r --arg a "$AGENT" \
  'select(.type=="claim" and .agent==$a) | (.paths // "") | split(",")[] | gsub("^\\s+|\\s+$";"") | select(length>0)' \
  "$LOG")"

changed="$(git -C "$REPO" diff --name-only "$BASE" "$HEAD")"

if [ -z "$claims" ]; then
  echo "check-claims: $AGENT has no claim events in $LOG" >&2
  [ -z "$changed" ] && exit 0
  while IFS= read -r f; do [ -n "$f" ] && echo "unclaimed $f"; done <<< "$changed"
  exit 1
fi

covered() {
  local f="$1" g
  while IFS= read -r g; do
    [ -z "$g" ] && continue
    # shellcheck disable=SC2254  # the glob is the point
    case "$f" in
      "$g"|$g|"$g"/*) return 0 ;;
    esac
  done <<< "$claims"
  return 1
}

status=0
while IFS= read -r f; do
  [ -z "$f" ] && continue
  if covered "$f"; then
    echo "ok        $f"
  else
    echo "unclaimed $f"
    status=1
  fi
done <<< "$changed"

if [ $status -eq 0 ]; then
  echo "check-claims: $AGENT stayed inside its claims ($BASE..$HEAD)"
else
  echo "check-claims: $AGENT touched files outside its claims ($BASE..$HEAD); consider: append-event.sh violation agent=$AGENT paths=<list>" >&2
fi
exit $status
