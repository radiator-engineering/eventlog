//! `eventlog-reactors setup`: the opt-in reactor policy and the Drove helper
//! generated from it. The log itself is set up by `eventlog setup`; this
//! module only adds what the reactors need on top of it.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

const SETUP_TOML: &str = include_str!("templates/eventlog-setup.toml");
/// The template `eventlog setup` shipped up to 0.5. An untouched copy, or its
/// v1 form, is upgraded to [`SETUP_TOML`]; anything else is project-owned.
const SETUP_TOML_0_5: &str = include_str!("templates/eventlog-setup-v2-0.5.toml");
/// The first v2 helper, which had no fingerprint; it is safe to migrate once.
const DROVE_REACTORS: &str = include_str!("templates/eventlog-reactors.star");
pub const SETUP_CONFIG: &str = ".context/eventlog-setup.toml";
const DROVE_REACTORS_PATH: &str = ".context/eventlog-reactors.star";

/// Reactor locks live next to the log; they are runtime state, never content.
const GITIGNORE_LINES: &[&str] = &[".context/*.reactor.lock/"];

/// The reactor policy file is project-managed configuration: projects are
/// expected to customize it, and an upgrade must preserve their edits.
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
            if text == legacy_setup_v1() || text == SETUP_TOML_0_5 {
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
    SETUP_TOML_0_5
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
    /// Absent from policies written before the reactors moved out of
    /// `eventlog`; those get the binary from PATH.
    #[serde(default = "default_reactors")]
    reactors: String,
}

fn default_reactors() -> String {
    "eventlog-reactors".into()
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
        "# Generated by eventlog-reactors setup. Load eventlog_reactors() into an existing workspace panes list.\ndef eventlog_reactors():\n    committer = herdr.tab({commit_model} + \": commit reactor\", panes = [\n        pane(\"committer-reactor\", serve = [{rx}, \"react\", \"--as\", {commit_id}, \"--on\", \"result\", \"--timeout\", {commit_timeout}, \"--git\", \"--\", {rx}, \"action\", \"commit\"], ready = output(\"watching\"), on_start = [{exe}, \"lifecycle\", \"start\", {commit_id}, \"--role\", \"reactor\", \"--model\", {commit_model}, \"--paths\", \".\"], on_stop = [{exe}, \"lifecycle\", \"stop\", {commit_id}]),\n    ])\n    docs = herdr.tab({docs_model} + \": docs reactor\", panes = [\n        pane(\"docs-reactor\", serve = [{rx}, \"react\", \"--as\", {docs_id}, \"--on\", \"ack\", \"--filter\", \"by=\" + {commit_id}, \"--filter\", \"outcome=committed\", \"--timeout\", {docs_timeout}, \"--\", {rx}, \"action\", \"docs\"], ready = output(\"watching\"), after = [\"committer-reactor\"], on_start = [{exe}, \"lifecycle\", \"start\", {docs_id}, \"--role\", \"reactor\", \"--model\", {docs_model}, \"--paths\", {docs_paths}], on_stop = [{exe}, \"lifecycle\", \"stop\", {docs_id}]),\n    ])\n    return [committer, docs]\n",
        exe = q(&policy.invocation.eventlog),
        rx = q(&policy.invocation.reactors),
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
        let paths = eventlog::model::paths::validate_paths(root)
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
pub fn apply_setup(repo_root: &Path, changes: &[SetupChange]) -> anyhow::Result<()> {
    for change in changes {
        let parent = change.path.parent().expect("setup config has a parent");
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    for change in changes {
        fs::write(&change.path, &change.content)
            .with_context(|| format!("writing {}", change.path.display()))?;
    }
    eventlog::scaffold::append_unique_lines(&repo_root.join(".gitignore"), GITIGNORE_LINES)
}
