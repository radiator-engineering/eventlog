//! The rule voter: what a reactor is allowed to touch, and what stops it.
//!
//! Spec section 7, step 4. [`authorize`] narrows a driving event's `paths=`
//! to what its writer actually claimed (4.1); [`check`] runs the three rules
//! against the fold at that moment (4.3); [`veto_binds`] reads the veto
//! window (4.4).

use std::path::Path;

use crate::model::config::Config;
use crate::model::event::Event;
use crate::model::paths::{self, RelPath};
use crate::query::State;

/// The split of a driving event's `paths=`: what the reactor may touch, and
/// what the writer asked for but never claimed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Authorized {
    pub paths: Vec<RelPath>,
    pub excess: Vec<RelPath>,
    /// Whose work the driving event is about: its `agent`, else its writer.
    /// A claim this subject holds never vetoes the action.
    pub subject: String,
}

/// Why the voter stopped the action. The `reason=` word of the `veto` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Veto {
    /// The writer named paths it does not hold a live claim on.
    UnclaimedPaths(Vec<RelPath>),
    /// The log file itself, or one of its lock dirs.
    LogOrLock(RelPath),
    /// A live claim on this path belongs to some other agent.
    ClaimedByOther { path: RelPath, owner: String },
    /// An open `escalate` names this reactor; the seq is that escalation's.
    OpenEscalation(u64),
}

impl Veto {
    /// The `reason=` word for the `veto` line.
    pub fn reason(&self) -> &'static str {
        match self {
            Veto::UnclaimedPaths(_) => "unclaimed-paths",
            Veto::LogOrLock(_) => "log-or-lock",
            Veto::ClaimedByOther { .. } => "claimed-by-other",
            Veto::OpenEscalation(_) => "open-escalation",
        }
    }
}

impl std::fmt::Display for Veto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Veto::UnclaimedPaths(paths) => {
                write!(f, "unclaimed-paths: {}", join(paths))
            }
            Veto::LogOrLock(path) => write!(f, "log-or-lock: {path}"),
            Veto::ClaimedByOther { path, owner } => {
                write!(f, "claimed-by-other: {path} is claimed by {owner}")
            }
            Veto::OpenEscalation(seq) => write!(f, "open-escalation: seq {seq} is open"),
        }
    }
}

fn join(paths: &[RelPath]) -> String {
    paths
        .iter()
        .map(RelPath::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

/// The authorized set (spec 7, step 4.1).
///
/// A driving event with no `by` is the controller's own: every path it names
/// is authorized, because the controller's `result` is its declaration that
/// the path is done. Anything else is intersected with the live claims of its
/// writer, so a reactor's own `result` can never widen its scope beyond what
/// the controller claimed for it at spawn. Entries that break the path rules
/// are dropped: the writer rejects them, so they cannot reach a real log.
pub fn authorize(driving: &Event, state: &State) -> Authorized {
    let named: Vec<RelPath> = driving
        .paths()
        .iter()
        .filter_map(|p| paths::validate_paths(p).ok())
        .flatten()
        .collect();

    let subject = driving
        .agent
        .clone()
        .unwrap_or_else(|| driving.writer().to_string());

    if driving.by.is_none() {
        return Authorized {
            paths: named,
            excess: Vec::new(),
            subject,
        };
    }

    let claims = state.claims_for(driving.writer());
    let mut auth = Authorized {
        subject,
        ..Authorized::default()
    };
    for path in named {
        if claims.iter().any(|glob| covers(glob, path.as_str())) {
            auth.paths.push(path);
        } else {
            auth.excess.push(path);
        }
    }
    auth
}

/// The rules (spec 7, step 4.3), in the order the reactor reports them:
/// nothing outside the writer's claims, no path that is the log or one of its
/// lock dirs, no path claimed by a different agent that is still open, and no
/// open `escalate` naming this reactor.
pub fn check(reactor: &str, auth: &Authorized, state: &State, cfg: &Config) -> Result<(), Veto> {
    if !auth.excess.is_empty() {
        return Err(Veto::UnclaimedPaths(auth.excess.clone()));
    }
    for path in &auth.paths {
        if paths::is_log_or_lock(&cfg.log.path, Path::new(path.as_str())) {
            return Err(Veto::LogOrLock(path.clone()));
        }
    }
    for path in &auth.paths {
        // A claim held by the driving event's own subject is what authorized
        // the path in the first place: a worker's result on its claimed
        // files, or the controller's `result agent=<worker>`, is not "other".
        if let Some(owner) = state.claim_owner(path.as_str())
            && owner != reactor
            && owner != auth.subject
        {
            return Err(Veto::ClaimedByOther {
                path: path.clone(),
                owner: owner.to_string(),
            });
        }
    }
    if let Some(seq) = open_escalation(reactor, state) {
        return Err(Veto::OpenEscalation(seq));
    }
    Ok(())
}

/// The seq of the first open `escalate` that names `reactor`, either as its
/// subject or as its writer.
fn open_escalation(reactor: &str, state: &State) -> Option<u64> {
    state
        .escalations
        .iter()
        .find(|e| e.subject() == reactor || e.writer() == reactor)
        .map(|e| e.seq)
}

/// The veto window (spec 7, step 4.4). A `veto` binds when its `for=` names
/// the driving seq and it was appended after `since_seq` — normally the
/// reactor's own `intent`. Which intent the vetoer saw does not matter, so a
/// restart between intent and action cannot lose a veto.
pub fn veto_binds(events: &[Event], driving_seq: u64, since_seq: u64) -> Option<&Event> {
    events
        .iter()
        .filter(|e| e.r#type == "veto" && e.seq > since_seq)
        .find(|e| e.seq_ref("for") == Some(driving_seq))
}

/// Does the claim `glob` cover `path`? Literal equality, a directory prefix
/// (`docs` covers `docs/reference/append.md`), or a glob match. Same rule as
/// `query::State::claim_owner`, so authorization and the rules agree.
fn covers(glob: &str, path: &str) -> bool {
    if glob == path {
        return true;
    }
    if path.starts_with(&format!("{}/", glob.trim_end_matches('/'))) {
        return true;
    }
    globset::Glob::new(glob)
        .ok()
        .map(|g| g.compile_matcher())
        .is_some_and(|m| m.is_match(path))
}
