# Committer action for `eventlog react`

The runtime owns locking, resume, intent, voting, the veto window, Git
accounting and acknowledgments. Use the packaged commit action to preserve
exact path boundaries and unrelated staged work, including filenames with
spaces and glob characters:

```sh
eventlog lifecycle start committer --role commit-reactor
eventlog react --as committer --on result --git -- eventlog action commit
```

Run the reactor in a persistent pane. A supervisor calls `lifecycle stop
committer` when it stops that process. Generated Drove hooks wire this pair
automatically.

The action reads `EVENTLOG_PATHS`, commits only that authorized set, and
reports `outcome` and actual commit `ref` values for the runtime's ack.
Use `--message` for a fixed message or configure `[commit].command` in
`.context/eventlog-setup.toml` for a model-backed author. The configured
command inherits the driving event on stdin in an isolated checkout.

Test against seq 42 in a disposable repository. This executes the action;
the runtime prints its coordination events instead of appending them:

```sh
eventlog react test 42 --as committer --git -- eventlog action commit
```

Add `--filter agent=<worker>` to react to one worker's results only, and
`--timeout 900s` when the action runs a model.
