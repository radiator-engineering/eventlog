# Action: `eventlog action`

Status: implemented. `src/cmd/action.rs` holds `commit` and `docs`, two
packaged actions meant to run as the command after `--` in
`eventlog react ... -- eventlog action ...`. Both read `EVENTLOG_PATHS`,
`EVENTLOG_LOG`, and the other environment variables `react` sets, and both
use argv throughout — no shell, so a reactor's configuration is never
interpolated into one.

```sh
eventlog action commit [--message <msg>]
eventlog action docs
```

## `eventlog action commit`

Stages and commits exactly the files named in `EVENTLOG_PATHS`, and nothing
else already staged:

1. Expands `EVENTLOG_PATHS` (a comma-separated list of paths or globs)
   against the working tree, including deleted files.
2. If any resolved path is already staged, fails with `outcome=failed` and
   the overlapping paths in `detail`, so a reactor never absorbs unrelated
   staged work into its own commit — see
   `commit_action_rejects_glob_and_directory_scopes_with_staged_target` in
   `tests/scaffold.rs`.
3. If `.context/eventlog-setup.toml`'s `[commit].command` is empty (the
   default), runs `git add -- <path>` for each resolved path, one at a time,
   then `git commit --only -m <message> -- <paths>`. `--only` commits just
   the named paths even if the index holds other staged changes, so those
   changes remain staged after the commit —
   `commit_action_keeps_unrelated_staging_out_of_the_commit` covers this.
4. On success, reports `outcome=committed`, `paths=<paths>`, and
   `ref=<commit oid>`.

`--message` defaults to `chore(eventlog): apply reactor result` and is only
useful outside a reactor; `react` fills in the real message. It is also
passed as `EVENTLOG_COMMIT_MESSAGE` to a configured `[commit].command`.

### Configured commit author

Set `[commit].command` (see [Setup](setup.md)) to run a model-backed commit
author instead of the direct `git commit` mode above:

1. `action commit` clones the repository into a disposable checkout, copies
   in the exact content of the resolved `EVENTLOG_PATHS` (additions, edits,
   and deletions), and stages them there. The command never sees the
   source repository's index.
2. It runs the configured argv in that checkout, with `EVENTLOG_MODEL`,
   `EVENTLOG_PATHS`, and `EVENTLOG_COMMIT_MESSAGE` set, and the driving
   event on stdin. An argv entry that is exactly `{model}` is replaced with
   `[commit].model`.
3. The command must create one or more commits in the checkout. `action
   commit` checks every commit it produced: each one touches only
   `EVENTLOG_PATHS`, none rewrites the checkout's starting history, none is
   a merge commit, and no authorized change is left uncommitted.
4. Before publishing, it confirms the source repository's `HEAD`, index,
   and the resolved paths did not change while the command ran, then fetches
   the validated commits onto the source `HEAD` and restores the working
   tree and index for `EVENTLOG_PATHS` to match.
5. On success, reports `outcome=committed`, `paths=<paths the command
   touched>`, and `ref=<comma-separated commit oids, oldest first>`. A
   command that creates valid commits and then exits nonzero still reports
   `committed` with those oids; the exit detail is appended to `detail`. A
   command that fails before producing any commit, or that touches an
   unauthorized path, reports `outcome=failed` and nothing is published.

A clean `EVENTLOG_PATHS` (nothing to commit) reports `outcome=skipped`
before either mode runs. `tests/commit_command.rs` covers the configured
path: authorized-path validation, unrelated-staging preservation, multiple
commits, a late nonzero exit after a successful commit, and rejection of an
unauthorized change, a merge commit, and rewritten history.

## `eventlog action docs`

Runs the project's configured documentation command and reports what it
changed:

1. Reads `.context/eventlog-setup.toml`'s `[docs]` table (see
   [Setup](setup.md)). Fails with `outcome=failed` if `docs.command` is
   empty — an unconfigured docs action is never silently a no-op.
2. If the driving event is an `ack`, resolves the `result` it closed and
   compares that result's writer to `docs.identity`; if they match, skips
   with `outcome=skipped` instead of running the command, breaking the
   `docs result → commit ack → docs run` loop a doc worker's own commit
   would otherwise start. `tests/scaffold.rs`'s
   `docs_action_emits_one_result_and_skips_its_own_result` covers this end
   to end, and `docs_action_validates_each_cumulative_ack_ref` checks every
   commit ref on a cumulative ack, not just the first.
3. Otherwise, snapshots every file's content hash, runs `docs.command` as
   argv (no shell), and snapshots again. Reports every path whose hash
   changed — created, deleted, or modified, including a file that was
   already dirty before the command ran. The active log file and its
   reserved sidecars (lock and checkpoint files) are excluded from both
   snapshots, so a reactor appending to the log while the docs command runs
   never fails the action; every other path, including the rest of
   `.context/`, is still snapshotted and checked.
4. If any changed path falls outside `docs.roots`, fails with
   `outcome=failed` and the exact out-of-scope paths in `detail`; nothing is
   committed. `docs_action_fails_with_the_exact_out_of_scope_paths` covers
   this.
5. If nothing changed, reports `outcome=skipped` with `no documentation
   changes` in `detail`, and does not append a result.
6. On success, reports `outcome=updated`, `paths=<changed>`, and, when run
   under a reactor (`EVENTLOG_LOG` set and an `ack` ref available), appends
   one `result agent=<docs.identity> ref=<ack ref> paths=<changed>` so the
   commit reactor lands the documentation change.

## Tests

`tests/scaffold.rs` covers: a commit that keeps unrelated staged work
intact; a commit that rejects a glob or directory scope overlapping an
already-staged path; a docs run that reports added, deleted, and
already-dirty files under `docs.roots`; a docs run that fails on out-of-scope
paths without committing; a docs run that emits exactly one `result` and
then skips its own resulting ack; and validation of every ref on a
cumulative ack. `tests/commit_command.rs` covers the configured commit
author described above. `tests/docs_snapshot.rs` covers the log-and-sidecar
exclusion and the `skipped` no-change outcome. Run them with:

```sh
cargo test
```

## See also

- [Setup](setup.md) — the `.context/eventlog-setup.toml` `[commit]` and `[docs]` tables `action` reads.
- [Lifecycle](lifecycle.md) — spawning and claiming the agent that runs an action.
- [React command](react-command.md) — `eventlog react`, which invokes an action as its command.
- [`eventlog` command list](eventlog-cli-surface.md)
