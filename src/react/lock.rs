//! The reactor lock (spec section 7 step 1).
//!
//! `<log>.<name>.reactor.lock/` holds a token: `{pid, start time, hostname,
//! boot id}`. A pid alone is not enough — pids are recycled, and a lock copied
//! to another host or surviving a reboot names a pid that means nothing here.
//! The lock is live only when every part of the token still describes a
//! running process on this machine.
//!
//! Reclaim is by atomic rename to `<lock>.stale.<random>`: only one of several
//! racing reclaimers wins the rename, and whoever then wins `create_dir`
//! re-reads the token to confirm the lock is its own.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::log::lock::LockError;

/// How many times `acquire` retries while another process is mid-write of its
/// token, or while a reclaim is in flight.
const ATTEMPTS: u32 = 25;
const BACKOFF: Duration = Duration::from_millis(20);
/// How long a fresh holder waits before confirming the lock is still its own.
/// It covers the one window rename cannot close: another process that decided
/// to reclaim the dead lock a moment before this one replaced it.
const CONFIRM: Duration = Duration::from_millis(50);

/// Who holds the lock, and on which running process.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Token {
    pub pid: u32,
    /// The process's start time as the OS reports it. Distinguishes a live
    /// process from a new one that recycled its pid.
    pub start_time: String,
    pub hostname: String,
    /// Empty when the OS does not tell us; then a reboot is invisible and the
    /// pid check carries the decision on its own.
    pub boot_id: String,
}

impl Token {
    /// The token for this process.
    pub fn current() -> Token {
        let pid = std::process::id();
        Token {
            pid,
            start_time: start_time(pid),
            hostname: hostname(),
            boot_id: boot_id(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Option<Token> {
        serde_json::from_str(text).ok()
    }

    /// Is the process this token names still running, here? `me` supplies the
    /// current host and boot id, so a token from another machine or from
    /// before a reboot is never treated as live.
    pub fn is_live(&self, me: &Token) -> bool {
        if self.hostname != me.hostname || self.boot_id != me.boot_id {
            return false;
        }
        if self.pid == me.pid {
            return self.start_time == me.start_time;
        }
        if !pid_alive(self.pid) {
            return false;
        }
        // An unreadable start time means the process just went away; a
        // readable one that differs means the pid was recycled.
        let now = start_time(self.pid);
        !now.is_empty() && now == self.start_time
    }
}

/// A held reactor lock. Dropping it releases the lock.
pub struct ReactorLock {
    dir: PathBuf,
}

impl ReactorLock {
    /// Take the lock at `dir`, reclaiming it if its holder is gone. Returns
    /// [`LockError::Busy`] when a live process holds it.
    pub fn acquire(dir: &Path) -> Result<ReactorLock, LockError> {
        let me = Token::current();
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent).map_err(|source| LockError::Io {
                dir: dir.to_path_buf(),
                source,
            })?;
        }
        for _ in 0..ATTEMPTS {
            match fs::create_dir(dir) {
                Ok(()) => {
                    fs::write(dir.join("token"), me.to_json()).map_err(|source| LockError::Io {
                        dir: dir.to_path_buf(),
                        source,
                    })?;
                    // Confirm the lock we hold is our own: a reclaimer that
                    // read the dead token a moment ago may have renamed this
                    // directory away and created its own.
                    std::thread::sleep(CONFIRM);
                    if read_token(dir).as_ref() == Some(&me) {
                        return Ok(ReactorLock {
                            dir: dir.to_path_buf(),
                        });
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    match read_token(dir) {
                        // No token yet: the holder is mid-write. Wait.
                        None => std::thread::sleep(BACKOFF),
                        Some(token) if token.is_live(&me) => {
                            return Err(LockError::Busy {
                                dir: dir.to_path_buf(),
                            });
                        }
                        Some(dead) => reclaim(dir, &dead, &me),
                    }
                }
                Err(source) => {
                    return Err(LockError::Io {
                        dir: dir.to_path_buf(),
                        source,
                    });
                }
            }
        }
        Err(LockError::Busy {
            dir: dir.to_path_buf(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for ReactorLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn read_token(dir: &Path) -> Option<Token> {
    Token::from_json(&fs::read_to_string(dir.join("token")).ok()?)
}

/// Rename `dir` aside and remove it. Rename is atomic, so only one of several
/// racing callers succeeds; the rest find the directory gone or already
/// replaced and retry.
///
/// The token is read again first: between reading a dead token and acting on
/// it, another process may have reclaimed the lock and taken it, and renaming
/// then would tear a live lock away from its holder.
fn reclaim(dir: &Path, dead: &Token, me: &Token) {
    match read_token(dir) {
        Some(now) if &now == dead && !now.is_live(me) => {}
        _ => return,
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut stale = dir.as_os_str().to_owned();
    stale.push(format!(".stale.{}-{}", std::process::id(), nanos));
    let stale = PathBuf::from(stale);
    if fs::rename(dir, &stale).is_ok() {
        let _ = fs::remove_dir_all(&stale);
    }
}

/// `true` if a process with `pid` exists. `kill -0` works the same on macOS
/// and Linux and needs no new dependency; when we cannot tell, we say alive,
/// so a lock is only reclaimed when its owner is certainly gone.
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(true)
}

/// The process start time, as `ps` prints it. Empty when the pid is gone or
/// `ps` is unavailable — an empty start time never matches a stored one.
fn start_time(pid: u32) -> String {
    run(&["ps", "-o", "lstart=", "-p", &pid.to_string()])
}

fn hostname() -> String {
    let name = run(&["hostname"]);
    if name.is_empty() {
        "unknown-host".to_string()
    } else {
        name
    }
}

/// Linux exposes a boot id directly; macOS has the boot time, which serves the
/// same purpose. Neither being available leaves the field empty.
fn boot_id() -> String {
    if let Ok(id) = fs::read_to_string("/proc/sys/kernel/random/boot_id") {
        return id.trim().to_string();
    }
    run(&["sysctl", "-n", "kern.boottime"])
}

fn run(argv: &[&str]) -> String {
    let Some((program, args)) = argv.split_first() else {
        return String::new();
    };
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}
