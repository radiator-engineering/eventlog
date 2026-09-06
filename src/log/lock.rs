//! An exclusive lock backed by a directory: `mkdir` is atomic, so whichever
//! process creates the directory holds the lock. A stale lock (its pid is
//! dead) is reclaimed by renaming the directory aside — rename is atomic, so
//! only one of several racing reclaimers wins — then removing it and retrying.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("lock busy: {dir}")]
    Busy { dir: PathBuf },
    #[error("io error acquiring lock at {dir}: {source}")]
    Io {
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub struct Lock {
    dir: PathBuf,
}

impl Lock {
    /// Acquire the lock at `dir`, waiting up to `wait` for a live holder to
    /// release it. A dead holder's lock is reclaimed immediately.
    pub fn acquire(dir: &Path, wait: Duration) -> Result<Lock, LockError> {
        let deadline = Instant::now() + wait;
        loop {
            match fs::create_dir(dir) {
                Ok(()) => {
                    let pid = std::process::id();
                    fs::write(dir.join("pid"), pid.to_string()).map_err(|source| {
                        LockError::Io {
                            dir: dir.to_path_buf(),
                            source,
                        }
                    })?;
                    return Ok(Lock {
                        dir: dir.to_path_buf(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if let Some(pid) = read_pid(dir)
                        && !pid_alive(pid)
                    {
                        reclaim(dir);
                        continue;
                    }
                    if Instant::now() >= deadline {
                        return Err(LockError::Busy {
                            dir: dir.to_path_buf(),
                        });
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(source) => {
                    return Err(LockError::Io {
                        dir: dir.to_path_buf(),
                        source,
                    });
                }
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn read_pid(dir: &Path) -> Option<u32> {
    fs::read_to_string(dir.join("pid"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// `true` if a process with `pid` exists, checked with `kill -0` (works on
/// both macOS and Linux without a new dependency). Unable to tell counts as
/// alive, so a lock is only ever reclaimed when we are sure its owner is gone.
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(true)
}

/// Rename `dir` aside and remove it. Only one of several racing callers
/// succeeds at the rename; the rest see it fail and fall back to retrying.
fn reclaim(dir: &Path) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut stale = dir.as_os_str().to_owned();
    stale.push(format!(".stale-{}-{}", std::process::id(), nanos));
    let stale = PathBuf::from(stale);
    if fs::rename(dir, &stale).is_ok() {
        let _ = fs::remove_dir_all(&stale);
    }
}
