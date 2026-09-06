//! Open an event's ref in $EDITOR or $PAGER.

use std::path::{Path, PathBuf};
use std::process::Command as SysCommand;

use anyhow::Context;

use crate::cli::{Args as CliArgs, Command, OpenArgs};
use crate::log::Log;
use crate::model::config;

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::Open(open_args) = &args.command else {
        anyhow::bail!("open::run called with wrong subcommand");
    };
    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&repo_root)?;
    let log_path = config::resolve_log(&cfg, args.log.as_deref());
    let log_path = absolutize(&repo_root, log_path);
    run_with_paths(open_args, &repo_root, &log_path)
}

fn run_with_paths(args: &OpenArgs, repo_root: &Path, log_path: &Path) -> anyhow::Result<i32> {
    let report = Log::open(log_path).read()?;
    let Some(event) = report.events.iter().find(|e| e.seq == args.seq) else {
        anyhow::bail!("no event with seq {}", args.seq);
    };
    let Some(ref_field) = event.fields.get("ref") else {
        eprintln!("eventlog open: seq {} has no ref", args.seq);
        return Ok(1);
    };
    if ref_field.trim().is_empty() {
        eprintln!("eventlog open: seq {} has no ref", args.seq);
        return Ok(1);
    }

    let target = if Path::new(ref_field).is_absolute() {
        PathBuf::from(ref_field)
    } else {
        repo_root.join(ref_field)
    };

    if args.pager {
        let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
        let status = SysCommand::new(&pager)
            .arg(&target)
            .status()
            .with_context(|| format!("running pager {pager}"))?;
        Ok(status.code().unwrap_or(1))
    } else {
        let editor = std::env::var("EDITOR").context("$EDITOR is not set")?;
        let status = SysCommand::new(&editor)
            .arg(&target)
            .status()
            .with_context(|| format!("running editor {editor}"))?;
        Ok(status.code().unwrap_or(1))
    }
}

fn absolutize(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
