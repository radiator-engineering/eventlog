//! The denylist: which actions may touch the log (spec section 9).
//!
//! Two rules decide a shell command. First, passthrough is never by substring:
//! a command is the sanctioned writer only if it parses as one simple command
//! whose argv[0] is `eventlog` or `append-event.sh`, with no operator and no
//! redirect anywhere in it. Second, any other command that *names* the log and
//! either is compound or carries a mutating shape is denied.
//!
//! Everything else — reads, and commands that never name the log — is allowed.

use std::path::Path;

use crate::guard::Action;

/// What the guard tells the agent to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(String),
}

/// Shell metacharacters that end "one simple command".
const OPERATORS: [&str; 7] = [";", "&&", "||", "|", "$(", "`", "\n"];

/// Commands named by the spec as mutating whenever they name the log.
const MUTATORS: [&str; 6] = ["rm", "mv", "cp", "truncate", "shred", "dd"];

/// The basename that marks a protected log: `EVENTLOG_GUARD_BASENAME` if set
/// (the shell guard took an ERE with the dot escaped, so backslashes are
/// dropped), else the basename of the configured log path.
fn protected_basename(log_path: &Path) -> String {
    if let Ok(base) = std::env::var("EVENTLOG_GUARD_BASENAME") {
        let base = base.replace('\\', "");
        if !base.is_empty() {
            return base;
        }
    }
    log_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "events.jsonl".to_string())
}

/// Does `text` name the protected log? Bounded, so `events.jsonl.bak` and
/// `myevents.jsonl` do not match.
fn names_log(text: &str, base: &str) -> bool {
    let boundary = |c: char| c.is_alphanumeric() || c == '_' || c == '.' || c == '-';
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(hit) = text[from..].find(base) {
        let start = from + hit;
        let end = start + base.len();
        let before_ok = start == 0 || !boundary(text[..start].chars().next_back().unwrap());
        let after_ok = end == bytes.len() || !boundary(text[end..].chars().next().unwrap());
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Split a command into words, dropping quotes but keeping what they held.
/// Redirects and operators become their own words.
fn words(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut had_quote = false;
    fn push(out: &mut Vec<String>, word: &mut String, had_quote: &mut bool) {
        if !word.is_empty() || *had_quote {
            out.push(std::mem::take(word));
            *had_quote = false;
        }
    }
    for c in command.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    word.push(c);
                }
            }
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    had_quote = true;
                }
                c if c.is_whitespace() => push(&mut out, &mut word, &mut had_quote),
                ';' | '|' | '&' | '>' | '<' => {
                    push(&mut out, &mut word, &mut had_quote);
                    out.push(c.to_string());
                }
                _ => word.push(c),
            },
        }
    }
    push(&mut out, &mut word, &mut had_quote);
    out
}

/// True when `command` is a single simple command run by the sanctioned
/// writer: argv[0] (after path) is `eventlog` or `append-event.sh`, and the
/// command carries no operator, redirect, or background `&`.
pub fn is_simple_sanctioned_writer(command: &str) -> bool {
    if OPERATORS.iter().any(|op| command.contains(op)) {
        return false;
    }
    if command.contains('>') || command.contains('<') || command.contains('&') {
        return false;
    }
    let Some(argv0) = command.split_whitespace().next() else {
        return false;
    };
    let argv0 = argv0.rsplit('/').next().unwrap_or(argv0);
    matches!(argv0, "eventlog" | "append-event.sh")
}

/// Is there a truncating redirect (`>` or `>|`, not `>>`) whose target names
/// the log? Quoted text is skipped: a `>` inside quotes is not a redirect.
fn truncating_redirect_into_log(command: &str, base: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                '>' => {
                    let doubled = chars.get(i + 1) == Some(&'>') || (i > 0 && chars[i - 1] == '>');
                    if !doubled {
                        // Target: the next word after the operator (and an
                        // optional `|` for `>|`).
                        let rest: String = chars[i + 1..].iter().collect();
                        let rest = rest.trim_start().trim_start_matches('|');
                        let target = rest.split_whitespace().next().unwrap_or("");
                        if names_log(target, base) {
                            return true;
                        }
                    }
                }
                _ => {}
            },
        }
        i += 1;
    }
    false
}

/// An interpreter opening a file for writing: `open(..., 'w')` and friends.
fn opens_for_writing(command: &str) -> bool {
    let mut from = 0;
    while let Some(hit) = command[from..].find("open(") {
        let start = from + hit;
        let tail = &command[start..];
        let end = tail.find(')').map(|e| start + e).unwrap_or(command.len());
        let call = &command[start..end];
        if call.contains("'w") || call.contains("\"w") {
            return true;
        }
        from = start + 1;
    }
    false
}

/// The mutating shape a command carries against the log, if any.
fn mutation_reason(command: &str, base: &str) -> Option<String> {
    // A quoted argument can hold a whole command (`bash -c 'rm <log>'`), so
    // split words open again and judge the pieces too.
    let words: Vec<String> = words(command)
        .iter()
        .flat_map(|w| std::iter::once(w.clone()).chain(w.split_whitespace().map(str::to_string)))
        .collect();
    let has = |w: &str| words.iter().any(|x| x == w);

    if truncating_redirect_into_log(command, base) {
        return Some(format!(
            "truncating redirect ('>' or '>|') into {base}; the log is append-only"
        ));
    }
    for m in MUTATORS {
        if has(m) {
            return Some(format!("'{m}' names {base}; the log is append-only"));
        }
    }
    let in_place = |tool: &str| {
        has(tool)
            && words.iter().any(|w| {
                w.starts_with("-i") || (w.starts_with('-') && tool == "perl" && w.contains('i'))
            })
    };
    for tool in ["sed", "perl", "ruby"] {
        if in_place(tool) {
            return Some(format!("in-place '{tool} -i' edit of {base}"));
        }
    }
    if has("tee")
        && !words
            .iter()
            .any(|w| w == "-a" || w == "--append" || (w.starts_with('-') && w.contains('a')))
    {
        return Some(format!(
            "non-append 'tee' into {base} (use 'tee -a', or append-event.sh)"
        ));
    }
    if opens_for_writing(command) {
        return Some(format!("interpreter opening {base} for writing"));
    }
    if has("git") && (has("checkout") || has("restore")) {
        return Some(format!("'git checkout'/'git restore' would rewind {base}"));
    }
    None
}

/// The one denylist, for every agent and every action shape.
pub fn decide(action: &Action, log_path: &Path) -> Decision {
    let base = protected_basename(log_path);
    match action {
        Action::Other => Decision::Allow,
        Action::Write { path } => {
            if names_log(path, &base) {
                Decision::Deny(format!("Write would overwrite the whole log ({path})."))
            } else {
                Decision::Allow
            }
        }
        Action::Edit { path } => {
            if names_log(path, &base) {
                Decision::Deny(format!(
                    "Edit would rewrite existing lines of the log ({path})."
                ))
            } else {
                Decision::Allow
            }
        }
        Action::Shell { command } => {
            if is_simple_sanctioned_writer(command) {
                return Decision::Allow;
            }
            if !names_log(command, &base) {
                return Decision::Allow;
            }
            if OPERATORS.iter().any(|op| command.contains(op)) {
                return Decision::Deny(format!(
                    "compound command naming {base} (';', '&&', '||', '|', '$(', backtick or newline); \
                     run the read alone, or append through append-event.sh."
                ));
            }
            match mutation_reason(command, &base) {
                Some(reason) => Decision::Deny(reason),
                None => Decision::Allow,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> &'static Path {
        Path::new(".context/events.jsonl")
    }

    #[test]
    fn sanctioned_writer_needs_a_single_simple_command() {
        assert!(is_simple_sanctioned_writer("append-event.sh result ref=x"));
        assert!(is_simple_sanctioned_writer(
            "/opt/bin/eventlog append note x=1"
        ));
        assert!(!is_simple_sanctioned_writer(
            "append-event.sh note x=1 && rm .context/events.jsonl"
        ));
        assert!(!is_simple_sanctioned_writer("echo append-event.sh"));
        assert!(!is_simple_sanctioned_writer("env X=1 append-event.sh note"));
    }

    #[test]
    fn boundaries_keep_neighbours_out() {
        assert!(names_log("cat .context/events.jsonl", "events.jsonl"));
        assert!(!names_log("rm .context/events.jsonl.bak", "events.jsonl"));
        assert!(!names_log("rm myevents.jsonl", "events.jsonl"));
    }

    #[test]
    fn appending_redirect_is_not_truncating() {
        assert!(!truncating_redirect_into_log(
            "echo x >> .context/events.jsonl",
            "events.jsonl"
        ));
        assert!(truncating_redirect_into_log(
            "echo x > .context/events.jsonl",
            "events.jsonl"
        ));
    }

    #[test]
    fn commands_that_never_name_the_log_are_allowed() {
        let action = Action::Shell {
            command: "rm -rf target".into(),
        };
        assert_eq!(decide(&action, log()), Decision::Allow);
    }
}
