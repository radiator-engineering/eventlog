# Action runner: `src/react/action.rs`

Status: `run`, `snapshot`, `touched`, and `outside` are implemented. Spec
section 7, steps 4.5–4.7. `src/react/mod.rs` calls these functions as the
action and git-violation steps of a reactor's pass over one driving event —
see the `Steps` trait in [`src/react/mod.rs`](../../src/react/mod.rs), not
covered here.

## `run` — step 4.5, running the command

```rust
pub struct ActionEnv {
    pub log: PathBuf,
    pub seq: u64,
    pub r#type: String,
    pub agent: String,
    pub by: String,
    pub paths: Vec<RelPath>,
    pub reference: Option<String>,
    pub resume: u64,
    pub outcome_file: PathBuf,
}

pub struct Outcome {
    pub exit: i32,
    pub fields: Vec<(String, String)>,
    pub timed_out: bool,
}

pub fn run(
    command: &[String],
    stdin_json: &str,
    env: &ActionEnv,
    timeout: Duration,
) -> Result<Outcome>;
```

`run` spawns `command`, writes `stdin_json` to its stdin, and sets one
environment variable per `ActionEnv` field: `EVENTLOG_LOG`, `EVENTLOG_SEQ`,
`EVENTLOG_TYPE`, `EVENTLOG_AGENT`, `EVENTLOG_BY`, `EVENTLOG_PATHS` (the
authorized paths, comma-joined), `EVENTLOG_REF`, `EVENTLOG_RESUME`, and
`EVENTLOG_OUTCOME_FILE`.

If the command runs past `timeout`, `run` kills it and returns
`Outcome.timed_out == true` with empty `fields`. `run` never retries; a
retry on `outcome=retryable` is the loop's job (Task 14), not this
function's.

### Reading the outcome

`Outcome.fields` comes from one of two places:

1. If `EVENTLOG_OUTCOME_FILE` exists after the command exits, `run` parses
   its `key=value` lines.
2. Otherwise, `run` falls back to the last stdout line containing
   `outcome=`, parsed the same way.

Either way, the keys `seq`, `ts`, `prev`, and `by` are dropped — a command
cannot use its outcome to override the fields the loop already owns for
this pass.

## `snapshot`, `touched`, `outside` — steps 4.6–4.7, the violation check

```rust
pub struct Snapshot {
    pub head: Option<String>,
    pub dirty: BTreeSet<String>,
}

pub fn snapshot(root: &Path) -> Result<Snapshot>;
pub fn touched(before: &Snapshot, after: &Snapshot, root: &Path) -> BTreeSet<String>;
pub fn outside(touched: &BTreeSet<String>, authorized: &[RelPath]) -> Vec<String>;
```

`snapshot` reads `git rev-parse HEAD` and `git status --porcelain` under
`root`. Call it once before running the action and once after.

`touched` combines two sources of change between the two snapshots: every
file in `git diff --name-only` between the two `HEAD`s (commits the action
made), plus the symmetric difference of the two `dirty` sets (working-tree
changes the action left uncommitted, in either direction). If `HEAD` did
not move, the commit side is empty.

`outside` filters `touched` down to the paths not covered by `authorized`
— the same authorized-set semantics as `voter::authorize` (literal path,
directory prefix, or glob). A non-empty result is the git violation: the
action touched a file the driving event never authorized.

## Tests

`tests/react_action.rs` covers spec section 7 steps 4.5–4.7: a command that
writes `outcome=committed` and `ref=abc` to the outcome file produces both
fields; a command with no outcome file falls back to the last
`outcome=...` stdout line; a command that outlives its timeout reports
`timed_out`; and, in a temporary git repository, committing two files
while only one is authorized makes `outside` report the other. Run them
with:

```sh
cargo test
```

## See also

- [Rule voter](react-voter.md) — `authorize`, whose authorized-set rules
  `outside` reuses to judge git violations.
- [Model contract](model-contract.md) — `RelPath`, used for `ActionEnv.paths`
  and `outside`'s `authorized` argument.
