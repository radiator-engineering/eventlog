//! The reactor runtime: one lock, one baseline, then one pass per matching
//! event (spec section 7).
//!
//! The loop owns everything the shell reactor scripts did except the action
//! itself. The four steps that are not the loop — computing the authorized
//! set, running the rule voter, running the command, and snapshotting git —
//! sit behind [`Steps`], so the loop is testable without a command or a repo,
//! and so `voter` and `action` can be built against a fixed interface.
//!
//! Install the real steps with [`Reactor::with_steps`]. [`Reactor::new`] fits
//! a placeholder that refuses to act, so a caller that forgets fails loudly on
//! the first event instead of silently acking everything.

pub mod action;
pub mod lock;
pub mod voter;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use globset::Glob;

use crate::log::Log;
use crate::log::append::{AppendRequest, append};
use crate::model::config::Config;
use crate::model::event::Event;
use crate::model::vocab::TypeSpec;
use crate::query::{State, fold};

pub use lock::ReactorLock;

/// How often the loop looks for new lines once it has caught up.
const POLL: Duration = Duration::from_millis(200);
/// A crash loop: this many restarts inside [`RESTART_WINDOW`] stops the
/// runtime instead of hammering the log.
const MAX_RESTARTS: usize = 5;
const RESTART_WINDOW: Duration = Duration::from_secs(600);

/// One reactor's configuration: what it is called, what it reacts to, and what
/// it runs.
#[derive(Clone, Debug)]
pub struct ReactorConfig {
    /// The `by=` on every line this runtime writes, and the name in its lock.
    pub name: String,
    /// Event types to react to (`--on`).
    pub on: Vec<String>,
    /// Extra `k=v` conditions the driving event must meet (`--filter`).
    pub filter: Vec<(String, String)>,
    /// The veto window: how long to wait after `intent` before acting.
    pub window: Duration,
    /// Take git snapshots around the action and report writes outside the
    /// authorized set.
    pub git: bool,
    /// The command and its arguments.
    pub command: Vec<String>,
    /// How long the command may run.
    pub pass_timeout: Duration,
}

impl Default for ReactorConfig {
    fn default() -> Self {
        ReactorConfig {
            name: "reactor".to_string(),
            on: Vec::new(),
            filter: Vec::new(),
            window: Duration::ZERO,
            git: false,
            command: Vec::new(),
            pass_timeout: Duration::from_secs(600),
        }
    }
}

/// `HEAD` and the working tree, as seen before and after the action.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitSnapshot {
    pub head: String,
    /// One entry per `git status --porcelain` line, path only.
    pub status: Vec<String>,
}

/// What the command reported: the `outcome` and any other `k=v` lines from the
/// outcome file, which become fields on the `ack`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub outcome: String,
    pub fields: Vec<(String, String)>,
}

/// The steps the loop delegates: `voter::authorize`, `voter::check`,
/// `action::run` and `action::snapshot`.
pub trait Steps {
    /// Spec 7.4.1. The paths this reaction may touch, or the veto reason when
    /// the driving event names more than its writer holds.
    fn authorize(&mut self, state: &State, driving: &Event) -> Result<Vec<String>, String>;

    /// Spec 7.4.3. The rule voter, run against the fold at this moment.
    /// `Some(rule)` vetoes.
    fn check(&mut self, state: &State, driving: &Event, authorized: &[String]) -> Option<String>;

    /// Spec 7.4.5. Run the command against the driving event.
    fn run(
        &mut self,
        cfg: &ReactorConfig,
        driving: &Event,
        authorized: &[String],
        resume: u64,
    ) -> anyhow::Result<Outcome>;

    /// Spec 7.4.6. `HEAD` and the working tree right now.
    fn snapshot(&mut self) -> anyhow::Result<GitSnapshot>;

    /// Spec 7.4.6. Files changed by the commits between the two snapshots:
    /// the only writes the action can be blamed for.
    fn committed(
        &mut self,
        _before: &GitSnapshot,
        _after: &GitSnapshot,
    ) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }
}

/// The steps [`Reactor::new`] fits until the caller installs the real ones.
struct NoSteps;

impl Steps for NoSteps {
    fn authorize(&mut self, _state: &State, _driving: &Event) -> Result<Vec<String>, String> {
        Err("no-steps-installed".to_string())
    }

    fn check(&mut self, _s: &State, _d: &Event, _a: &[String]) -> Option<String> {
        Some("no-steps-installed".to_string())
    }

    fn run(
        &mut self,
        _cfg: &ReactorConfig,
        _driving: &Event,
        _authorized: &[String],
        _resume: u64,
    ) -> anyhow::Result<Outcome> {
        anyhow::bail!("reactor has no Steps installed: call Reactor::with_steps")
    }

    fn snapshot(&mut self) -> anyhow::Result<GitSnapshot> {
        Ok(GitSnapshot::default())
    }
}

pub struct Reactor {
    cfg: ReactorConfig,
    log: Log,
    config: Config,
    root: PathBuf,
    steps: Box<dyn Steps>,
}

impl Reactor {
    pub fn new(cfg: ReactorConfig, log: Log, config: Config, root: PathBuf) -> Reactor {
        Reactor {
            cfg,
            log,
            config: with_reactor_fields(config),
            root,
            steps: Box::new(NoSteps),
        }
    }

    /// Install the real `voter` and `action` steps.
    pub fn with_steps(mut self, steps: Box<dyn Steps>) -> Reactor {
        self.steps = steps;
        self
    }

    pub fn config(&self) -> &ReactorConfig {
        &self.cfg
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<log>.<name>.reactor.lock`.
    pub fn lock_dir(&self) -> PathBuf {
        PathBuf::from(format!(
            "{}.{}.reactor.lock",
            self.log.path.display(),
            self.cfg.name
        ))
    }

    /// Take the lock, then catch up and keep catching up until killed.
    pub fn run(&mut self) -> anyhow::Result<()> {
        let dir = self.lock_dir();
        let _lock = ReactorLock::acquire(&dir)?;
        loop {
            self.catch_up()?;
            std::thread::sleep(POLL);
        }
    }

    /// One pass: baseline if this reactor has never acked, then close any
    /// interrupted intent, then handle every matching unacked event in `seq`
    /// order. Returns the events it appended.
    pub fn catch_up(&mut self) -> anyhow::Result<Vec<Event>> {
        let events = self.log.read()?.events;
        let state = fold(&events, &self.config);

        if state
            .reactors
            .get(&self.cfg.name)
            .and_then(|r| r.last_ack_seq)
            .is_none()
        {
            return self.baseline(&events);
        }

        let mut written = Vec::new();
        // Spec 7.3: an intent with no ack behind it means the runtime died
        // after declaring intent. The effect may have happened, so the event
        // is closed and escalated, never re-run.
        let interrupted = self.interrupted_intents(&state);
        for intent in &interrupted {
            written.extend(self.close_interrupted(intent)?);
        }
        let skip: Vec<u64> = interrupted
            .iter()
            .filter_map(|i| i.seq_ref("for"))
            .collect();

        let on: Vec<&str> = self.cfg.on.iter().map(String::as_str).collect();
        let pending: Vec<Event> = state
            .unacked(&self.cfg.name, &on, &events)
            .into_iter()
            .filter(|seq| !skip.contains(seq))
            .filter_map(|seq| events.iter().find(|e| e.seq == seq).cloned())
            .filter(|e| self.matches(e))
            .collect();

        for driving in pending {
            written.extend(self.handle(&driving, false)?);
        }
        Ok(written)
    }

    /// Spec 7.2. With no own `ack` in the log, ack the tip as skipped and
    /// never replay what came before this runtime existed.
    pub fn baseline(&mut self, events: &[Event]) -> anyhow::Result<Vec<Event>> {
        let tip = events.iter().map(|e| e.seq).max().unwrap_or(0);
        let ack = self.emit(
            false,
            "ack",
            &[
                ("seq_done", tip.to_string()),
                ("outcome", "skipped".to_string()),
                ("detail", "baseline".to_string()),
            ],
        )?;
        Ok(vec![ack])
    }

    /// Spec 7.4. One event, start to finish. With `dry`, returns the events it
    /// would append and writes none — `react test`.
    pub fn handle(&mut self, driving: &Event, dry: bool) -> anyhow::Result<Vec<Event>> {
        let events = self.log.read()?.events;
        let state = fold(&events, &self.config);
        let resume = state
            .reactors
            .get(&self.cfg.name)
            .and_then(|r| r.last_ack_seq)
            .unwrap_or(0);
        let mut written = Vec::new();

        // 7.4.1 the authorized set.
        let authorized = match self.steps.authorize(&state, driving) {
            Ok(paths) => paths,
            Err(reason) => {
                written.extend(self.veto(dry, driving, None, &reason)?);
                return Ok(written);
            }
        };

        // 7.4.2 declare intent before acting, so a crash is visible.
        let intent = self.emit(
            dry,
            "intent",
            &[
                ("for", driving.seq.to_string()),
                ("action", self.cfg.name.clone()),
                ("paths", authorized.join(",")),
            ],
        )?;
        written.push(intent.clone());

        // 7.4.3 the rule voter, against the fold as it is now.
        if let Some(reason) = self.steps.check(&state, driving, &authorized) {
            written.extend(self.veto(dry, driving, Some(&intent), &reason)?);
            return Ok(written);
        }

        // 7.4.4 the veto window. A veto naming the driving seq binds whichever
        // intent it saw, so a restart in between cannot lose it.
        if !dry
            && !self.cfg.window.is_zero()
            && let Some(veto) = self.wait_for_veto(driving)?
        {
            let reason = veto
                .fields
                .get("reason")
                .cloned()
                .unwrap_or_else(|| "vetoed".to_string());
            written.push(self.ack(dry, driving, &intent, "vetoed", &[], Some(&reason))?);
            return Ok(written);
        }

        // 7.4.5 run the command; one retry, and only when it asks for one.
        let before = if self.cfg.git {
            Some(self.steps.snapshot()?)
        } else {
            None
        };
        let mut outcome = self
            .steps
            .run(&self.cfg, driving, &authorized, resume)
            .unwrap_or_else(|e| Outcome {
                outcome: "failed".to_string(),
                fields: vec![("detail".to_string(), e.to_string())],
            });
        if outcome.outcome == "retryable" {
            outcome = self
                .steps
                .run(&self.cfg, driving, &authorized, resume)
                .unwrap_or_else(|e| Outcome {
                    outcome: "failed".to_string(),
                    fields: vec![("detail".to_string(), e.to_string())],
                });
        }

        // 7.4.6 what the action committed outside the authorized set. Detection
        // after the fact: the commit stands. Only commits are the action's
        // doing (decision `violation-scope`): the tree is shared, so a file
        // that became dirty meanwhile is another agent's work in progress.
        // Under an open claim it is expected and silent; unclaimed, it is
        // recorded as `observed` so the controller can see it, without blame.
        if let Some(before) = before {
            let after = self.steps.snapshot()?;
            let committed = self.steps.committed(&before, &after)?;
            let outside: Vec<String> = committed
                .iter()
                .filter(|p| !authorized.iter().any(|a| covers(a, p)))
                .cloned()
                .collect();
            if !outside.is_empty() {
                written.push(self.emit(
                    dry,
                    "violation",
                    &[
                        ("agent", self.cfg.name.clone()),
                        ("for", driving.seq.to_string()),
                        ("paths", outside.join(",")),
                    ],
                )?);
            }
            let observed: Vec<String> = newly_dirty(&before, &after, &authorized)
                .into_iter()
                .filter(|p| state.claim_owner(p).is_none())
                .collect();
            if !observed.is_empty() {
                written.push(self.emit(
                    dry,
                    "observed",
                    &[
                        ("for", driving.seq.to_string()),
                        ("paths", observed.join(",")),
                    ],
                )?);
            }
        }

        // 7.4.7 one ack, carrying the outcome file's fields.
        written.push(self.ack(
            dry,
            driving,
            &intent,
            &outcome.outcome.clone(),
            &outcome.fields,
            None,
        )?);
        Ok(written)
    }

    /// This reactor's open intents: an `intent` of its own with no `ack for=`
    /// behind it.
    fn interrupted_intents(&self, state: &State) -> Vec<Event> {
        state
            .intents
            .iter()
            .filter(|i| i.writer() == self.cfg.name && i.seq_ref("for").is_some())
            .cloned()
            .collect()
    }

    fn close_interrupted(&mut self, intent: &Event) -> anyhow::Result<Vec<Event>> {
        let driving = intent.seq_ref("for").unwrap_or_default();
        let ack = self.emit(
            false,
            "ack",
            &[
                ("seq_done", driving.to_string()),
                ("for", intent.seq.to_string()),
                ("outcome", "interrupted".to_string()),
                ("detail", format!("intent {} has no ack", intent.seq)),
            ],
        )?;
        let escalate = self.emit(
            false,
            "escalate",
            &[
                ("subject", self.cfg.name.clone()),
                (
                    "msg",
                    format!(
                        "interrupted after intent {} for seq {}; not re-run, the effect may have happened",
                        intent.seq, driving
                    ),
                ),
            ],
        )?;
        Ok(vec![ack, escalate])
    }

    /// A veto and the `ack outcome=vetoed` that closes the event.
    fn veto(
        &mut self,
        dry: bool,
        driving: &Event,
        intent: Option<&Event>,
        reason: &str,
    ) -> anyhow::Result<Vec<Event>> {
        let mut fields = vec![
            ("for".to_string(), driving.seq.to_string()),
            ("role".to_string(), "voter".to_string()),
            ("reason".to_string(), reason.to_string()),
        ];
        if let Some(intent) = intent {
            fields.push(("intent".to_string(), intent.seq.to_string()));
        }
        let veto = self.emit_owned(dry, "veto", fields)?;
        let mut out = vec![veto];
        let mut ack = vec![
            ("seq_done".to_string(), driving.seq.to_string()),
            ("outcome".to_string(), "vetoed".to_string()),
            ("detail".to_string(), reason.to_string()),
        ];
        if let Some(intent) = intent {
            ack.push(("for".to_string(), intent.seq.to_string()));
        }
        out.push(self.emit_owned(dry, "ack", ack)?);
        Ok(out)
    }

    fn ack(
        &mut self,
        dry: bool,
        driving: &Event,
        intent: &Event,
        outcome: &str,
        extra: &[(String, String)],
        detail: Option<&str>,
    ) -> anyhow::Result<Event> {
        let mut fields = vec![
            ("seq_done".to_string(), driving.seq.to_string()),
            ("for".to_string(), intent.seq.to_string()),
            ("outcome".to_string(), outcome.to_string()),
        ];
        if let Some(detail) = detail {
            fields.push(("detail".to_string(), detail.to_string()));
        }
        // The outcome file may not restate what the runtime owns.
        for (k, v) in extra {
            if matches!(
                k.as_str(),
                "seq" | "ts" | "prev" | "by" | "seq_done" | "for" | "outcome"
            ) {
                continue;
            }
            fields.push((k.clone(), v.clone()));
        }
        self.emit_owned(dry, "ack", fields)
    }

    /// Wait out the veto window and return the first `veto` naming the driving
    /// seq, whoever wrote it.
    fn wait_for_veto(&self, driving: &Event) -> anyhow::Result<Option<Event>> {
        let deadline = Instant::now() + self.cfg.window;
        loop {
            let found = self
                .log
                .read()?
                .events
                .into_iter()
                .find(|e| e.r#type == "veto" && e.seq_ref("for") == Some(driving.seq));
            if found.is_some() {
                return Ok(found);
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(POLL.min(self.cfg.window));
        }
    }

    /// Does this event drive this reactor? `--on` names the type; `--filter`
    /// adds `k=v` conditions, where `by` is the writer and `agent` the subject.
    fn matches(&self, event: &Event) -> bool {
        if !self.cfg.on.iter().any(|t| t == &event.r#type) {
            return false;
        }
        self.cfg
            .filter
            .iter()
            .all(|(k, v)| field_of(event, k).is_some_and(|got| &got == v))
    }

    fn emit(&mut self, dry: bool, ty: &str, fields: &[(&str, String)]) -> anyhow::Result<Event> {
        let owned = fields
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect();
        self.emit_owned(dry, ty, owned)
    }

    fn emit_owned(
        &mut self,
        dry: bool,
        ty: &str,
        fields: Vec<(String, String)>,
    ) -> anyhow::Result<Event> {
        let req = AppendRequest {
            r#type: ty.to_string(),
            fields: fields.into_iter().filter(|(_, v)| !v.is_empty()).collect(),
            writer: self.cfg.name.clone(),
            // The reactor writes about events that already exist and agents it
            // does not own; the strict rules are the controller's.
            strict: false,
            dry_run: dry,
        };
        Ok(append(&self.log, &self.config, req, None)?)
    }
}

/// The reference and label fields the reactor writes, added to the loaded
/// vocabulary so its own lines validate: `intent action=`, `ack for=`,
/// `veto intent=`, `violation for=`, `observed for=`.
fn with_reactor_fields(mut config: Config) -> Config {
    let optional = |names: &[&str]| TypeSpec {
        fields: Vec::new(),
        optional: names.iter().map(|s| (*s).to_string()).collect(),
    };
    let mut extra: BTreeMap<String, TypeSpec> = BTreeMap::new();
    extra.insert("intent".to_string(), optional(&["action"]));
    extra.insert("ack".to_string(), optional(&["for"]));
    extra.insert("veto".to_string(), optional(&["intent"]));
    extra.insert("violation".to_string(), optional(&["for"]));
    extra.insert(
        "observed".to_string(),
        TypeSpec {
            fields: vec!["paths".to_string()],
            optional: ["for", "ref", "detail"].map(str::to_string).to_vec(),
        },
    );
    config.vocabulary.merge_file(extra);
    config
}

/// A field by name, reading `type`, `by` and `agent` from where they live on
/// the event rather than from the loose fields.
fn field_of(event: &Event, key: &str) -> Option<String> {
    match key {
        "type" => Some(event.r#type.clone()),
        "by" => Some(event.writer().to_string()),
        "agent" => Some(event.subject().to_string()),
        _ => event.fields.get(key).cloned(),
    }
}

/// Paths dirty after the action that were not dirty before and that no
/// authorized path covers: work in progress seen while the action ran.
fn newly_dirty(before: &GitSnapshot, after: &GitSnapshot, authorized: &[String]) -> Vec<String> {
    after
        .status
        .iter()
        .filter(|p| !before.status.contains(p))
        .filter(|p| !authorized.iter().any(|a| covers(a, p)))
        .cloned()
        .collect()
}

/// Does the claim `glob` cover `path`? Literal equality, a glob match, or a
/// directory prefix.
fn covers(glob: &str, path: &str) -> bool {
    if glob == path || path.starts_with(&format!("{}/", glob.trim_end_matches('/'))) {
        return true;
    }
    Glob::new(glob).is_ok_and(|g| g.compile_matcher().is_match(path))
}

/// Keep a reactor running: restart it when it dies, note every restart, and
/// stop with an `escalate` when it will not stay up.
pub fn supervise(
    cfg: ReactorConfig,
    log: Log,
    config: Config,
    root: PathBuf,
    steps: impl Fn() -> Box<dyn Steps>,
) -> ! {
    let mut restarts: Vec<Instant> = Vec::new();
    loop {
        let mut reactor = Reactor::new(
            cfg.clone(),
            Log::open(&log.path),
            config.clone(),
            root.clone(),
        )
        .with_steps(steps());
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reactor.run()));
        let detail = match outcome {
            Ok(Ok(())) => "loop returned".to_string(),
            Ok(Err(e)) => e.to_string(),
            Err(_) => "panicked".to_string(),
        };

        restarts.push(Instant::now());
        restarts.retain(|t| t.elapsed() < RESTART_WINDOW);
        let _ = reactor.emit(
            false,
            "note",
            &[("msg", format!("reactor {} restarting: {detail}", cfg.name))],
        );

        if restarts.len() >= MAX_RESTARTS {
            let _ = reactor.emit(
                false,
                "escalate",
                &[
                    ("subject", cfg.name.clone()),
                    (
                        "msg",
                        format!(
                            "reactor {} restarted {} times in {} minutes; stopping",
                            cfg.name,
                            restarts.len(),
                            RESTART_WINDOW.as_secs() / 60
                        ),
                    ),
                ],
            );
            std::process::exit(1);
        }
        std::thread::sleep(POLL);
    }
}
