#!/usr/bin/env bash
# link-scripts.sh — symlink the user-facing scripts onto PATH so you can call
# them by bare name (append-event.sh, init-eventlog.sh, install-guard.sh) from
# any repo, instead of an absolute path. Idempotent; run once.
#
#   link-scripts.sh                 # into ~/.local/bin
#   link-scripts.sh --bindir DIR    # into DIR
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINDIR="${HOME}/.local/bin"
[ "${1:-}" = "--bindir" ] && { BINDIR="$2"; shift 2; }

mkdir -p "$BINDIR"
for s in init-eventlog.sh append-event.sh install-guard.sh protect-log.sh safety-check.sh check-claims.sh eventlog-view.sh; do
  ln -sf "$HERE/$s" "$BINDIR/$s"
  echo "linked $BINDIR/$s -> $HERE/$s"
done

case ":$PATH:" in
  *":$BINDIR:"*) echo "ok: $BINDIR is on PATH — call the scripts by bare name now." ;;
  *) echo "note: $BINDIR is NOT on PATH. Add it to your shell profile:  export PATH=\"$BINDIR:\$PATH\"" ;;
esac
