use std::path::Path;

use anyhow::Context;

use crate::cli::Command;
use crate::log::Log;
use crate::model::config::{load, resolve_log};
use crate::tui::{App, run_terminal};

pub fn run(args: &crate::cli::Args) -> anyhow::Result<i32> {
    let Command::Tui(_) = &args.command else {
        anyhow::bail!("tui::run called with wrong subcommand");
    };

    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = load(&repo_root)?;
    let path = resolve_log_path(&repo_root, &cfg, args.log.as_deref());
    let log = Log::open(path);
    let report = log.read()?;
    let app = App::new(report.events, cfg);

    run_terminal(app, log)?;
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
