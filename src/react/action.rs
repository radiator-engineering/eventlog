//! Run reactor action commands and detect git violations (spec section 7, steps 4.5–4.7).

use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::model::paths::RelPath;

const DROPPED_OUTCOME_KEYS: &[&str] = &["seq", "ts", "prev", "by"];
/// Enough context to diagnose a failed action while always fitting an ack
/// field (whose maximum is 2 KiB) with room for its other diagnostics.
const STDERR_TAIL_BYTES: usize = 1024;
const STDERR_COLLECT_GRACE: Duration = Duration::from_millis(100);

/// Environment passed to an action command.
pub struct ActionEnv {
    pub log: PathBuf,
    pub seq: u64,
    pub r#type: String,
    pub agent: String,
    pub by: String,
    pub paths: Vec<RelPath>,
    pub reference: Option<String>,
    pub resume: u64,
    pub outcome_file: PathBuf,
}

/// Result of running an action command.
pub struct Outcome {
    pub exit: i32,
    pub fields: Vec<(String, String)>,
    pub timed_out: bool,
}

/// Git state before or after an action.
pub struct Snapshot {
    pub head: Option<String>,
    pub dirty: BTreeSet<String>,
}

/// Run `command` with `stdin_json` on stdin and reactor env vars set.
pub fn run(
    command: &[String],
    stdin_json: &str,
    env: &ActionEnv,
    timeout: Duration,
) -> Result<Outcome> {
    let (program, args) = command.split_first().context("action command is empty")?;

    let mut action = Command::new(program);
    action
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Keep stderr separate from the outcome protocol.  Piping it is still
        // important: inheriting it can make a reactor noisy, and a pipe must
        // be drained alongside stdout or it can deadlock the action.
        .stderr(Stdio::piped())
        .env("EVENTLOG_LOG", &env.log)
        .env("EVENTLOG_SEQ", env.seq.to_string())
        .env("EVENTLOG_TYPE", &env.r#type)
        .env("EVENTLOG_AGENT", &env.agent)
        .env("EVENTLOG_BY", &env.by)
        .env(
            "EVENTLOG_PATHS",
            env.paths
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(","),
        )
        .env("EVENTLOG_REF", env.reference.as_deref().unwrap_or_default())
        .env("EVENTLOG_RESUME", env.resume.to_string())
        .env("EVENTLOG_OUTCOME_FILE", &env.outcome_file);
    configure_process_group(&mut action);

    let mut child = action
        .spawn()
        .with_context(|| format!("spawn action command {program}"))?;

    // All three pipes run concurrently.  In particular, do not wait for the
    // child before reading stdout: a command that writes more than a pipe
    // buffer while it is still running otherwise waits forever.
    let stdin = child.stdin.take();
    let stdin_json = stdin_json.as_bytes().to_vec();
    let stdin_writer = std::thread::spawn(move || -> std::io::Result<()> {
        if let Some(mut stdin) = stdin {
            stdin.write_all(&stdin_json)?;
        }
        Ok(())
    });
    let stdout_reader = child.stdout.take().map(read_pipe);
    let stderr_reader = child.stderr.take().map(read_stderr_tail);

    let (timed_out, status) = wait_with_timeout(
        &mut child,
        timeout,
        &stdin_writer,
        stdout_reader.as_ref(),
        stderr_reader.as_ref(),
    )?;
    let exit = status.map(|s| s.code().unwrap_or(1)).unwrap_or(1);

    // A timed out pipe holder can be outside the direct child's lifetime.
    // The handles are deliberately not joined in that case: process-tree
    // termination made them eligible to finish, but an inherited descriptor
    // outside that tree must never make the reactor exceed its deadline.
    let stdout = if timed_out {
        String::new()
    } else {
        join_pipe(stdout_reader, "read action stdout")?
    };
    let stderr = if timed_out {
        join_finished_pipe(stderr_reader, "read action stderr", STDERR_COLLECT_GRACE)?
    } else {
        Some(join_pipe(stderr_reader, "read action stderr")?)
    };
    if !timed_out {
        let written = stdin_writer
            .join()
            .map_err(|_| anyhow::anyhow!("action stdin writer panicked"))?;
        // An action may deliberately close stdin without consuming its event.
        // Keep its exit status and outcome; only other write errors are fatal.
        if let Err(err) = written
            && err.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(err).context("write action stdin");
        }
    }

    // A deadline is authoritative. An action can write an optimistic outcome
    // and then hang in a descendant; carrying that outcome into the ack would
    // incorrectly normalize a timed-out pass as successful.
    let mut fields = if timed_out {
        Vec::new()
    } else if env.outcome_file.is_file() {
        let content = std::fs::read_to_string(&env.outcome_file)
            .with_context(|| format!("read {}", env.outcome_file.display()))?;
        parse_outcome_file(&content)
    } else {
        parse_stdout_fallback(&stdout)
    };
    if (timed_out || exit != 0)
        && let Some(stderr) = stderr.filter(|stderr| !stderr.is_empty())
    {
        // `to_outcome` puts unknown action fields into ack `detail`, keeping
        // stderr separate from the outcome protocol and preserving an action
        // supplied `detail` rather than overwriting it.
        fields.push(("stderr".to_string(), stderr));
    }

    Ok(Outcome {
        exit,
        fields,
        timed_out,
    })
}

/// Capture `HEAD` and porcelain status under `root`.
pub fn snapshot(root: &Path) -> Result<Snapshot> {
    let head = git_output(root, &["rev-parse", "HEAD"]).ok();
    let status = git_output_bytes(root, &["status", "--porcelain=v1", "-z"]).unwrap_or_default();
    Ok(Snapshot {
        head,
        dirty: parse_porcelain(&status),
    })
}

/// Files changed by the commits the action made between the snapshots:
/// `git diff --name-only before..after`. Only what was committed counts as
/// the action's doing; the working tree is shared with every other agent,
/// so a file that merely became dirty is reported by [`newly_dirty`], never
/// blamed on the action.
pub fn touched(before: &Snapshot, after: &Snapshot, root: &Path) -> BTreeSet<String> {
    commit_touched(before.head.as_deref(), after.head.as_deref(), root)
}

/// Paths dirty after the action that were not dirty before. Somebody's work
/// in progress, observed while the action ran; not the action's own writes.
pub fn newly_dirty(before: &Snapshot, after: &Snapshot) -> BTreeSet<String> {
    after.dirty.difference(&before.dirty).cloned().collect()
}

/// Paths in `touched` that are not covered by `authorized`.
pub fn outside(touched: &BTreeSet<String>, authorized: &[RelPath]) -> Vec<String> {
    let allowed: HashSet<&str> = authorized.iter().map(|p| p.as_str()).collect();
    touched
        .iter()
        .filter(|p| !allowed.contains(p.as_str()))
        .cloned()
        .collect()
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
    stdin_writer: &JoinHandle<std::io::Result<()>>,
    stdout_reader: Option<&JoinHandle<std::io::Result<Vec<u8>>>>,
    stderr_reader: Option<&JoinHandle<std::io::Result<Vec<u8>>>>,
) -> Result<(bool, Option<std::process::ExitStatus>)> {
    let start = Instant::now();
    let mut status = None;
    loop {
        if status.is_none() {
            status = child.try_wait().context("wait on action command")?;
        }
        if status.is_some()
            && stdin_writer.is_finished()
            && pipe_finished(stdout_reader)
            && pipe_finished(stderr_reader)
        {
            return Ok((false, status));
        }
        if start.elapsed() >= timeout {
            terminate_process_tree(child);
            return Ok((true, None));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn read_pipe<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<std::io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut output = Vec::new();
        pipe.read_to_end(&mut output)?;
        Ok(output)
    })
}

/// Drain stderr continuously but retain only its final diagnostic context.
/// The tail lets a verbose failed action remain observable without giving an
/// untrusted action an unbounded memory budget in its supervising reactor.
fn read_stderr_tail<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<std::io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut tail = Vec::with_capacity(STDERR_TAIL_BYTES);
        let mut chunk = [0_u8; 8192];
        loop {
            let count = pipe.read(&mut chunk)?;
            if count == 0 {
                return Ok(tail);
            }
            tail.extend_from_slice(&chunk[..count]);
            if tail.len() > STDERR_TAIL_BYTES {
                let excess = tail.len() - STDERR_TAIL_BYTES;
                tail.drain(..excess);
            }
        }
    })
}

fn pipe_finished(pipe: Option<&JoinHandle<std::io::Result<Vec<u8>>>>) -> bool {
    pipe.is_none_or(JoinHandle::is_finished)
}

fn join_pipe(pipe: Option<JoinHandle<std::io::Result<Vec<u8>>>>, context: &str) -> Result<String> {
    let Some(pipe) = pipe else {
        return Ok(String::new());
    };
    let bytes = pipe
        .join()
        .map_err(|_| anyhow::anyhow!("{context} thread panicked"))?
        .with_context(|| context.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// After killing a timed-out process tree, give a drained pipe a small bounded
/// window to hand back its tail. A descriptor leaked outside that tree is
/// dropped instead of extending the action deadline indefinitely.
fn join_finished_pipe(
    pipe: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
    context: &str,
    grace: Duration,
) -> Result<Option<String>> {
    let Some(pipe) = pipe else {
        return Ok(Some(String::new()));
    };
    let deadline = Instant::now() + grace;
    while !pipe.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if pipe.is_finished() {
        return join_pipe(Some(pipe), context).map(Some);
    }
    Ok(None)
}

/// Put an action in its own process group where the platform offers groups.
/// This lets a deadline cover children which inherited its stdin/stdout pipes.
#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

fn terminate_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let pid = child.id();
        signal_process_group(pid, 15); // SIGTERM
        let grace = Instant::now() + Duration::from_millis(100);
        while Instant::now() < grace {
            if child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        signal_process_group(pid, 9); // SIGKILL, including descendants.
    }
    #[cfg(windows)]
    {
        // Windows has no Unix-style process groups. taskkill's /T walks the
        // descendant tree; child.kill remains a fallback when it is absent.
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
fn signal_process_group(pid: u32, signal: i32) {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    // A negative pid addresses the process group created above. PIDs exposed
    // by Child fit in a signed C pid_t on supported Unix platforms.
    if let Ok(pid) = i32::try_from(pid) {
        unsafe { kill(-pid, signal) };
    }
}

fn parse_outcome_file(content: &str) -> Vec<(String, String)> {
    let dropped: HashSet<&str> = DROPPED_OUTCOME_KEYS.iter().copied().collect();
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            if dropped.contains(key) {
                return None;
            }
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn parse_stdout_fallback(stdout: &str) -> Vec<(String, String)> {
    let Some(line) = stdout.lines().rev().find(|l| l.contains("outcome=")) else {
        return Vec::new();
    };
    parse_kv_tokens(line)
}

fn parse_kv_tokens(line: &str) -> Vec<(String, String)> {
    let dropped: HashSet<&str> = DROPPED_OUTCOME_KEYS.iter().copied().collect();
    line.split_whitespace()
        .filter_map(|token| {
            let (key, value) = token.split_once('=')?;
            if dropped.contains(key) {
                return None;
            }
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn git_output(root: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8_lossy(&git_output_bytes(root, args)?)
        .trim_end()
        .to_string())
}

fn git_output_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(out.stdout)
}

/// Parse porcelain v1's NUL-delimited form. Unlike its line form it preserves
/// spaces, quotes, newlines and literal ` -> ` in names without C quoting.
fn parse_porcelain(status: &[u8]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut entries = status.split(|byte| *byte == 0);
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let status_code = &entry[..2];
        insert_git_path(&mut out, &entry[3..]);
        // In -z mode a rename/copy carries the other path in the next field.
        // Retaining both sides means a rename cannot hide an out-of-scope
        // deletion or make pre-existing work look newly dirty.
        if (status_code.contains(&b'R') || status_code.contains(&b'C'))
            && let Some(other) = entries.next()
        {
            insert_git_path(&mut out, other);
        }
    }
    out
}

fn insert_git_path(paths: &mut BTreeSet<String>, path: &[u8]) {
    if !path.is_empty() {
        paths.insert(String::from_utf8_lossy(path).into_owned());
    }
}

fn commit_touched(before: Option<&str>, after: Option<&str>, root: &Path) -> BTreeSet<String> {
    let (Some(before), Some(after)) = (before, after) else {
        return BTreeSet::new();
    };
    if before == after {
        return BTreeSet::new();
    }
    git_output_bytes(
        root,
        &["diff", "--name-only", "--no-renames", "-z", before, after],
    )
    .map(|out| {
        out.split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect()
    })
    .unwrap_or_default()
}
