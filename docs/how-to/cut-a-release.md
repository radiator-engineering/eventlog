# Cut a release

This guide releases a new version of the two crates in this workspace:
`eventlog-cli` (the `eventlog` binary, in the repo root) and
`eventlog-reactors` (the optional `eventlog-reactors` binary, in
`reactors/`). Both share one version and one tag. It assumes you are on
`main` with nothing left to land.

## 1. Write commits that git-cliff can parse

`CHANGELOG.md` is generated from commit messages by
[git-cliff](https://git-cliff.org), configured in `cliff.toml`. Never edit
`CHANGELOG.md` by hand — the next release regenerates it and your edit is
lost. This repo runs the optional commit reactor, which writes each commit
message from a worker's `result` summary, so write `result` summaries as the
user-facing line you want to read in the changelog.

`cliff.toml` sorts commits into changelog sections by their conventional
commit type:

| Prefix | Changelog section |
|---|---|
| `feat` | Added |
| `fix` | Fixed |
| `perf` | Performance |
| `refactor`, `build`, `react`, `view` | Changed |
| `docs` | Documentation |
| `test`, `style`, `chore`, `ci` | omitted |
| anything else | Other |

## 2. Generate the changelog

```sh
git cliff --tag vX.Y.Z -o CHANGELOG.md
```

This rewrites `CHANGELOG.md`, moving everything currently under
`[Unreleased]` into a new `## [X.Y.Z]` section.

## 3. Bump the version and tag

Update the version in three places, then commit:

- `version` in `Cargo.toml` (`eventlog-cli`);
- `version` in `reactors/Cargo.toml` (`eventlog-reactors`);
- the `eventlog-cli` dependency's `version` in `reactors/Cargo.toml`.
  `eventlog-reactors` depends on `eventlog-cli` by path and version, and
  crates.io resolves the version.

Then tag:

```sh
git tag vX.Y.Z
git push && git push --tags
```

cargo-dist reads the tag and lifts that version's `CHANGELOG.md` section
into the GitHub release notes. The release carries archives and a Homebrew
formula for each binary: `eventlog` and `eventlog-reactors`.

## 4. Publish

Publish `eventlog-cli` first. `eventlog-reactors` cannot publish until the
`eventlog-cli` version it names is on crates.io.

```sh
cargo publish -p eventlog-cli
cargo publish -p eventlog-reactors
```

## Related

- [Decision: changelog is git-cliff-generated](../../.context/DECISIONS.md) — why `CHANGELOG.md` is never hand-edited.
- [`cliff.toml`](../../cliff.toml) — the git-cliff configuration this guide describes.
