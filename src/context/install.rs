//! `eventlog context install`: write the eventlog-context mod, or print the
//! classic `SessionStart` hook for users without function hooks.

use std::path::Path;

use anyhow::Context as _;
use include_dir::{Dir, include_dir};

static MOD: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/mods/eventlog-context");
const TARGET: &str = ".claude/skills/eventlog-context";

const CLASSIC: &str = r#"Add this to "hooks" in .claude/settings.json (use it only when function hooks are off):

"SessionStart": [
  { "matcher": "compact",
    "hooks": [{ "type": "command", "command": "eventlog context" }] }
]
"#;

pub fn run(root: &Path, classic: bool, force: bool) -> anyhow::Result<i32> {
    if classic {
        print!("{CLASSIC}");
        return Ok(0);
    }
    if !force && has_classic_hook(root) {
        anyhow::bail!(
            "{} or {} already runs `eventlog context` as a classic hook; the mod and the classic hook would both add the packet. Remove the classic hook, or pass --force",
            root.join(".claude/settings.json").display(),
            root.join(".claude/settings.local.json").display(),
        );
    }
    let target = root.join(TARGET);
    if target.is_dir() {
        std::fs::remove_dir_all(&target)
            .with_context(|| format!("removing stale {}", target.display()))?;
    }
    write_dir(&MOD, &target)?;
    println!("wrote {TARGET}");
    println!(
        "Start Claude Code with CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1 (Claude Code 2.1.278 or later)."
    );
    println!(
        "The mod loads only in a trusted workspace ({}), since it lives under .claude/skills/.",
        root.display()
    );
    Ok(0)
}

/// `.claude/settings.json` or `.claude/settings.local.json` names the
/// classic hook (S22): either file can carry it, and Claude Code merges
/// both, so a hook in either would double up with the mod.
fn has_classic_hook(root: &Path) -> bool {
    [".claude/settings.json", ".claude/settings.local.json"]
        .iter()
        .any(|p| file_has_classic_hook(&root.join(p)))
}

/// Parse a settings file as JSON, and look for a `SessionStart` hook whose
/// command contains `eventlog context`.
fn file_has_classic_hook(settings: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(settings) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let Some(entries) = value
        .get("hooks")
        .and_then(|h| h.get("SessionStart"))
        .and_then(|s| s.as_array())
    else {
        return false;
    };
    entries.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .is_some_and(|hooks| {
                hooks.iter().any(|hook| {
                    hook.get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(|c| c.contains("eventlog context"))
                })
            })
    })
}

fn write_dir(dir: &Dir<'_>, target: &Path) -> anyhow::Result<()> {
    for file in dir.files() {
        if file
            .path()
            .file_name()
            .is_some_and(|n| n == "tsconfig.json")
        {
            continue;
        }
        let dest = target.join(file.path());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&dest, file.contents())
            .with_context(|| format!("writing {}", dest.display()))?;
    }
    for sub in dir.dirs() {
        if sub.path().file_name().is_some_and(|n| n == "tests") {
            continue;
        }
        write_dir(sub, target)?;
    }
    Ok(())
}
