# event-log-coordination

A skill for coordinating several coding agents through one append-only JSONL
event log, driven by the `eventlog` CLI. One controller writes it. Every
agent and every human reads it. The log works alone. Reactors, which act on it
and record their own acks, are an optional add-on: the separate
`eventlog-reactors` binary.

| Path | What it is |
|---|---|
| `SKILL.md` | Command shapes, the rules `append` enforces, worker lifecycle, reading the log, and the optional reactors |
| `references/log-reactors.md` | Optional `eventlog-reactors`: what the runtime does per event, start and resume rules, sanctioning writers |
| `references/reactor-example.md` | Optional `eventlog-reactors`: the packaged commit action driven by `result` |
| `references/herdr-integration.md` | herdr actions mapped to events, and where an optional reactor runs |

The binary embeds this directory. `eventlog skill install` writes it to
`~/.claude/skills` (`--dir` for another root, `--force` to overwrite a newer
stamp). `eventlog doctor --fix` does the same install.

## Quick start

```sh
eventlog doctor --fix
eventlog init
eventlog append spawn  agent=t2 model=sonnet role=impl
eventlog append claim  agent=t2 paths=src/api,docs/index.md
eventlog append prompt agent=t2 ref=.context/handoffs/t2.md
eventlog claims t2 main
eventlog append result agent=t2 ref=.context/handoffs/t2.md paths=src/api/ping.rs summary="add ping"
eventlog append retire agent=t2 disposition=accepted
eventlog view -f
```

## Enforcement

- `eventlog guard` is a tool-call hook for Claude Code, Cursor and Codex. It
  denies an edit or write on the log, a mutating shell shape naming it, and
  any compound command naming it. Reads run alone pass.
- `eventlog protect` sets the OS append-only flag (`chflags uappnd`,
  `chattr +a`). Appends still work; truncate, overwrite and rm do not, under
  any agent. Opt-in per log, never run by `doctor --fix`.
- `eventlog verify` walks the hash chain (`prev` on every line).
- `--as` is a declaration, not authentication.
