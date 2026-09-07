//! Built-in actions intended for `eventlog react ... -- eventlog action ...`.
//! They use argv throughout; configuration is never interpolated into a shell.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command as ProcessCommand;

use anyhow::Context;
use serde::Deserialize;

use crate::cli::{ActionInner, Args, Command};
use crate::log::Log;
use crate::log::append::{AppendRequest, append};
use crate::model::config as log_config;

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Action(action) = &args.command else {
        anyhow::bail!("action::run called with wrong subcommand");
    };
    match &action.inner {
        ActionInner::Commit { message } => commit(message),
        ActionInner::Docs => docs(),
    }
}

fn commit(message: &str) -> anyhow::Result<i32> {
    let scopes = action_paths()?;
    if scopes.is_empty() {
        report("skipped", "no EVENTLOG_PATHS", None);
        return Ok(0);
    }
    let paths = resolve_scope(&scopes)?;
    if paths.is_empty() {
        report("skipped", "no files in EVENTLOG_PATHS", None);
        return Ok(0);
    }
    let staged = git_output(&["diff", "--cached", "--name-only", "-z"])?;
    let staged: BTreeSet<String> = nul_list(&staged).into_iter().collect();
    let overlap: Vec<_> = paths
        .iter()
        .filter(|path| staged.contains(*path))
        .cloned()
        .collect();
    if !overlap.is_empty() {
        report(
            "failed",
            &format!("requested paths already staged: {}", overlap.join(",")),
            None,
        );
        return Ok(1);
    }

    let mut status_args = vec!["--literal-pathspecs", "status", "--porcelain", "-z", "--"];
    status_args.extend(paths.iter().map(String::as_str));
    if git_output(&status_args)?.is_empty() {
        report("skipped", "no changes in EVENTLOG_PATHS", None);
        return Ok(0);
    }
    let policy = match std::fs::read_to_string(crate::scaffold::SETUP_CONFIG) {
        Ok(text) => toml::from_str::<SetupConfig>(&text).context("parse commit setup config")?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => SetupConfig::default(),
        Err(err) => return Err(err).context("read commit setup config"),
    };
    if !policy.commit.command.is_empty() {
        return match super::commit_command::run(
            &paths,
            &policy.commit.command,
            &policy.commit.model,
            message,
        ) {
            Ok(result) => {
                report_with_ref(
                    "committed",
                    &result.detail,
                    Some(&result.paths.join(",")),
                    Some(&result.refs.join(",")),
                );
                Ok(0)
            }
            Err(err) => {
                report("failed", &format!("{err:#}"), None);
                Ok(1)
            }
        };
    }

    // `git add -- <paths>` admits untracked files and records deletions, while
    // `commit --only` ensures unrelated pre-staged work remains staged rather
    // than being swept into this reactor's commit.
    // Add each path separately: passing all paths after one `--` is safe, but
    // individual argv values also make the exact commit scope obvious here.
    for path in &paths {
        git_status(&["add", "--", path])?;
    }
    let mut command = ProcessCommand::new("git");
    command.args(["commit", "--only", "-m", message, "--"]);
    command.args(&paths);
    let output = command.output().context("running git commit --only")?;
    if !output.status.success() {
        report("failed", &stderr_detail(&output.stderr), None);
        return Ok(1);
    }
    let oid = String::from_utf8(git_output(&["rev-parse", "--verify", "HEAD^{commit}"])?)
        .context("git returned a non-UTF-8 commit OID")?
        .trim()
        .to_string();
    report_with_ref("committed", "", Some(&paths.join(",")), Some(&oid));
    Ok(0)
}

/// Expand Git's pathspecs before touching the index. This makes the staged
/// overlap test exact for directory and glob scopes rather than comparing the
/// raw pathspec string to an index path.
fn resolve_scope(scopes: &[String]) -> anyhow::Result<Vec<String>> {
    let mut concrete = BTreeSet::new();
    for scope in scopes {
        if scope.contains('[') || scope.contains(']') {
            anyhow::bail!("unsupported EVENTLOG_PATHS pathspec: {scope}");
        }
        let listed = git_output(&["ls-files", "-co", "--exclude-standard", "-z", "--", scope])?;
        concrete.extend(nul_list(&listed));
        let deleted = git_output(&["diff", "--name-only", "--diff-filter=D", "-z", "--", scope])?;
        concrete.extend(nul_list(&deleted));
    }
    Ok(concrete.into_iter().collect())
}

#[derive(Deserialize, Default)]
struct SetupConfig {
    #[serde(default)]
    commit: CommitConfig,
    #[serde(default)]
    docs: DocsConfig,
}

#[derive(Deserialize)]
struct CommitConfig {
    #[serde(default = "default_commit_model")]
    model: String,
    #[serde(default)]
    command: Vec<String>,
}

impl Default for CommitConfig {
    fn default() -> Self {
        Self {
            model: default_commit_model(),
            command: Vec::new(),
        }
    }
}

fn default_commit_model() -> String {
    "composer-2.5-fast".into()
}

#[derive(Deserialize, Default)]
struct DocsConfig {
    #[serde(default = "default_docs_identity")]
    identity: String,
    #[serde(default)]
    roots: Vec<String>,
    #[serde(default)]
    command: Vec<String>,
}

fn default_docs_identity() -> String {
    "doc-worker".into()
}

fn docs() -> anyhow::Result<i32> {
    let root = std::env::current_dir().context("current directory")?;
    let config_path = root.join(crate::scaffold::SETUP_CONFIG);
    let text = std::fs::read_to_string(&config_path).with_context(|| {
        format!(
            "read {}; run eventlog setup apply first",
            config_path.display()
        )
    })?;
    let config: SetupConfig = toml::from_str(&text).context("parse eventlog setup config")?;
    let refs = ack_context(&config.docs.identity)?;
    if refs.as_ref().is_some_and(|context| context.docs_origin) {
        report(
            "skipped",
            "committer ack originated from documentation worker",
            None,
        );
        return Ok(0);
    }
    if config.docs.command.is_empty() {
        report(
            "failed",
            "docs.command is empty; configure a stub or documentation command",
            None,
        );
        return Ok(1);
    }
    let before = repo_snapshot()?;
    let (program, argv) = config
        .docs
        .command
        .split_first()
        .expect("non-empty checked");
    let status = ProcessCommand::new(program)
        .args(argv)
        .status()
        .with_context(|| format!("run documentation command {program}"))?;
    if !status.success() {
        report(
            "failed",
            &format!("documentation command exited {status}"),
            None,
        );
        return Ok(1);
    }
    let after = repo_snapshot()?;
    let changed: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let outside: Vec<_> = changed
        .iter()
        .filter(|path| !in_doc_roots(path, &config.docs.roots))
        .cloned()
        .collect();
    if !outside.is_empty() {
        report(
            "failed",
            &format!(
                "docs action modified outside configured roots: {}",
                outside.join(",")
            ),
            None,
        );
        return Ok(1);
    }
    let refs = refs.map(|context| context.refs).unwrap_or_default();
    append_docs_result(&config.docs.identity, &refs, &changed)?;
    report_with_ref(
        "updated",
        "",
        Some(&changed.join(",")),
        (!refs.is_empty()).then_some(refs.as_str()),
    );
    Ok(0)
}

/// In a reactor, turn the actual filesystem delta into a normal `result` so
/// the committer can process it. Direct invocations deliberately only report
/// the delta, which makes local/stub testing side-effect free.
fn append_docs_result(identity: &str, refs: &str, changed: &[String]) -> anyhow::Result<()> {
    let Ok(log_path) = std::env::var("EVENTLOG_LOG") else {
        return Ok(());
    };
    if refs.is_empty() {
        return Ok(());
    }
    let root = std::env::current_dir().context("current directory")?;
    let cfg = log_config::load(&root)?;
    append(
        &Log::open(log_path),
        &cfg,
        AppendRequest {
            r#type: "result".into(),
            fields: vec![
                ("agent".into(), identity.into()),
                ("ref".into(), refs.into()),
                ("paths".into(), changed.join(",")),
                ("summary".into(), "update documentation".into()),
            ],
            writer: identity.into(),
            strict: false,
            dry_run: false,
        },
        None,
    )
    .map(|_| ())
    .map_err(|err| anyhow::anyhow!("append documentation result: {err}"))
}

struct AckContext {
    refs: String,
    docs_origin: bool,
}

/// The document reactor reacts to a committer ack, not a source result. Its
/// origin is therefore recovered through `ack.seq_done`; old logs that carried
/// `origin=` on the ack remain supported as a fallback.
fn ack_context(docs_identity: &str) -> anyhow::Result<Option<AckContext>> {
    if std::env::var("EVENTLOG_TYPE").ok().as_deref() != Some("ack") {
        return Ok(None);
    }
    let log_path = std::env::var("EVENTLOG_LOG").context("docs ack has no EVENTLOG_LOG")?;
    let seq: u64 = std::env::var("EVENTLOG_SEQ")
        .context("docs ack has no EVENTLOG_SEQ")?
        .parse()
        .context("docs ack EVENTLOG_SEQ is invalid")?;
    let events = Log::open(log_path).read()?.events;
    let ack = events
        .iter()
        .find(|event| event.seq == seq)
        .context("docs ack sequence is absent from the log")?;
    if ack.r#type != "ack" {
        anyhow::bail!("docs action expected an ack event");
    }
    let result = ack
        .seq_ref("seq_done")
        .and_then(|done| events.iter().find(|event| event.seq == done));
    let origin = result
        .map(|event| event.writer())
        .or_else(|| ack.fields.get("origin").map(String::as_str));
    let refs = ack.fields.get("ref").cloned().unwrap_or_default();
    validate_commit_refs(&refs)?;
    Ok(Some(AckContext {
        refs,
        docs_origin: origin == Some(docs_identity),
    }))
}

fn validate_commit_refs(refs: &str) -> anyhow::Result<()> {
    let members: Vec<_> = refs
        .split(',')
        .map(str::trim)
        .filter(|member| !member.is_empty())
        .collect();
    if members.is_empty() {
        anyhow::bail!("ack has no commit ref");
    }
    for member in &members {
        let revision = format!("{member}^{{commit}}");
        let output = ProcessCommand::new("git")
            .args(["rev-parse", "--verify", &revision])
            .output()?;
        if !output.status.success() {
            anyhow::bail!("ack ref is not a commit: {member}");
        }
    }
    Ok(())
}

fn action_paths() -> anyhow::Result<Vec<String>> {
    let raw = std::env::var("EVENTLOG_PATHS").unwrap_or_default();
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let paths = crate::model::paths::validate_paths(&raw)
        .map_err(|err| anyhow::anyhow!("invalid EVENTLOG_PATHS: {err}"))?;
    Ok(paths
        .into_iter()
        .map(|path| path.as_str().to_string())
        .collect())
}

fn git_output(args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let output = ProcessCommand::new("git")
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        anyhow::bail!("git {}: {}", args.join(" "), stderr_detail(&output.stderr))
    }
}

fn git_status(args: &[&str]) -> anyhow::Result<()> {
    let output = ProcessCommand::new("git")
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("git {}: {}", args.join(" "), stderr_detail(&output.stderr))
    }
}

fn nul_list(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .split('\0')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn repo_snapshot() -> anyhow::Result<BTreeMap<String, String>> {
    let root = std::env::current_dir().context("current directory")?;
    let mut out = BTreeMap::new();
    snapshot_path(&root, &root, &mut out)?;
    Ok(out)
}

fn snapshot_path(
    root: &std::path::Path,
    path: &std::path::Path,
    out: &mut BTreeMap<String, String>,
) -> anyhow::Result<()> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    if meta.is_dir() {
        for entry in std::fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
            let entry = entry?;
            if entry.file_name() == ".git" {
                continue;
            }
            snapshot_path(root, &entry.path(), out)?;
        }
        return Ok(());
    }
    if !meta.is_file() {
        return Ok(());
    }
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();
    out.insert(relative, format!("{:x}", Sha256::digest(bytes)));
    Ok(())
}

fn in_doc_roots(path: &str, roots: &[String]) -> bool {
    roots.iter().any(|root| {
        path == root
            || path
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn stderr_detail(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr).trim().replace('\n', " ")
}

fn report(outcome: &str, detail: &str, paths: Option<&str>) {
    report_with_ref(outcome, detail, paths, None);
}

fn report_with_ref(outcome: &str, detail: &str, paths: Option<&str>, reference: Option<&str>) {
    let mut lines = vec![format!("outcome={outcome}")];
    if !detail.is_empty() {
        lines.push(format!("detail={detail}"));
    }
    if let Some(paths) = paths {
        lines.push(format!("paths={paths}"));
    }
    if let Some(reference) = reference {
        lines.push(format!("ref={reference}"));
    }
    let text = lines.join("\n") + "\n";
    if let Ok(path) = std::env::var("EVENTLOG_OUTCOME_FILE") {
        let _ = std::fs::write(path, text);
    } else {
        print!("{text}");
    }
}
