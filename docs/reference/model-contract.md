# Model contract: `src/model`

Status: implemented and frozen. `src/model/{event,config,vocab,allow,paths}.rs`
define the log's data types. No `eventlog` command uses them yet — each
command still prints `not implemented` and exits 1 ([command
list](eventlog-cli-surface.md)) — but the signatures below are fixed. A later
task that needs a different signature appends `escalate` instead of changing
these files itself.

## `event` — one log line

```rust
pub struct Event {
    pub seq: u64,
    pub ts: String,
    pub r#type: String,
    pub prev: Option<String>,
    pub by: Option<String>,
    pub agent: Option<String>,
    pub fields: indexmap::IndexMap<String, String>, // every other field, in file order
}
```

| Function | Behavior |
|---|---|
| `Event::parse_line(line: &str) -> Result<Event, ParseError>` | Parses one JSON line. Rejects anything that is not a flat object with an integer `seq`, a string `ts`, and a string `type`. Never panics on bad input. |
| `to_line(&self) -> String` | Serializes back to a line, field order `seq`, `ts`, `type`, `prev`, `by`, `agent`, then the rest in the order they were read. No trailing newline. |
| `seq_ref(&self, field: &str) -> Option<u64>` | Reads a reference field (for example `seq_done`) as the `seq` it names. `None` if the field is missing or not a non-negative integer. |
| `paths(&self) -> Vec<String>` | Splits the `paths` field on `,`. Empty if there is no `paths` field. |
| `writer(&self) -> &str` | The line's writer: `by`, or `"controller"` if `by` is absent. |
| `subject(&self) -> &str` | Whose work the line is about: `agent`, or `"controller"` if `agent` is absent. |

`ParseError { line_hint: String, reason: String }` carries the first 80
characters of the offending line (control characters stripped) so a caller
can name the bad line without echoing the whole thing.

A field on disk is always a JSON string, including reference fields like
`seq_done` — `seq_ref` is what turns `"50"` into `50` for comparison. `seq`
itself is a JSON integer.

## `config` — loading and precedence

```rust
pub struct Config { pub log: LogConfig, pub vocabulary: Vocabulary, pub writers: Allowlist, pub view: ViewConfig, pub keys: KeysConfig }
pub struct LogConfig { pub path: PathBuf, pub fsync: bool, pub named: BTreeMap<String, PathBuf> }

pub fn load(repo_root: &Path) -> anyhow::Result<Config>;
pub fn resolve_log(cfg: &Config, selector: Option<&str>) -> PathBuf;
```

`load` reads `<repo_root>/.context/eventlog.toml` if it exists, otherwise
`~/.config/eventlog/config.toml`, otherwise it returns the built-in defaults.
The two files are alternatives: only the first one found is read, not both.

`resolve_log` turns an `eventlog --log <selector>` value into a path: no
selector returns `cfg.log.path` (the default log); a selector that matches a
key in `[log.named]` returns that log's path; anything else is treated as a
literal path.

## `vocab` — which fields each event type carries

```rust
pub struct TypeSpec { pub fields: Vec<String>, pub optional: Vec<String> }
pub struct Vocabulary(BTreeMap<String, TypeSpec>);

impl Vocabulary {
    pub fn builtin() -> Self;
    pub fn merge_file(&mut self, extra: BTreeMap<String, TypeSpec>);
    pub fn get(&self, ty: &str) -> Option<&TypeSpec>;
    pub fn types(&self) -> Vec<&str>;
}
pub const REFERENCE_FIELDS: &[&str] = &["seq_done", "for", "for_ack", "intent"];
```

`Vocabulary::builtin()` returns the type table documented in
`.context/EVENTLOG.md`, plus `ack`, `note`, `intent`, `veto`, and
`violation`. `merge_file` applies a `[vocabulary.<type>]` table from
`.context/eventlog.toml`: it can add a new type or add fields to an existing
one, but it can never remove a built-in type or a built-in required field.

`REFERENCE_FIELDS` lists the fields that name another line's `seq`. They stay
JSON strings on disk; `Event::seq_ref` is how code reads them as integers.

## `allow` — who may write each event type

```rust
pub struct Allowlist(BTreeMap<String, Vec<String>>);

impl Allowlist {
    pub fn builtin() -> Self;
    pub fn merge_file(&mut self, extra: BTreeMap<String, Vec<String>>);
    pub fn apply_decision(&mut self, value: &str);
    pub fn permits(&self, writer: &str, ty: &str) -> bool;
}
```

`Allowlist::builtin()` lets only `controller` write `spawn`, `prompt`,
`claim`, `decision`, `retire`, and `approval`; any `by=`-tagged writer may
write `ack`, `note`, `escalate`, `violation`, `intent`, `veto`, `result`,
`progress`, `message`, `drain`, and `seam`.

`merge_file` applies the `[writers]` table from the config file: each type
the file names gets exactly the writers the file lists, replacing that
type's default; other types keep their default.

`apply_decision` applies one `decision key=log-writers value=...` line.
`controller-plus-reactors` restores the built-in table. Any other value has
the form `name:type1|type2;name2:type3` and **replaces the whole map** —
revoking a writer means a later decision that omits it, not an edit to this
one. Whatever the value says, `controller` always keeps `spawn`, `prompt`,
`claim`, `decision`, `retire`, and `approval`.

`permits(writer, ty)` returns `false` for a type the allowlist has no entry
for.

Applying `merge_file` (config) and `apply_decision` (log) in that order,
oldest decision first, evaluates the allowlist as of each line's own `seq` —
see [why the model contract locks this order](../explanation/model-contract-precedence.md).

## `paths` — the `paths=` field

```rust
pub struct RelPath(String);
pub fn validate_paths(field: &str) -> Result<Vec<RelPath>, PathError>;
pub fn canonicalize(root: &Path, p: &RelPath) -> Result<PathBuf, PathError>;
pub fn is_log_or_lock(cfg_log: &Path, p: &Path) -> bool;
```

`validate_paths` splits a `paths=` field on `,` and checks each entry: no
empty entries, no absolute paths, and no path that escapes the repo root
after resolving `..` (an interior `a/b/../c.rs` is fine; a leading `../x` is
not). It returns a `PathError` naming the entry that failed, and never
touches the filesystem.

`canonicalize` resolves a validated path under a repo root and rejects any
path component that is a symlink, so a claim cannot be widened by pointing a
link outside the repo. A component that does not exist yet is not treated as
an error — only an existing symlink is rejected.

`is_log_or_lock` reports whether a path is the configured log itself or one
of its sidecar files (`<log>.lock`, `<log>.commit.reactor.lock`, and
similar) — the ones nothing but the log writer may touch.

## Tests

20 tests across `tests/model_event.rs`, `tests/model_allow.rs`, and
`tests/model_paths.rs` cover parsing, round-trip serialization, writer
allowlists, config precedence, and repo-relative path rules. Run them with:

```sh
cargo test
```

## See also

- [Command list](eventlog-cli-surface.md) — no command uses this contract yet.
- [Why the model layer is frozen as a contract](../explanation/model-contract-precedence.md)
