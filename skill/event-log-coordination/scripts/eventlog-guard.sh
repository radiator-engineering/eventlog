#!/usr/bin/env bash
# eventlog-guard.sh — PreToolUse hook that enforces append-only on the
# coordination log. This is the mechanical half of the guarantee: without it,
# "append-only" is just a convention the model chooses to follow.
#
# Wire it in settings.json under hooks.PreToolUse with matcher "Edit|Write|Bash"
# (scripts/install-guard.sh does this for you). It reads the hook payload on
# stdin and blocks (exit 2) any operation that would rewrite or truncate a
# protected log. The only sanctioned writer is append-event.sh (an atomic >>
# append); everything else that mutates the file is denied. Reads
# (cat/tail/grep/jq/tail -f, and read-mode interpreter opens) are always allowed.
#
# Protected file: basename == events.jsonl by default. Override the basename
# (ERE, dot already escaped for you) with EVENTLOG_GUARD_BASENAME, e.g.
#   EVENTLOG_GUARD_BASENAME='coord\.jsonl'
# For full control of the whole-path match, set EVENTLOG_GUARD_GLOB (a POSIX ERE
# matched against a Write/Edit file_path).
#
# Guarantee: airtight for Edit and Write (they can only overwrite/patch, so any
# hit is denied). Best-effort denylist for Bash — it catches the common
# truncating, in-place-edit, and interpreter-write shapes, but a determined
# rewrite through an unusual tool can still get through. For a hard, tool-proof
# guarantee also run `chflags uappnd <log>` (macOS) or `chattr +a <log>` (Linux);
# see SKILL.md "Enforcement".
#
# Fail-open: on any parse error the hook exits 0 so it can never brick your
# tools. It only ever *adds* a deny for a clearly-mutating op on the log.
set -uo pipefail

BASE="${EVENTLOG_GUARD_BASENAME:-events\.jsonl}"
# Whole-path match, for the Write/Edit file_path field (a single path argument).
PATH_RE="${EVENTLOG_GUARD_GLOB:-(^|/)${BASE}$}"
# Token match, for scanning a Bash command string where the log may appear
# mid-line. Bounded so 'events.jsonl.bak' and 'myevents.jsonl' don't match.
SCAN_RE="(^|[^[:alnum:]_.-])${BASE}([^[:alnum:]_.-]|$)"

payload="$(cat)"
command -v jq >/dev/null 2>&1 || exit 0

tool="$(printf '%s' "$payload" | jq -r '.tool_name // empty' 2>/dev/null || true)"
[ -n "$tool" ] || exit 0

deny() {
  # PreToolUse: exit 2 blocks the call; stderr is shown to the model.
  echo "eventlog-guard: BLOCKED. $1" >&2
  echo "The coordination log is append-only. The only sanctioned writer is append-event.sh, e.g.:" >&2
  echo "  append-event.sh <type> key=value ...   (adds one JSONL line, atomically)" >&2
  echo "To read it, use cat/tail/grep/jq. To record a large artifact, write the artifact to its own file and append an event with ref=<path>." >&2
  exit 2
}

hits_path() { printf '%s' "$1" | grep -Eq "$PATH_RE"; }
hits_scan() { printf '%s' "$1" | grep -Eq "$SCAN_RE"; }

case "$tool" in
  Write)
    f="$(printf '%s' "$payload" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"
    [ -n "$f" ] && hits_path "$f" && deny "Write would overwrite the whole log ($f)."
    ;;
  Edit|NotebookEdit)
    f="$(printf '%s' "$payload" | jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' 2>/dev/null || true)"
    [ -n "$f" ] && hits_path "$f" && deny "Edit would rewrite existing lines of the log ($f)."
    ;;
  Bash)
    cmd="$(printf '%s' "$payload" | jq -r '.tool_input.command // empty' 2>/dev/null || true)"
    [ -n "$cmd" ] || exit 0
    # Only inspect commands that mention a protected log at all.
    hits_scan "$cmd" || exit 0
    # append-event.sh is the sanctioned path — let it through.
    printf '%s' "$cmd" | grep -q 'append-event\.sh' && exit 0

    # Denylist of mutating shapes against the log.
    # truncating redirect  '>' or '>|' immediately before the log path (not '>>')
    if printf '%s' "$cmd" | grep -Eq '(^|[^>])>\|?[[:space:]]*[^>[:space:]]*'"$BASE"; then
      deny "Truncating redirect ('>' / '>|') into the log detected."
    fi
    # in-place edit / delete / move / shred of the log
    if printf '%s' "$cmd" | grep -Eq '(sed[[:space:]]+-i|perl[[:space:]]+-[a-zA-Z]*i|truncate|shred|(^|[[:space:]])rm[[:space:]]|(^|[[:space:]])mv[[:space:]]|dd[[:space:]])'; then
      deny "In-place edit / delete / move of the log detected."
    fi
    # non-append tee into the log
    if printf '%s' "$cmd" | grep -Eq 'tee[[:space:]]' && ! printf '%s' "$cmd" | grep -Eq 'tee[[:space:]]+(-a|--append)'; then
      deny "Non-append 'tee' into the log detected (use 'tee -a' or append-event.sh)."
    fi
    # inline interpreter opening the log for writing: open(...,'w'/'w+'/'wb'/'a+ as truncate?)
    # read-mode ('r') and append-mode ('a') opens are allowed. hits_scan above
    # already proved the command references the log.
    if printf '%s' "$cmd" | grep -Eq '(python[0-9.]*|perl|ruby|node|deno|bun)[[:space:]]' \
       && printf '%s' "$cmd" | grep -Eq "open\([^)]*,[[:space:]]*['\"]w[b+]*['\"]"; then
      deny "Inline interpreter opening the log for writing detected."
    fi
    ;;
esac
exit 0
