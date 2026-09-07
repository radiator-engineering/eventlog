# Cut a release

This guide walks through releasing a new version of `eventlog`, from a
correct commit history to a published crate. It assumes you're on `main`
with nothing left to land.

## 1. Write commits that git-cliff can parse

`CHANGELOG.md` is generated from commit messages by
[git-cliff](https://git-cliff.org), configured in `cliff.toml`. Never edit
`CHANGELOG.md` by hand — the next release regenerates it and your edit is
lost. In this repo, the commit reactor writes each commit message from a
worker's `result` summary, so write `result` summaries as the user-facing
line you want to read in the changelog.

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

Update the version in `Cargo.toml`, commit, then tag:

```sh
git tag vX.Y.Z
git push && git push --tags
```

cargo-dist reads the tag and lifts that version's `CHANGELOG.md` section
into the GitHub release notes.

## 4. Publish

```sh
cargo publish
```

## Related

- [Decision: changelog is git-cliff-generated](../../.context/DECISIONS.md) — why `CHANGELOG.md` is never hand-edited.
- [`cliff.toml`](../../cliff.toml) — the git-cliff configuration this guide describes.
