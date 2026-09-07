//! Scaffold: `init`, `protect`, and shared helpers for `doctor`.

pub mod doctor;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

const EVENTLOG_MD: &str = include_str!("templates/EVENTLOG.md");
const EVENTLOG_TOML: &str = include_str!("templates/eventlog.toml");

const GITIGNORE_LINES: &[&str] = &[
    ".context/events.jsonl",
    ".context/events.jsonl.lock",
    ".context/*.reactor.lock/",
    ".context/layout.json",
];

const GITATTRIBUTES_LINE: &str = ".context/events.jsonl -text";

/// Create `.context/events.jsonl`, templates, and git metadata lines. Idempotent.
pub fn init(repo_root: &Path) -> anyhow::Result<()> {
    let context = repo_root.join(".context");
    fs::create_dir_all(&context).context("creating .context")?;

    let log = context.join("events.jsonl");
    if !log.exists() {
        fs::File::create(&log).context("creating events.jsonl")?;
    }

    // Templates are starting points the project then maintains by hand, so a
    // rerun must never overwrite one that exists, edited or not.
    write_if_missing(&context.join("EVENTLOG.md"), EVENTLOG_MD)?;
    write_if_missing(&context.join("eventlog.toml"), EVENTLOG_TOML)?;

    append_unique_lines(&repo_root.join(".gitignore"), GITIGNORE_LINES)?;
    append_unique_line(&repo_root.join(".gitattributes"), GITATTRIBUTES_LINE)?;
    Ok(())
}

/// Enable or disable OS-level append-only protection on `log_path`.
pub fn protect(log_path: &Path, enable: bool) -> anyhow::Result<()> {
    if !log_path.is_file() {
        anyhow::bail!("log file not found: {}", log_path.display());
    }
    #[cfg(target_os = "macos")]
    {
        let flag = if enable { "uappnd" } else { "nouappnd" };
        run_status(Command::new("chflags").arg(flag).arg(log_path), "chflags")?;
    }
    #[cfg(target_os = "linux")]
    {
        let flag = if enable { "+a" } else { "-a" };
        run_status(Command::new("chattr").arg(flag).arg(log_path), "chattr")?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = enable;
        anyhow::bail!("OS-level log protection is not supported on this platform");
    }
    Ok(())
}

/// Report whether `log_path` is append-only protected.
pub fn is_protected(log_path: &Path) -> anyhow::Result<bool> {
    if !log_path.is_file() {
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("stat")
            .args(["-f", "%Sf", &log_path.display().to_string()])
            .output()
            .context("stat")?;
        if !output.status.success() {
            return Ok(false);
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .any(|f| f == "uappnd"))
    }
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("lsattr")
            .arg(log_path)
            .output()
            .context("lsattr")?;
        if !output.status.success() {
            return Ok(false);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        return Ok(text
            .split_whitespace()
            .nth(1)
            .is_some_and(|attrs| attrs.contains('a')));
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = log_path;
        Ok(false)
    }
}

/// Default log path under a repo root.
pub fn default_log(repo_root: &Path) -> PathBuf {
    repo_root.join(".context/events.jsonl")
}

fn write_if_missing(path: &Path, content: &str) -> anyhow::Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, content).with_context(|| format!("writing {}", path.display()))
}

fn append_unique_lines(path: &Path, lines: &[&str]) -> anyhow::Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut out = existing.clone();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    let mut changed = false;
    for line in lines {
        if !existing.lines().any(|l| l.trim() == *line) {
            out.push_str(line);
            out.push('\n');
            changed = true;
        }
    }
    if changed {
        fs::write(path, out).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

fn append_unique_line(path: &Path, line: &str) -> anyhow::Result<()> {
    append_unique_lines(path, &[line])
}

fn run_status(cmd: &mut Command, name: &str) -> anyhow::Result<()> {
    let output = cmd.output().with_context(|| format!("running {name}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{name} failed: {stderr}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_is_idempotent() {
        let dir = tempdir().unwrap();
        init(dir.path()).unwrap();
        let md1 = fs::read_to_string(dir.path().join(".context/EVENTLOG.md")).unwrap();
        init(dir.path()).unwrap();
        let md2 = fs::read_to_string(dir.path().join(".context/EVENTLOG.md")).unwrap();
        assert_eq!(md1, md2);
    }

    /// The templates are starting points the project edits by hand; a rerun
    /// must leave those edits alone rather than restore the template.
    #[test]
    fn init_keeps_a_hand_edited_template() {
        let dir = tempdir().unwrap();
        init(dir.path()).unwrap();
        let md = dir.path().join(".context/EVENTLOG.md");
        let toml = dir.path().join(".context/eventlog.toml");
        fs::write(&md, "# mine\n").unwrap();
        fs::write(&toml, "[log]\nfsync = true\n").unwrap();
        init(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(&md).unwrap(), "# mine\n");
        assert_eq!(fs::read_to_string(&toml).unwrap(), "[log]\nfsync = true\n");
    }
}
