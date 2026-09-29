//! Uncommitted changes outside `.context/`, read from git.

/// One changed path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    /// Porcelain status: `M`, `A`, `D`, `R`, `??`.
    pub status: String,
    /// `+added -removed`, `new` for untracked, `binary`, or empty.
    pub stat: String,
    /// Modification time in Unix seconds; 0 for a deleted file.
    pub mtime: u64,
    /// The agent whose open claim covers the path, other than the controller.
    pub owner: Option<String>,
}

/// The working tree as the packet and `check` see it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkTree {
    /// False outside a git repository or when git fails.
    pub available: bool,
    pub changes: Vec<Change>,
}

use crate::query::State;
use globset::Glob;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

/// Who owns `path` for the purpose of the boundary rule: the first *live,
/// non-controller* claim that covers it, so a narrower worker claim (for
/// example `src/a.rs`) is not hidden by a wider controller claim (for example
/// `src`) that happens to come earlier in the claim list. A path with only a
/// controller claim, or no claim at all, has no owner.
fn owner_of(state: &State, path: &str) -> Option<String> {
    state
        .claims
        .iter()
        .find(|(glob, owner)| owner != "controller" && claim_covers(glob, path))
        .map(|(_, owner)| owner.clone())
}

/// Mirrors `query::covers`: literal equality, a directory prefix, or a glob
/// match.
fn claim_covers(glob: &str, path: &str) -> bool {
    if glob == path {
        return true;
    }
    if path.starts_with(&format!("{}/", glob.trim_end_matches('/'))) {
        return true;
    }
    Glob::new(glob)
        .map(|g| g.compile_matcher().is_match(path))
        .unwrap_or(false)
}

/// Parse `git status --porcelain=v1 -z`. A rename entry is followed by its
/// old path as a separate NUL-terminated field; keep the new name only.
pub fn parse_porcelain_z(raw: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(raw);
    let mut fields = text.split('\0').filter(|f| !f.is_empty());
    let mut out = Vec::new();
    while let Some(f) = fields.next() {
        if f.len() < 4 {
            continue;
        }
        let code = &f[..2];
        let path = f[3..].to_string();
        let status = if code == "??" {
            "??".to_string()
        } else {
            code.trim().chars().next().unwrap_or('M').to_string()
        };
        if code.contains('R') || code.contains('C') {
            fields.next(); // the old path
        }
        out.push((status, path));
    }
    out
}

/// Parse `git diff --numstat -z HEAD`: NUL-separated records of
/// `added\tremoved\tpath`, or, for a rename, `added\tremoved\t` followed by
/// two further NUL-separated fields (old path, new path). `-` marks binary.
/// Keyed by the new (current) path.
pub fn parse_numstat(raw: &str) -> BTreeMap<String, String> {
    let mut fields = raw.split('\0').filter(|f| !f.is_empty());
    let mut out = BTreeMap::new();
    while let Some(rec) = fields.next() {
        let mut parts = rec.splitn(3, '\t');
        let (Some(a), Some(r), Some(p)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let stat = if a == "-" {
            "binary".to_string()
        } else {
            format!("+{a} -{r}")
        };
        if p.is_empty() {
            // Rename: the next two NUL-separated fields are old and new path.
            let _old = fields.next();
            if let Some(new) = fields.next() {
                out.insert(new.to_string(), stat);
            }
        } else {
            out.insert(p.to_string(), stat);
        }
    }
    out
}

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// Read the working tree. Never fails: outside a repository, or when git
/// is missing, it answers `available: false`.
pub fn collect(root: &Path, state: &State) -> WorkTree {
    let Some(status) = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    ) else {
        return WorkTree {
            available: false,
            changes: vec![],
        };
    };
    let numstat = git(root, &["diff", "--numstat", "-z", "HEAD"])
        .map(|b| parse_numstat(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    let mut changes: Vec<Change> = parse_porcelain_z(&status)
        .into_iter()
        .filter(|(_, p)| !p.starts_with(".context/"))
        .map(|(status, path)| {
            let stat = if status == "??" {
                "new".to_string()
            } else {
                numstat.get(&path).cloned().unwrap_or_default()
            };
            let mtime = std::fs::metadata(root.join(&path))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let owner = owner_of(state, &path);
            Change {
                path,
                status,
                stat,
                mtime,
                owner,
            }
        })
        .collect();
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    WorkTree {
        available: true,
        changes,
    }
}
