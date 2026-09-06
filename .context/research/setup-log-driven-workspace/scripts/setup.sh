#!/usr/bin/env bash
# setup.sh — scaffold a log-driven workspace in the current git repo. FILES ONLY:
# no herdr panes, no agents started (that is layout.sh). Idempotent: existing
# files are kept unless --force; decisions are appended once.
#
#   setup.sh [--committer-model M] [--doc-model M] [--doc-paths a,b]
#            [--churn PATH,...] [--protect] [--commit] [--force] [--dry-run]
#
#   --commit           commit the scaffold this script wrote (hook, ignores, .context/)
#                      so the tree is clean before the reactors start
#   --committer-model  cursor-agent model for the committer   (composer-2.5-fast)
#   --doc-model        claude -p model for the doc worker      (sonnet)
#   --doc-paths        comma list of doc roots the doc worker may edit (docs,README.md)
#   --churn            files an editor rewrites on its own (e.g. .obsidian/workspace.json);
#                      untracked + ignored so they cannot keep the tree dirty
#   --protect          OS-protect the log (chflags uappnd / chattr +a) — opt-in
#   --force            overwrite reactor scripts / briefs / hooks / AGENTS.md block from the
#                      templates (never DECISIONS.md or workspace.env); restart reactors after
#   --orders           ONLY refresh the standing orders: AGENTS.md block, CLAUDE.md import,
#                      Stop hook. Safe on a live repo with running reactors.
#
# What it lays down:
#   .context/events.jsonl (+ EVENTLOG.md)      via event-log-coordination's init
#   .context/bin/{cursor-commit-reactor,doc-sync-reactor,run-reactor}.sh
#   .context/handoffs/{cursor-committer,doc-worker}.md
#   .context/DECISIONS.md, .context/workspace.env
#   .githooks/commit-msg  + git config core.hooksPath .githooks
#   CLAUDE.md + AGENTS.md controller block (marker-delimited, kept unless --force)
#   .context/bin/controller-stop-hook.sh + Stop hook in .claude/settings.json
#   .gitignore entries for the log, locks, churn files
#   decision events: log-writers, commit-agent, doc-agent
set -euo pipefail
HERE="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TPL="$HERE/../templates"
EVENTLOG_SKILL="${EVENTLOG_SKILL:-$HOME/.claude/skills/event-log-coordination}"

COMMITTER_MODEL="composer-2.5-fast"; DOC_MODEL="sonnet"; DOC_PATHS="docs,README.md,AGENTS.md"
CHURN=""; PROTECT=0; FORCE=0; DRY=0; COMMIT=0; ORDERS=0
while [ $# -gt 0 ]; do case "$1" in
  --commit)          COMMIT=1; shift ;;
  --orders)          ORDERS=1; FORCE=1; shift ;;   # standing orders + Stop hook only; reactors/env untouched
  --committer-model) COMMITTER_MODEL="$2"; shift 2 ;;
  --doc-model)       DOC_MODEL="$2"; shift 2 ;;
  --doc-paths)       DOC_PATHS="$2"; shift 2 ;;
  --churn)           CHURN="$2"; shift 2 ;;
  --protect)         PROTECT=1; shift ;;
  --force)           FORCE=1; shift ;;
  --dry-run)         DRY=1; shift ;;
  -h|--help)         sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  *) echo "setup: unknown arg $1" >&2; exit 2 ;;
esac; done

say()  { printf 'setup  %s\n' "$*"; }
run()  { if [ "$DRY" = 1 ]; then { printf '+'; printf ' %q' "$@"; echo; } >&2; else "$@"; fi; }
fail() { echo "setup: $*" >&2; exit 1; }

# --- 0. prerequisites ----------------------------------------------------------
REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "run inside a git repository"
cd "$REPO"
[ -d "$EVENTLOG_SKILL/scripts" ] || fail "event-log-coordination skill not found at $EVENTLOG_SKILL (set EVENTLOG_SKILL)"
missing=""
for c in jq git; do command -v "$c" >/dev/null || missing="$missing $c"; done
[ -z "$missing" ] && true || fail "required on PATH:$missing"
for c in cursor-agent claude; do command -v "$c" >/dev/null || say "WARN: $c not on PATH — its reactor will refuse to start until it is"; done
command -v timeout >/dev/null || command -v gtimeout >/dev/null || say "WARN: no timeout/gtimeout — reactor passes will not be time-boxed (brew install coreutils)"

put() {   # put <template> <dest> [mode]
  local src="$TPL/$1" dst="$2" mode="${3:-0644}"
  if [ -e "$dst" ] && [ "$FORCE" != 1 ]; then say "keep  $dst (use --force to overwrite)"; return 0; fi
  run mkdir -p "$(dirname "$dst")"
  run install -m "$mode" "$src" "$dst"; say "wrote $dst"
}
if [ "$ORDERS" != 1 ]; then
# --- 1. log tooling: doctor links commands onto PATH + registers the guard -------
run "$EVENTLOG_SKILL/scripts/safety-check.sh" --doctor </dev/null || say "WARN: doctor reported gaps (see above)"
hash -r 2>/dev/null || true
command -v append-event.sh >/dev/null || export PATH="$EVENTLOG_SKILL/scripts:$PATH"

# --- 2. the log ------------------------------------------------------------------
if [ ! -e .context/events.jsonl ]; then run "$EVENTLOG_SKILL/scripts/init-eventlog.sh"; else say "log exists: .context/events.jsonl"; fi

# --- 3. reactors, briefs, hook (templates) ---------------------------------------
put cursor-commit-reactor.sh .context/bin/cursor-commit-reactor.sh 0755
put doc-sync-reactor.sh      .context/bin/doc-sync-reactor.sh      0755
put run-reactor.sh           .context/bin/run-reactor.sh           0755
put cursor-committer.md      .context/handoffs/cursor-committer.md
put doc-worker.md            .context/handoffs/doc-worker.md
put commit-msg               .githooks/commit-msg                  0755
run git config core.hooksPath .githooks
fi
# a running reactor keeps its old script until restarted
[ "$FORCE" = 1 ] && [ "$ORDERS" != 1 ] && ls -d .context/events.jsonl.*.reactor.lock >/dev/null 2>&1 \
  && say "NOTE: reactors refreshed on disk; restart them (teardown.sh, then layout.sh) to run the new scripts"
if [ -e .context/workspace.env ]; then   # an existing repo's real doc roots win: the block must name them
  unset DOC_PATHS; . .context/workspace.env; DOC_PATHS="${DOC_PATHS:-docs,README.md,AGENTS.md}"
fi

# --- 3b. the controller's standing orders (also the whole of --orders)
put controller-stop-hook.sh  .context/bin/controller-stop-hook.sh  0755
#         a managed block in AGENTS.md (imported by CLAUDE.md),
#         and a Stop hook that refuses to end a turn while changed files have no result
#         One source of truth: the block lives in AGENTS.md (read by every agent tool);
#         CLAUDE.md imports AGENTS.md with an `@AGENTS.md` line (Claude Code import syntax).
BSTART='<!-- log-driven-workspace:start'; BEND='<!-- log-driven-workspace:end -->'
render_block() { sed -e "s|{{DOC_PATHS}}|$DOC_PATHS|g" "$TPL/CONTROLLER.md"; }
splice_block() {   # splice_block <file> <template-or-/dev/null>: replace the marker-delimited block (empty template = remove)
  awk -v s="$BSTART" -v e="$BEND" -v tpl="$2" '
    index($0,s)==1 {while ((getline l < tpl) > 0) print l; skip=1; next}
    skip && index($0,e)==1 {skip=0; next}
    !skip {print}' "$1" > "$1.tmp" && mv "$1.tmp" "$1"
}
block_into() {   # block_into <file>: insert/refresh the controller block
  local f="$1"
  if [ -e "$f" ] && grep -qF "$BSTART" "$f"; then
    if [ "$FORCE" = 1 ]; then
      if [ "$DRY" = 1 ]; then say "would refresh controller block in $f"; else
        render_block > "$f.block"; splice_block "$f" "$f.block"; rm -f "$f.block"; say "refreshed controller block in $f"; fi
    else say "keep  controller block in $f (--force to refresh)"; fi
  elif [ "$DRY" = 1 ]; then say "would add controller block to $f"
  else
    if [ -e "$f" ] && [ -s "$f" ]; then printf '\n' >> "$f"
    elif [ ! -e "$f" ]; then printf '# AGENTS.md\n\nGuidance for AI coding agents working in this repo. Keep it short and current.\nClaude Code reads it through CLAUDE.md, which imports it.\n\n' > "$f"; fi
    render_block >> "$f"; say "added controller block to $f"; fi
}
block_into AGENTS.md
# CLAUDE.md: import AGENTS.md once; an older copy of the block in CLAUDE.md is removed (it would load twice)
if [ "$DRY" = 1 ]; then say "would make CLAUDE.md import AGENTS.md"; else
  if [ -e CLAUDE.md ] && grep -qF "$BSTART" CLAUDE.md; then splice_block CLAUDE.md /dev/null; say "removed duplicate controller block from CLAUDE.md (now imported from AGENTS.md)"; fi
  if [ -e CLAUDE.md ] && grep -qE '^@AGENTS\.md[[:space:]]*$' CLAUDE.md; then say "CLAUDE.md already imports AGENTS.md"
  elif [ -e CLAUDE.md ] && [ -s CLAUDE.md ]; then printf '\n@AGENTS.md\n' >> CLAUDE.md; say "CLAUDE.md now imports AGENTS.md"
  else printf '# CLAUDE.md\n\nProject guidance for Claude Code lives in `AGENTS.md` (the cross-tool standard,\nalso read by other agents). It is imported below so there is one source of truth\n— edit `AGENTS.md`, not this file.\n\n@AGENTS.md\n' > CLAUDE.md; say "wrote CLAUDE.md (imports AGENTS.md)"; fi
fi
HOOK_CMD='bash "$CLAUDE_PROJECT_DIR/.context/bin/controller-stop-hook.sh"'
if [ "$DRY" = 1 ]; then say "would register Stop hook in .claude/settings.json"; else
  mkdir -p .claude; [ -s .claude/settings.json ] || echo '{}' > .claude/settings.json
  if jq -e --arg c "$HOOK_CMD" '[.hooks.Stop[]?.hooks[]? | select(.command==$c)] | length > 0' .claude/settings.json >/dev/null 2>&1; then
    say "Stop hook already registered in .claude/settings.json"
  else
    jq --arg c "$HOOK_CMD" '.hooks.Stop = ((.hooks.Stop // []) + [{hooks:[{type:"command",command:$c}]}])' .claude/settings.json > .claude/settings.json.tmp \
      && mv .claude/settings.json.tmp .claude/settings.json && say "registered Stop hook in .claude/settings.json (takes effect in a new Claude session)"
  fi
fi
case ",$DOC_PATHS," in *,AGENTS.md,*) ;; *) say "NOTE: AGENTS.md is not a doc root (DOC_PATHS=$DOC_PATHS in .context/workspace.env); add it so the doc worker maintains the rules file" ;; esac
if [ "$ORDERS" = 1 ]; then
  say "standing orders refreshed (AGENTS.md block, CLAUDE.md import, Stop hook). Reactors and workspace.env untouched. Restart the controller session to load the hook."
  exit 0
fi

# --- 4. per-repo settings the reactors source ------------------------------------
if [ -e .context/workspace.env ]; then say "keep  .context/workspace.env (never regenerated; edit it directly)"
elif [ "$DRY" = 1 ]; then say "would write .context/workspace.env"; else
cat > .context/workspace.env <<EOF
# Sourced by the reactors in .context/bin. Shell env wins over these defaults.
: "\${MODEL:=$COMMITTER_MODEL}"          # committer (cursor-agent) model
: "\${DOC_MODEL:=$DOC_MODEL}"            # doc worker (claude -p) model
: "\${DOC_PATHS:=$DOC_PATHS}"            # doc roots the doc worker may edit
: "\${PASS_TIMEOUT:=300}"
export MODEL DOC_MODEL DOC_PATHS PASS_TIMEOUT
EOF
say "wrote .context/workspace.env"; fi

# --- 5. DECISIONS.md from the template (placeholders filled) ---------------------
if [ ! -e .context/DECISIONS.md ]; then   # never regenerated, not even with --force: it holds the project's own decisions
  if [ "$DRY" = 1 ]; then say "would write .context/DECISIONS.md"; else
    sed -e "s|{{COMMITTER_MODEL}}|$COMMITTER_MODEL|g" -e "s|{{DOC_MODEL}}|$DOC_MODEL|g" \
        -e "s|{{DOC_PATHS}}|$DOC_PATHS|g" -e "s|{{CHURN}}|${CHURN:-none}|g" \
        "$TPL/DECISIONS.md" > .context/DECISIONS.md; say "wrote .context/DECISIONS.md"; fi
else say "keep  .context/DECISIONS.md (never regenerated; it holds the project's decisions)"; fi

# --- 6. .gitignore + editor churn --------------------------------------------------
ign() { grep -qxF "$1" .gitignore 2>/dev/null || { [ "$DRY" = 1 ] && say "would ignore $1" || { echo "$1" >> .gitignore; say "ignore $1"; }; }; }
touch .gitignore
ign ".context/events.jsonl"; ign ".context/events.jsonl.lock"; ign ".context/*.reactor.lock/"; ign ".context/layout.json"; ign ".DS_Store"
if [ -n "$CHURN" ]; then
  for f in $(tr ',' ' ' <<<"$CHURN"); do
    ign "$f"
    if git ls-files --error-unmatch "$f" >/dev/null 2>&1; then run git rm -q --cached "$f"; say "untracked churn file $f (commit this)"; fi
  done
fi

# --- 7. record the decisions once ------------------------------------------------
has_decision() { jq -e --arg k "$1" 'select(.type=="decision" and .key==$k)' .context/events.jsonl >/dev/null 2>&1; }
dec() { local k="$1"; shift; if has_decision "$k"; then say "decision $k already recorded"; else run append-event.sh decision key="$k" "$@" ref=.context/DECISIONS.md >/dev/null; say "decision $k recorded"; fi; }
if [ -e .context/events.jsonl ] || [ "$DRY" = 1 ]; then
  dec log-writers  value=controller-plus-reactors
  dec commit-agent value="cursor-commit-reactor+$COMMITTER_MODEL" mode=autonomous
  dec doc-agent    value="doc-sync-reactor+claude-$DOC_MODEL" mode=autonomous
fi

# --- 8. OS protection (opt-in) -----------------------------------------------------
if [ "$PROTECT" = 1 ]; then run "$EVENTLOG_SKILL/scripts/protect-log.sh"; say "log OS-protected (append-only)"; fi

# --- 8b. monitor helper: agentsview has no TUI; this follows the newest session ----
if ! command -v agentmon >/dev/null && command -v agentsview >/dev/null && ! command -v agentsview-follow >/dev/null; then
  run install -m 0755 "$TPL/agentsview-follow" "$HOME/.local/bin/agentsview-follow" && say "installed ~/.local/bin/agentsview-follow (monitor event-stream tab)"
fi

# --- 9. the scaffold this script wrote is itself a change: commit it (--commit) or say so
SCAFFOLD=".gitignore .githooks/commit-msg .context/bin .context/handoffs .context/DECISIONS.md .context/EVENTLOG.md .context/workspace.env CLAUDE.md AGENTS.md .claude/settings.json"
if [ "$COMMIT" = 1 ] && [ "$DRY" != 1 ]; then
  git add -- $SCAFFOLD 2>/dev/null || true
  if ! git diff --cached --quiet; then
    git commit -q -m "chore: add log-driven workspace scaffold" -m "Reactors, briefs, commit-msg hook, decisions and ignores laid down by setup-log-driven-workspace/setup.sh." \
      && say "committed the scaffold ($(git rev-parse --short HEAD))"
  else say "scaffold already committed"; fi
elif [ "$DRY" != 1 ] && ! git diff --quiet -- $SCAFFOLD 2>/dev/null || git ls-files --others --exclude-standard -- $SCAFFOLD 2>/dev/null | grep -q .; then
  say "NOTE: the scaffold is uncommitted — rerun with --commit, or: git add $SCAFFOLD && git commit"
fi
# anything ELSE dirty outside .context/ gets swept into the committer's first pass
if git status --porcelain | grep -vE '^.. (\.context/|\.githooks/|\.claude/|\.gitignore|CLAUDE\.md|AGENTS\.md)' | grep -q .; then
  say "NOTE: other paths are dirty outside .context/ — commit or stash them before layout.sh, or the first result event hands them to the committer"
fi

say "done. Next: run scripts/layout.sh from inside a herdr pane to place the panes and start the reactors."
