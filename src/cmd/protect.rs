use std::path::{Path, PathBuf};

use crate::cli::{Args, Command};
use crate::model::config;

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Protect(protect_args) = &args.command else {
        unreachable!("cmd::protect::run dispatched for a non-Protect command")
    };
    let repo_root = std::env::current_dir()?;
    let cfg = config::load(&repo_root)?;
    let log = log_path(&repo_root, &cfg, args.log.as_deref());

    if protect_args.status {
        let protected = crate::scaffold::is_protected(&log)?;
        return Ok(if protected { 0 } else { 1 });
    }

    let enable = !protect_args.off;
    crate::scaffold::protect(&log, enable)?;
    Ok(0)
}

fn log_path(repo_root: &Path, cfg: &config::Config, selector: Option<&str>) -> PathBuf {
    let path = config::resolve_log(cfg, selector);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}
