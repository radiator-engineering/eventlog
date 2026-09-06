#!/usr/bin/env bash
# commit-action.sh — the committer's action, run by `eventlog react`.
#
# The runtime owns the lock, the resume point, the intent, the voter, the veto
# window, the violation check and the ack. This script is only the pass: hand
# the driving event and the diff to headless Composer, then report whether HEAD
# moved. It NEVER touches the log.
#
# Run it as:
#   eventlog react --as cursor-committer --on result --git -- \
#     bash .context/bin/commit-action.sh
#
# In: the driving event as JSON on stdin, plus EVENTLOG_SEQ, EVENTLOG_TYPE,
#     EVENTLOG_BY, EVENTLOG_PATHS (the authorized set), EVENTLOG_REF,
#     EVENTLOG_RESUME, EVENTLOG_OUTCOME_FILE.
# Out: `outcome=committed ref=<shas> model=<m>` or `outcome=skipped detail=...`
#     written to EVENTLOG_OUTCOME_FILE.
# Env: MODEL (composer-2.5-fast) from .context/workspace.env.
set -uo pipefail

REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "commit-action: not in a git repo" >&2; exit 1; }
cd "$REPO"
[ -f "$REPO/.context/workspace.env" ] && . "$REPO/.context/workspace.env"
MODEL="${MODEL:-composer-2.5-fast}"
PATHS="${EVENTLOG_PATHS:-}"
SEQ="${EVENTLOG_SEQ:-0}"
RESUME="${EVENTLOG_RESUME:-0}"
OUT="${EVENTLOG_OUTCOME_FILE:-/dev/stdout}"

report() { printf '%s\n' "$@" > "$OUT"; }

command -v cursor-agent >/dev/null || {
  report "outcome=skipped" "detail=cursor-agent not on PATH"; exit 0; }

# A stale index.lock left by a killed git makes every commit fail. Report it;
# never remove it — a live git may own it.
if [ -e "$REPO/.git/index.lock" ] && ! pgrep -x git >/dev/null 2>&1; then
  report "outcome=skipped" "detail=stale .git/index.lock present; remove it by hand"
  exit 0
fi

# Nothing to commit is a normal outcome, not a failure.
if ! git status --porcelain 2>/dev/null | grep -q .; then
  report "outcome=skipped" "detail=clean tree"; exit 0
fi

scope=""
[ -n "$PATHS" ] && scope=" It names these files as the task's scope: $PATHS — stage ONLY those unless the diff makes another file obviously part of the same change."

read -r -d '' PROMPT <<EOF
You are the committer for this repo (Composer 2.5 Fast), running headless from a log reactor. You are not the controller: the coordination block in AGENTS.md (its "never git commit" and "append a result" rules) addresses the controller, not you. Committing is your job; the reactor records the ack for you.

Read THE TWO and turn finished work into meaningful, well-structured git commits:
1. The coordination log .context/events.jsonl — new events since seq $RESUME tell you WHAT happened and WHY (result/decision/claim, each with a ref= file).
2. The working tree — \`git status --porcelain\` and \`git diff\` show the ACTUAL changes.

The triggering event is seq $SEQ.$scope

Do this now:
- Group the changed files into coherent commits (one commit = one logical change; split unrelated edits).
- For each: \`git add <explicit paths>\` (NEVER \`git add -A\`), then \`git commit\` with a concise imperative subject and a body explaining WHY, grounded in the log (cite the driving event seq).
- Never stage the log or its lock dirs (.context/events.jsonl and .context/events.jsonl.*). Everything else under .context/ is tracked and is committable when the driving event names it.
- No AI attribution in messages (no Co-Authored-By, no "Generated with").
- Do NOT write, edit, or append to .context/events.jsonl — the reactor records the outcome itself.
- If nothing coheres into a commit, make no commit and say so.

Commit the landed work now.
EOF

head_before="$(git rev-parse HEAD 2>/dev/null)"
cursor-agent -p --force --trust --model "$MODEL" --output-format text "$PROMPT"
rc=$?
head_after="$(git rev-parse HEAD 2>/dev/null)"

if [ "$head_before" != "$head_after" ]; then
  shas="$(git log --format=%h "$head_before..$head_after" 2>/dev/null | paste -sd, -)"
  report "outcome=committed" "ref=$shas" "model=$MODEL"
elif [ "$rc" -ne 0 ]; then
  report "outcome=skipped" "detail=cursor-agent failed (rc=$rc); work remains uncommitted"
else
  report "outcome=skipped" "detail=model ran, made no commit"
fi
exit 0
