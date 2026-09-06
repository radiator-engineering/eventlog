use anyhow::{Context, bail};
use include_dir::{Dir, include_dir};
use std::path::{Path, PathBuf};

static SKILL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/skill");

const SKILL_NAME: &str = "event-log-coordination";
const STAMP_FILE: &str = ".eventlog-version";

/// Default install root: `~/.claude/skills`.
pub fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".claude").join("skills")
}

/// Write the embedded skill under `dest_root`, stamping it with the
/// binary's version. Refuses to overwrite a newer existing stamp unless
/// `force` is set. Returns the path the skill was written to.
pub fn install(dest_root: &Path, force: bool) -> anyhow::Result<PathBuf> {
    let dest = dest_root.join(SKILL_NAME);
    let stamp_path = dest.join(STAMP_FILE);
    let new_version = env!("CARGO_PKG_VERSION");

    if !force && stamp_path.exists() {
        let existing = std::fs::read_to_string(&stamp_path)
            .with_context(|| format!("reading {}", stamp_path.display()))?;
        let existing = existing.trim();
        if is_newer(existing, new_version) {
            bail!(
                "installed skill stamp {existing} is newer than this binary's {new_version}; use --force to overwrite"
            );
        }
    }

    std::fs::create_dir_all(dest_root)
        .with_context(|| format!("creating {}", dest_root.display()))?;
    SKILL_DIR
        .extract(dest_root)
        .with_context(|| format!("extracting skill into {}", dest_root.display()))?;
    std::fs::write(&stamp_path, new_version)
        .with_context(|| format!("writing {}", stamp_path.display()))?;

    Ok(dest)
}

fn parse_version(s: &str) -> Vec<u64> {
    s.trim_start_matches('v')
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

fn is_newer(existing: &str, current: &str) -> bool {
    parse_version(existing) > parse_version(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_is_numeric_not_lexical() {
        assert!(is_newer("99.0.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.9.0", "0.10.0"));
    }
}
