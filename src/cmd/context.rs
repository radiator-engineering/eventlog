//! `eventlog context`: the controller's context rendered from the log.

use std::io::Write;

use anyhow::Context as _;

use crate::cli::{Args, Command, ContextInner};
use crate::cmd::agents::load_context;
use crate::context::{check, packet, worktree};
use crate::model::config;

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Context(ctx_args) = &args.command else {
        anyhow::bail!("context::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;

    if ctx_args.budget.is_some() && ctx_args.inner.is_some() {
        anyhow::bail!("--budget is not valid with a `context` subcommand");
    }

    if let Some(ContextInner::Install { classic, force }) = &ctx_args.inner {
        // Install writes under `<root>/.claude/skills/`, which Claude Code only
        // loads from the project's git top level, not from a subdirectory's own
        // `.claude/`; every other subcommand only reads files, so the cwd is fine.
        let install_root = git_toplevel(&root).unwrap_or_else(|| root.clone());
        return crate::context::install::run(&install_root, *classic, *force);
    }

    let q = load_context(args, None)?;
    q.cfg.context.validate()?;

    if let Some(ContextInner::Check { percent, growth }) = &ctx_args.inner {
        let v = check::decide(
            *percent,
            *growth,
            &q.cfg.context,
            &q.events,
            &q.state,
            || worktree::collect(&root, &q.state),
        );
        println!("{}", v.to_json());
        return Ok(0);
    }

    let work = worktree::collect(&root, &q.state);
    let budget = ctx_args.budget.unwrap_or(q.cfg.context.budget_chars);
    let log_path = config::resolve_log(&q.cfg, args.log.as_deref())
        .display()
        .to_string();
    let read = |p: &str| std::fs::read_to_string(root.join(p)).ok();
    let exists = |p: &str| root.join(p).exists();
    let p = packet::build(
        &packet::PacketInput {
            events: &q.events,
            state: &q.state,
            log_path: &log_path,
            work: &work,
            read_file: &read,
            exists: &exists,
        },
        budget,
    );
    if p.over_budget {
        eprintln!(
            "eventlog: context packet is over budget ({budget} chars); kept sections were not cut"
        );
    }
    let mut out = std::io::stdout().lock();
    if args.json {
        writeln!(out, "{}", p.to_json(&q.cfg.context))?;
    } else {
        write!(out, "{}", p.markdown())?;
    }
    Ok(0)
}

/// The git repository's top level as seen from `cwd`, or `None` outside a
/// git repository (or if `git` is unavailable).
fn git_toplevel(cwd: &std::path::Path) -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    Some(std::path::PathBuf::from(text.trim_end_matches('\n')))
}
