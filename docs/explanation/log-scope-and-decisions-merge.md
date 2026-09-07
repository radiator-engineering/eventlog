# Why the log stays untracked and `DECISIONS.md` merges as union

## What git tracks under `.context/`

The live log (`.context/events.jsonl`), its writer lock
(`events.jsonl.lock`), the reactor lock directories (`*.reactor.lock/`), and
`layout.json` stay out of git. `eventlog init` already added the first three
to `.gitignore`; `layout.json` joins them because it is herdr's own runtime
state, not a coordination artifact.

Everything the log points at — a `ref=` field — is tracked, one file per
artifact: `DECISIONS.md`, `EVENTLOG.md`, the worker briefs under
`.context/handoffs/`, the reactor action scripts, and `eventlog.toml`. A
clone of the repo gets every artifact the log ever referenced; it does not
get the log itself, which is local, per-clone coordination state.

This follows what the project's own prior-art research recommends: one
writer and no branchable log (the neuroarxiv report's "Avoid" list), small
`ref=` events pointing at tracked files (survey rank 1), and deferring
archiving or snapshots until a log outgrows full replay (event-sourcing
survey, rows 11 and 17). If a log's full history ever needs to survive a
clone, the answer is a frozen archive copy, never a git merge of the log
itself.

## Why `DECISIONS.md` merges with `merge=union`

`.gitattributes` sets:

```
.context/DECISIONS.md merge=union
```

Each entry in `DECISIONS.md` is a dated, self-contained section — one
decision, one rationale, no cross-references that require sequencing. Two
branches that each append a different section have nothing to resolve: a
union merge keeps both, in whatever order git produces, instead of raising a
conflict over adjacent insertions.

## `eventlog.toml` ships with its scaffolded defaults

`eventlog init` scaffolds `.context/eventlog.toml`; this repo keeps the
scaffolded defaults, including `fsync = false`. The
[event-sourcing survey](../../research/README.md) treats fsync as an
optional strict mode, not a requirement — appropriate here because this log
is local coordination state, not a durable system of record.

`eventlog init` used to overwrite a hand-maintained `EVENTLOG.md` or
`eventlog.toml` whenever its content no longer matched the embedded
template — which is every hand-edited copy. `init` now writes each template
only if the file does not already exist (decision `init-templates`), so a
rerun leaves a hand-edited `EVENTLOG.md` or `eventlog.toml` alone; see
[`eventlog init`](../reference/scaffold.md#eventlog-init--create-the-coordination-scaffold).

## See also

- [`eventlog init`](../reference/scaffold.md) — what gets scaffolded and
  what `.gitignore`/`.gitattributes` lines it adds on its own.
- [`.context/DECISIONS.md`](../../.context/DECISIONS.md) — the
  `log-scope` and `eventlog.toml` decision entries this doc explains.
