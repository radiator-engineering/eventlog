# Committer action script for `eventlog react`

Pass this script after `--`. The runtime owns the lock, resume, intent, voter,
veto window, violation detection, and ack. Your script receives the driving
event as JSON on stdin and reads `EVENTLOG_PATHS` and `EVENTLOG_OUTCOME_FILE`.

```bash
#!/usr/bin/env bash
# Committer action: one git commit per decision key=commit-message.
set -euo pipefail
read -r event
msg=$(jq -r '.value' <<<"$event")
# shellcheck disable=SC2086
git add -- ${EVENTLOG_PATHS//,/ }
if git diff --cached --quiet; then
  printf 'outcome=skipped\ndetail=nothing staged\n' > "$EVENTLOG_OUTCOME_FILE"
  exit 0
fi
git commit -m "$msg"
sha=$(git rev-parse --short HEAD)
printf 'outcome=committed\nref=%s\nsummary=%s\n' "$sha" "$msg" > "$EVENTLOG_OUTCOME_FILE"
```

Launch:

```bash
eventlog react --as committer --on decision --filter key=commit-message --git -- \
  bash path/to/this-script
```

Dry-run against seq 42:

```bash
eventlog react test 42 --as committer --git -- bash path/to/this-script
```
