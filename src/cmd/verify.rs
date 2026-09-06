use std::path::Path;

use anyhow::Context;

use crate::cli::Command;
use crate::log::Log;
use crate::log::verify::verify;
use crate::model::config::{load, resolve_log};

pub fn run(args: &crate::cli::Args) -> anyhow::Result<i32> {
    let Command::Verify(_) = &args.command else {
        anyhow::bail!("verify::run called with wrong subcommand");
    };

    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = load(&repo_root)?;
    let path = resolve_log_path(&repo_root, &cfg, args.log.as_deref());
    let report = verify(&Log::open(path))?;

    if let Some(failure) = report.failure {
        eprintln!("{failure} (last good seq: {})", report.last_good);
        return Ok(1);
    }

    println!("ok: {} events, chain intact", report.checked);
    Ok(0)
}

fn resolve_log_path(
    repo_root: &Path,
    cfg: &crate::model::config::Config,
    selector: Option<&str>,
) -> std::path::PathBuf {
    let path = resolve_log(cfg, selector);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}
