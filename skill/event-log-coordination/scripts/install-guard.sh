#!/usr/bin/env bash
# install-guard.sh — wire (or check) the append-only PreToolUse guard in
# Claude Code settings. Idempotent: adds the guard once, never touches your
# other hooks, backs the file up first.
#
#   install-guard.sh            # install into ~/.claude/settings.json
#   install-guard.sh --check    # report whether it's registered (exit 0/1)
#   install-guard.sh --settings /path/to/settings.json   # target a different file
#
# The guard only takes effect in NEW sessions (Claude Code reads hooks at
# session start). After installing, restart the session or run /hooks.
set -euo pipefail

# Resolve through symlinks so the sibling guard is found even when this script
# is invoked via a symlink on PATH (see link-scripts.sh).
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do
  DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"
  SOURCE="$(readlink "$SOURCE")"
  [[ "$SOURCE" != /* ]] && SOURCE="$DIR/$SOURCE"
done
HERE="$(cd -P "$(dirname "$SOURCE")" && pwd)"
GUARD="$HERE/eventlog-guard.sh"
SETTINGS="${HOME}/.claude/settings.json"
CHECK=0

while [ $# -gt 0 ]; do
  case "$1" in
    --check) CHECK=1; shift ;;
    --settings) SETTINGS="$2"; shift 2 ;;
    *) echo "install-guard: unknown arg: $1" >&2; exit 2 ;;
  esac
done

command -v jq >/dev/null || { echo "install-guard: jq is required" >&2; exit 2; }
[ -x "$GUARD" ] || { echo "install-guard: guard not executable at $GUARD" >&2; exit 2; }

registered() {
  [ -f "$SETTINGS" ] || return 1
  jq -e --arg cmd "$GUARD" '
    (.hooks.PreToolUse // []) | any(.hooks[]?.command == $cmd)
  ' "$SETTINGS" >/dev/null 2>&1
}

if [ "$CHECK" = 1 ]; then
  if registered; then echo "guard: registered in $SETTINGS"; exit 0
  else echo "guard: NOT registered in $SETTINGS"; exit 1; fi
fi

[ -f "$SETTINGS" ] || { mkdir -p "$(dirname "$SETTINGS")"; echo '{}' > "$SETTINGS"; }

if registered; then
  echo "guard: already registered in $SETTINGS — nothing to do"
  exit 0
fi

cp "$SETTINGS" "$SETTINGS.bak.$(date +%s)"
tmp="$(mktemp)"
jq --arg cmd "$GUARD" '
  .hooks = (.hooks // {})
  | .hooks.PreToolUse = ((.hooks.PreToolUse // []) + [
      { "matcher": "Edit|Write|Bash",
        "hooks": [ { "type": "command", "command": $cmd } ] } ])
' "$SETTINGS" > "$tmp"
jq empty "$tmp"          # refuse to install invalid JSON
mv "$tmp" "$SETTINGS"

echo "guard: installed into $SETTINGS (backup alongside as *.bak.*)"
echo "self-test:"
printf '%s' '{"tool_name":"Write","tool_input":{"file_path":".context/events.jsonl"}}' \
  | "$GUARD" >/dev/null 2>&1 && echo "  UNEXPECTED: guard did not block a Write to the log" \
  || echo "  OK: guard blocks a Write to the log (exit 2)"
echo "restart the session (or run /hooks) so Claude Code reloads hooks."
