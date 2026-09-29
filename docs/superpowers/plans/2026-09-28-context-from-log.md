# Context Rebuilt From the Log Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Claude Code's LLM compaction in the controller session with a context packet rendered from `.context/events.jsonl`, plus the last few conversation turns kept word for word.

**Architecture:** Rust owns all log logic: a pure packet builder (`src/context/packet.rs`), a pure rebuild policy (`src/context/check.rs`), and a git working-tree reader (`src/context/worktree.rs`), exposed as `eventlog context`, `eventlog context check` and `eventlog context install`. A thin TypeScript mod (`mods/eventlog-context/`) calls those commands from Claude Code function hooks and replaces the conversation during `session.compact`. The binary embeds the mod and installs it.

**Tech Stack:** Rust 2024 (clap 4, serde_json, chrono, globset, include_dir, assert_cmd), TypeScript against the Claude Code mods API (`claude-code` types, `claude-code/testing`), Claude Code ≥ 2.1.278 with `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.

**Spec:** `docs/superpowers/specs/2026-09-28-context-from-log-design.md`. Read it before any task.

## Global Constraints

- This repo is log-driven (`AGENTS.md`). Nobody runs `git commit`. A worker reports back; the controller runs `eventlog append result ref=<main file> paths=<files> summary="<line>"` and the commit reactor commits. Every "Report" step below means that.
- Never edit, truncate or delete `.context/events.jsonl`. Run any command that names it alone, not in a pipeline or `&&` chain (the guard blocks compound commands).
- Claude Code floor: 2.1.278. Function hooks need `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.
- Packet default budget: 12,000 characters. Settings defaults: `floor_percent = 25`, `backstop_percent = 60`, `keep_turns = 3`, `tail_chars = 40000`, `budget_chars = 12000`.
- The packet is deterministic: same log and working tree give the same bytes. Ages are measured back from the newest event's `ts`, never from the wall clock.
- The packet inlines one file only: the newest open intent's `ref`, capped at `budget_chars`.
- The mod fails open: on any `eventlog` failure it calls `next(e)` and appends `rebuild trigger=<t> reason=engine-fallback`. It never removes the conversation without a replacement.
- The mod subprocess timeout is 5,000 ms.
- `rebuild` vocabulary: `required=[trigger] optional=[as_of, reason, tokens_before, tokens_after, kept_turns]`.
- No new Rust dependencies.
- Prose in docs and help text follows plain technical English: short sentences, active voice, one term per thing.

## Review Focus

1. **Fallback loop.** The mod falls back to engine compaction while the log sits at a boundary. Expected: exactly one compaction, because the fallback appends a `rebuild` event and `check` then answers `mid-task`. Pinned in Task 4 (`boundary_needs_result_after_last_rebuild`) and Task 6 (`fallback appends rebuild`).
2. **Huge task file.** An open intent's `ref` points at a 1 MB file. Expected: the packet inlines `budget_chars` characters, then the truncation line. Pinned in Task 2 (`current_task_is_capped`).
3. **Not a git repository, or `git` missing.** Expected: `eventlog context` still prints a packet; open work says "(working tree unavailable)"; `check` treats the tree as clean. Pinned in Task 3 (`collect_outside_git_is_unavailable`) and Task 5 (`context_outside_git_still_renders`).
4. **Paths with spaces, quotes, or renames.** Expected: the path appears once, unquoted, under its new name. Pinned in Task 3 (`parse_porcelain_z_*`).
5. **Crate packaging drops the mod.** `cargo publish` must ship `mods/eventlog-context/`. Expected: `cargo package --list` shows the mod files. Pinned in Task 7, Step 6.

---

## File Structure

| Path | Responsibility |
|---|---|
| `src/context/mod.rs` | Module root; re-exports. |
| `src/context/age.rs` | `age_between(ts, tip)`: deterministic ages. |
| `src/context/packet.rs` | Pure packet builder: sections, budget, markdown, JSON. |
| `src/context/worktree.rs` | `git status -z` and `git diff --numstat` into `WorkTree`. |
| `src/context/check.rs` | Pure rebuild policy: `decide(...) -> Verdict`. |
| `src/context/install.rs` | Writes the embedded mod; prints the classic hook. |
| `src/cmd/context.rs` | CLI glue for the three commands. |
| `src/cli.rs` | `Context(ContextArgs)`, `ContextInner`, dispatch. |
| `src/model/config.rs` | `[context]` table: `ContextConfig`. |
| `src/model/vocab.rs` | `rebuild` type. |
| `src/lib.rs` | `pub mod context;` |
| `mods/eventlog-context/**` | The mod: manifest, `register.ts`, `policy.ts`, tests. |
| `tests/context_*.rs` | Integration tests. |
| `docs/how-to/rebuild-context-from-the-log.md` | User guide. |
| `.context/reports/context-rebuild-spike.md` | Task 1 findings. |
| `.context/reports/context-rebuild-probes.md` | Task 8 results. |

---

### Task 1: Engine spike (throwaway mod)

This task answers three questions on the real engine before any product code. Nothing built here is kept except the report.

**Files:**
- Create: `<scratchpad>/spike-mod/.claude-plugin/plugin.json`, `<scratchpad>/spike-mod/hooks/hooks.json`, `<scratchpad>/spike-mod/hooks/register.ts` (throwaway, outside the repo)
- Create: `.context/reports/context-rebuild-spike.md`

**Interfaces:**
- Produces: the report's three answers. Task 6 reads answer 2 to choose between a direct call and `$.clock.after`.

- [ ] **Step 1: Write the spike mod**

`plugin.json`:
```json
{ "name": "spike-rebuild", "version": "0.0.1", "description": "Throwaway: tests session.compact replacement" }
```
`hooks.json`:
```json
{ "description": "spike", "modules": ["./register.ts"] }
```
`register.ts`:
```ts
import type { On } from 'claude-code'

export function register(on: On): void {
  on('session.compact', async ($, e, next) => {
    if (e.agentId || e.trigger === 'precompute') return next(e)
    $.ui.log(`spike: compact trigger=${e.trigger} messages=${e.messages.length}`)
    const tail = e.messages.slice(-2)
    return {
      messages: [
        { role: 'user', text: 'SPIKE PACKET: the secret word is heliotrope.', toolUses: [] },
        ...tail,
      ],
    }
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId || e.reason !== 'answer') return result
    const { context } = await $.session.usage()
    $.ui.log(`spike: percent=${context.percent}`)
    if (e.answer.includes('SPIKE-DIRECT')) {
      try {
        await $.session.compact({ instructions: 'spike-direct' })
        $.ui.log('spike: direct compact ok')
      } catch (err) {
        $.ui.log(`spike: direct compact rejected: ${String(err)}`)
      }
    }
    if (e.answer.includes('SPIKE-DEFER')) {
      $.clock.after(0, async () => {
        try {
          await $.session.compact({ instructions: 'spike-defer' })
          $.ui.log('spike: deferred compact ok')
        } catch (err) {
          $.ui.log(`spike: deferred compact rejected: ${String(err)}`)
        }
      })
    }
    return result
  })
}
```

- [ ] **Step 2: Run the three probes**

Run: `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1 claude --plugin-dir <scratchpad>/spike-mod`

1. Type `/compact`. Then ask "What is the secret word?" Expected if a handle-less message works: the answer is "heliotrope".
2. Ask "Reply with exactly SPIKE-DIRECT". Read the transcript log lines. Then start a new session and ask "Reply with exactly SPIKE-DEFER".
3. Copy the spike mod to `.claude/skills/spike-rebuild/` in a scratch git repo that Claude Code trusts. Start `claude` there with the variable set and type `/compact`. Expected if that load path works: the `spike: compact` line appears.

- [ ] **Step 3: Write the report**

Write `.context/reports/context-rebuild-spike.md` with one section per question: the exact log lines seen, and the answer (yes or no):
1. A handle-less packet message is accepted as the first message.
2. `$.session.compact` from `turn.complete`: direct works / deferred works / neither.
3. `.claude/skills/<name>/` loads the mod.

- [ ] **Step 4: Gate**

If question 1 is "no", stop. Report to the user: the spec's core mechanism does not work on this engine version. If question 2 is "neither", Task 6 moves the trigger to `prompt.submit` (spec, Risks item 1); report that before continuing. If question 3 is "no", Task 7 installs to the path the user chooses and prints `claude --plugin-dir <path>`.

- [ ] **Step 5: Report**

`eventlog append result ref=.context/reports/context-rebuild-spike.md paths=.context/reports/context-rebuild-spike.md summary="Spike: engine accepts log-built compaction (findings in report)"`. Delete the scratchpad spike and `.claude/skills/spike-rebuild/` in the scratch repo.

---

### Task 2: `rebuild` vocabulary, `[context]` settings, and the packet builder

**Files:**
- Modify: `src/model/vocab.rs` (builtin table, after the `veto` row)
- Modify: `src/model/config.rs` (`Config`, `FileConfig`, `load`)
- Create: `src/context/mod.rs`, `src/context/age.rs`, `src/context/packet.rs`
- Modify: `src/lib.rs`
- Test: `tests/context_packet.rs`, `tests/model_event.rs` (vocab), `tests/context_config.rs`

**Interfaces:**
- Consumes: `eventlog::query::{fold, State}`, `eventlog::model::event::Event`, `crate::cmd::agents::{active_agents, phase_label, REACTOR_ON}`.
- Produces:
  ```rust
  // src/model/config.rs
  pub struct ContextConfig { pub floor_percent: u8, pub backstop_percent: u8,
      pub keep_turns: u32, pub tail_chars: usize, pub budget_chars: usize }
  // Config gains: pub context: ContextConfig

  // src/context/worktree.rs (types only here; Task 3 fills collect())
  pub struct Change { pub path: String, pub status: String, pub stat: String,
      pub mtime: u64, pub owner: Option<String> }
  pub struct WorkTree { pub available: bool, pub changes: Vec<Change> }

  // src/context/packet.rs
  pub struct PacketInput<'a> { pub events: &'a [Event], pub state: &'a State,
      pub log_path: &'a str, pub work: &'a WorkTree,
      pub read_file: &'a dyn Fn(&str) -> Option<String>,
      pub exists: &'a dyn Fn(&str) -> bool }
  pub struct Section { pub key: &'static str, pub title: &'static str, pub lines: Vec<String> }
  pub struct Packet { pub as_of: u64, pub sections: Vec<Section>, pub over_budget: bool }
  pub fn build(input: &PacketInput, budget: usize) -> Packet
  impl Packet { pub fn markdown(&self) -> String;
      pub fn to_json(&self, cfg: &ContextConfig) -> serde_json::Value }
  // src/context/age.rs
  pub fn age_between(ts: &str, tip: &str) -> String
  ```

- [ ] **Step 1: Write the failing vocab and config tests**

Append to `tests/model_event.rs`:
```rust
#[test]
fn rebuild_type_requires_only_trigger() {
    let vocab = eventlog::model::vocab::Vocabulary::builtin();
    let spec = vocab.get("rebuild").expect("rebuild is built in");
    assert_eq!(spec.fields, vec!["trigger".to_string()]);
    for f in ["as_of", "reason", "tokens_before", "tokens_after", "kept_turns"] {
        assert!(spec.optional.contains(&f.to_string()), "missing optional {f}");
    }
}
```
(If `Vocabulary` has no `get`, use the accessor `src/cmd/vocab.rs` uses to print a type; read that file first.)

Create `tests/context_config.rs`:
```rust
use eventlog::model::config;

fn write_cfg(body: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(dir.path().join(".context/eventlog.toml"), body).unwrap();
    dir
}

#[test]
fn context_defaults() {
    let dir = write_cfg("");
    let cfg = config::load(dir.path()).unwrap();
    assert_eq!(cfg.context.floor_percent, 25);
    assert_eq!(cfg.context.backstop_percent, 60);
    assert_eq!(cfg.context.keep_turns, 3);
    assert_eq!(cfg.context.tail_chars, 40_000);
    assert_eq!(cfg.context.budget_chars, 12_000);
}

#[test]
fn context_table_overrides() {
    let dir = write_cfg("[context]\nfloor_percent = 10\nbudget_chars = 500\n");
    let cfg = config::load(dir.path()).unwrap();
    assert_eq!(cfg.context.floor_percent, 10);
    assert_eq!(cfg.context.budget_chars, 500);
    assert_eq!(cfg.context.backstop_percent, 60);
}

#[test]
fn context_floor_must_be_below_backstop() {
    let dir = write_cfg("[context]\nfloor_percent = 70\nbackstop_percent = 60\n");
    let err = config::load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("floor_percent"), "{err}");
}

#[test]
fn context_unknown_key_is_an_error() {
    let dir = write_cfg("[context]\nkeep_turn = 3\n");
    assert!(config::load(dir.path()).is_err());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test model_event rebuild_type && cargo test --test context_config`
Expected: FAIL (no `rebuild` type; no field `context` on `Config`).

- [ ] **Step 3: Add the vocab row and the config table**

In `src/model/vocab.rs`, after the `veto` row:
```rust
            (
                "rebuild",
                &["trigger"],
                &["as_of", "reason", "tokens_before", "tokens_after", "kept_turns"],
            ),
```
Also add `rebuild` to the module doc's list of types beyond `EVENTLOG.md`, and add a `rebuild` row to `.context/EVENTLOG.md`'s parallel-work table: `| rebuild | trigger, as_of, reason, kept_turns | the controller's context was rebuilt from the log |`.

In `src/model/config.rs`:
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextConfig {
    pub floor_percent: u8,
    pub backstop_percent: u8,
    pub keep_turns: u32,
    pub tail_chars: usize,
    pub budget_chars: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        ContextConfig {
            floor_percent: 25,
            backstop_percent: 60,
            keep_turns: 3,
            tail_chars: 40_000,
            budget_chars: 12_000,
        }
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileContext {
    floor_percent: Option<u8>,
    backstop_percent: Option<u8>,
    keep_turns: Option<u32>,
    tail_chars: Option<usize>,
    budget_chars: Option<usize>,
}
```
Add `pub context: ContextConfig,` to `Config`, and `#[serde(default)] context: Option<FileContext>,` to `FileConfig`. At the end of `load`, before `Ok(cfg)`:
```rust
    if let Some(c) = file.context {
        if let Some(v) = c.floor_percent { cfg.context.floor_percent = v; }
        if let Some(v) = c.backstop_percent { cfg.context.backstop_percent = v; }
        if let Some(v) = c.keep_turns { cfg.context.keep_turns = v; }
        if let Some(v) = c.tail_chars { cfg.context.tail_chars = v; }
        if let Some(v) = c.budget_chars { cfg.context.budget_chars = v; }
    }
    if cfg.context.floor_percent >= cfg.context.backstop_percent {
        anyhow::bail!(
            "{}: [context] floor_percent ({}) must be below backstop_percent ({})",
            path.display(), cfg.context.floor_percent, cfg.context.backstop_percent
        );
    }
```
Run `cargo fmt` after editing.

- [ ] **Step 4: Run them to verify they pass**

Run: `cargo test --test model_event rebuild_type && cargo test --test context_config`
Expected: PASS.

- [ ] **Step 5: Write the failing packet tests**

Create `tests/context_packet.rs`:
```rust
use eventlog::context::packet::{build, PacketInput};
use eventlog::context::worktree::{Change, WorkTree};
use eventlog::model::config::{Config, ContextConfig};
use eventlog::model::event::Event;
use eventlog::query;

fn ev(line: &str) -> Event {
    Event::parse_line(line).unwrap()
}

fn log() -> Vec<Event> {
    vec![
        ev(r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"decision","key":"crate-name","value":"eventlog-cli","ref":".context/DECISIONS.md"}"#),
        ev(r#"{"seq":2,"ts":"2026-09-01T01:00:00Z","type":"spawn","agent":"w1"}"#),
        ev(r#"{"seq":3,"ts":"2026-09-01T01:01:00Z","type":"prompt","agent":"w1","ref":".context/handoffs/w1.md"}"#),
        ev(r#"{"seq":4,"ts":"2026-09-01T01:02:00Z","type":"claim","agent":"w1","paths":"src/a"}"#),
        ev(r#"{"seq":5,"ts":"2026-09-01T02:00:00Z","type":"result","agent":"controller","ref":"src/lib.rs","paths":"src/lib.rs","summary":"add lib"}"#),
        ev(r#"{"seq":6,"ts":"2026-09-01T03:00:00Z","type":"intent","msg":"write the packet","ref":".context/handoffs/task.md"}"#),
    ]
}

fn render(events: &[Event], work: &WorkTree, budget: usize, files: &[(&str, &str)]) -> eventlog::context::packet::Packet {
    let state = query::fold(events, &Config::default());
    let files: Vec<(String, String)> = files.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let read = |p: &str| files.iter().find(|(k, _)| k == p).map(|(_, v)| v.clone());
    let exists = |p: &str| files.iter().any(|(k, _)| k == p);
    build(
        &PacketInput { events, state: &state, log_path: ".context/events.jsonl", work, read_file: &read, exists: &exists },
        budget,
    )
}

fn clean() -> WorkTree {
    WorkTree { available: true, changes: vec![] }
}

#[test]
fn sections_in_attention_order() {
    let p = render(&log(), &clean(), 12_000, &[(".context/handoffs/task.md", "Task body")]);
    let keys: Vec<&str> = p.sections.iter().map(|s| s.key).collect();
    assert_eq!(keys, ["header", "decisions", "agents", "history", "artifacts", "open_work", "current_task"]);
    assert_eq!(p.as_of, 6);
}

#[test]
fn decisions_carry_seq_and_ref() {
    let md = render(&log(), &clean(), 12_000, &[]).markdown();
    assert!(md.contains("- crate-name=eventlog-cli (seq 1, .context/DECISIONS.md)"), "{md}");
}

#[test]
fn agents_show_phase_claims_and_brief() {
    let md = render(&log(), &clean(), 12_000, &[]).markdown();
    assert!(md.contains("- w1 phase=claimed claims=src/a brief=.context/handoffs/w1.md"), "{md}");
}

#[test]
fn history_ages_are_relative_to_the_tip() {
    let md = render(&log(), &clean(), 12_000, &[]).markdown();
    assert!(md.contains("- seq 5 (1h ago) result controller: add lib paths=src/lib.rs"), "{md}");
}

#[test]
fn artifact_index_marks_missing_paths() {
    let md = render(&log(), &clean(), 12_000, &[(".context/DECISIONS.md", "x")]).markdown();
    assert!(md.contains("- .context/DECISIONS.md (seq 1)\n"), "{md}");
    assert!(md.contains("- src/lib.rs (seq 5) (missing)"), "{md}");
}

#[test]
fn open_work_lists_intents_and_changes_with_owner() {
    let work = WorkTree {
        available: true,
        changes: vec![
            Change { path: "src/a/x.rs".into(), status: "M".into(), stat: "+3 -1".into(), mtime: 0, owner: Some("w1".into()) },
            Change { path: "notes.md".into(), status: "??".into(), stat: "new".into(), mtime: 0, owner: None },
        ],
    };
    let md = render(&log(), &work, 12_000, &[]).markdown();
    assert!(md.contains("- intent seq 6 controller: write the packet ref=.context/handoffs/task.md"), "{md}");
    assert!(md.contains("- M src/a/x.rs +3 -1 [claimed by w1]"), "{md}");
    assert!(md.contains("- ?? notes.md new"), "{md}");
}

#[test]
fn working_tree_unavailable_is_said() {
    let work = WorkTree { available: false, changes: vec![] };
    let md = render(&log(), &work, 12_000, &[]).markdown();
    assert!(md.contains("(working tree unavailable)"), "{md}");
}

#[test]
fn current_task_inlines_the_intent_ref_last() {
    let md = render(&log(), &clean(), 12_000, &[(".context/handoffs/task.md", "Task body")]).markdown();
    assert!(md.trim_end().ends_with("Task body"), "{md}");
}

#[test]
fn current_task_is_capped() {
    let big = "x".repeat(1_000_000);
    let md = render(&log(), &clean(), 12_000, &[(".context/handoffs/task.md", &big)]).markdown();
    assert!(md.len() < 30_000, "len {}", md.len());
    assert!(md.contains("(truncated; read `.context/handoffs/task.md` for the rest)"));
}

#[test]
fn budget_drops_history_before_artifacts_and_keeps_decisions() {
    let mut events = log();
    for i in 0..40u64 {
        events.push(ev(&format!(
            r#"{{"seq":{},"ts":"2026-09-01T04:00:00Z","type":"note","msg":"note number {i} with some padding text","ref":"docs/n{i}.md"}}"#,
            7 + i
        )));
    }
    let full = render(&events, &clean(), 1_000_000, &[]);
    // Two history lines are about 150 characters: this forces two or three
    // of the oldest out and no more.
    let small = render(&events, &clean(), full.markdown().len() - 150, &[]);
    assert!(!small.over_budget);
    let hist = |p: &eventlog::context::packet::Packet| p.sections.iter().find(|s| s.key == "history").unwrap().lines.len();
    assert!(hist(&small) < hist(&full));
    let md = small.markdown();
    assert!(md.contains("crate-name=eventlog-cli"));
    assert!(md.contains("write the packet"));
    // History is the last 15 of 42: notes 25..39. The oldest go first.
    assert!(full.markdown().contains("note number 25 "));
    assert!(md.contains("note number 39"));
    assert!(!md.contains("note number 25 "));
    // Artifacts are untouched while history still has lines to drop.
    let arts = |p: &eventlog::context::packet::Packet| p.sections.iter().find(|s| s.key == "artifacts").unwrap().lines.len();
    assert_eq!(arts(&small), arts(&full));
}

#[test]
fn over_budget_when_kept_sections_alone_exceed_it() {
    let p = render(&log(), &clean(), 50, &[]);
    assert!(p.over_budget);
}

#[test]
fn same_input_same_bytes() {
    let a = render(&log(), &clean(), 12_000, &[]).markdown();
    let b = render(&log(), &clean(), 12_000, &[]).markdown();
    assert_eq!(a, b);
}

#[test]
fn empty_log_renders() {
    let p = render(&[], &clean(), 12_000, &[]);
    assert_eq!(p.as_of, 0);
    assert!(p.markdown().contains("- (none)"));
}

#[test]
fn json_carries_markdown_and_settings() {
    let p = render(&log(), &clean(), 12_000, &[]);
    let v = p.to_json(&ContextConfig::default());
    assert_eq!(v["v"], 1);
    assert_eq!(v["as_of"], 6);
    assert_eq!(v["settings"]["keep_turns"], 3);
    assert_eq!(v["settings"]["tail_chars"], 40_000);
    assert_eq!(v["markdown"].as_str().unwrap(), p.markdown());
    assert!(v["sections"]["decisions"].is_array());
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `cargo test --test context_packet`
Expected: FAIL to compile (`eventlog::context` does not exist).

- [ ] **Step 7: Write the module**

`src/lib.rs`: add `pub mod context;` after `pub mod cmd;`.

`src/context/mod.rs`:
```rust
//! The controller's context, rendered from the log (spec
//! `docs/superpowers/specs/2026-09-28-context-from-log-design.md`).

pub mod age;
pub mod packet;
pub mod worktree;
```

`src/context/age.rs`:
```rust
//! Ages measured back from the newest event, so a packet is deterministic.

use chrono::{DateTime, Utc};

/// How long before `tip` the time `ts` was: `0s`, `42s`, `5m`, `3h`, `2d`.
/// `?` when either time does not parse.
pub fn age_between(ts: &str, tip: &str) -> String {
    let (Ok(t), Ok(tip)) = (DateTime::parse_from_rfc3339(ts), DateTime::parse_from_rfc3339(tip)) else {
        return "?".into();
    };
    let secs = tip.with_timezone(&Utc).signed_duration_since(t.with_timezone(&Utc)).num_seconds().max(0);
    match secs {
        s if s >= 86_400 => format!("{}d", s / 86_400),
        s if s >= 3_600 => format!("{}h", s / 3_600),
        s if s >= 60 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}
```

`src/context/worktree.rs` (types now; Task 3 adds `collect`):
```rust
//! Uncommitted changes outside `.context/`, read from git.

/// One changed path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    /// Porcelain status: `M`, `A`, `D`, `R`, `??`.
    pub status: String,
    /// `+added -removed`, `new` for untracked, `binary`, or empty.
    pub stat: String,
    /// Modification time in Unix seconds; 0 for a deleted file.
    pub mtime: u64,
    /// The agent whose open claim covers the path, other than the controller.
    pub owner: Option<String>,
}

/// The working tree as the packet and `check` see it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkTree {
    /// False outside a git repository or when git fails.
    pub available: bool,
    pub changes: Vec<Change>,
}
```

`src/context/packet.rs`:
```rust
//! The context packet: a pure render of the log, state, and working tree.

use serde_json::{Map, Value, json};

use crate::cmd::agents::{REACTOR_ON, active_agents, phase_label};
use crate::context::age::age_between;
use crate::context::worktree::WorkTree;
use crate::model::config::ContextConfig;
use crate::model::event::Event;
use crate::query::State;

const HISTORY_TYPES: &[&str] = &["result", "decision", "violation", "observed", "note"];
const HISTORY_LEN: usize = 15;
const STALE_ACK_SECS: i64 = 86_400;

pub struct PacketInput<'a> {
    pub events: &'a [Event],
    pub state: &'a State,
    pub log_path: &'a str,
    pub work: &'a WorkTree,
    pub read_file: &'a dyn Fn(&str) -> Option<String>,
    pub exists: &'a dyn Fn(&str) -> bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub key: &'static str,
    pub title: &'static str,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub as_of: u64,
    pub sections: Vec<Section>,
    pub over_budget: bool,
}

pub fn build(input: &PacketInput, budget: usize) -> Packet {
    let tip = input.events.iter().max_by_key(|e| e.seq);
    let as_of = tip.map(|e| e.seq).unwrap_or(0);
    let tip_ts = tip.map(|e| e.ts.clone()).unwrap_or_default();

    let history: Vec<&Event> = {
        let mut h: Vec<&Event> = input
            .events
            .iter()
            .filter(|e| HISTORY_TYPES.contains(&e.r#type.as_str()))
            .collect();
        let skip = h.len().saturating_sub(HISTORY_LEN);
        h.drain(..skip);
        h
    };

    let mut sections = vec![
        Section { key: "header", title: "", lines: header(input.log_path, as_of) },
        Section { key: "decisions", title: "Decisions in force", lines: decisions(input) },
        Section { key: "agents", title: "Agents", lines: agents(input) },
        Section { key: "history", title: "Recent history", lines: history.iter().map(|e| history_line(e, &tip_ts)).collect() },
        Section { key: "artifacts", title: "Artifact index", lines: artifacts(&history, input.exists) },
    ];
    let reactors = reactor_health(input, &tip_ts);
    if !reactors.is_empty() {
        sections.push(Section { key: "reactors", title: "Reactor health", lines: reactors });
    }
    sections.push(Section { key: "open_work", title: "Open work", lines: open_work(input) });
    if let Some(lines) = current_task(input, budget) {
        sections.push(Section { key: "current_task", title: "Current task", lines });
    }
    for s in &mut sections {
        if s.lines.is_empty() && s.key != "header" {
            s.lines.push("- (none)".into());
        }
    }

    let mut packet = Packet { as_of, sections, over_budget: false };
    packet.fit(budget);
    packet
}

impl Packet {
    pub fn markdown(&self) -> String {
        let mut out = String::from("# Context rebuilt from the event log\n\n");
        for s in &self.sections {
            if !s.title.is_empty() {
                out.push_str(&format!("## {}\n\n", s.title));
            }
            for line in &s.lines {
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        out
    }

    pub fn to_json(&self, cfg: &ContextConfig) -> Value {
        let mut sections = Map::new();
        for s in &self.sections {
            sections.insert(s.key.into(), json!(s.lines));
        }
        json!({
            "v": 1,
            "as_of": self.as_of,
            "over_budget": self.over_budget,
            "markdown": self.markdown(),
            "sections": sections,
            "settings": { "keep_turns": cfg.keep_turns, "tail_chars": cfg.tail_chars },
        })
    }

    /// Drop the oldest history lines, then the oldest artifact lines, until
    /// the markdown fits. Nothing else is ever dropped.
    fn fit(&mut self, budget: usize) {
        while self.markdown().len() > budget {
            if self.drop_first_line("history") || self.drop_last_line("artifacts") {
                continue;
            }
            self.over_budget = true;
            return;
        }
    }

    fn drop_first_line(&mut self, key: &str) -> bool {
        let Some(s) = self.sections.iter_mut().find(|s| s.key == key) else { return false };
        if s.lines.first().is_some_and(|l| l != "- (none)") {
            s.lines.remove(0);
            if s.lines.is_empty() {
                s.lines.push("- (none)".into());
            }
            return true;
        }
        false
    }

    fn drop_last_line(&mut self, key: &str) -> bool {
        let Some(s) = self.sections.iter_mut().find(|s| s.key == key) else { return false };
        if s.lines.last().is_some_and(|l| l != "- (none)") {
            s.lines.pop();
            if s.lines.is_empty() {
                s.lines.push("- (none)".into());
            }
            return true;
        }
        false
    }
}

fn header(log_path: &str, as_of: u64) -> Vec<String> {
    vec![
        format!("Rebuilt from `{log_path}` as of seq {as_of}. The log is the source of truth."),
        "Read an event's artifact with `eventlog open <seq>`. Read recent events with `eventlog view --last 20`.".into(),
    ]
}

fn field<'a>(e: &'a Event, k: &str) -> Option<&'a str> {
    e.fields.get(k).map(String::as_str)
}

fn decisions(input: &PacketInput) -> Vec<String> {
    input
        .state
        .decisions
        .iter()
        .map(|(k, (v, seq))| {
            let r = input.events.iter().find(|e| e.seq == *seq).and_then(|e| field(e, "ref"));
            match r {
                Some(r) => format!("- {k}={v} (seq {seq}, {r})"),
                None => format!("- {k}={v} (seq {seq})"),
            }
        })
        .collect()
}

fn agents(input: &PacketInput) -> Vec<String> {
    active_agents(input.state)
        .into_iter()
        .map(|a| {
            let brief = input
                .events
                .iter()
                .rev()
                .find(|e| e.r#type == "prompt" && e.agent.as_deref() == Some(a.name.as_str()))
                .and_then(|e| field(e, "ref"))
                .unwrap_or("-");
            format!("- {} phase={} claims={} brief={}", a.name, phase_label(a.phase), a.claims.join(","), brief)
        })
        .collect()
}

fn history_line(e: &Event, tip_ts: &str) -> String {
    let who = e.agent.as_deref().unwrap_or_else(|| e.writer());
    let what = field(e, "summary")
        .or_else(|| field(e, "msg"))
        .map(str::to_string)
        .or_else(|| Some(format!("{}={}", field(e, "key")?, field(e, "value")?)))
        .or_else(|| field(e, "paths").map(|p| format!("paths={p}")))
        .unwrap_or_default();
    let mut line = format!("- seq {} ({} ago) {} {}: {}", e.seq, age_between(&e.ts, tip_ts), e.r#type, who, what);
    if e.r#type == "result" {
        if let Some(p) = field(e, "paths") {
            line.push_str(&format!(" paths={p}"));
        }
    }
    line
}

fn artifacts(history: &[&Event], exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    let mut lines = Vec::new();
    for e in history.iter().rev() {
        let Some(r) = field(e, "ref") else { continue };
        if seen.contains(&r) {
            continue;
        }
        seen.push(r);
        let missing = if exists(r) { "" } else { " (missing)" };
        lines.push(format!("- {r} (seq {}){missing}", e.seq));
    }
    lines
}

fn reactor_health(input: &PacketInput, tip_ts: &str) -> Vec<String> {
    use chrono::DateTime;
    let secs_before_tip = |ts: &str| -> Option<i64> {
        let t = DateTime::parse_from_rfc3339(ts).ok()?;
        let tip = DateTime::parse_from_rfc3339(tip_ts).ok()?;
        Some(tip.signed_duration_since(t).num_seconds())
    };
    input
        .state
        .reactors
        .values()
        .filter_map(|r| {
            let unacked = input.state.unacked(&r.name, REACTOR_ON, input.events).len();
            let stale = r.last_ack_ts.as_deref().and_then(secs_before_tip).is_some_and(|s| s > STALE_ACK_SECS);
            (unacked > 0 || stale).then(|| {
                let age = r.last_ack_ts.as_deref().map(|t| age_between(t, tip_ts)).unwrap_or_else(|| "-".into());
                format!("- {} unacked={} last_ack={} ago", r.name, unacked, age)
            })
        })
        .collect()
}

fn open_work(input: &PacketInput) -> Vec<String> {
    let mut lines = Vec::new();
    for e in &input.state.intents {
        let mut l = format!("- intent seq {} {}: {}", e.seq, e.writer(), field(e, "msg").unwrap_or(""));
        if let Some(p) = field(e, "paths") {
            l.push_str(&format!(" paths={p}"));
        }
        if let Some(r) = field(e, "ref") {
            l.push_str(&format!(" ref={r}"));
        }
        lines.push(l);
    }
    for e in &input.state.escalations {
        let what = field(e, "subject").or_else(|| field(e, "msg")).unwrap_or("");
        lines.push(format!("- escalation seq {}: {what}", e.seq));
    }
    if !input.work.available {
        lines.push("- (working tree unavailable)".into());
    }
    for c in &input.work.changes {
        let mut l = format!("- {} {} {}", c.status, c.path, c.stat).trim_end().to_string();
        if let Some(o) = &c.owner {
            l.push_str(&format!(" [claimed by {o}]"));
        }
        lines.push(l);
    }
    lines
}

fn current_task(input: &PacketInput, budget: usize) -> Option<Vec<String>> {
    let r = input.state.intents.iter().rev().find_map(|e| field(e, "ref"))?;
    let Some(text) = (input.read_file)(r) else {
        return Some(vec![format!("- {r} (missing)")]);
    };
    let mut lines = vec![format!("From `{r}`:"), String::new()];
    if text.len() > budget {
        let cut = text.char_indices().map(|(i, _)| i).take_while(|i| *i <= budget).last().unwrap_or(0);
        lines.extend(text[..cut].lines().map(str::to_string));
        lines.push(format!("(truncated; read `{r}` for the rest)"));
    } else {
        lines.extend(text.lines().map(str::to_string));
    }
    Some(lines)
}
```

Note: `current_task` caps at `budget` and the packet total then exceeds the budget by at most the other kept sections; `fit` marks that `over_budget` and the command warns (Task 5).

- [ ] **Step 8: Run the packet tests to verify they pass**

Run: `cargo test --test context_packet`
Expected: PASS. If `history_ages_are_relative_to_the_tip` fails on the `who` value, check `Event::writer()` returns `controller` for a line with no `by`.

- [ ] **Step 9: Run the full suite and clippy**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS, no warnings. `tests/docs_snapshot.rs` may fail if it snapshots `eventlog vocab`; update that snapshot to include the `rebuild` row.

- [ ] **Step 10: Report**

`eventlog append result ref=src/context/packet.rs paths=src/lib.rs,src/context/mod.rs,src/context/age.rs,src/context/packet.rs,src/context/worktree.rs,src/model/vocab.rs,src/model/config.rs,.context/EVENTLOG.md,tests/context_packet.rs,tests/context_config.rs,tests/model_event.rs summary="Add the context packet builder, the rebuild event type, and [context] settings"` (add the docs snapshot file if Step 9 changed it).

---

### Task 3: Working tree reader

**Files:**
- Modify: `src/context/worktree.rs`
- Modify: `src/cmd/claims.rs` (make `covers` `pub(crate)`) only if `State::claim_owner` is not usable; it is (`src/query/mod.rs:352`), so no change is expected.
- Test: `tests/context_worktree.rs`

**Interfaces:**
- Consumes: `State::claim_owner(&self, path: &str) -> Option<&str>`.
- Produces:
  ```rust
  pub fn parse_porcelain_z(raw: &[u8]) -> Vec<(String, String)>   // (status, path)
  pub fn parse_numstat(raw: &str) -> std::collections::BTreeMap<String, String> // path -> "+a -r"
  pub fn collect(root: &std::path::Path, state: &State) -> WorkTree
  ```

- [ ] **Step 1: Write the failing tests**

Create `tests/context_worktree.rs`:
```rust
use eventlog::context::worktree::{collect, parse_numstat, parse_porcelain_z};
use eventlog::model::config::Config;
use eventlog::model::event::Event;
use eventlog::query;
use std::process::Command;

#[test]
fn parse_porcelain_z_plain_and_untracked() {
    let raw = b" M src/a.rs\0?? notes.md\0";
    assert_eq!(
        parse_porcelain_z(raw),
        vec![("M".into(), "src/a.rs".into()), ("??".into(), "notes.md".into())]
    );
}

#[test]
fn parse_porcelain_z_spaces_are_unquoted() {
    let raw = b"?? my file.md\0";
    assert_eq!(parse_porcelain_z(raw), vec![("??".into(), "my file.md".into())]);
}

#[test]
fn parse_porcelain_z_rename_keeps_new_name() {
    // -z rename: "R  new\0old\0"
    let raw = b"R  src/new.rs\0src/old.rs\0 M x.rs\0";
    assert_eq!(
        parse_porcelain_z(raw),
        vec![("R".into(), "src/new.rs".into()), ("M".into(), "x.rs".into())]
    );
}

#[test]
fn parse_numstat_reads_counts_and_binary() {
    let map = parse_numstat("3\t1\tsrc/a.rs\n-\t-\timg.png\n");
    assert_eq!(map["src/a.rs"], "+3 -1");
    assert_eq!(map["img.png"], "binary");
}

#[test]
fn collect_outside_git_is_unavailable() {
    let dir = tempfile::TempDir::new().unwrap();
    let state = query::fold(&[], &Config::default());
    let wt = collect(dir.path(), &state);
    assert!(!wt.available);
    assert!(wt.changes.is_empty());
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let ok = Command::new("git").args(args).current_dir(dir).status().unwrap().success();
    assert!(ok, "git {args:?}");
}

#[test]
fn collect_skips_context_dir_and_marks_claim_owner() {
    let dir = tempfile::TempDir::new().unwrap();
    let d = dir.path();
    git(d, &["init", "-q"]);
    git(d, &["config", "user.email", "t@t"]);
    git(d, &["config", "user.name", "t"]);
    std::fs::create_dir_all(d.join("src/a")).unwrap();
    std::fs::write(d.join("src/a/x.rs"), "one\n").unwrap();
    git(d, &["add", "."]);
    git(d, &["commit", "-qm", "init"]);
    std::fs::write(d.join("src/a/x.rs"), "one\ntwo\n").unwrap();
    std::fs::create_dir_all(d.join(".context")).unwrap();
    std::fs::write(d.join(".context/scratch.md"), "x").unwrap();
    std::fs::write(d.join("new file.md"), "x").unwrap();

    let events = vec![
        Event::parse_line(r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"spawn","agent":"w1"}"#).unwrap(),
        Event::parse_line(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"claim","agent":"w1","paths":"src/a"}"#).unwrap(),
    ];
    let state = query::fold(&events, &Config::default());
    let wt = collect(d, &state);
    assert!(wt.available);
    let paths: Vec<&str> = wt.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["new file.md", "src/a/x.rs"]);
    let x = wt.changes.iter().find(|c| c.path == "src/a/x.rs").unwrap();
    assert_eq!(x.status, "M");
    assert_eq!(x.stat, "+1 -0");
    assert_eq!(x.owner.as_deref(), Some("w1"));
    assert!(x.mtime > 0);
    let n = wt.changes.iter().find(|c| c.path == "new file.md").unwrap();
    assert_eq!((n.status.as_str(), n.stat.as_str(), n.owner.as_deref()), ("??", "new", None));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test context_worktree`
Expected: FAIL to compile (functions missing).

- [ ] **Step 3: Implement**

Append to `src/context/worktree.rs`:
```rust
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

use crate::query::State;

/// Parse `git status --porcelain=v1 -z`. A rename entry is followed by its
/// old path as a separate NUL-terminated field; keep the new name only.
pub fn parse_porcelain_z(raw: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(raw);
    let mut fields = text.split('\0').filter(|f| !f.is_empty());
    let mut out = Vec::new();
    while let Some(f) = fields.next() {
        if f.len() < 4 {
            continue;
        }
        let code = &f[..2];
        let path = f[3..].to_string();
        let status = if code == "??" { "??".to_string() } else { code.trim().chars().next().unwrap_or('M').to_string() };
        if code.contains('R') || code.contains('C') {
            fields.next(); // the old path
        }
        out.push((status, path));
    }
    out
}

/// Parse `git diff --numstat HEAD`: `added\tremoved\tpath`, `-` for binary.
pub fn parse_numstat(raw: &str) -> BTreeMap<String, String> {
    raw.lines()
        .filter_map(|l| {
            let mut parts = l.splitn(3, '\t');
            let (a, r, p) = (parts.next()?, parts.next()?, parts.next()?);
            let stat = if a == "-" { "binary".to_string() } else { format!("+{a} -{r}") };
            Some((p.to_string(), stat))
        })
        .collect()
}

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git").args(args).current_dir(root).output().ok()?;
    out.status.success().then_some(out.stdout)
}

/// Read the working tree. Never fails: outside a repository, or when git
/// is missing, it answers `available: false`.
pub fn collect(root: &Path, state: &State) -> WorkTree {
    let Some(status) = git(root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]) else {
        return WorkTree { available: false, changes: vec![] };
    };
    let numstat = git(root, &["diff", "--numstat", "HEAD"])
        .map(|b| parse_numstat(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    let mut changes: Vec<Change> = parse_porcelain_z(&status)
        .into_iter()
        .filter(|(_, p)| !p.starts_with(".context/"))
        .map(|(status, path)| {
            let stat = if status == "??" { "new".to_string() } else { numstat.get(&path).cloned().unwrap_or_default() };
            let mtime = std::fs::metadata(root.join(&path))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let owner = state.claim_owner(&path).filter(|o| *o != "controller").map(str::to_string);
            Change { path, status, stat, mtime, owner }
        })
        .collect();
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    WorkTree { available: true, changes }
}
```

- [ ] **Step 4: Run them to verify they pass**

Run: `cargo test --test context_worktree`
Expected: PASS.

- [ ] **Step 5: Report**

`eventlog append result ref=src/context/worktree.rs paths=src/context/worktree.rs,tests/context_worktree.rs summary="Read uncommitted changes and claim owners for the context packet"`

---

### Task 4: Rebuild policy (`check`)

**Files:**
- Create: `src/context/check.rs`
- Modify: `src/context/mod.rs` (`pub mod check;`)
- Test: `tests/context_check.rs`

**Interfaces:**
- Consumes: `ContextConfig`, `WorkTree`, `Change`, `Event`, `State`.
- Produces:
  ```rust
  pub struct Verdict { pub rebuild: bool, pub reason: &'static str }
  pub fn decide(percent: u8, cfg: &ContextConfig, events: &[Event], state: &State, work: &WorkTree) -> Verdict
  impl Verdict { pub fn to_json(&self) -> serde_json::Value }  // {"rebuild":bool,"reason":str}
  ```
  Reasons: `below-floor`, `backstop`, `boundary`, `mid-task`.

- [ ] **Step 1: Write the failing tests**

Create `tests/context_check.rs`:
```rust
use eventlog::context::check::decide;
use eventlog::context::worktree::{Change, WorkTree};
use eventlog::model::config::{Config, ContextConfig};
use eventlog::model::event::Event;
use eventlog::query;

fn ev(l: &str) -> Event { Event::parse_line(l).unwrap() }
const RESULT: &str = r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"result","agent":"controller","ref":"a","summary":"s"}"#;
const RESULT_TS: u64 = 1_788_220_800; // 2026-09-01T00:00:00Z

fn run(percent: u8, events: &[Event], work: &WorkTree) -> (bool, &'static str) {
    let state = query::fold(events, &Config::default());
    let v = decide(percent, &ContextConfig::default(), events, &state, work);
    (v.rebuild, v.reason)
}
fn clean() -> WorkTree { WorkTree { available: true, changes: vec![] } }
fn change(mtime: u64, owner: Option<&str>) -> WorkTree {
    WorkTree { available: true, changes: vec![Change { path: "f".into(), status: "M".into(), stat: String::new(), mtime, owner: owner.map(Into::into) }] }
}

#[test] fn below_floor_never_rebuilds() { assert_eq!(run(24, &[ev(RESULT)], &clean()), (false, "below-floor")); }
#[test] fn backstop_rebuilds_mid_task() {
    let events = [ev(RESULT), ev(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"intent","msg":"m"}"#)];
    assert_eq!(run(60, &events, &clean()), (true, "backstop"));
}
#[test] fn boundary_after_result() { assert_eq!(run(30, &[ev(RESULT)], &clean()), (true, "boundary")); }
#[test] fn no_result_is_mid_task() { assert_eq!(run(30, &[], &clean()), (false, "mid-task")); }
#[test] fn boundary_needs_result_after_last_rebuild() {
    let events = [ev(RESULT), ev(r#"{"seq":2,"ts":"2026-09-01T00:00:02Z","type":"rebuild","trigger":"auto","reason":"engine-fallback"}"#)];
    assert_eq!(run(30, &events, &clean()), (false, "mid-task"));
}
#[test] fn open_controller_intent_blocks_boundary() {
    let events = [ev(RESULT), ev(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"intent","msg":"m"}"#)];
    assert_eq!(run(30, &events, &clean()), (false, "mid-task"));
}
#[test] fn closed_controller_intent_allows_boundary() {
    let events = [
        ev(r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"intent","msg":"m"}"#),
        ev(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"ack","seq_done":"1","outcome":"done","for":"1"}"#),
        ev(r#"{"seq":3,"ts":"2026-09-01T00:00:02Z","type":"result","agent":"controller","ref":"a"}"#),
    ];
    assert_eq!(run(30, &events, &clean()), (true, "boundary"));
}
#[test] fn reactor_intent_does_not_block_boundary() {
    let events = [ev(RESULT), ev(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"intent","by":"cursor-committer","for":"1"}"#)];
    assert_eq!(run(30, &events, &clean()), (true, "boundary"));
}
#[test] fn unreported_change_blocks_boundary() {
    assert_eq!(run(30, &[ev(RESULT)], &change(RESULT_TS + 60, None)), (false, "mid-task"));
}
#[test] fn change_older_than_result_is_reported() {
    assert_eq!(run(30, &[ev(RESULT)], &change(RESULT_TS - 60, None)), (true, "boundary"));
}
#[test] fn change_under_other_claim_is_not_ours() {
    assert_eq!(run(30, &[ev(RESULT)], &change(RESULT_TS + 60, Some("w1"))), (true, "boundary"));
}
#[test] fn unavailable_tree_counts_as_clean() {
    assert_eq!(run(30, &[ev(RESULT)], &WorkTree { available: false, changes: vec![] }), (true, "boundary"));
}
#[test] fn result_by_a_reactor_is_not_a_controller_result() {
    let events = [ev(r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"result","by":"doc-worker","agent":"doc-worker","ref":"a"}"#)];
    assert_eq!(run(30, &events, &clean()), (false, "mid-task"));
}
```
Before running, confirm the `ack ... for=` shape closes an intent by reading `src/query/mod.rs:150-180`; adjust the `closed_controller_intent_allows_boundary` fixture to the fields `eventlog vocab` requires for `ack`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test context_check`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`src/context/check.rs`:
```rust
//! When to rebuild: the log's own task boundaries, with a fill backstop.

use chrono::DateTime;
use serde_json::{Value, json};

use crate::context::worktree::WorkTree;
use crate::model::config::ContextConfig;
use crate::model::event::Event;
use crate::query::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub rebuild: bool,
    pub reason: &'static str,
}

impl Verdict {
    pub fn to_json(&self) -> Value {
        json!({ "rebuild": self.rebuild, "reason": self.reason })
    }
}

pub fn decide(percent: u8, cfg: &ContextConfig, events: &[Event], state: &State, work: &WorkTree) -> Verdict {
    if percent < cfg.floor_percent {
        return Verdict { rebuild: false, reason: "below-floor" };
    }
    if percent >= cfg.backstop_percent {
        return Verdict { rebuild: true, reason: "backstop" };
    }
    if at_boundary(events, state, work) {
        return Verdict { rebuild: true, reason: "boundary" };
    }
    Verdict { rebuild: false, reason: "mid-task" }
}

fn at_boundary(events: &[Event], state: &State, work: &WorkTree) -> bool {
    let last_rebuild = events.iter().filter(|e| e.r#type == "rebuild").map(|e| e.seq).max().unwrap_or(0);
    let Some(result) = events
        .iter()
        .filter(|e| e.r#type == "result" && e.writer() == "controller")
        .max_by_key(|e| e.seq)
    else {
        return false;
    };
    if result.seq <= last_rebuild {
        return false;
    }
    if state.intents.iter().any(|e| e.writer() == "controller") {
        return false;
    }
    let Ok(result_ts) = DateTime::parse_from_rfc3339(&result.ts) else {
        return false;
    };
    let result_secs = result_ts.timestamp().max(0) as u64;
    !work.changes.iter().any(|c| c.owner.is_none() && c.mtime > result_secs)
}
```
Add `pub mod check;` to `src/context/mod.rs`.

- [ ] **Step 4: Run them to verify they pass**

Run: `cargo test --test context_check`
Expected: PASS.

- [ ] **Step 5: Report**

`eventlog append result ref=src/context/check.rs paths=src/context/check.rs,src/context/mod.rs,tests/context_check.rs summary="Decide when to rebuild context: at log task boundaries, with a fill backstop"`

---

### Task 5: `eventlog context` and `eventlog context check` commands

**Files:**
- Modify: `src/cli.rs` (enum variant, args, dispatch)
- Create: `src/cmd/context.rs`
- Modify: `src/cmd/mod.rs` (`pub mod context;`)
- Test: `tests/context_cmd.rs`, `tests/cli_surface.rs` (if it lists commands)

**Interfaces:**
- Consumes: `packet::build`, `check::decide`, `worktree::collect`, `crate::cmd::agents::load_context`.
- Produces: CLI `eventlog context [--budget N] [--json]`, `eventlog context check --percent N` (always JSON), `eventlog context install [--classic] [--force]` (Task 7 fills `install`).

- [ ] **Step 1: Write the failing tests**

Create `tests/context_cmd.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn repo_with_log(lines: &[&str]) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(dir.path().join(".context/events.jsonl"), lines.join("\n") + "\n").unwrap();
    dir
}

const LOG: &[&str] = &[
    r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"decision","key":"k","value":"v"}"#,
    r#"{"seq":2,"ts":"2026-09-01T01:00:00Z","type":"result","agent":"controller","ref":"a.md","summary":"did a"}"#,
];

#[test]
fn context_outside_git_still_renders() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog").unwrap().arg("context").current_dir(dir.path()).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let md = String::from_utf8_lossy(&out.stdout);
    assert!(md.starts_with("# Context rebuilt from the event log"));
    assert!(md.contains("- k=v (seq 1)"));
    assert!(md.contains("(working tree unavailable)"));
}

#[test]
fn context_json_has_settings() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog").unwrap().args(["context", "--json"]).current_dir(dir.path()).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["as_of"], 2);
    assert_eq!(v["settings"]["keep_turns"], 3);
}

#[test]
fn check_prints_json_verdict() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog").unwrap().args(["context", "check", "--percent", "30"]).current_dir(dir.path()).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v, serde_json::json!({"rebuild": true, "reason": "boundary"}));
}

#[test]
fn check_rejects_percent_over_100() {
    let dir = repo_with_log(LOG);
    Command::cargo_bin("eventlog").unwrap().args(["context", "check", "--percent", "101"]).current_dir(dir.path()).assert().failure();
}

#[test]
fn over_budget_warns_on_stderr() {
    let dir = repo_with_log(LOG);
    let out = Command::cargo_bin("eventlog").unwrap().args(["context", "--budget", "50"]).current_dir(dir.path()).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("over budget"));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test context_cmd`
Expected: FAIL (`unrecognized subcommand 'context'`).

- [ ] **Step 3: Wire the CLI**

In `src/cli.rs`, add after `Skill(SkillArgs),`:
```rust
    /// Render the controller's context from the log, or decide when to rebuild it.
    Context(ContextArgs),
```
Add the args:
```rust
#[derive(ClapArgs, Debug)]
pub struct ContextArgs {
    /// Packet budget in characters (default: [context] budget_chars).
    #[arg(long)]
    pub budget: Option<usize>,
    #[command(subcommand)]
    pub inner: Option<ContextInner>,
}

#[derive(Subcommand, Debug)]
pub enum ContextInner {
    /// Print {"rebuild": bool, "reason": text} for the given context fill.
    Check {
        /// Context window fill, 0 to 100.
        #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100))]
        percent: u8,
    },
    /// Install the eventlog-context mod, or print the classic hook.
    Install {
        /// Print the classic SessionStart hook instead of writing the mod.
        #[arg(long)]
        classic: bool,
        /// Write the mod even when settings.json already has the classic hook.
        #[arg(long)]
        force: bool,
    },
}
```
In `run()`'s match: `Command::Context(_) => crate::cmd::context::run(&args),`. Add `pub mod context;` to `src/cmd/mod.rs`.

`src/cmd/context.rs`:
```rust
//! `eventlog context`: the controller's context rendered from the log.

use std::io::Write;

use anyhow::Context as _;

use crate::cli::{Args, Command, ContextInner};
use crate::cmd::agents::load_context;
use crate::context::{check, packet, worktree};

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Context(ctx_args) = &args.command else {
        anyhow::bail!("context::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    let q = load_context(args, None)?;
    let work = worktree::collect(&root, &q.state);
    match &ctx_args.inner {
        Some(ContextInner::Check { percent }) => {
            let v = check::decide(*percent, &q.cfg.context, &q.events, &q.state, &work);
            println!("{}", v.to_json());
            Ok(0)
        }
        Some(ContextInner::Install { classic, force }) => crate::context::install::run(&root, *classic, *force),
        None => {
            let budget = ctx_args.budget.unwrap_or(q.cfg.context.budget_chars);
            let log_path = q.cfg.log.path.display().to_string();
            let read = |p: &str| std::fs::read_to_string(root.join(p)).ok();
            let exists = |p: &str| root.join(p).exists();
            let p = packet::build(
                &packet::PacketInput { events: &q.events, state: &q.state, log_path: &log_path, work: &work, read_file: &read, exists: &exists },
                budget,
            );
            if p.over_budget {
                eprintln!("eventlog: context packet is over budget ({budget} chars); kept sections were not cut");
            }
            let mut out = std::io::stdout().lock();
            if args.json {
                writeln!(out, "{}", p.to_json(&q.cfg.context))?;
            } else {
                write!(out, "{}", p.markdown())?;
            }
            Ok(0)
        }
    }
}
```
Task 7 creates `src/context/install.rs`. Until then, add a stub so this compiles, in `src/context/install.rs`:
```rust
pub fn run(_root: &std::path::Path, _classic: bool, _force: bool) -> anyhow::Result<i32> {
    anyhow::bail!("context install arrives in Task 7")
}
```
and `pub mod install;` in `src/context/mod.rs`.

- [ ] **Step 4: Run them to verify they pass**

Run: `cargo test --test context_cmd && cargo test`
Expected: PASS. If `tests/cli_surface.rs` or `tests/docs_snapshot.rs` snapshot `--help`, update the snapshot to include `context`.

- [ ] **Step 5: Try it on this repo's log**

Run: `cargo run -q -- context | head -60` and `cargo run -q -- context check --percent 30`
Expected: a packet with this repo's decisions (for example `crate-name=eventlog-cli`), and a JSON verdict. Read the packet once as the controller would. Note anything confusing in the Task 8 report.

- [ ] **Step 6: Report**

`eventlog append result ref=src/cmd/context.rs paths=src/cli.rs,src/cmd/mod.rs,src/cmd/context.rs,src/context/mod.rs,src/context/install.rs,tests/context_cmd.rs summary="Add eventlog context and eventlog context check"` (plus any snapshot files changed).

---

### Task 6: The `eventlog-context` mod

**Files:**
- Create: `mods/eventlog-context/.claude-plugin/plugin.json`
- Create: `mods/eventlog-context/hooks/hooks.json`
- Create: `mods/eventlog-context/hooks/register.ts`
- Create: `mods/eventlog-context/hooks/policy.ts`
- Create: `mods/eventlog-context/tests/policy.test.ts`
- Create: `mods/eventlog-context/tests/register.test.ts`
- Create: `mods/eventlog-context/tsconfig.json`

**Interfaces:**
- Consumes: `eventlog context --json` → `{v, as_of, markdown, sections, settings: {keep_turns, tail_chars}}`; `eventlog context check --percent N` → `{rebuild, reason}`; `eventlog append rebuild trigger=… [as_of=…] [reason=…] [kept_turns=…]`. Task 1's report, answer 2.
- Produces (`policy.ts`, pure):
  ```ts
  export const REBUILD_PREFIX = 'eventlog-rebuild:'
  export type Packet = { as_of: number; markdown: string; settings: { keep_turns: number; tail_chars: number } }
  export type Verdict = { rebuild: boolean; reason: string }
  export function parsePacket(run: { exitCode: number; stdout: string }): Packet | null
  export function parseVerdict(run: { exitCode: number; stdout: string }): Verdict | null
  export function selectTail(messages: readonly SessionMessage[], keepTurns: number, tailChars: number): SessionMessage[]
  export function triggerOf(trigger: string, instructions?: string): string
  export function rebuildArgs(trigger: string, fields: Record<string, string | number>): string[]
  ```

- [ ] **Step 1: Get the type declarations**

Run `/plugin-types` in a Claude Code session in this repo, or copy `mods/types/claude-code.d.ts` from `https://github.com/anthropics/claude-code/tree/main/mods`. Write `mods/eventlog-context/tsconfig.json`:
```json
{
  "compilerOptions": {
    "strict": true, "target": "ES2022", "module": "ESNext", "moduleResolution": "Bundler",
    "noEmit": true, "allowImportingTsExtensions": true,
    "paths": { "claude-code": ["../../.claude/types/claude-code.d.ts"] }
  },
  "include": ["hooks/**/*.ts", "tests/**/*.ts"]
}
```
Point `paths` at wherever `/plugin-types` wrote the file; the types file itself is not part of this task's paths.

- [ ] **Step 2: Write the failing policy tests**

`mods/eventlog-context/tests/policy.test.ts`:
```ts
import type { SessionMessage } from 'claude-code'
import { describe, expect, test } from 'claude-code/testing'

import { parsePacket, parseVerdict, rebuildArgs, selectTail, triggerOf } from '../hooks/policy'

const user = (text: string): SessionMessage => ({ role: 'user', text, toolUses: [], handle: `h-${text}` })
const toolResult = (text: string): SessionMessage => ({ role: 'user', text, toolUses: [], toolResults: [{ tool_use_id: 't', text, isError: false } as never], handle: `h-${text}` })
const asst = (text: string): SessionMessage => ({ role: 'assistant', text, toolUses: [], handle: `h-${text}` })

describe('policy', () => {
  test('selectTail keeps the last N turns, starting at a real user message', () => {
    const msgs = [user('u1'), asst('a1'), user('u2'), asst('a2'), toolResult('r2'), asst('a2b'), user('u3'), asst('a3')]
    const tail = selectTail(msgs, 2, 1_000_000)
    expect(tail.map(m => m.text)).toEqual(['u2', 'a2', 'r2', 'a2b', 'u3', 'a3'])
  })

  test('selectTail never starts at a tool result', () => {
    const msgs = [user('u1'), asst('a1'), toolResult('r1'), asst('a1b')]
    expect(selectTail(msgs, 5, 1_000_000)[0].text).toBe('u1')
  })

  test('selectTail drops oldest turns to fit tail_chars but keeps the newest', () => {
    const big = 'x'.repeat(500)
    const msgs = [user(big), asst(big), user('u2'), asst(big + big)]
    const tail = selectTail(msgs, 3, 600)
    expect(tail.map(m => m.text.slice(0, 2))).toEqual(['u2', 'xx'])
  })

  test('selectTail keeps handles', () => {
    expect(selectTail([user('u1'), asst('a1')], 1, 1000)[1].handle).toBe('h-a1')
  })

  test('parsePacket rejects failure, bad JSON, and empty markdown', () => {
    expect(parsePacket({ exitCode: 1, stdout: '{}' })).toBeNull()
    expect(parsePacket({ exitCode: 0, stdout: 'nope' })).toBeNull()
    expect(parsePacket({ exitCode: 0, stdout: JSON.stringify({ as_of: 1, markdown: '', settings: { keep_turns: 3, tail_chars: 10 } }) })).toBeNull()
    const ok = parsePacket({ exitCode: 0, stdout: JSON.stringify({ as_of: 7, markdown: '# x', settings: { keep_turns: 3, tail_chars: 10 } }) })
    expect(ok?.as_of).toBe(7)
  })

  test('parseVerdict reads the check output', () => {
    expect(parseVerdict({ exitCode: 0, stdout: '{"rebuild":true,"reason":"boundary"}\n' })).toEqual({ rebuild: true, reason: 'boundary' })
    expect(parseVerdict({ exitCode: 2, stdout: '' })).toBeNull()
  })

  test('triggerOf maps engine triggers and our own reasons', () => {
    expect(triggerOf('auto')).toBe('auto')
    expect(triggerOf('manual')).toBe('manual')
    expect(triggerOf('plugin', 'eventlog-rebuild:boundary')).toBe('boundary')
    expect(triggerOf('plugin', 'something else')).toBe('plugin')
  })

  test('rebuildArgs builds the append argv', () => {
    expect(rebuildArgs('auto', { as_of: 7, kept_turns: 3 })).toEqual(['eventlog', 'append', 'rebuild', 'trigger=auto', 'as_of=7', 'kept_turns=3'])
  })
})
```

- [ ] **Step 3: Run them to verify they fail**

Run: `claude plugin test mods/eventlog-context`
Expected: FAIL (module `../hooks/policy` not found).

- [ ] **Step 4: Write `policy.ts`**

```ts
import type { SessionMessage } from 'claude-code'

/** Marks a compaction this mod asked for; the text after it is the reason. */
export const REBUILD_PREFIX = 'eventlog-rebuild:'

export type Packet = { as_of: number; markdown: string; settings: { keep_turns: number; tail_chars: number } }
export type Verdict = { rebuild: boolean; reason: string }
type Run = { exitCode: number; stdout: string }

function json(run: Run): unknown {
  if (run.exitCode !== 0) return null
  try {
    return JSON.parse(run.stdout)
  } catch {
    return null
  }
}

export function parsePacket(run: Run): Packet | null {
  const v = json(run) as Partial<Packet> | null
  if (!v || typeof v.markdown !== 'string' || v.markdown.trim() === '') return null
  if (typeof v.as_of !== 'number' || !v.settings) return null
  return v as Packet
}

export function parseVerdict(run: Run): Verdict | null {
  const v = json(run) as Partial<Verdict> | null
  if (!v || typeof v.rebuild !== 'boolean' || typeof v.reason !== 'string') return null
  return v as Verdict
}

/** A turn starts at a user message that is not a tool result. */
function isTurnStart(m: SessionMessage): boolean {
  return m.role === 'user' && !(m.toolResults && m.toolResults.length > 0)
}

function size(m: SessionMessage): number {
  return m.text.length + JSON.stringify(m.toolUses ?? []).length + JSON.stringify(m.toolResults ?? []).length
}

/**
 * The last `keepTurns` turns, whole, oldest first. Drops whole turns from the
 * oldest end until the tail fits `tailChars`; always keeps the newest turn.
 */
export function selectTail(messages: readonly SessionMessage[], keepTurns: number, tailChars: number): SessionMessage[] {
  const starts: number[] = []
  messages.forEach((m, i) => { if (isTurnStart(m)) starts.push(i) })
  if (starts.length === 0 || keepTurns <= 0) return []
  const turns = starts.map((s, i) => messages.slice(s, starts[i + 1] ?? messages.length))
  let kept = turns.slice(-keepTurns)
  while (kept.length > 1 && kept.flat().reduce((n, m) => n + size(m), 0) > tailChars) {
    kept = kept.slice(1)
  }
  return kept.flat()
}

export function triggerOf(trigger: string, instructions?: string): string {
  if (trigger === 'plugin' && instructions?.startsWith(REBUILD_PREFIX)) {
    return instructions.slice(REBUILD_PREFIX.length)
  }
  return trigger
}

export function rebuildArgs(trigger: string, fields: Record<string, string | number>): string[] {
  return ['eventlog', 'append', 'rebuild', `trigger=${trigger}`, ...Object.entries(fields).map(([k, v]) => `${k}=${v}`)]
}
```

- [ ] **Step 5: Run the policy tests to verify they pass**

Run: `claude plugin test mods/eventlog-context`
Expected: `policy` PASS.

- [ ] **Step 6: Write the failing register tests**

`mods/eventlog-context/tests/register.test.ts`:
```ts
import type { SessionMessage } from 'claude-code'
import { describe, expect, mock, test, tier } from 'claude-code/testing'

tier('installed')

const PACKET = JSON.stringify({ v: 1, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 1, tail_chars: 100000 } })
const msgs: SessionMessage[] = [
  { role: 'user', text: 'old', toolUses: [], handle: 'h1' },
  { role: 'assistant', text: 'old answer', toolUses: [], handle: 'h2' },
  { role: 'user', text: 'new', toolUses: [], handle: 'h3' },
  { role: 'assistant', text: 'new answer', toolUses: [], handle: 'h4' },
]

function processAnswers(on: any, answers: Record<string, { exitCode: number; stdout: string }>, calls: string[][]) {
  on('process.run', ($: unknown, e: { argv: string[] }) => {
    calls.push(e.argv)
    const key = e.argv.slice(1, 3).join(' ')
    return { value: { stderr: '', ...(answers[key] ?? { exitCode: 0, stdout: '' }) } }
  })
}

describe('register', () => {
  test('session.compact returns the packet plus the tail without the summarizer', async ($, on) => {
    const calls: string[][] = []
    processAnswers(on, { 'context --json': { exitCode: 0, stdout: PACKET } }, calls)
    on('session.compact', () => { throw new Error('summarizer must not run') })
    const r = await $.session.compact({ instructions: 'eventlog-rebuild:boundary', messages: msgs } as never)
    expect(r.messages?.map(m => m.text)).toEqual(['# Context rebuilt from the event log', 'new', 'new answer'])
    expect(r.messages?.[1].handle).toBe('h3')
    expect(calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=boundary', 'as_of=42', 'kept_turns=1'])
  })

  test('fallback: eventlog fails, engine compacts, rebuild event records it', async ($, on) => {
    const calls: string[][] = []
    processAnswers(on, { 'context --json': { exitCode: 1, stdout: '' } }, calls)
    let summarized = false
    on('session.compact', () => { summarized = true; return { messages: [] } })
    await $.session.compact({ messages: msgs } as never)
    expect(summarized).toBe(true)
    expect(calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=plugin', 'reason=engine-fallback'])
  })

  test('precompute is skipped', async ($, on) => {
    processAnswers(on, {}, [])
    const r = await $.session.compact({ trigger: 'precompute', messages: msgs } as never)
    expect(r.skip).toContain('eventlog-context')
  })

  test('a subagent compaction passes through and runs no eventlog process', async ($, on) => {
    const calls: string[][] = []
    processAnswers(on, { 'context --json': { exitCode: 0, stdout: PACKET } }, calls)
    let passed = false
    on('session.compact', () => { passed = true; return { messages: [] } })
    await $.session.compact({ agentId: 'sub-1', messages: msgs } as never)
    expect(passed).toBe(true)
    expect(calls).toEqual([])
  })
})
```
These tests raise `session.compact` with a chosen `trigger`, `agentId` and `messages`. Before running them, read `mods/diff/tests/register.test.ts` and the `claude-code/testing` types for how the kit sets those fields on a dispatch. If `$.session.compact(args)` does not accept them, change only the four dispatch calls to the kit's form, and keep every assertion unchanged.

- [ ] **Step 7: Run them to verify they fail**

Run: `claude plugin test mods/eventlog-context`
Expected: `register` FAIL (no module).

- [ ] **Step 8: Write the manifest, hooks.json and `register.ts`**

`.claude-plugin/plugin.json`:
```json
{
  "name": "eventlog-context",
  "version": "0.1.0",
  "description": "Replaces context compaction in a log-driven repo: the conversation is rebuilt from .context/events.jsonl by `eventlog context`, plus the last few turns word for word. Rebuilds at the log's task boundaries and at a fill backstop; /rebuild does it now. Falls back to the engine's compaction when eventlog fails.",
  "author": { "name": "radiator-engineering" }
}
```
`hooks/hooks.json`:
```json
{
  "description": "session.compact answers with a packet from `eventlog context --json` plus the last turns; turn.complete asks `eventlog context check` and compacts on yes; command.run handles /rebuild.",
  "modules": ["./register.ts"]
}
```
`hooks/register.ts`:
```ts
import type { On } from 'claude-code'

import { REBUILD_PREFIX, parsePacket, parseVerdict, rebuildArgs, selectTail, triggerOf } from './policy'

const TIMEOUT_MS = 5_000

export function register(on: On): void {
  on('session.start', async ($, e, next) => {
    const started = await next(e)
    await $.command.register({ name: 'rebuild', description: 'Rebuild the context from the event log now' })
    return started
  })

  on('command.run', { command: 'rebuild' }, async ($) => {
    const r = await $.session.compact({ instructions: `${REBUILD_PREFIX}command` })
    return { text: r.skip ? `rebuild skipped: ${r.skip}` : 'Context rebuilt from the event log.' }
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId || e.reason !== 'answer') return result
    try {
      const { context } = await $.session.usage()
      const percent = Math.min(100, Math.max(0, Math.round(context.percent ?? 0)))
      const run = await $.process.run(['eventlog', 'context', 'check', '--percent', String(percent)], { timeoutMs: TIMEOUT_MS })
      const verdict = parseVerdict(run)
      if (verdict?.rebuild) {
        // Task 1 answer 2 decides this call's form. Deferred is the default:
        // it works whether or not the turn still counts as running here.
        $.clock.after(0, () => $.session.compact({ instructions: `${REBUILD_PREFIX}${verdict.reason}` }))
      }
    } catch (err) {
      $.ui.log(`eventlog-context: check failed: ${String(err)}`)
    }
    return result
  })

  on('session.compact', async ($, e, next) => {
    if (e.agentId) return next(e)
    if (e.trigger === 'precompute') return { skip: 'eventlog-context rebuilds on demand' }
    const trigger = triggerOf(e.trigger, e.instructions)
    const append = async (fields: Record<string, string | number>) => {
      try {
        const r = await $.process.run(rebuildArgs(trigger, fields), { timeoutMs: TIMEOUT_MS })
        if (r.exitCode !== 0) $.ui.log(`eventlog-context: rebuild event not appended: ${r.stderr.trim()}`)
      } catch (err) {
        $.ui.log(`eventlog-context: rebuild event not appended: ${String(err)}`)
      }
    }
    let packet = null
    try {
      packet = parsePacket(await $.process.run(['eventlog', 'context', '--json'], { timeoutMs: TIMEOUT_MS }))
    } catch {
      packet = null
    }
    if (!packet) {
      await append({ reason: 'engine-fallback' })
      return next(e)
    }
    const tail = selectTail(e.messages, packet.settings.keep_turns, packet.settings.tail_chars)
    const keptTurns = tail.filter(m => m.role === 'user' && !(m.toolResults && m.toolResults.length > 0)).length
    await append({ as_of: packet.as_of, kept_turns: keptTurns })
    return { messages: [{ role: 'user', text: packet.markdown, toolUses: [] }, ...tail] }
  })
}
```
If Task 1 found that a direct call works and a deferred one does not, replace the `$.clock.after(...)` line with `await $.session.compact({ instructions: \`${REBUILD_PREFIX}${verdict.reason}\` })`. If neither works, move the check-and-compact block into an `on('prompt.submit', …)` hook that runs it before `next(e)`, and say so in the report.

- [ ] **Step 9: Run all mod tests and the typecheck**

Run: `claude plugin test mods/eventlog-context && npx -y -p typescript tsc -p mods/eventlog-context/tsconfig.json`
Expected: PASS, no type errors.

- [ ] **Step 10: Report**

`eventlog append result ref=mods/eventlog-context/hooks/register.ts paths=mods/eventlog-context summary="Add the eventlog-context mod: rebuild context from the log in place of compaction, with /rebuild"`

---

### Task 7: Install the mod or the classic hook

**Files:**
- Modify: `src/context/install.rs` (replace the stub)
- Modify: `Cargo.toml` only if Step 6 shows the mod missing from the package
- Test: `tests/context_install.rs`

**Interfaces:**
- Consumes: `mods/eventlog-context/**` (embedded with `include_dir`).
- Produces: `pub fn run(root: &Path, classic: bool, force: bool) -> anyhow::Result<i32>`.

- [ ] **Step 1: Write the failing tests**

`tests/context_install.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn install(dir: &TempDir, extra: &[&str]) -> std::process::Output {
    std::fs::create_dir_all(dir.path().join(".context")).unwrap();
    std::fs::write(dir.path().join(".context/events.jsonl"), "").unwrap();
    Command::cargo_bin("eventlog").unwrap().args(["context", "install"]).args(extra).current_dir(dir.path()).output().unwrap()
}

#[test]
fn install_writes_the_mod_without_tests() {
    let dir = TempDir::new().unwrap();
    let out = install(&dir, &[]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let base = dir.path().join(".claude/skills/eventlog-context");
    assert!(base.join(".claude-plugin/plugin.json").is_file());
    assert!(base.join("hooks/hooks.json").is_file());
    assert!(base.join("hooks/register.ts").is_file());
    assert!(base.join("hooks/policy.ts").is_file());
    assert!(!base.join("tests").exists());
    assert!(String::from_utf8_lossy(&out.stdout).contains("CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1"));
}

#[test]
fn install_is_repeatable() {
    let dir = TempDir::new().unwrap();
    assert!(install(&dir, &[]).status.success());
    assert!(install(&dir, &[]).status.success());
}

#[test]
fn install_refuses_when_classic_hook_present() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(dir.path().join(".claude/settings.json"), r#"{"hooks":{"SessionStart":[{"matcher":"compact","hooks":[{"type":"command","command":"eventlog context"}]}]}}"#).unwrap();
    let out = install(&dir, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("classic"));
    assert!(install(&dir, &["--force"]).status.success());
}

#[test]
fn classic_prints_the_hook_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let out = install(&dir, &["--classic"]);
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains(r#""matcher": "compact""#));
    assert!(s.contains(r#""command": "eventlog context""#));
    assert!(!dir.path().join(".claude").exists());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --test context_install`
Expected: FAIL (`context install arrives in Task 7`).

- [ ] **Step 3: Implement**

`src/context/install.rs`:
```rust
//! Install the eventlog-context mod, or print the classic fallback hook.

use std::path::Path;

use anyhow::Context as _;
use include_dir::{Dir, include_dir};

static MOD: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/mods/eventlog-context");
const TARGET: &str = ".claude/skills/eventlog-context";

const CLASSIC: &str = r#"Add this to "hooks" in .claude/settings.json (use it only when function hooks are off):

"SessionStart": [
  { "matcher": "compact",
    "hooks": [{ "type": "command", "command": "eventlog context" }] }
]
"#;

pub fn run(root: &Path, classic: bool, force: bool) -> anyhow::Result<i32> {
    if classic {
        print!("{CLASSIC}");
        return Ok(0);
    }
    let settings = root.join(".claude/settings.json");
    if !force
        && std::fs::read_to_string(&settings).is_ok_and(|s| s.contains("\"eventlog context\""))
    {
        anyhow::bail!(
            "{} already runs `eventlog context` as a classic hook; the mod and the classic hook would both add the packet. Remove the classic hook, or pass --force",
            settings.display()
        );
    }
    let target = root.join(TARGET);
    write_dir(&MOD, &target)?;
    println!("wrote {TARGET}");
    println!("Start Claude Code with CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1 (Claude Code 2.1.278 or later).");
    Ok(0)
}

fn write_dir(dir: &Dir<'_>, target: &Path) -> anyhow::Result<()> {
    for file in dir.files() {
        let dest = target.join(file.path());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&dest, file.contents()).with_context(|| format!("writing {}", dest.display()))?;
    }
    for sub in dir.dirs() {
        if sub.path().file_name().is_some_and(|n| n == "tests") {
            continue;
        }
        write_dir(sub, target)?;
    }
    Ok(())
}
```
`file.path()` in `include_dir` is relative to the embedded root, so `target.join(file.path())` lands in the right place for nested files.

- [ ] **Step 4: Run them to verify they pass**

Run: `cargo test --test context_install`
Expected: PASS.

- [ ] **Step 5: Install into this repo and confirm Claude Code loads it**

Run: `cargo run -q -- context install`. Then start `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1 claude` in this repo and type `/rebuild`.
Expected: "Context rebuilt from the event log.", and `eventlog view --last 3` shows a `rebuild` event with `trigger=command` and an `as_of`.

- [ ] **Step 6: Confirm the crate ships the mod**

Run: `cargo package --list --allow-dirty | grep mods/eventlog-context`
Expected: `plugin.json`, `hooks.json`, `register.ts`, `policy.ts` are listed. If not, add `mods/eventlog-context/**` to an `include` list in `Cargo.toml` that also keeps `src/**`, `Cargo.toml`, `README.md`, `LICENSE*`, and rerun.

- [ ] **Step 7: Report**

`eventlog append result ref=src/context/install.rs paths=src/context/install.rs,tests/context_install.rs,.claude/skills/eventlog-context summary="Add eventlog context install: write the eventlog-context mod or print the classic hook"` (add `Cargo.toml` if Step 6 changed it). Then record the decision: `eventlog append decision key=context-rebuild value=eventlog-context-mod ref=.context/DECISIONS.md`, and add its entry to `.context/DECISIONS.md`.

---

### Task 8: Verify on real sessions, and document

**Files:**
- Create: `.context/reports/context-rebuild-probes.md`
- Create: `docs/how-to/rebuild-context-from-the-log.md`

**Interfaces:**
- Consumes: everything above, installed in this repo.

- [ ] **Step 1: Run three real sessions**

Use the controller for normal work in this repo with the mod installed until each session passes 50% of the window at least once. Do not force it with filler text. For each session, confirm in `eventlog view` that at least one `rebuild` event appears, and note its `trigger`.

- [ ] **Step 2: Ask the probes after each rebuild**

Right after a rebuild, ask the controller, in order:
1. "Which files have changed since the last commit, and who owns each?"
2. "What is the decision in force for `<a key from the packet>`?"
3. "What open work is there?"
4. "What should happen next?"

Compare answers 1–3 with `git status` and `eventlog state`. Score each: correct, partly correct, wrong.

- [ ] **Step 3: Write the report**

`.context/reports/context-rebuild-probes.md`: per session, the rebuild seq and trigger, packet size in characters, the four answers, and scores. Add anything the controller did in the first turns after a rebuild that shows missing context (re-reading files it had just read, asking what it was doing).

- [ ] **Step 4: Gate**

Phase 1 is done only when every answer to probes 1 and 2 is correct. If any is wrong, write down which packet section should have carried the fact and stop. Report to the user before changing the design.

- [ ] **Step 5: Write the how-to**

`docs/how-to/rebuild-context-from-the-log.md`, following the style of the other files in `docs/how-to/`:
1. What it does, in three sentences.
2. Requirements: Claude Code 2.1.278+, `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`, `eventlog` on `PATH`.
3. Install: `eventlog context install`; without function hooks, `eventlog context install --classic`.
4. When it rebuilds: the boundary rule, the backstop, `/rebuild`, and the engine's own triggers.
5. Settings: the `[context]` table with defaults.
6. Recording intents so a rebuild knows the current task: `eventlog append intent msg="<task>" ref=<brief>`, and closing it when done.
7. How to check it worked: `eventlog view --last 5` shows `rebuild`; `eventlog context` shows what the controller received.

Apply the plain-technical-english final gate to the page before reporting.

- [ ] **Step 6: Report**

`eventlog append result ref=docs/how-to/rebuild-context-from-the-log.md paths=docs/how-to/rebuild-context-from-the-log.md,.context/reports/context-rebuild-probes.md summary="Document rebuilding context from the event log; record probe results from three real sessions"`
