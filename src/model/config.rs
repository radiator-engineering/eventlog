//! Config loading and precedence (spec section 4).
//!
//! `.context/eventlog.toml` in the repo wins; then
//! `~/.config/eventlog/config.toml`; then the built-in defaults. Only the
//! first file found is read — the two are alternatives, not layers.

use crate::model::allow::Allowlist;
use crate::model::vocab::{TypeSpec, Vocabulary};
use anyhow::Context;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_LOG: &str = ".context/events.jsonl";
const REPO_CONFIG: &str = ".context/eventlog.toml";

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub log: LogConfig,
    pub vocabulary: Vocabulary,
    pub writers: Allowlist,
    pub view: ViewConfig,
    pub keys: KeysConfig,
}

#[derive(Clone, Debug)]
pub struct LogConfig {
    pub path: PathBuf,
    pub fsync: bool,
    /// Extra logs, addressed by `eventlog --log <name>`.
    pub named: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ViewConfig {
    pub columns: Vec<String>,
    /// type -> ANSI SGR parameters, e.g. `result = "1;36"`.
    pub colors: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct KeysConfig {
    pub filter: String,
    pub follow: String,
    pub open: String,
    pub panes: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            path: PathBuf::from(DEFAULT_LOG),
            fsync: false,
            named: BTreeMap::new(),
        }
    }
}

impl Default for ViewConfig {
    fn default() -> Self {
        ViewConfig {
            columns: ["seq", "type", "agent", "summary", "ref"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            colors: BTreeMap::new(),
        }
    }
}

impl Default for KeysConfig {
    fn default() -> Self {
        KeysConfig {
            filter: "/".to_string(),
            follow: "f".to_string(),
            open: "o".to_string(),
            panes: "tab".to_string(),
        }
    }
}

/// The file shape. Every table is optional; a missing table keeps the default.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    log: Option<FileLog>,
    #[serde(default)]
    vocabulary: BTreeMap<String, TypeSpec>,
    #[serde(default)]
    writers: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    view: Option<FileView>,
    #[serde(default)]
    keys: Option<FileKeys>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLog {
    path: Option<PathBuf>,
    fsync: Option<bool>,
    #[serde(default)]
    named: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileView {
    columns: Option<Vec<String>>,
    #[serde(default)]
    colors: BTreeMap<String, String>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileKeys {
    filter: Option<String>,
    follow: Option<String>,
    open: Option<String>,
    panes: Option<String>,
}

/// Load `<repo_root>/.context/eventlog.toml`, else
/// `~/.config/eventlog/config.toml`, else the defaults.
pub fn load(repo_root: &Path) -> anyhow::Result<Config> {
    let mut cfg = Config::default();
    let Some(path) = config_path(repo_root) else {
        return Ok(cfg);
    };
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let file: FileConfig =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;

    if let Some(log) = file.log {
        if let Some(p) = log.path {
            cfg.log.path = p;
        }
        if let Some(f) = log.fsync {
            cfg.log.fsync = f;
        }
        cfg.log.named = log.named;
    }
    cfg.vocabulary.merge_file(file.vocabulary);
    cfg.writers.merge_file(file.writers);
    if let Some(view) = file.view {
        if let Some(c) = view.columns {
            cfg.view.columns = c;
        }
        cfg.view.colors = view.colors;
    }
    if let Some(keys) = file.keys {
        if let Some(k) = keys.filter {
            cfg.keys.filter = k;
        }
        if let Some(k) = keys.follow {
            cfg.keys.follow = k;
        }
        if let Some(k) = keys.open {
            cfg.keys.open = k;
        }
        if let Some(k) = keys.panes {
            cfg.keys.panes = k;
        }
    }
    Ok(cfg)
}

fn config_path(repo_root: &Path) -> Option<PathBuf> {
    let repo = repo_root.join(REPO_CONFIG);
    if repo.is_file() {
        return Some(repo);
    }
    let home = std::env::var_os("HOME")?;
    let user = PathBuf::from(home).join(".config/eventlog/config.toml");
    user.is_file().then_some(user)
}

/// Which log a `--log` selector names: nothing means the default log, a key in
/// `[log.named]` means that log, anything else is taken as a path.
pub fn resolve_log(cfg: &Config, selector: Option<&str>) -> PathBuf {
    match selector {
        None => cfg.log.path.clone(),
        Some(s) => match cfg.log.named.get(s) {
            Some(p) => p.clone(),
            None => PathBuf::from(s),
        },
    }
}
