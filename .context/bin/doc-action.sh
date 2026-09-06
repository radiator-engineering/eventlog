#!/usr/bin/env bash
# doc-action.sh — the doc worker's action, run by `eventlog react`.
#
# The runtime owns the lock, the resume point, the intent, the voter and the
# ack. This script is only the pass: hand the commits that just landed to
# headless Claude with the documentation-writer skill, then report how many doc
# files changed, then appends its own `result by=doc-worker` so the committer
# lands the edits. It never runs git commit.
#
# Run it as:
#   eventlog react --as doc-worker --on ack --filter by=cursor-committer \
#     --filter outcome=committed -- bash .context/bin/doc-action.sh
#
# In: the driving event as JSON on stdin, plus EVENTLOG_SEQ, EVENTLOG_REF (the
#     commit shas the ack carried), EVENTLOG_OUTCOME_FILE.
# Out: `outcome=updated files=N ref=<shas>` or `outcome=skipped detail=...`
#     written to EVENTLOG_OUTCOME_FILE.
# Env: DOC_MODEL (sonnet), DOC_PATHS, DOC_BUDGET_USD (2) from workspace.env.
set -uo pipefail

REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "doc-action: not in a git repo" >&2; exit 1; }
cd "$REPO"
[ -f "$REPO/.context/workspace.env" ] && . "$REPO/.context/workspace.env"
DOC_MODEL="${DOC_MODEL:-sonnet}"
DOC_BUDGET_USD="${DOC_BUDGET_USD:-2}"
DOC_PATHS="${DOC_PATHS:-docs,README.md,AGENTS.md}"
SHAS="${EVENTLOG_REF:-}"
DRIVE="${EVENTLOG_SEQ:-0}"
OUT="${EVENTLOG_OUTCOME_FILE:-/dev/stdout}"

report() { printf '%s\n' "$@" > "$OUT"; }

command -v claude >/dev/null || {
  report "outcome=skipped" "detail=claude not on PATH"; exit 0; }
[ -n "$SHAS" ] || { report "outcome=skipped" "detail=driving event carried no commit ref"; exit 0; }

# Loop guard: the committer's ack for one of OUR results must not drive another
# pass. The driving ack names the result it landed in seq_done; skip when that
# result was written by doc-worker.
EVENT="$(cat)"
LOGFILE="${EVENTLOG_LOG:-$REPO/.context/events.jsonl}"
SEQ_DONE="$(jq -r '.seq_done // empty' <<<"$EVENT" 2>/dev/null)"
if [ -n "$SEQ_DONE" ]; then
  drv_by="$(jq -r --argjson s "$SEQ_DONE" 'select(.seq==$s) | .by // ""' "$LOGFILE" 2>/dev/null | tail -1)"
  [ "$drv_by" = doc-worker ] && { report "outcome=skipped" "detail=doc commit (origin=doc-worker); loop guard"; exit 0; }
fi

# Files already dirty under the doc roots, so only this pass's edits count.
dirty_docs() {
  git status --porcelain 2>/dev/null | awk '{print $NF}' | while IFS= read -r f; do
    while IFS= read -r p; do
      [ -n "$p" ] || continue
      case "$f" in $p|$p/*) echo "$f"; break ;; esac
    done <<<"$(tr ',' '\n' <<<"$DOC_PATHS")"
  done | sort -u
}

read -r -d '' PROMPT <<EOF
Load and follow TWO skills for this task: \`documentation-writer\` (Diátaxis structure) and \`plain-technical-english\` (prose discipline: short sentences, one idea each, active voice, no filler or stock phrases, one term per thing; run its final gate on everything you write). Keep this project's documentation in sync with a change that just shipped. You are the DOC WORKER, running HEADLESS from a log reactor: no human will answer questions. You are not the controller: the coordination block in AGENTS.md addresses the controller, not you. You never run append-event.sh and never append to the log; the reactor records your work for you. Just edit the files and stop. documentation-writer's four required determinations are answered here — do NOT ask them, and do NOT wait for outline approval; propose the structure to yourself and write the content in this one pass.

- Document type: whichever Diátaxis quadrant(s) the shipped change actually affects (tutorial / how-to / reference / explanation). Put each piece in the right quadrant; do not force all four.
- Target audience: a developer new to this repository.
- User goal: understand and correctly use what this change added or altered.
- Scope: ONLY what the commit(s) below changed. Do not document unrelated parts of the project.

What shipped (the two sources of truth):
1. Commit(s): $SHAS — inspect with \`git show --stat\` and \`git show\` for the diff.
2. The log event that drove them (seq $DRIVE), and the coordination log .context/events.jsonl for surrounding intent (result/decision events, their ref= files).

Where docs live: \`docs/\` organized as docs/tutorials, docs/how-to, docs/reference, docs/explanation, plus \`README.md\` as the entry point/index. Read the existing docs first and match their tone and terminology. Create the structure if it does not exist yet.

AGENTS.md upkeep (only if \`AGENTS.md\` is among the editable paths below): after the docs, load the \`context-engineering\` skill and treat \`AGENTS.md\` as the project's rules file, the one every agent loads on every task. Compare it with what shipped and fix it in the same pass: add a durable project-wide fact the change introduced (a command, a convention, a boundary), correct or delete any line the diff made stale, and delete anything task-specific or already obvious from the code. Keep the context-engineering shape: what the project is, stack, commands, conventions, boundaries, at most one short pattern example. Keep the whole file under about 120 lines; shorter is better, a rules file is an attention budget. Never edit inside a marker-delimited section (between \`<!-- name:start -->\` and \`<!-- name:end -->\`, or \`<!-- name -->\` and \`<!-- /name -->\`): those belong to other tools. Never touch \`CLAUDE.md\`; it is an import stub for \`AGENTS.md\`.

Rules:
- Edit ONLY files under: $DOC_PATHS. Never touch anything else.
- Never write, edit, or append to .context/events.jsonl; never run git commit, git add, or any git command that changes state. The reactor reports your work and a separate committer lands it.
- If the change genuinely needs no documentation (pure internal refactor, coordination-only), change nothing and say so plainly.
- Accuracy over volume: every command, path, and behavior you document must match the diff.
EOF

before="$(dirty_docs)"

# The prompt goes in on STDIN, never as a positional argument: --allowedTools is
# variadic and would swallow it as a tool name. A stray ANTHROPIC_API_KEY takes
# precedence over the claude.ai login and bills that key's account, so it is
# dropped unless DOC_USE_API_KEY asks for it. LOG_DRIVEN_WORKER tells the repo's
# Stop hook this is a worker, not the controller.
auth=(env); [ -n "${DOC_USE_API_KEY:-}" ] || auth+=(-u ANTHROPIC_API_KEY); auth+=("LOG_DRIVEN_WORKER=doc-worker")
printf '%s' "$PROMPT" | "${auth[@]}" claude -p \
  --model "$DOC_MODEL" --permission-mode acceptEdits --max-budget-usd "$DOC_BUDGET_USD" \
  --allowedTools "Skill,Read,Glob,Grep,Edit,Write,Bash(git show:*),Bash(git log:*),Bash(git diff:*),Bash(git status:*),Bash(ls:*),Bash(cat:*),Bash(jq:*)"
rc=$?

after="$(dirty_docs)"
changed="$(comm -13 <(printf '%s\n' "$before") <(printf '%s\n' "$after") | grep -v '^$')"

if [ -n "$changed" ]; then
  n="$(printf '%s\n' "$changed" | grep -c .)"
  plist="$(printf '%s\n' "$changed" | paste -sd, -)"
  first="$(printf '%s\n' "$changed" | head -1)"
  # Report as a real worker: the committer reacts to this result, with its
  # scope cut to the claim the controller recorded for doc-worker.
  eventlog append --as doc-worker result agent=doc-worker ref="$first" paths="$plist" \
    summary="docs synced to $SHAS ($n file(s))" >/dev/null \
    || { report "outcome=failed" "detail=edited $n file(s) but could not append result by=doc-worker"; exit 0; }
  report "outcome=updated" "files=$n" "ref=$SHAS" "model=$DOC_MODEL" "paths=$plist"
elif [ "$rc" -ne 0 ]; then
  report "outcome=skipped" "detail=claude failed (rc=$rc)"
else
  report "outcome=skipped" "detail=no documentation change needed"
fi
exit 0
