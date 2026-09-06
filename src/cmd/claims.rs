//! Report changed files that fall outside an agent's live claims.

use std::path::{Path, PathBuf};
use std::process::Command as SysCommand;

use anyhow::Context;
use globset::{Glob, GlobMatcher};

use crate::cli::{Args as CliArgs, ClaimsArgs, Command};
use crate::log::Log;
use crate::model::config::{self, Config};
use crate::model::paths::{self, RelPath};
use crate::query;

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::Claims(claims_args) = &args.command else {
        anyhow::bail!("claims::run called with wrong subcommand");
    };
    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&repo_root)?;
    let log_path = config::resolve_log(&cfg, args.log.as_deref());
    let log_path = absolutize(&repo_root, log_path);
    run_with_paths(claims_args, &cfg, &repo_root, &log_path)
}

fn run_with_paths(
    args: &ClaimsArgs,
    cfg: &Config,
    repo_root: &Path,
    log_path: &Path,
) -> anyhow::Result<i32> {
    ensure_git_repo(repo_root)?;

    let report = Log::open(log_path).read()?;
    let state = query::fold(&report.events, cfg);
    let claims = state.claims_for(&args.agent);
    if claims.is_empty() {
        eprintln!(
            "eventlog claims: {} has no live claims in {}",
            args.agent,
            log_path.display()
        );
    }

    let head = args.head.as_deref().unwrap_or("HEAD");
    let changed = changed_files(repo_root, &args.base, head)?;
    let mut gaps = Vec::new();
    for path in &changed {
        let canonical = canonical_rel(repo_root, path).unwrap_or_else(|| path.clone());
        if !claims.iter().any(|glob| covers(glob, &canonical)) {
            println!("unclaimed {canonical}");
            gaps.push(canonical);
        }
    }

    if gaps.is_empty() {
        if claims.is_empty() && changed.is_empty() {
            return Ok(0);
        }
        if !claims.is_empty() {
            println!(
                "eventlog claims: {} stayed inside its claims ({}..{})",
                args.agent, args.base, head
            );
        }
        return Ok(0);
    }

    eprintln!(
        "eventlog claims: {} touched {} unclaimed file(s) ({}..{}); consider: append-event.sh violation agent={} paths={}",
        args.agent,
        gaps.len(),
        args.base,
        head,
        args.agent,
        gaps.join(",")
    );
    Ok(1)
}

fn ensure_git_repo(root: &Path) -> anyhow::Result<()> {
    let status = SysCommand::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .context("running git rev-parse")?;
    if !status.status.success() {
        anyhow::bail!("{} is not a git repository", root.display());
    }
    Ok(())
}

fn changed_files(root: &Path, base: &str, head: &str) -> anyhow::Result<Vec<String>> {
    let mut paths = git_diff_names(root, base, head)?;
    paths.extend(untracked_files(root)?);
    paths.sort_unstable();
    paths.dedup();
    Ok(paths)
}

fn git_diff_names(root: &Path, base: &str, head: &str) -> anyhow::Result<Vec<String>> {
    let output = SysCommand::new("git")
        .args(["diff", "--name-only", "--find-renames", base, head])
        .current_dir(root)
        .output()
        .with_context(|| format!("git diff --name-only --find-renames {base} {head}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(parse_lines(&output.stdout))
}

fn untracked_files(root: &Path) -> anyhow::Result<Vec<String>> {
    let output = SysCommand::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .context("git status --porcelain")?;
    if !output.status.success() {
        anyhow::bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(parse_lines(&output.stdout)
        .into_iter()
        .filter_map(parse_untracked)
        .collect())
}

fn parse_untracked(line: String) -> Option<String> {
    if line.len() < 4 {
        return None;
    }
    let status = &line[..2];
    if status != "??" {
        return None;
    }
    let path = line[3..].trim_end().trim_end_matches('/').to_string();
    (!path.is_empty()).then_some(path)
}

fn parse_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn canonical_rel(root: &Path, path: &str) -> Option<String> {
    let rel = paths::validate_paths(path).ok()?.into_iter().next()?;
    let candidate = root.join(rel.as_str());
    if candidate.exists() {
        let abs = paths::canonicalize(root, &rel).ok()?;
        let root = root.canonicalize().ok()?;
        return abs
            .strip_prefix(&root)
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
    }
    Some(normalize_rel(rel))
}

fn normalize_rel(rel: RelPath) -> String {
    rel.as_str().replace('\\', "/")
}

fn covers(glob: &str, path: &str) -> bool {
    if glob == path {
        return true;
    }
    if path.starts_with(&format!("{}/", glob.trim_end_matches('/'))) {
        return true;
    }
    matcher(glob).is_some_and(|m| m.is_match(path))
}

fn matcher(glob: &str) -> Option<GlobMatcher> {
    Glob::new(glob).ok().map(|g| g.compile_matcher())
}

fn absolutize(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
