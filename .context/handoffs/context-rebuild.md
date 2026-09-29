# Worker brief: context-rebuild

Spawned by the controller. Subagent-driven: one fresh implementer per plan task,
each given its own task brief under `.superpowers/sdd/2026-09-28-context-from-log/`.

- Worktree: `/Users/jjmartin/Development/event-log/.worktrees/context-rebuild`,
  branch `feat/context-rebuild`, from `main` at `bfc3b46`.
- Spec (pinned contract): `docs/superpowers/specs/2026-09-28-context-from-log-design.md`.
- Plan: `docs/superpowers/plans/2026-09-28-context-from-log.md`.
- Claimed paths: `src/context`, `src/cmd/context.rs`, `src/cmd/mod.rs`, `src/cli.rs`,
  `src/lib.rs`, `src/model/vocab.rs`, `src/model/config.rs`, `.context/EVENTLOG.md`,
  `.context/reports`, `mods/eventlog-context`, `tests/context_*`, `tests/model_event.rs`,
  `tests/cli_surface.rs`, `tests/docs_snapshot.rs`, `docs/how-to/rebuild-context-from-the-log.md`,
  `Cargo.toml`, `.claude/skills/eventlog-context`, `src/cmd/append.rs`,
  `src/scaffold/templates/EVENTLOG.md`, `tests/fixtures/context`.

Rules for every implementer:
- Work only inside the worktree and the claimed paths.
- Do not run `git commit`, `git stash`, push, or publish. Do not append to the log.
- Do not spawn agents.
- Report back in your task report file; the controller records `result` events.
