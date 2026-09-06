#!/usr/bin/env bash
# protect-log.sh — make the event log append-only at the OS level, so no tool
# and no agent can rewrite or delete it. This is the AGENT-INDEPENDENT
# enforcement path: unlike the PreToolUse guard (which only fires inside Claude
# Code), this holds under Cursor, Codex, aider, plain shells — anything.
#
# Appends (>>) still succeed, so append-event.sh keeps working; truncate,
# overwrite, in-place edit and delete are refused by the kernel.
#
#   protect-log.sh [PATH]              # protect (default .context/events.jsonl)
#   protect-log.sh --unprotect [PATH]  # lift protection (needed before rm/checkout)
#   protect-log.sh --status   [PATH]   # report whether it's protected
#
# macOS: uses `chflags uappnd`. Linux: uses `chattr +a` (needs root / sudo on
# most setups, and an ext4/xfs-style fs). Other platforms: unsupported — the
# script says so and exits non-zero rather than pretending.
set -euo pipefail

MODE=protect
case "${1:-}" in
  --unprotect) MODE=unprotect; shift ;;
  --status)    MODE=status;    shift ;;
  -*)          echo "protect-log: unknown flag $1" >&2; exit 2 ;;
esac
LOG="${1:-.context/events.jsonl}"
[ -e "$LOG" ] || { echo "protect-log: no such file: $LOG" >&2; exit 1; }

os="$(uname -s)"
case "$os" in
  Darwin)
    case "$MODE" in
      protect)   chflags uappnd "$LOG"; echo "protected (macOS uappnd): $LOG — appends still work; truncate/overwrite/rm refused" ;;
      unprotect) chflags nouappnd "$LOG"; echo "unprotected: $LOG" ;;
      status)    ls -lO "$LOG" | grep -q uappnd && echo "protected: $LOG" || echo "NOT protected: $LOG" ;;
    esac ;;
  Linux)
    SUDO=""; [ "$(id -u)" -ne 0 ] && command -v sudo >/dev/null && SUDO=sudo
    case "$MODE" in
      protect)   $SUDO chattr +a "$LOG" && echo "protected (Linux +a): $LOG — appends still work; truncate/overwrite/rm refused" ;;
      unprotect) $SUDO chattr -a "$LOG" && echo "unprotected: $LOG" ;;
      status)    lsattr "$LOG" 2>/dev/null | grep -q -- '-a-' && echo "protected: $LOG" || echo "NOT protected: $LOG" ;;
    esac ;;
  *)
    echo "protect-log: OS-level immutability unsupported on '$os'." >&2
    echo "Fall back to the append helper + convention, or run this on macOS/Linux." >&2
    exit 3 ;;
esac
