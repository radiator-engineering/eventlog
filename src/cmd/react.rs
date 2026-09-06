//! `eventlog react` and `eventlog react test` (spec section 7).
//!
//! This is the wiring, not the policy: it turns the command line into a
//! [`ReactorConfig`], fits the real voter and action behind [`Steps`], and
//! then hands over to [`supervise`] (the live loop) or to
//! [`Reactor::handle`] with `dry = true` (the dry run, which prints the
//! events it would append and writes none).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;

use crate::cli::{Args as CliArgs, Command, ReactInner, ReactOpts};
use crate::log::Log;
use crate::model::config::{self, Config};
use crate::model::event::Event;
use crate::model::paths::{self, RelPath};
use crate::query::State;
use crate::react::action::{self, ActionEnv};
use crate::react::voter::{self, Authorized};
use crate::react::{GitSnapshot, Outcome, Reactor, ReactorConfig, Steps, supervise};

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::React(react_args) = &args.command else {
        anyhow::bail!("react::run called with wrong subcommand");
    };
    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&repo_root)?;
    let log_path = config::resolve_log(&cfg, args.log.as_deref());
    let log_path = if log_path.is_absolute() {
        log_path
    } else {
        repo_root.join(log_path)
    };

    match &react_args.inner {
        Some(ReactInner::Test(test)) => {
            let reactor_cfg = build_config(&test.opts)?;
            dry_run(test.seq, reactor_cfg, cfg, repo_root, log_path)
        }
        None => {
            let reactor_cfg = build_config(&react_args.opts)?;
            if reactor_cfg.on.is_empty() {
                anyhow::bail!("react: --on names no event type");
            }
            live(reactor_cfg, cfg, repo_root, log_path)
        }
    }
}

/// The live loop. Never returns: [`supervise`] restarts the reactor until it
/// will not stay up, then exits the process.
fn live(
    reactor_cfg: ReactorConfig,
    cfg: Config,
    repo_root: PathBuf,
    log_path: PathBuf,
) -> anyhow::Result<i32> {
    eprintln!(
        "eventlog react: {} on [{}] watching {}",
        reactor_cfg.name,
        reactor_cfg.on.join(","),
        log_path.display()
    );
    let steps_cfg = cfg.clone();
    let steps_root = repo_root.clone();
    let name = reactor_cfg.name.clone();
    let log_for_steps = log_path.clone();
    supervise(
        reactor_cfg,
        Log::open(&log_path),
        cfg,
        repo_root,
        move || -> Box<dyn Steps> {
            Box::new(RealSteps::new(
                name.clone(),
                steps_cfg.clone(),
                steps_root.clone(),
                log_for_steps.clone(),
            ))
        },
    )
}

/// `react test <seq>`: one reaction, printed rather than written.
fn dry_run(
    seq: u64,
    reactor_cfg: ReactorConfig,
    cfg: Config,
    repo_root: PathBuf,
    log_path: PathBuf,
) -> anyhow::Result<i32> {
    let log = Log::open(&log_path);
    let driving = log
        .read()?
        .events
        .into_iter()
        .find(|e| e.seq == seq)
        .with_context(|| format!("no event with seq {seq} in {}", log_path.display()))?;

    let steps = RealSteps::new(
        reactor_cfg.name.clone(),
        cfg.clone(),
        repo_root.clone(),
        log_path.clone(),
    );
    let mut reactor = Reactor::new(reactor_cfg, log, cfg, repo_root).with_steps(Box::new(steps));
    for event in reactor.handle(&driving, true)? {
        println!("{}", event.to_line());
    }
    Ok(0)
}

fn build_config(opts: &ReactOpts) -> anyhow::Result<ReactorConfig> {
    let name = opts
        .name
        .clone()
        .or_else(|| std::env::var("EVENTLOG_AS").ok())
        .context("react: --as names no reactor")?;
    if opts.command.is_empty() {
        anyhow::bail!("react: no action command after `--`");
    }
    let mut filter = Vec::new();
    for raw in &opts.filter {
        let (key, value) = raw
            .split_once('=')
            .with_context(|| format!("react: --filter {raw} is not key=value"))?;
        filter.push((key.to_string(), value.to_string()));
    }
    Ok(ReactorConfig {
        name,
        on: opts.on.iter().map(|t| t.trim().to_string()).collect(),
        filter,
        window: parse_duration(&opts.window).context("react: --window")?,
        git: opts.git,
        command: opts.command.clone(),
        pass_timeout: parse_duration(&opts.timeout).context("react: --timeout")?,
    })
}

/// `30`, `30s`, `5m` or `1h`. Bare digits are seconds, as `PASS_TIMEOUT` was.
fn parse_duration(text: &str) -> anyhow::Result<Duration> {
    let text = text.trim();
    let (digits, unit) = match text.strip_suffix(['s', 'm', 'h']) {
        Some(rest) => (rest, text.as_bytes()[text.len() - 1]),
        None => (text, b's'),
    };
    let value: u64 = digits
        .parse()
        .with_context(|| format!("{text} is not a duration"))?;
    let seconds = match unit {
        b'm' => value * 60,
        b'h' => value * 3600,
        _ => value,
    };
    Ok(Duration::from_secs(seconds))
}

/// The real steps: `voter::authorize`, `voter::check`, `action::run` and
/// `action::snapshot`, adapted to the loop's string-path interface.
struct RealSteps {
    name: String,
    cfg: Config,
    root: PathBuf,
    log_path: PathBuf,
    outcome_file: PathBuf,
    /// Fields an `ack` may carry, from the loaded vocabulary. Anything else a
    /// command reports goes into `detail`, so an outcome file can never make
    /// the ack unappendable.
    ack_fields: BTreeSet<String>,
}

impl RealSteps {
    fn new(name: String, cfg: Config, root: PathBuf, log_path: PathBuf) -> RealSteps {
        let outcome_file =
            std::env::temp_dir().join(format!("eventlog-{name}-{}.outcome", std::process::id()));
        let ack_fields = match cfg.vocabulary.get("ack") {
            Some(spec) => spec.fields.iter().chain(&spec.optional).cloned().collect(),
            None => BTreeSet::new(),
        };
        RealSteps {
            name,
            cfg,
            root,
            log_path,
            outcome_file,
            ack_fields,
        }
    }
}

impl Steps for RealSteps {
    fn authorize(&mut self, state: &State, driving: &Event) -> Result<Vec<String>, String> {
        let auth = voter::authorize(driving, state);
        if !auth.excess.is_empty() {
            return Err(voter::Veto::UnclaimedPaths(auth.excess).to_string());
        }
        Ok(auth.paths.iter().map(|p| p.as_str().to_string()).collect())
    }

    fn check(&mut self, state: &State, _driving: &Event, authorized: &[String]) -> Option<String> {
        let auth = Authorized {
            paths: rel_paths(authorized),
            excess: Vec::new(),
        };
        voter::check(&self.name, &auth, state, &self.cfg)
            .err()
            .map(|veto| veto.to_string())
    }

    fn run(
        &mut self,
        cfg: &ReactorConfig,
        driving: &Event,
        authorized: &[String],
        resume: u64,
    ) -> anyhow::Result<Outcome> {
        // A file left by the previous pass would be read as this pass's
        // answer, so it goes before the command runs, not after.
        let _ = std::fs::remove_file(&self.outcome_file);
        let env = ActionEnv {
            log: self.log_path.clone(),
            seq: driving.seq,
            r#type: driving.r#type.clone(),
            agent: driving.subject().to_string(),
            by: driving.writer().to_string(),
            paths: rel_paths(authorized),
            reference: driving.fields.get("ref").cloned(),
            resume,
            outcome_file: self.outcome_file.clone(),
        };
        let result = action::run(&cfg.command, &driving.to_line(), &env, cfg.pass_timeout)?;
        let _ = std::fs::remove_file(&self.outcome_file);
        Ok(to_outcome(result, cfg.pass_timeout, &self.ack_fields))
    }

    fn snapshot(&mut self) -> anyhow::Result<GitSnapshot> {
        let snap = action::snapshot(&self.root)?;
        Ok(GitSnapshot {
            head: snap.head.unwrap_or_default(),
            status: snap.dirty.into_iter().collect(),
        })
    }
}

/// Spec 7.4.7: the outcome file names the outcome; otherwise exit 0 is
/// `committed` and anything else `failed`, with the reason in `detail`.
///
/// A command may report any `k=v` it likes. Only the keys the `ack` vocabulary
/// knows become fields; the rest are carried in `detail`, so a command can
/// never write an ack the log would reject.
fn to_outcome(
    result: action::Outcome,
    timeout: Duration,
    ack_fields: &BTreeSet<String>,
) -> Outcome {
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut spillover: Vec<String> = Vec::new();
    let mut detail: Option<String> = None;
    let mut outcome = None;
    for (key, value) in result.fields {
        match key.as_str() {
            "outcome" => outcome = Some(value),
            "detail" => detail = Some(value),
            _ if ack_fields.contains(&key) => fields.push((key, value)),
            _ => spillover.push(format!("{key}={value}")),
        }
    }
    let outcome = outcome.unwrap_or_else(|| {
        if result.timed_out {
            spillover.push(format!("timed out after {}s", timeout.as_secs()));
            "failed".to_string()
        } else if result.exit == 0 {
            "committed".to_string()
        } else {
            spillover.push(format!("exit {}", result.exit));
            "failed".to_string()
        }
    });
    let detail = match (detail, spillover.is_empty()) {
        (Some(d), true) => Some(d),
        (Some(d), false) => Some(format!("{d}; {}", spillover.join(" "))),
        (None, false) => Some(spillover.join(" ")),
        (None, true) => None,
    };
    if let Some(detail) = detail {
        fields.push(("detail".to_string(), detail));
    }
    Outcome { outcome, fields }
}

/// Paths the loop carries as strings, back as validated [`RelPath`]s. The
/// loop only ever passes on what `authorize` already validated, so an entry
/// that fails here cannot be authorized and is dropped.
fn rel_paths(paths: &[String]) -> Vec<RelPath> {
    paths
        .iter()
        .filter_map(|p| paths::validate_paths(p).ok())
        .flatten()
        .collect()
}
