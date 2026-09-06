//! Path rules for the `paths=` field (spec section 3, "Paths").
//!
//! An entry is relative to the repo root, contains no comma, is not absolute,
//! and may not escape the root once `..` is resolved. The writer rejects
//! entries that break these rules; the voter canonicalizes the rest before any
//! rule runs.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("empty path entry in {field:?}")]
    Empty { field: String },
    #[error("absolute path not allowed: {path}")]
    Absolute { path: String },
    #[error("path escapes the repo root: {path}")]
    Escape { path: String },
    #[error("path component is a symlink: {path}")]
    Symlink { path: String },
    #[error("cannot resolve {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// A repo-relative path that passed [`validate_paths`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

impl RelPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RelPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<Path> for RelPath {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

/// Split a `paths=` field on `,` and check every entry.
pub fn validate_paths(field: &str) -> Result<Vec<RelPath>, PathError> {
    let mut out = Vec::new();
    for raw in field.split(',') {
        let entry = raw.trim();
        if entry.is_empty() {
            return Err(PathError::Empty {
                field: field.to_string(),
            });
        }
        let p = Path::new(entry);
        if p.is_absolute() || entry.starts_with('~') {
            return Err(PathError::Absolute {
                path: entry.to_string(),
            });
        }
        let mut depth = 0i32;
        for c in p.components() {
            match c {
                Component::Prefix(_) | Component::RootDir => {
                    return Err(PathError::Absolute {
                        path: entry.to_string(),
                    });
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(PathError::Escape {
                            path: entry.to_string(),
                        });
                    }
                }
                Component::Normal(_) => depth += 1,
            }
        }
        out.push(RelPath(entry.to_string()));
    }
    Ok(out)
}

/// Resolve `p` under `root`, rejecting any component that is a symlink, so a
/// claim cannot be widened by pointing a link outside the repo.
pub fn canonicalize(root: &Path, p: &RelPath) -> Result<PathBuf, PathError> {
    let root = root.canonicalize().map_err(|source| PathError::Io {
        path: root.display().to_string(),
        source,
    })?;
    let mut resolved = root.clone();
    for c in Path::new(p.as_str()).components() {
        match c {
            Component::CurDir => continue,
            Component::ParentDir => {
                if !resolved.pop() || !resolved.starts_with(&root) {
                    return Err(PathError::Escape { path: p.0.clone() });
                }
                continue;
            }
            Component::Normal(part) => resolved.push(part),
            Component::Prefix(_) | Component::RootDir => {
                return Err(PathError::Absolute { path: p.0.clone() });
            }
        }
        match std::fs::symlink_metadata(&resolved) {
            Ok(md) if md.file_type().is_symlink() => {
                return Err(PathError::Symlink {
                    path: resolved.display().to_string(),
                });
            }
            Ok(_) => {}
            // A path that does not exist yet is fine: nothing can be a symlink
            // below a missing component either.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(PathError::Io {
                    path: resolved.display().to_string(),
                    source,
                });
            }
        }
    }
    if !resolved.starts_with(&root) {
        return Err(PathError::Escape { path: p.0.clone() });
    }
    Ok(resolved)
}

/// Is `p` the log itself or one of its sidecars (`<log>.lock`,
/// `<log>.commit.reactor.lock`, …)? Nothing but the writer may touch those.
pub fn is_log_or_lock(cfg_log: &Path, p: &Path) -> bool {
    if p == cfg_log {
        return true;
    }
    if p.parent() != cfg_log.parent() {
        return false;
    }
    let (Some(log_name), Some(name)) = (cfg_log.file_name(), p.file_name()) else {
        return false;
    };
    let (Some(log_name), Some(name)) = (log_name.to_str(), name.to_str()) else {
        return false;
    };
    name.starts_with(&format!("{log_name}."))
}
