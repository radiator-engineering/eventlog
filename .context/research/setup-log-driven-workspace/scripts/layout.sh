#!/usr/bin/env bash
# layout.sh — shape the herdr session around the Claude that runs this script,
# start both reactors, and record the worker lifecycle in the log. Run it from
# INSIDE the pane of the Claude Code session that will be the controller, after
# setup.sh. That pane is not renamed or moved: it becomes the top of
# control › coordinator.
#
# Target shape (matches the reference session):
#   workspace `control`        tab `coordinator`             top: this Claude (controller)
#                                                            bottom: pane `eventlog`  eventlog-view.sh -f
#                              tab `monitor: system + agents` pane `agentmon`  agentmon --since <cutoff>
#   workspace `maintenance`    tab `lazygit`                           pane `gitlog`          lazygit
#                              tab `<committer model>: commit reactor`  pane `commit-reactor`  run-reactor.sh
#                              tab `<doc model>: doc sync`             pane `doc-sync`        run-reactor.sh doc-sync-reactor.sh
#   workspace `files`          one tab, one pane                       spiceedit (if installed)
# There is no separate "event-log controller" tab: the coordinator Claude is the controller.
#
# IDEMPOTENT: it discovers before it creates — workspaces and tabs are matched by
# label + a pane whose cwd is this repo; a reactor whose lock pid is alive is
# left alone (a dead one restarts in its pane); spawn/prompt events are appended
# only when the agent has no open (un-retired) spawn. A second run prints
# "reuse" on every line and changes nothing.
#
#   layout.sh [--no-monitor] [--no-files] [--dry-run]
set -uo pipefail
HERE="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LAYOUTS="${HERDR_LAYOUTS_SKILL:-$HOME/.claude/skills/herdr-layouts}/scripts"
MONITOR=1; FILES=1; DRY=0
while [ $# -gt 0 ]; do case "$1" in
  --no-monitor) MONITOR=0; shift ;;
  --no-files) FILES=0; shift ;;
  --dry-run) DRY=1; shift ;;
  --force|--controller-model) [ "$1" = --controller-model ] && shift; shift ;;   # accepted, no longer needed
  -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  *) echo "layout: unknown arg $1" >&2; exit 2 ;;
esac; done

say()  { printf 'layout %s\n' "$*"; }
fail() { echo "layout: $*" >&2; exit 1; }
run()  { if [ "$DRY" = 1 ]; then { printf '+'; printf ' %q' "$@"; echo; } >&2; echo '{}'; else "$@"; fi; }
id()   { jq -r "$1 // \"<$2>\"" 2>/dev/null; }   # id '<jq path>' <placeholder>

REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "run inside the repo"
cd "$REPO"
[ -x .context/bin/run-reactor.sh ] || fail "no .context/bin/run-reactor.sh — run setup.sh first"
[ -f .context/workspace.env ] && . .context/workspace.env
MODEL="${MODEL:-composer-2.5-fast}"; DOC_MODEL="${DOC_MODEL:-sonnet}"
LOG="$REPO/.context/events.jsonl"
if [ "$DRY" != 1 ]; then
  [ "${HERDR_ENV:-}" = 1 ] || fail "not inside a herdr pane (HERDR_ENV != 1)"
  command -v append-event.sh >/dev/null || fail "append-event.sh not on PATH — run setup.sh"
fi
WS="${HERDR_WORKSPACE_ID:-<ws>}"; TAB="${HERDR_TAB_ID:-<tab>}"; PANE="${HERDR_PANE_ID:-<pane>}"

# --- discovery helpers (read-only) ---------------------------------------------------
find_tab() {   # <workspace> <label> → "tab_id pane_id" (root pane cwd == repo), or empty
  [ "$DRY" = 1 ] && return 0
  local ws="$1" label="$2" tabs panes
  tabs="$(herdr tab list --workspace "$ws" 2>/dev/null | jq -r --arg l "$label" '.result.tabs[]? | select(.label==$l) | .tab_id')"
  [ -n "$tabs" ] || return 0
  panes="$(herdr pane list --workspace "$ws" 2>/dev/null)"
  for t in $tabs; do
    p="$(jq -r --arg t "$t" --arg cwd "$REPO" '.result.panes[]? | select(.tab_id==$t and (.cwd==$cwd or .foreground_cwd==$cwd)) | .pane_id' <<<"$panes" | head -1)"
    [ -n "$p" ] && { echo "$t $p"; return 0; }
  done
}
find_workspace() {   # <label> → workspace_id holding a pane in this repo, or empty
  [ "$DRY" = 1 ] && return 0
  local ids
  ids="$(herdr workspace list 2>/dev/null | jq -r --arg l "$1" '.result.workspaces[]? | select(.label==$l) | .workspace_id')"
  for w in $ids; do
    herdr pane list --workspace "$w" 2>/dev/null | jq -e --arg cwd "$REPO" '.result.panes[]? | select(.cwd==$cwd)' >/dev/null && { echo "$w"; return 0; }
  done
}
find_pane_in_tab() {   # <tab> <pane-label> → pane_id or empty
  [ "$DRY" = 1 ] && return 0
  herdr pane list 2>/dev/null | jq -r --arg t "$1" --arg l "$2" '.result.panes[]? | select(.tab_id==$t and .label==$l) | .pane_id' | head -1
}
ws_label() { [ "$DRY" = 1 ] && return 0; herdr workspace list 2>/dev/null | jq -r --arg w "$1" '.result.workspaces[]? | select(.workspace_id==$w) | .label'; }
reactor_alive() { local pid; pid="$(cat "$LOG.$1.reactor.lock/pid" 2>/dev/null || true)"; [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; }
spawn_open() {
  [ -e "$LOG" ] || return 1
  local s r
  s="$(jq -r --arg a "$1" 'select(.type=="spawn" and .agent==$a) | .seq' "$LOG" 2>/dev/null | tail -1)"
  r="$(jq -r --arg a "$1" 'select(.type=="retire" and .agent==$a) | .seq' "$LOG" 2>/dev/null | tail -1)"
  [ -n "$s" ] && [ "${r:-0}" -lt "$s" ]
}
ensure_tab() {   # <workspace> <label> <pane-slug> [cmd...] → E_TAB E_PANE E_NEW; reuse or create
  local ws="$1" label="$2" slug="$3"; shift 3
  local found; found="$(find_tab "$ws" "$label")"
  if [ -n "$found" ]; then E_TAB="${found%% *}"; E_PANE="${found##* }"; E_NEW=0; say "reuse  $label → $E_TAB ($E_PANE)"; return 0; fi
  local o; o="$(run herdr tab create --workspace "$ws" --cwd "$REPO" --label "$label" --no-focus)"
  E_TAB="$(id '.result.tab.tab_id' tab <<<"$o")"; E_PANE="$(id '.result.root_pane.pane_id' pane <<<"$o")"; E_NEW=1
  run herdr pane rename "$E_PANE" "$slug" >/dev/null
  [ $# -gt 0 ] && run herdr pane run "$E_PANE" "$@" >/dev/null
  say "create $label → $E_TAB ($E_PANE)"
}
ensure_workspace() {   # <label> → W_ID W_TAB W_PANE W_NEW; reuse by label+cwd or create (root tab/pane returned)
  local label="$1" found; found="$(find_workspace "$label")"
  if [ -n "$found" ]; then W_ID="$found"; W_TAB=""; W_PANE=""; W_NEW=0; say "reuse  workspace $label → $W_ID"; return 0; fi
  local o; o="$(run herdr workspace create --cwd "$REPO" --label "$label" --no-focus)"
  W_ID="$(id '.result.workspace.workspace_id' ws <<<"$o")"; W_TAB="$(id '.result.tab.tab_id' tab <<<"$o")"
  W_PANE="$(id '.result.root_pane.pane_id' pane <<<"$o")"; W_NEW=1
  say "create workspace $label → $W_ID"
}

# --- 1. control: this Claude is the controller; the workspace and tab take the names ---
[ "$(ws_label "$WS")" = control ] || run herdr workspace rename "$WS" control >/dev/null
run herdr tab rename "$TAB" coordinator >/dev/null
say "control: coordinator = this pane ($PANE)"
EPANE="$(find_pane_in_tab "$TAB" eventlog)"
if [ -n "$EPANE" ]; then say "reuse  eventlog pane → $EPANE"; else
  o="$(run herdr pane split "$PANE" --direction down --ratio 0.5 --cwd "$REPO" --no-focus)"
  EPANE="$(id '.result.pane.pane_id' epane <<<"$o")"
  run herdr pane rename "$EPANE" eventlog >/dev/null
  run herdr pane run "$EPANE" eventlog-view.sh -f >/dev/null
  say "create eventlog pane below the controller → $EPANE"
fi
MTAB="<none>"
if [ "$MONITOR" = 1 ]; then
  if command -v agentmon >/dev/null; then
    # cutoff = earliest agent process in this herdr session (agent-since.sh); display filter only
    SINCE="$("$HERE/agent-since.sh" 2>/dev/null || echo launch)"
    ensure_tab "$WS" "monitor: system + agents" agentmon agentmon --since "$SINCE"
  else
    top_cmd=htop; command -v htop >/dev/null || top_cmd=top
    say "WARN: agentmon not on PATH — monitor tab falls back to $top_cmd"
    ensure_tab "$WS" "monitor: system + agents" "$top_cmd" "$top_cmd"
  fi
  MTAB="$E_TAB"
fi

# --- 2. maintenance: lazygit first, then the two reactors, one full-width tab each -------
ensure_workspace maintenance; MAINT="$W_ID"
CLABEL="$MODEL: commit reactor"; DLABEL="$DOC_MODEL: doc sync"
if [ "$W_NEW" = 1 ]; then   # a new workspace's root tab is created first: it becomes the lazygit tab
  run herdr tab rename "$W_TAB" lazygit >/dev/null; run herdr pane rename "$W_PANE" gitlog >/dev/null
  if command -v lazygit >/dev/null; then run herdr pane run "$W_PANE" lazygit >/dev/null
  else say "WARN: lazygit not on PATH — first maintenance tab left as a shell"; fi
  say "create lazygit → $W_TAB ($W_PANE)"
else
  if command -v lazygit >/dev/null; then ensure_tab "$MAINT" lazygit gitlog lazygit
  else ensure_tab "$MAINT" lazygit gitlog; fi
fi
LTAB="$E_TAB"; LPANE="$E_PANE"; [ "$W_NEW" = 1 ] && { LTAB="$W_TAB"; LPANE="$W_PANE"; }
ensure_reactor() {   # <label> <slug> <by-name> <reactor-arg-or-empty> → R_TAB R_PANE
  local label="$1" slug="$2" by="$3" arg="$4"
  ensure_tab "$MAINT" "$label" "$slug"
  R_TAB="$E_TAB"; R_PANE="$E_PANE"
  if reactor_alive "$by"; then
    say "       $by already running (pid $(cat "$LOG.$by.reactor.lock/pid")) — not starting a second one"
  else
    if [ -n "$arg" ]; then run herdr pane run "$R_PANE" bash "$REPO/.context/bin/run-reactor.sh" "$arg" >/dev/null
    else run herdr pane run "$R_PANE" bash "$REPO/.context/bin/run-reactor.sh" >/dev/null; fi
    say "       started $by in $R_PANE"
    if [ "$DRY" != 1 ] && ! herdr pane wait-output "$R_PANE" --match "watching" --timeout 20000 >/dev/null 2>&1; then
      say "WARN: $R_PANE did not report 'watching' in 20s — inspect: herdr pane read $R_PANE"
    fi
  fi
}
ensure_reactor "$CLABEL" commit-reactor cursor-committer ""
CTAB="$R_TAB"; CPANE="$R_PANE"
ensure_reactor "$DLABEL" doc-sync doc-worker doc-sync-reactor.sh
DTAB="$R_TAB"; DPANE="$R_PANE"

# --- 3. lifecycle: one open spawn per agent, never a duplicate ---------------------------
if spawn_open cursor-committer; then say "       cursor-committer already has an open spawn in the log"; else
  run append-event.sh spawn  agent=cursor-committer model="$MODEL" tab="$CTAB" pane="$CPANE" role=commit-reactor runtime=cursor-headless >/dev/null
  run append-event.sh prompt agent=cursor-committer ref=.context/handoffs/cursor-committer.md >/dev/null
fi
if spawn_open doc-worker; then say "       doc-worker already has an open spawn in the log"; else
  run append-event.sh spawn  agent=doc-worker model="$DOC_MODEL" tab="$DTAB" pane="$DPANE" role=doc-sync runtime=claude-headless >/dev/null
  run append-event.sh prompt agent=doc-worker ref=.context/handoffs/doc-worker.md >/dev/null
fi

# --- 4. files: one workspace, one pane, the file tool ------------------------------------
FWS="<none>"
if [ "$FILES" = 1 ]; then
  ensure_workspace files; FWS="$W_ID"
  if [ "$W_NEW" = 1 ]; then
    if command -v spiceedit >/dev/null; then run herdr pane run "$W_PANE" spiceedit >/dev/null
    else say "WARN: spiceedit not on PATH — files workspace left as a shell"; fi
  fi
fi

# --- 5. back to the controller; persist the ids for status/teardown ----------------------
run herdr tab focus "$TAB" >/dev/null
if [ "$DRY" != 1 ]; then
  jq -nc --arg ws "$WS" --arg tab "$TAB" --arg pane "$PANE" --arg epane "$EPANE" --arg mtab "$MTAB" \
         --arg maint "$MAINT" --arg ltab "$LTAB" --arg lpane "$LPANE" --arg ctab "$CTAB" --arg cpane "$CPANE" --arg dtab "$DTAB" --arg dpane "$DPANE" \
         --arg fws "$FWS" --arg model "$MODEL" --arg doc "$DOC_MODEL" \
    '{controller:{workspace:$ws,tab:$tab,pane:$pane,eventlog_pane:$epane,monitor_tab:$mtab},
      maintenance:{workspace:$maint,lazygit_tab:$ltab,lazygit_pane:$lpane},
      committer:{tab:$ctab,pane:$cpane,model:$model,agent:"cursor-committer"},
      doc_worker:{tab:$dtab,pane:$dpane,model:$doc,agent:"doc-worker"},
      files:{workspace:$fws}}' > .context/layout.json
  say "wrote .context/layout.json"
  [ -x "$LAYOUTS/layout-map.sh" ] && "$LAYOUTS/layout-map.sh" --doctor --workspace "$MAINT" || true
fi
say "ready. Work flows: append a controller 'result … paths=<files>' → committer commits → doc worker documents → committer lands docs. Never ping a reactor."
