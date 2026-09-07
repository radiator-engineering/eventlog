# Committer action script for `eventlog react`

The runtime owns lock, resume, intent, voter, veto window, git check and
ack. This script is only the pass. It reads the driving `result` event from
stdin and stages exactly `EVENTLOG_PATHS`, the authorized set.

```bash
#!/usr/bin/env bash
# Committer action: one commit per `result` that names paths.
set -euo pipefail
read -r event
msg=$(jq -r '.summary // "land result \(.seq)"' <<<"$event")
if [ -z "${EVENTLOG_PATHS:-}" ]; then
  printf 'outcome=skipped\ndetail=no paths on the driving event\n' > "$EVENTLOG_OUTCOME_FILE"
  exit 0
fi
# shellcheck disable=SC2086
git add -- ${EVENTLOG_PATHS//,/ }
if git diff --cached --quiet; then
  printf 'outcome=skipped\ndetail=nothing staged\n' > "$EVENTLOG_OUTCOME_FILE"
  exit 0
fi
git commit -q -m "$msg"
printf 'outcome=committed\nref=%s\n' "$(git rev-parse --short HEAD)" > "$EVENTLOG_OUTCOME_FILE"
```

`outcome` and `ref` land on the `ack`. Any other key you write is folded
into `detail=`.

Launch, in a pane that outlives the agents' turns:

```sh
eventlog react --as committer --on result --git -- bash .context/bin/commit-action.sh
eventlog append spawn agent=committer role=commit-reactor runtime=eventlog-react
```

Dry-run against seq 42 (writes nothing):

```sh
eventlog react test 42 --as committer --git -- bash .context/bin/commit-action.sh
```

Add `--filter agent=<worker>` to react to one worker's results only, and
`--timeout 900s` when the action runs a model.
