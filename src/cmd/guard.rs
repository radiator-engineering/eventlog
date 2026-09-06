//! `eventlog guard` (read a hook payload, decide) and `eventlog guard install`.

use std::io::Read;
use std::path::Path;

use anyhow::Context;

use crate::cli::{Args as CliArgs, Command, GuardArgs, GuardInner};
use crate::guard::{self, Agent, Decision, ParseFail};
use crate::model::config;

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::Guard(guard_args) = &args.command else {
        anyhow::bail!("guard::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    match &guard_args.inner {
        Some(GuardInner::Install { agent }) => {
            install(agent.or(guard_args.agent), &root, &guard::current_bin())
        }
        None => {
            let mut payload = String::new();
            std::io::stdin()
                .read_to_string(&mut payload)
                .context("reading the hook payload from stdin")?;
            let cfg = config::load(&root)?;
            let log_path = config::resolve_log(&cfg, args.log.as_deref());
            Ok(check(&payload, guard_args, &log_path))
        }
    }
}

/// Decide one payload. Never returns an error: a guard that fails loudly on
/// its own bugs would block every tool call in the session.
fn check(payload: &str, args: &GuardArgs, log_path: &Path) -> i32 {
    let action = match guard::parse(payload, args.agent) {
        Ok((_, action)) => action,
        // Not JSON: something else is on stdin. Fail open.
        Err(ParseFail::NotJson) => return 0,
        // JSON we cannot read is JSON we cannot clear. Fail closed.
        Err(ParseFail::UnknownShape) => {
            return deny("unrecognized payload: no tool_name, command or tool_input field");
        }
    };
    match guard::decide(&action, log_path) {
        Decision::Allow => 0,
        Decision::Deny(reason) => deny(&reason),
    }
}

/// stderr carries the reason to the model; stdout carries Cursor's verdict.
/// Exit 2 is what blocks the call.
fn deny(reason: &str) -> i32 {
    let message = format!(
        "eventlog guard: BLOCKED. {reason} The coordination log is append-only; \
         the only sanctioned writer is `eventlog append` (or append-event.sh). \
         Read it with cat/tail/grep/jq. To record a large artifact, write the \
         artifact to its own file and append an event with ref=<path>."
    );
    eprintln!("{message}");
    println!(
        "{}",
        serde_json::json!({ "permission": "deny", "userMessage": message })
    );
    2
}

/// Install the hook for one agent, or for all three when none is named.
fn install(agent: Option<Agent>, root: &Path, bin: &Path) -> anyhow::Result<i32> {
    let agents: Vec<Agent> = match agent {
        Some(a) => vec![a],
        None => Agent::ALL.to_vec(),
    };
    for agent in agents {
        let changed = guard::install(agent, root, bin)?;
        println!(
            "{} {}",
            if changed { "installed" } else { "unchanged" },
            agent.as_str()
        );
    }
    Ok(0)
}

/// A guard that reports Other for a tool it does not judge must still allow it.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::Action;

    #[test]
    fn other_actions_are_allowed() {
        assert_eq!(
            guard::decide(&Action::Other, Path::new(".context/events.jsonl")),
            Decision::Allow
        );
    }
}
