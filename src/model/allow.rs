//! Per-type writer allowlist (spec section 4).
//!
//! Precedence, fixed: `decision key=log-writers` lines in the log win over
//! `.context/eventlog.toml`, which wins over the built-in default. Apply them
//! in that order — [`Allowlist::builtin`], then [`Allowlist::merge_file`],
//! then one [`Allowlist::apply_decision`] per decision line in `seq` order —
//! so the allowlist is evaluated as of each line's own `seq` and a later
//! revocation never turns history into a breach.

use std::collections::BTreeMap;

/// Types only the controller may write, whatever a decision says.
pub const CONTROLLER_CORE: &[&str] =
    &["spawn", "prompt", "claim", "decision", "retire", "approval"];

/// Types any `by=`-tagged reactor may write by default.
const REACTOR_TYPES: &[&str] = &[
    "ack",
    "note",
    "escalate",
    "violation",
    "observed",
    "intent",
    "veto",
    "result",
    "progress",
    "message",
    "drain",
    "seam",
];

/// The writer name that stands for "any writer".
const ANY: &str = "*";

/// type -> the writers allowed to append it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowlist(BTreeMap<String, Vec<String>>);

impl Allowlist {
    /// The built-in default: only `controller` writes the core types; any
    /// `by=`-tagged reactor writes the reactor types.
    pub fn builtin() -> Self {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for ty in CONTROLLER_CORE {
            map.insert(ty.to_string(), vec!["controller".to_string()]);
        }
        for ty in REACTOR_TYPES {
            map.entry(ty.to_string()).or_default().push(ANY.to_string());
        }
        Allowlist(map)
    }

    /// Merge a `[writers]` table from the config file. Each type the file
    /// names replaces that type's writers; other types are untouched.
    pub fn merge_file(&mut self, extra: BTreeMap<String, Vec<String>>) {
        for (ty, writers) in extra {
            self.0.insert(ty, writers);
        }
    }

    /// Apply one `decision key=log-writers` value.
    ///
    /// `controller-plus-reactors` restores the built-in. Any other value is
    /// `name:type1|type2;name2:type3` and replaces the whole map; `controller`
    /// always keeps [`CONTROLLER_CORE`].
    pub fn apply_decision(&mut self, value: &str) {
        if value.trim() == "controller-plus-reactors" {
            *self = Self::builtin();
            return;
        }
        if !value.contains(':') {
            // A label with no `name:types` clause (for example
            // `controller-plus-reactors-plus-briefed-workers`) changes nothing;
            // the grant lives in the briefs, not in the allowlist.
            return;
        }
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for clause in value.split(';') {
            let clause = clause.trim();
            if clause.is_empty() {
                continue;
            }
            let Some((name, types)) = clause.split_once(':') else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            for ty in types.split('|') {
                let ty = ty.trim();
                if ty.is_empty() {
                    continue;
                }
                let writers = map.entry(ty.to_string()).or_default();
                if !writers.iter().any(|w| w == name) {
                    writers.push(name.to_string());
                }
            }
        }
        for ty in CONTROLLER_CORE {
            let writers = map.entry(ty.to_string()).or_default();
            if !writers.iter().any(|w| w == "controller") {
                writers.push("controller".to_string());
            }
        }
        *self = Allowlist(map);
    }

    /// May `writer` append a line of type `ty`? The controller (the default
    /// writer, the one that omits `by=`) may append every type; for any other
    /// writer an unlisted type is denied.
    pub fn permits(&self, writer: &str, ty: &str) -> bool {
        if writer == "controller" {
            return true;
        }
        match self.0.get(ty) {
            Some(writers) => writers.iter().any(|w| w == writer || w == ANY),
            None => false,
        }
    }
}

impl Default for Allowlist {
    fn default() -> Self {
        Self::builtin()
    }
}
