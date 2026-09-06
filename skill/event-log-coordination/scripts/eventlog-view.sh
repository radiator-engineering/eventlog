#!/usr/bin/env bash
# eventlog-view.sh — render the coordination log for humans: one colored,
# aligned line per event. Read-only; it never writes the log.
#
#   eventlog-view.sh                 # print the whole log, colored
#   eventlog-view.sh -f              # follow (tail -f), the pane-friendly mode
#   eventlog-view.sh -f --last 20    # follow, starting from the last 20 events
#   eventlog-view.sh --type result,decision --agent t2-settings
#   eventlog-view.sh --compact       # drop the timestamp column
#   eventlog-view.sh --no-color      # plain text (also when stdout is not a tty)
#   eventlog-view.sh --log PATH      # default: $EVENTLOG_PATH or .context/events.jsonl
#   eventlog-view.sh --wrap          # word-wrap long summaries under the summary
#                                    # column (default at a terminal)
#   eventlog-view.sh --truncate      # one line per event, cut with an ellipsis
#   eventlog-view.sh --width N       # override the terminal width for wrap/truncate
#
# Line shape:
#   seq  time  TYPE      agent         summary                         ref
# `summary` is the first present of: msg, verdict, key=value, subject, paths,
# disposition, task. Verdicts are colored on their own (ACCEPT/CLEAN green,
# CHANGES/REJECT/FAIL red). Unknown types render in the default color, so new
# event types never break the view. Wrapped continuation lines start under the
# summary column; the ref goes on its own line when it does not fit.
#
# Colors are SGR parameter strings (see `man console_codes`; "1;32" = bold
# green). Override per type with an env var or a config file, env wins:
#   EVENTLOG_COLOR_RESULT="1;36" eventlog-view.sh -f
#   .context/eventlog-view.conf   lines like  result=1;36   ts=2   agent=1
# Keys: any event type, plus  seq  ts  agent  ref  verdict_ok  verdict_bad.
set -euo pipefail

LOG="${EVENTLOG_PATH:-.context/events.jsonl}"
FOLLOW=0; LAST=""; TYPES=""; AGENT=""; COMPACT=0; COLOR="auto"; MODE="auto"; WIDTH=""

die() { echo "eventlog-view: $*" >&2; exit 2; }

while [ $# -gt 0 ]; do
  case "$1" in
    -f|--follow) FOLLOW=1; shift ;;
    --last)      [ $# -ge 2 ] || die "--last needs a number"; LAST="$2"; shift 2 ;;
    --type)      [ $# -ge 2 ] || die "--type needs a list";   TYPES="$2"; shift 2 ;;
    --agent)     [ $# -ge 2 ] || die "--agent needs a name";  AGENT="$2"; shift 2 ;;
    --log)       [ $# -ge 2 ] || die "--log needs a path";    LOG="$2"; shift 2 ;;
    --compact)   COMPACT=1; shift ;;
    --no-color)  COLOR=never; shift ;;
    --color)     COLOR=always; shift ;;
    --wrap)      MODE=wrap; shift ;;
    --truncate)  MODE=truncate; shift ;;
    --no-wrap)   MODE=none; shift ;;
    --width)     [ $# -ge 2 ] || die "--width needs a number"; WIDTH="$2"; shift 2 ;;
    -h|--help)   sed -n '2,32p' "$0"; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

command -v jq >/dev/null || die "jq is required"
[ -f "$LOG" ] || die "no log at $LOG"

# ---- palette: defaults < config file < env -----------------------------------
declare -A C=(
  [seq]=2 [ts]=2 [agent]=1 [ref]=2
  [verdict_ok]="1;32" [verdict_bad]="1;31"
  [spawn]="1;32" [prompt]=36 [message]=36 [drain]=2
  [result]="1;36" [decision]="1;33" [escalate]="1;35" [approval]=35
  [retire]=2 [claim]=34 [progress]=2 [seam]=33 [violation]="1;31"
  [milestone]="1;37" [note]=2
)
SGR_RE='^[0-9;]*$'
CONF="$(dirname "$LOG")/eventlog-view.conf"
if [ -f "$CONF" ]; then
  while IFS='=' read -r k v; do
    k="${k## }"; k="${k%% }"
    [[ "$k" =~ ^[a-z][a-z0-9_]*$ ]] || continue
    [[ "$v" =~ $SGR_RE ]] || continue
    C[$k]="$v"
  done < <(grep -v '^[[:space:]]*#' "$CONF" || true)
fi
for k in "${!C[@]}"; do
  ev="EVENTLOG_COLOR_$(printf '%s' "$k" | tr '[:lower:]' '[:upper:]')"
  [ -n "${!ev:-}" ] && C[$k]="${!ev}"
done
# env can also introduce a type the defaults don't know
while IFS='=' read -r ev v; do
  k="$(printf '%s' "${ev#EVENTLOG_COLOR_}" | tr '[:upper:]' '[:lower:]')"
  [[ "$v" =~ $SGR_RE ]] && C[$k]="$v"
done < <(env | grep '^EVENTLOG_COLOR_' || true)

use_color=0
case "$COLOR" in
  always) use_color=1 ;;
  never)  use_color=0 ;;
  auto)   [ -t 1 ] && use_color=1 || use_color=0 ;;
esac

# wrap/truncate: default to wrap at a terminal, none when piped
if [ "$MODE" = auto ]; then
  [ -t 1 ] && MODE=wrap || MODE=none
fi
if [ -z "$WIDTH" ]; then
  WIDTH="${COLUMNS:-}"
  [ -n "$WIDTH" ] || WIDTH="$(tput cols 2>/dev/null || true)"
  [[ "$WIDTH" =~ ^[0-9]+$ ]] || WIDTH=120
fi
[[ "$WIDTH" =~ ^[0-9]+$ ]] || die "--width must be a number"
mode_n=0; case "$MODE" in wrap) mode_n=1 ;; truncate) mode_n=2 ;; esac

# palette as a JSON object for jq
palette="$(for k in "${!C[@]}"; do printf '%s\t%s\n' "$k" "${C[$k]}"; done \
  | jq -Rn '[inputs | split("\t") | {(.[0]): .[1]}] | add')"

# ---- source -------------------------------------------------------------------
if [ "$FOLLOW" -eq 1 ]; then
  src=(tail -n "${LAST:-+1}" -f "$LOG")
elif [ -n "$LAST" ]; then
  src=(tail -n "$LAST" "$LOG")
else
  src=(cat "$LOG")
fi

# ---- render -------------------------------------------------------------------
"${src[@]}" | jq -R -r --argjson P "$palette" --argjson color "$use_color" \
  --argjson compact "$COMPACT" --arg types "$TYPES" --arg agent "$AGENT" \
  --argjson mode "$mode_n" --argjson width "$WIDTH" '
  def sgr(k): if $color == 1 then "[" + ($P[k] // "0") + "m" else "" end;
  def off: if $color == 1 then "[0m" else "" end;
  def pad(n): tostring | . + (" " * ((n - length) | if . < 0 then 0 else . end));
  def paint(k; s): sgr(k) + s + off;
  # visible length: strip SGR escapes
  def vlen: gsub("\\[[0-9;]*m"; "") | length;
  # greedy word wrap into lines no wider than w; a lone long word stays whole
  def wrap(w):
    if w < 8 then [.] else
    reduce (split(" ") | map(select(. != ""))[]) as $wd ([];
      if length == 0 then [$wd]
      elif ((.[-1] | vlen) + 1 + ($wd | vlen)) <= w then .[:-1] + [.[-1] + " " + $wd]
      else . + [$wd] end)
    end;

  # raw lines in; a partial or malformed line is skipped, never fatal
  (try fromjson catch empty) | select(type == "object") |

  ( if $types == "" then . else select(.type as $t | ($types | split(",")) | index($t)) end ) |
  ( if $agent == "" then . else select(.agent == $agent or .from == $agent or .to == $agent
                                        or ((.agents // "") | split(",") | index($agent))) end ) |

  (.ts // "" | if length >= 19 then .[11:19] else . end) as $time |
  (.type // "?") as $t |
  (.agent // .from // "") as $ag |
  (.verdict // "") as $v |
  ( if .msg then .msg
    elif .key then "\(.key) = \(.value // "")"
    elif .subject then .subject
    elif .paths then "paths: \(.paths)"
    elif .disposition then .disposition
    elif .task then "task \(.task)"
    elif .from then "→ \(.to // "?")"
    else "" end ) as $summary |
  ( [ (if .commit then "@\(.commit)" else empty end),
      (if .tests then .tests else empty end),
      (if .note then "note: \(.note)" else empty end),
      (if .pane then .pane elif .tab then .tab else empty end) ] | join("  ") ) as $extra |
  ( if $v == "" then ""
    elif ($v | test("^(ACCEPT|CLEAN|OK|PASS)"; "i")) then paint("verdict_ok"; $v)
    elif ($v | test("^(CHANGES|REJECT|FAIL|BLOCK)"; "i")) then paint("verdict_bad"; $v)
    else $v end ) as $vc |

  ( [ paint("seq"; (.seq // "" | pad(4))),
      (if $compact == 1 then empty else paint("ts"; $time) end),
      paint($t; ($t | ascii_upcase | pad(10))),
      paint("agent"; ($ag | pad(14)))
    ] | join("  ") + "  " ) as $prefix |
  ($prefix | vlen) as $indent |
  ([$vc, $summary, $extra] | map(select(. != "")) | join("  ")) as $body |
  (if .ref then paint("ref"; "→ \(.ref)") else "" end) as $refc |
  ($width - $indent) as $avail |

  if $mode == 0 then
    $prefix + ([$body, $refc] | map(select(. != "")) | join("  "))
  elif $mode == 2 then
    ([$body, $refc] | map(select(. != "")) | join("  ")) as $line |
    if ($line | vlen) <= $avail then $prefix + $line
    else
      # keep the colored ref only if the whole thing fits; otherwise cut the plain body
      ($body | gsub("\\[[0-9;]*m"; "")) as $plain |
      $prefix + ($plain[:($avail - 1 | if . < 1 then 1 else . end)]) + "…"
    end
  else
    ($body | wrap($avail)) as $lines |
    ( if $refc == "" then $lines
      elif ($lines | length) > 0 and (($lines[-1] | vlen) + 2 + ($refc | vlen)) <= $avail
        then $lines[:-1] + [$lines[-1] + "  " + $refc]
      else $lines + [$refc] end ) as $all |
    ( if ($all | length) == 0 then [""] else $all end ) as $all |
    $prefix + $all[0]
      + ( $all[1:] | map("\n" + (" " * $indent) + .) | join("") )
  end
'
