//! Run a commit author in a disposable clone, validating its history before
//! publishing it. The source index is never exposed to the configured command.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, bail, ensure};

pub(super) struct Committed {
    pub refs: Vec<String>,
    pub paths: Vec<String>,
    pub detail: String,
}

pub(super) fn run(
    paths: &[String],
    argv: &[String],
    model: &str,
    message: &str,
) -> anyhow::Result<Committed> {
    let root = PathBuf::from(git_text(Path::new("."), &["rev-parse", "--show-toplevel"])?);
    let before = head(&root)?;
    let index_path = PathBuf::from(git_text(&root, &["rev-parse", "--git-path", "index"])?);
    let index_path = if index_path.is_absolute() {
        index_path
    } else {
        root.join(index_path)
    };
    let index_before = read_optional(&index_path)?;
    let mut originals = BTreeMap::new();
    for path in paths {
        let rel = crate::model::paths::validate_paths(path)?;
        let resolved = crate::model::paths::canonicalize(&root, &rel[0])?;
        originals.insert(path.clone(), file_state(&resolved)?);
    }

    let scratch = tempfile::tempdir().context("create isolated commit directory")?;
    let checkout = scratch.path().join("repo");
    checked(
        git(&root)
            .args(["clone", "--quiet", "--shared", "--no-checkout", "--"])
            .arg(&root)
            .arg(&checkout)
            .output()?,
        "clone commit checkout",
    )?;
    if let Some(oid) = &before {
        checked(
            git(&checkout)
                .args(["checkout", "--quiet", "--detach", oid])
                .output()?,
            "check out commit baseline",
        )?;
    }
    // A model must not accidentally push its temporary branch to the source.
    checked(
        git(&checkout)
            .args(["remote", "remove", "origin"])
            .output()?,
        "remove temporary remote",
    )?;
    for key in [
        "user.name",
        "user.email",
        "user.signingkey",
        "commit.gpgsign",
        "gpg.format",
        "gpg.program",
        "core.hooksPath",
    ] {
        let value = git(&root).args(["config", "--get", key]).output()?;
        if value.status.success() {
            checked(
                git(&checkout)
                    .args(["config", key, String::from_utf8(value.stdout)?.trim()])
                    .output()?,
                "copy commit identity",
            )?;
        }
    }
    for (path, original) in &originals {
        let destination = checkout.join(path);
        if let Some((bytes, permissions)) = original {
            fs::create_dir_all(destination.parent().context("commit path has no parent")?)?;
            fs::write(&destination, bytes)?;
            fs::set_permissions(&destination, permissions.clone())?;
        } else if destination.exists() {
            fs::remove_file(destination)?;
        }
    }
    checked(
        git(&checkout)
            .args(["--literal-pathspecs", "add", "--"])
            .args(paths)
            .output()?,
        "stage authorized commit paths",
    )?;

    let (program, args) = argv.split_first().context("commit.command is empty")?;
    let output = Command::new(program)
        .args(
            args.iter()
                .map(|arg| if arg == "{model}" { model } else { arg }),
        )
        .current_dir(&checkout)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("EVENTLOG_OUTCOME_FILE")
        .env("EVENTLOG_MODEL", model)
        .env("EVENTLOG_PATHS", paths.join(","))
        .env("EVENTLOG_COMMIT_MESSAGE", message)
        .stdin(Stdio::inherit())
        .output()
        .with_context(|| format!("run commit command {program}"))?;
    let after = head(&checkout)?;
    if after == before || after.is_none() {
        bail!(
            "commit command produced no commit (exit {}): {}",
            output.status.code().unwrap_or(1),
            tail(&output.stderr)
        );
    }
    let after = after.unwrap();
    if let Some(oid) = &before {
        ensure!(
            git(&checkout)
                .args(["merge-base", "--is-ancestor", oid, &after])
                .status()?
                .success(),
            "commit command rewrote the baseline history"
        );
    }
    let range = before
        .as_ref()
        .map(|oid| format!("{oid}..{after}"))
        .unwrap_or_else(|| after.clone());
    let refs: Vec<String> = git_text(&checkout, &["rev-list", "--reverse", &range])?
        .lines()
        .map(str::to_owned)
        .collect();
    ensure!(
        git_text(&checkout, &["rev-list", "--min-parents=2", &range])?.is_empty(),
        "commit command created a merge commit"
    );
    let allowed: BTreeSet<_> = paths.iter().cloned().collect();
    let mut committed = BTreeSet::new();
    // Validate every commit, not only the net tree: an outside change that is
    // reverted by a later commit is still outside the authorized history.
    for oid in &refs {
        let names = checked(
            git(&checkout)
                .args([
                    "diff-tree",
                    "--root",
                    "--no-commit-id",
                    "--name-only",
                    "--no-renames",
                    "-r",
                    "-z",
                    oid,
                ])
                .output()?,
            "read committed paths",
        )?;
        for name in names
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let name = String::from_utf8(name.to_vec()).context("committed path is not UTF-8")?;
            ensure!(
                allowed.contains(&name),
                "commit command touched an unauthorized path: {name}"
            );
            committed.insert(name);
        }
    }
    ensure!(
        !committed.is_empty(),
        "commit command created no file changes"
    );
    let pending = checked(
        git(&checkout)
            .args(["--literal-pathspecs", "status", "--porcelain", "-z", "--"])
            .args(paths)
            .output()?,
        "check remaining authorized changes",
    )?;
    ensure!(
        pending.is_empty(),
        "commit command left authorized changes uncommitted"
    );

    // The source may have changed while the model ran. Do not overwrite that
    // work or attach a candidate to a different baseline.
    ensure!(
        head(&root)? == before,
        "source HEAD changed while commit command ran"
    );
    ensure!(
        read_optional(&index_path)? == index_before,
        "source index changed while commit command ran"
    );
    for (path, original) in &originals {
        ensure!(
            &file_state(&root.join(path))? == original,
            "source path changed while commit command ran: {path}"
        );
    }
    checked(
        git(&root)
            .args(["fetch", "--quiet", "--no-tags", "--no-write-fetch-head"])
            .arg(&checkout)
            .arg(&after)
            .output()?,
        "import validated commits",
    )?;
    let expected = before.unwrap_or_else(|| "0".repeat(after.len()));
    checked(
        git(&root)
            .args([
                "update-ref",
                "-m",
                "eventlog: publish configured commits",
                "HEAD",
                &after,
                &expected,
            ])
            .output()?,
        "publish validated commits",
    )?;

    let mut detail = if output.status.success() {
        String::new()
    } else {
        format!(
            "commits verified despite command exit {}: {}",
            output.status.code().unwrap_or(1),
            tail(&output.stderr)
        )
    };
    // HEAD has advanced. Any subsequent failure must still report the actual
    // commit refs, rather than claim that the commit never happened.
    let restore = git(&root)
        .args([
            "--literal-pathspecs",
            "restore",
            "--source",
            &after,
            "--staged",
            "--worktree",
            "--",
        ])
        .args(paths)
        .output()
        .and_then(|out| {
            if out.status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other(tail(&out.stderr)))
            }
        });
    if let Err(err) = restore {
        detail.push_str(&format!(
            "; commits published but scoped index/worktree synchronization failed: {err}"
        ));
    }
    Ok(Committed {
        refs,
        paths: committed.into_iter().collect(),
        detail,
    })
}

fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    command
}

fn checked(output: Output, context: &str) -> anyhow::Result<Vec<u8>> {
    ensure!(
        output.status.success(),
        "{context}: {}",
        tail(&output.stderr)
    );
    Ok(output.stdout)
}

fn git_text(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    Ok(
        String::from_utf8(checked(git(root).args(args).output()?, "git command")?)?
            .trim()
            .to_owned(),
    )
}

fn head(root: &Path) -> anyhow::Result<Option<String>> {
    let output = git(root)
        .args(["rev-parse", "--verify", "HEAD^{commit}"])
        .output()?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

fn read_optional(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

type FileState = Option<(Vec<u8>, fs::Permissions)>;

fn file_state(path: &Path) -> anyhow::Result<FileState> {
    match read_optional(path)? {
        Some(bytes) => Ok(Some((bytes, fs::metadata(path)?.permissions()))),
        None => Ok(None),
    }
}

fn tail(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(1024)..])
        .trim()
        .replace('\n', " ")
}
