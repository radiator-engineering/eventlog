//! `eventlog-reactors doctor`: the reactor policy, its generated helper, and
//! the reactor locks next to the log.

use std::fs;
use std::path::Path;

use anyhow::Context;

use eventlog::model::config;

use crate::cli::{Args, Command};
use crate::react::lock::Token;

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Doctor(_) = &args.command else {
        anyhow::bail!("doctor::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&root)?;
    let log = config::resolve_log(&cfg, args.log.as_deref());
    let log = if log.is_absolute() {
        log
    } else {
        root.join(log)
    };

    let mut failed = false;
    if !root.join(crate::setup::SETUP_CONFIG).is_file() {
        println!(
            "[WARN] reactor setup: {} missing; run eventlog-reactors setup apply",
            crate::setup::SETUP_CONFIG
        );
    } else {
        match crate::setup::setup_changes(&root) {
            Ok(changes) if changes.is_empty() => println!("[ OK ] reactor setup"),
            Ok(_) => {
                println!("[WARN] reactor setup: out of date; run eventlog-reactors setup upgrade")
            }
            Err(err) => {
                failed = true;
                println!("[FAIL] reactor setup: {err:#}");
            }
        }
    }
    for (name, stale) in reactor_locks(&log) {
        if stale {
            println!("[WARN] reactor lock {name}: stale or unreadable");
        } else {
            println!("[ OK ] reactor lock {name}");
        }
    }
    Ok(if failed { 1 } else { 0 })
}

/// Every `<log>.<name>.reactor.lock/` next to the log, and whether its
/// token no longer names a live process.
fn reactor_locks(log: &Path) -> Vec<(String, bool)> {
    let (Some(parent), Some(file)) = (log.parent(), log.file_name().and_then(|n| n.to_str()))
    else {
        return Vec::new();
    };
    let prefix = format!("{file}.");
    let me = Token::current();
    let Ok(read) = fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut locks = Vec::new();
    for entry in read.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".reactor.lock") || !entry.path().is_dir()
        {
            continue;
        }
        let token = fs::read_to_string(entry.path().join("token"))
            .ok()
            .and_then(|t| Token::from_json(&t));
        let stale = token.as_ref().is_none_or(|t| !t.is_live(&me));
        locks.push((name, stale));
    }
    locks.sort();
    locks
}
