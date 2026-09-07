use anyhow::Context;

use crate::cli::{Args, Command, LifecycleInner};
use crate::cmd::append::FoldContext;
use crate::log::Log;
use crate::log::append::{AppendRequest, append};
use crate::model::config;
use crate::query;

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Lifecycle(lifecycle) = &args.command else {
        anyhow::bail!("lifecycle::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&root)?;
    let path = config::resolve_log(&cfg, args.log.as_deref());
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    match &lifecycle.inner {
        LifecycleInner::Start {
            agent,
            model,
            role,
            paths,
        } => start(&path, &cfg, agent, model.as_deref(), role, paths),
        LifecycleInner::Stop { agent } => stop(&path, &cfg, agent),
    }
}

fn start(
    log_path: &std::path::Path,
    cfg: &config::Config,
    agent: &str,
    model: Option<&str>,
    role: &str,
    paths: &[String],
) -> anyhow::Result<i32> {
    let log = Log::open(log_path);
    validate_claim_paths(paths)?;
    let state = query::fold(&log.read()?.events, cfg);
    let open = state
        .agents
        .get(agent)
        .is_some_and(|a| a.retired_at.is_none());
    if !open {
        let mut fields = vec![("agent".into(), agent.into()), ("role".into(), role.into())];
        if let Some(model) = model {
            fields.push(("model".into(), model.into()));
        }
        append_request(&log, cfg, "spawn", fields)?;
    }
    let state = query::fold(&log.read()?.events, cfg);
    let existing = state
        .agents
        .get(agent)
        .map(|a| a.claims.clone())
        .unwrap_or_default();
    let missing: Vec<_> = paths
        .iter()
        .filter(|p| !existing.contains(*p))
        .cloned()
        .collect();
    if !missing.is_empty() {
        append_request(
            &log,
            cfg,
            "claim",
            vec![
                ("agent".into(), agent.into()),
                ("paths".into(), missing.join(",")),
            ],
        )?;
    }
    println!("lifecycle: {agent} active");
    Ok(0)
}

fn stop(log_path: &std::path::Path, cfg: &config::Config, agent: &str) -> anyhow::Result<i32> {
    let log = Log::open(log_path);
    let state = query::fold(&log.read()?.events, cfg);
    if state
        .agents
        .get(agent)
        .is_some_and(|a| a.retired_at.is_none())
    {
        append_request(
            &log,
            cfg,
            "retire",
            vec![
                ("agent".into(), agent.into()),
                ("disposition".into(), "stopped".into()),
            ],
        )?;
    }
    println!("lifecycle: {agent} stopped");
    Ok(0)
}

fn append_request(
    log: &Log,
    cfg: &config::Config,
    ty: &str,
    fields: Vec<(String, String)>,
) -> anyhow::Result<()> {
    let state = query::fold(&log.read()?.events, cfg);
    let context = FoldContext::new(&state);
    append(
        log,
        cfg,
        AppendRequest {
            r#type: ty.into(),
            fields,
            writer: "controller".into(),
            strict: true,
            dry_run: false,
        },
        Some(&context),
    )
    .map(|_| ())
    .map_err(|err| anyhow::anyhow!(err))
}

/// Check every requested claim before spawning a new lifecycle, avoiding a
/// half-created lifecycle when its first claim is invalid. `append` repeats
/// the authoritative strict check immediately before the claim is written.
fn validate_claim_paths(paths: &[String]) -> anyhow::Result<()> {
    for raw in paths {
        for path in crate::model::paths::validate_paths(raw)
            .map_err(|err| anyhow::anyhow!("invalid lifecycle claim: {err}"))?
        {
            let candidate = std::path::Path::new(path.as_str());
            if candidate.exists() {
                continue;
            }
            if path.as_str().contains(['*', '?', '[']) {
                let mut builder = globset::GlobSetBuilder::new();
                builder.add(globset::Glob::new(path.as_str())?);
                let set = builder.build()?;
                if crate::log::append::glob_matches_under(std::path::Path::new("."), &set)
                    .map_err(anyhow::Error::msg)?
                {
                    continue;
                }
            }
            anyhow::bail!("strict: claim-path-missing");
        }
    }
    Ok(())
}
