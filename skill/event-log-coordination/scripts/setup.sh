#!/usr/bin/env bash
# setup.sh — one-shot, idempotent machine setup for event-log-coordination.
# The AGENT runs this; the user never runs a script by hand. Safe to run every
# time the skill is used — it does nothing if already set up.
#
# It (1) symlinks the event-log commands onto PATH so they're callable by bare
# name, and (2) registers the append-only PreToolUse guard in settings.json.
#
#   setup.sh            # link + install guard + report
#   setup.sh --check    # report only: are commands on PATH and guard registered?
set -euo pipefail

# resolve through symlinks so siblings are found even when invoked via a link
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do
  DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"; SOURCE="$(readlink "$SOURCE")"
  [[ "$SOURCE" != /* ]] && SOURCE="$DIR/$SOURCE"
done
HERE="$(cd -P "$(dirname "$SOURCE")" && pwd)"

if [ "${1:-}" = "--check" ]; then
  echo "commands on PATH:"; for s in init-eventlog.sh append-event.sh; do
    command -v "$s" >/dev/null && echo "  ok  $s" || echo "  --  $s (not linked)"; done
  "$HERE/install-guard.sh" --check || true
  exit 0
fi

echo "== linking commands onto PATH =="
"$HERE/link-scripts.sh"
echo "== registering append-only guard =="
"$HERE/install-guard.sh"
echo "== setup complete =="
echo "Note: the guard loads in NEW sessions — restart the session (or /hooks) once."
