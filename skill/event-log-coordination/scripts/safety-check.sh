#!/usr/bin/env bash
# safety-check.sh — the doctor for append-only enforcement. It checks the three
# things that decide whether your log is actually protected, tells you what
# ISN'T, and heals the gaps. Protection is per-log and (for non-Claude agents)
# opt-in, so nothing is ever protected silently — the doctor makes the state
# visible and gets your explicit yes before protecting a log.
#
#   safety-check.sh [LOG]            # report posture only (no changes)
#   safety-check.sh --doctor [LOG]   # check AND fix each gap (interactive on a terminal)
#   safety-check.sh --doctor --yes   # fix every gap, no prompts
#   safety-check.sh --protect [LOG]  # just OS-protect this log now
#
# It checks: (1) is the tooling on PATH, (2) is the Claude Code guard registered
# in each settings.json (+ the new-session caveat), (3) is THIS log OS-protected
# — the only protection that holds under Cursor/Codex/other agents.
#
# Fix policy: on a terminal, every fix is a y/N prompt. When an agent runs it
# (no terminal), idempotent plumbing (linking commands, registering the guard)
# is applied automatically, but protecting a log — a real, reversible decision —
# is only REPORTED unless you pass --yes. --protect/--yes are your explicit opt-in.
set -uo pipefail

OK="[ OK ]"; WARN="[WARN]"; INFO="[ -- ]"; FIX="[ FIX]"
MODE=report; YES=0
while [ $# -gt 0 ]; do case "$1" in
  --doctor)  MODE=doctor; shift ;;
  --yes|-y)  YES=1; MODE=doctor; shift ;;
  --protect) MODE=protect; shift ;;
  -*)        echo "safety-check: unknown flag $1" >&2; exit 2 ;;
  *)         break ;;
esac; done
LOG="${1:-.context/events.jsonl}"

HERE="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
say() { printf '%s %s\n' "$1" "$2"; }
GAPS=()  # human-readable list of anything still unhealed at the end

# --protect: the one-shot opt-in, then done.
if [ "$MODE" = protect ]; then
  [ -e "$LOG" ] || { echo "safety-check: no log at $LOG (init-eventlog.sh creates one)"; exit 1; }
  "$HERE/protect-log.sh" "$LOG"; exit $?
fi

# fix_gap KIND "description" cmd...  — heal one gap per the fix policy above.
#   KIND=plumbing → auto-apply when an agent runs it; KIND=protect → opt-in only.
fix_gap() {
  local kind="$1" desc="$2"; shift 2
  if [ "$MODE" != doctor ]; then GAPS+=("$desc"); return; fi
  if [ "$YES" = 1 ]; then say "$FIX" "$desc"; "$@" && return; GAPS+=("$desc (fix failed)"); return; fi
  if [ -t 0 ]; then
    printf '      fix now? %s [y/N] ' "$desc"; read -r a
    case "$a" in [Yy]*) "$@" || GAPS+=("$desc (fix failed)") ;; *) GAPS+=("$desc") ;; esac
    return
  fi
  # agent-run (no terminal): auto-heal plumbing, leave protection as a spoken opt-in
  if [ "$kind" = plumbing ]; then say "$FIX" "$desc (auto)"; "$@" || GAPS+=("$desc (fix failed)")
  else GAPS+=("$desc"); fi
}

echo "== event-log safety posture${MODE:+ (${MODE})} =="

# 1) tooling on PATH
if command -v append-event.sh >/dev/null && command -v protect-log.sh >/dev/null; then
  say "$OK" "tooling installed (commands on PATH)"
else
  say "$WARN" "tooling not on PATH"
  fix_gap plumbing "link commands + register guard (setup.sh)" "$HERE/setup.sh"
fi

# 2) Claude Code guard registered? per settings.json found
guard_any=0
for s in "$HOME/.claude/settings.json" "$HOME"/.claudewho-*/settings.json; do
  [ -f "$s" ] || continue
  if jq -e '(.hooks.PreToolUse // [])|any(.hooks[]?.command|test("eventlog-guard"))' "$s" >/dev/null 2>&1; then
    say "$OK" "Claude guard registered: ${s/#$HOME/~}"; guard_any=1
  else
    say "$WARN" "Claude guard NOT in: ${s/#$HOME/~}"
    fix_gap plumbing "register guard in ${s/#$HOME/~}" "$HERE/install-guard.sh" --settings "$s"
  fi
done
[ "$guard_any" = 1 ] && say "$INFO" "  (guard activates per session at startup — restart / run /hooks to make it live)"

# 3) THIS log's OS-level protection — the cross-agent guarantee
echo "-- log: $LOG --"
if [ ! -e "$LOG" ]; then
  say "$INFO" "no log yet (init-eventlog.sh creates one)"
elif "$HERE/protect-log.sh" --status "$LOG" 2>/dev/null | grep -q '^protected:'; then
  say "$OK" "OS-protected (append-only under ANY agent — Claude, Cursor, Codex, stray process)"
else
  say "$WARN" "NOT OS-protected — under Cursor/Codex/etc this log can still be rewritten or deleted"
  fix_gap protect "protect $LOG (append-only, all agents)" "$HERE/protect-log.sh" "$LOG"
fi

# 4) verdict
echo "-- summary --"
say "$INFO" "Claude Code: guard blocks Claude's own rewrites (if registered + session restarted)."
say "$INFO" "Cursor/Codex/other agents: NO automatic protection — only protect-log.sh covers them."
if [ ${#GAPS[@]} -eq 0 ]; then
  say "$OK" "no gaps."
else
  say "$WARN" "gaps remain:"; for g in "${GAPS[@]}"; do echo "        - $g"; done
  if [ "$MODE" = doctor ]; then
    say "$INFO" "re-run with --yes to apply, or protect a log with: safety-check.sh --protect $LOG"
  else
    say "$INFO" "heal them: safety-check.sh --doctor   (add --yes to skip prompts)"
  fi
fi
