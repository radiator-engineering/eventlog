# Scaffold and setup: `eventlog init`, `doctor`, `protect`

Status: implemented. `src/scaffold/mod.rs` holds `init`, `protect`, and
`is_protected`; `src/scaffold/doctor.rs` holds the `doctor` checks.
`src/cmd/init.rs`, `src/cmd/doctor.rs`, and `src/cmd/protect.rs` expose them
as commands.

## `eventlog init` — create the coordination scaffold

```sh
eventlog init
```

```rust
pub fn init(repo_root: &Path) -> anyhow::Result<()>;
```

`init` creates `.context/`, then:

- Creates `.context/events.jsonl` if it does not already exist. An existing
  log is left alone.
- Writes `.context/EVENTLOG.md` and `.context/eventlog.toml` from templates
  embedded in the binary, only if the file does not already exist. Once a
  file exists, `init` never rewrites it, even if its content no longer
  matches the template — so a hand-edited copy is never reverted on a later
  run (decision [`init-templates`](../../.context/DECISIONS.md)).
- Appends any missing line from a fixed set to `.gitignore`:
  `.context/events.jsonl`, `.context/events.jsonl.lock`,
  `.context/*.reactor.lock/`, `.context/layout.json`.
- Appends `.context/events.jsonl -text` to `.gitattributes` if not already
  present.

Running `init` twice leaves every file byte-for-byte identical to the first
run.

## `eventlog doctor` — diagnose setup problems

```sh
eventlog doctor [--fix] [--protect]
```

```rust
pub struct Options { pub fix: bool, pub protect: bool }
pub fn run(repo_root: &Path, cfg: &Config, opts: &Options) -> anyhow::Result<i32>;
```

`doctor` prints one `[ OK ]`, `[WARN]`, or `[FAIL]` row per check, then exits
1 if any row is `[FAIL]`, 0 otherwise. Checks:

| Row | Status meaning |
|---|---|
| `binary on PATH` | `[WARN]` if `eventlog` is not found on `PATH`. |
| `guard <agent>` (Claude, Cursor, Codex) | `[WARN]` if the agent's hook file is missing, or exists but does not call `eventlog guard`. |
| `protection` | `[WARN]` if the log is not OS-level append-only protected. |
| `malformed lines` | `[FAIL]` if the log has lines that don't parse. |
| `unsanctioned writer` | `[FAIL]` per event whose writer is not permitted by the allowlist folded as of that event's own `seq` (not the log's current tip), so a later `decision key=log-writers` can't retroactively excuse an earlier write. |
| `strict rule` | `[FAIL]` per event that breaks a strict history rule: a `result`/`progress`/`claim`/`retire` from an agent that isn't open at the prior `seq`, a `claim` naming paths that don't exist, or a `claim` on a path another live claim already owns. |
| `reference` | `[FAIL]` per event whose reference field (see `REFERENCE_FIELDS`) points at a `seq` that doesn't exist or isn't earlier than the event itself. |
| `open lifecycles` | `[WARN]` listing any agent or intent still open at the log's tip; `[ OK ]` if none. |
| `reactor lock <name>` | `[WARN]` per `*.reactor.lock` directory whose token is missing, unreadable, or not live (see [reactor lock liveness](../explanation/reactor-lock-liveness.md)). |
| `skill stamp` | `[WARN]` if the installed skill's `.eventlog-version` doesn't match this binary's version, or the skill isn't installed. |
| `old script on PATH` | `[WARN]` per legacy shell script (`append-event.sh`, `eventlog-view.sh`, and others) still resolvable on `PATH`. |

`--fix` runs before the checks: it installs the guard hook for every agent
config directory (`.claude`, `.cursor`, `.codex`) found under the repo root,
removes symlinks to the legacy shell scripts, and installs the skill.
`--protect` also runs before the checks and enables OS-level append-only
protection on the log (equivalent to `eventlog protect`).

## `eventlog protect` — OS-level append-only protection

```sh
eventlog protect [--off] [--status]
```

```rust
pub fn protect(log_path: &Path, enable: bool) -> anyhow::Result<()>;
pub fn is_protected(log_path: &Path) -> anyhow::Result<bool>;
```

With no flags, `protect` turns append-only protection **on** for the
configured log (`--log`, or the config default). `--off` turns it off.
`--status` checks the current state instead of changing it: exit 0 if
protected, exit 1 if not.

Protection is OS-level, not a file permission:

- **macOS**: `chflags uappnd` / `chflags nouappnd`; `is_protected` checks
  `stat -f %Sf` for the `uappnd` flag.
- **Linux**: `chattr +a` / `chattr -a`; `is_protected` checks `lsattr` for
  the `a` attribute.
- **Other platforms**: `protect` fails with an error; `is_protected` always
  returns `false`.

With the flag set, the file can still be opened for appending, but no
process — including `rm`, a truncating redirect, or `sed -i` — can shorten
or overwrite it. This is the same protection the `protect-log.sh`
predecessor script offered, now built into the binary.

## Tests

`src/scaffold/mod.rs` has a unit test, `init_keeps_a_hand_edited_template` —
it runs `init`, overwrites `EVENTLOG.md` and `eventlog.toml` with arbitrary
content, runs `init` again, and asserts both files still hold the
overwritten content, not the template.

`tests/scaffold.rs` covers:

- `init_twice_leaves_identical_files` — running `init` twice produces the
  same `.context/events.jsonl`, `EVENTLOG.md`, `eventlog.toml`,
  `.gitignore`, and `.gitattributes`.
- `doctor_flags_unsanctioned_writer_without_grant` — a `claim` from a writer
  never granted by a `decision key=log-writers` fails with `[FAIL]
  unsanctioned writer`.
- `doctor_ok_when_log_writers_grants_the_writer` — the same `claim` passes
  once a prior `decision key=log-writers` grants that writer the `claim`
  type.
- `protect_status_before_and_after_protect` (ignored by default; needs
  `chflags`/`chattr` permission — run with `cargo test --test scaffold --
  --ignored`) — `protect --status` fails before protecting and succeeds
  after.

Run the non-ignored tests with:

```sh
cargo test
```

## See also

- [eventlog CLI surface](eventlog-cli-surface.md) — where `init`, `doctor`, and `protect` sit among the other commands.
- [Reactor lock liveness](../explanation/reactor-lock-liveness.md) — what the `reactor lock` check in `doctor` is verifying.
