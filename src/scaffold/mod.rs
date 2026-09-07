//! Scaffold: `init`, `protect`, and shared helpers for `doctor`.

pub mod doctor;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

const EVENTLOG_MD: &str = include_str!("templates/EVENTLOG.md");
const EVENTLOG_TOML: &str = include_str!("templates/eventlog.toml");
const SETUP_TOML: &str = include_str!("templates/eventlog-setup.toml");
const DROVE_REACTORS: &str = include_str!("templates/eventlog-reactors.star");
pub const SETUP_CONFIG: &str = ".context/eventlog-setup.toml";
const DROVE_REACTORS_PATH: &str = ".context/eventlog-reactors.star";

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

/// The reusable setup's project-managed configuration. This is deliberately
/// separate from `eventlog.toml`: the latter is a log vocabulary, while this
/// file contains reactor policy that projects are expected to customize.
pub fn setup_changes(repo_root: &Path) -> anyhow::Result<Vec<SetupChange>> {
    let path = repo_root.join(SETUP_CONFIG);
    match fs::read_to_string(&path) {
        Ok(text) => {
            let policy: SetupPolicy = toml::from_str(&text).map_err(|err| {
                anyhow::anyhow!(
                    "setup configuration conflict in {}: {err}; fix it before applying an upgrade",
                    path.display()
                )
            })?;
            let mut changes = Vec::new();
            if text == legacy_setup_v1() {
                changes.push(SetupChange {
                    path: path.clone(),
                    content: SETUP_TOML.into(),
                });
            }
            reconcile_drove_reactors(repo_root, &policy, &mut changes)
                .with_context(|| format!("validating setup policy in {}", path.display()))?;
            Ok(changes)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let policy: SetupPolicy =
                toml::from_str(SETUP_TOML).expect("shipped setup TOML is valid");
            Ok(vec![
                SetupChange {
                    path,
                    content: SETUP_TOML.into(),
                },
                SetupChange {
                    path: repo_root.join(DROVE_REACTORS_PATH),
                    content: render_drove_reactors(&policy)
                        .expect("shipped setup TOML renders a valid helper"),
                },
            ])
        }
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

fn legacy_setup_v1() -> String {
    SETUP_TOML
        .replacen("v2", "v1", 1)
        .replacen("version = 2", "version = 1", 1)
}

pub struct SetupChange {
    pub path: PathBuf,
    pub content: String,
}

#[derive(serde::Deserialize)]
struct SetupPolicy {
    commit: ReactorPolicy,
    docs: DocsPolicy,
    invocation: Invocation,
}

#[derive(serde::Deserialize)]
struct ReactorPolicy {
    identity: String,
    model: String,
    timeout: String,
}
#[derive(serde::Deserialize)]
struct DocsPolicy {
    identity: String,
    model: String,
    timeout: String,
    roots: Vec<String>,
}
#[derive(serde::Deserialize)]
struct Invocation {
    eventlog: String,
}

fn reconcile_drove_reactors(
    repo_root: &Path,
    policy: &SetupPolicy,
    changes: &mut Vec<SetupChange>,
) -> anyhow::Result<()> {
    let path = repo_root.join(DROVE_REACTORS_PATH);
    let expected = render_drove_reactors(policy)?;
    match fs::read_to_string(&path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => changes.push(SetupChange {
            path,
            content: expected,
        }),
        Ok(existing) if existing == expected => {}
        // The prior v2 helper had no fingerprint and is safe to migrate once.
        Ok(existing) if existing == DROVE_REACTORS => changes.push(SetupChange {
            path,
            content: expected,
        }),
        Ok(existing) if managed_helper_is_intact(&existing) => changes.push(SetupChange {
            path,
            content: expected,
        }),
        Ok(_) => anyhow::bail!(
            "generated Drove reactor helper was customized: {}; preview refuses to overwrite it",
            path.display()
        ),
        Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
    }
    Ok(())
}

fn render_drove_reactors(policy: &SetupPolicy) -> anyhow::Result<String> {
    let q = |value: &str| serde_json::to_string(value).expect("string JSON");
    let docs_paths = docs_claim_paths(&policy.docs.roots)?;
    let body = format!(
        "# Generated by eventlog setup. Load eventlog_reactors() into an existing workspace panes list.\ndef eventlog_reactors():\n    committer = herdr.tab({commit_model} + \": commit reactor\", panes = [\n        pane(\"committer-reactor\", serve = [{exe}, \"react\", \"--as\", {commit_id}, \"--on\", \"result\", \"--timeout\", {commit_timeout}, \"--git\", \"--\", {exe}, \"action\", \"commit\"], ready = output(\"watching\"), on_start = [{exe}, \"lifecycle\", \"start\", {commit_id}, \"--model\", {commit_model}, \"--paths\", \".\"], on_stop = [{exe}, \"lifecycle\", \"stop\", {commit_id}]),\n    ])\n    docs = herdr.tab({docs_model} + \": docs reactor\", panes = [\n        pane(\"docs-reactor\", serve = [{exe}, \"react\", \"--as\", {docs_id}, \"--on\", \"ack\", \"--filter\", \"by=\" + {commit_id}, \"--filter\", \"outcome=committed\", \"--timeout\", {docs_timeout}, \"--\", {exe}, \"action\", \"docs\"], ready = output(\"watching\"), after = [\"committer-reactor\"], on_start = [{exe}, \"lifecycle\", \"start\", {docs_id}, \"--model\", {docs_model}, \"--paths\", {docs_paths}], on_stop = [{exe}, \"lifecycle\", \"stop\", {docs_id}]),\n    ])\n    return [committer, docs]\n",
        exe = q(&policy.invocation.eventlog),
        commit_id = q(&policy.commit.identity),
        commit_model = q(&policy.commit.model),
        commit_timeout = q(&policy.commit.timeout),
        docs_id = q(&policy.docs.identity),
        docs_model = q(&policy.docs.model),
        docs_timeout = q(&policy.docs.timeout),
        docs_paths = q(&docs_paths),
    );
    use sha2::{Digest, Sha256};
    Ok(format!(
        "# eventlog-reactors managed body-sha256={:x}\n{}",
        Sha256::digest(body.as_bytes()),
        body
    ))
}

fn managed_helper_is_intact(text: &str) -> bool {
    let Some((header, body)) = text.split_once('\n') else {
        return false;
    };
    let Some(expected) = header.strip_prefix("# eventlog-reactors managed body-sha256=") else {
        return false;
    };
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(body.as_bytes())) == expected
}

fn docs_claim_paths(roots: &[String]) -> anyhow::Result<String> {
    if roots.is_empty() {
        anyhow::bail!("docs.roots must name at least one root");
    }
    let mut claims = Vec::new();
    for root in roots {
        let paths = crate::model::paths::validate_paths(root)
            .map_err(|err| anyhow::anyhow!("invalid docs.root {root:?}: {err}"))?;
        if paths.len() != 1 {
            anyhow::bail!("docs.root {root:?} must name one path");
        }
        let root = paths[0].as_str();
        claims.push(root.to_string());
    }
    Ok(claims.join(","))
}

/// Apply all validated setup changes. Callers must obtain `setup_changes`
/// first, so conflicts are always discovered before the first write.
pub fn apply_setup(changes: &[SetupChange]) -> anyhow::Result<()> {
    for change in changes {
        let parent = change.path.parent().expect("setup config has a parent");
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    for change in changes {
        fs::write(&change.path, &change.content)
            .with_context(|| format!("writing {}", change.path.display()))?;
    }
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
