# Skill install and shell completions: `eventlog skill`, `eventlog completions`

Status: implemented. `src/skill/mod.rs` embeds the `event-log-coordination`
skill in the binary and writes it to disk; `src/cmd/skill.rs` and
`src/cmd/completions.rs` expose that and `clap_complete`'s shell
completions as commands.

## `eventlog skill install` — writing the embedded skill

```sh
eventlog skill install [--dir <path>] [--force]
```

```rust
pub fn default_dir() -> PathBuf;                                   // ~/.claude/skills
pub fn install(dest_root: &Path, force: bool) -> anyhow::Result<PathBuf>;
```

The skill source under `skill/event-log-coordination/` in this repo is
embedded into the binary at compile time (`include_dir!`). `install`
extracts it under `dest_root` (`--dir`, default `~/.claude/skills`), as
`<dest_root>/event-log-coordination/`, then writes a stamp file,
`.eventlog-version`, holding the binary's own `CARGO_PKG_VERSION`. On
success it prints `installed skill to <path>` and exits 0.

Before writing, `install` compares stamps: if `.eventlog-version` already
exists at the destination and its version is numerically newer than the
running binary's, `install` refuses and leaves the existing files alone,
printing `eventlog skill install: installed skill stamp <existing> is
newer than this binary's <new>; use --force to overwrite` and exiting 1.
`--force` skips that check and overwrites regardless of the existing
stamp. Version comparison is numeric per dot-separated component (`0.9.0`
< `0.10.0`), not lexical string comparison, and a leading `v` is stripped
before comparing.

## `eventlog completions` — shell completions

```sh
eventlog completions <shell>
```

`<shell>` is one of the shells `clap_complete::Shell` supports (`bash`,
`elvish`, `fish`, `powershell`, `zsh`). The command builds the `clap`
command tree from `Args` and writes the completion script for `<shell>` to
stdout; there is no `--dir` or install step; pipe the output to wherever
the shell expects completion scripts, for example:

```sh
eventlog completions zsh > ~/.zfunc/_eventlog
```

## Release packaging

This task also added `dist-workspace.toml` (cargo-dist configuration) and
the generated `.github/workflows/release.yml`, so that tagging a release
builds and publishes `eventlog` binaries; it does not change any `eventlog`
subcommand.

## Tests

`tests/skill.rs` covers `skill install` writing the skill and stamp,
refusing a newer existing stamp, `--force` overriding that refusal, and
`completions zsh` producing output that contains `_eventlog`. Run them
with:

```sh
cargo test
```

## See also

- [eventlog CLI surface](eventlog-cli-surface.md) — where `skill` and `completions` sit among the other commands.
