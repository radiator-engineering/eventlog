//! The event vocabulary: which fields each `type` carries.
//!
//! [`Vocabulary::builtin`] is the table in `.context/EVENTLOG.md` plus `ack`,
//! `note`, `intent`, `veto` and `violation`. A `[vocabulary.<type>]` table in
//! `.context/eventlog.toml` adds types and adds fields to existing ones; it
//! can never remove a built-in type or a built-in required field.
//!
//! ## `decision key=log-writers` grammar
//!
//! The value `controller-plus-reactors` keeps the built-in allowlist. Any
//! other value is `name:type1|type2;name2:type3` and **replaces the whole
//! map** — revoking a writer is a new decision that omits it. `controller`
//! always retains `spawn`, `prompt`, `claim`, `decision`, `retire` and
//! `approval`, whatever the value says. See [`crate::model::allow::Allowlist::apply_decision`];
//! `.context/EVENTLOG.md` repeats this for humans.

use std::collections::BTreeMap;

/// Fields that name another line's `seq`. They stay JSON strings on disk and
/// are read as integers by [`crate::model::event::Event::seq_ref`].
pub const REFERENCE_FIELDS: &[&str] = &["seq_done", "for", "for_ack", "intent"];

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TypeSpec {
    /// Fields a line of this type must carry.
    #[serde(default)]
    pub fields: Vec<String>,
    /// Fields it may carry.
    #[serde(default)]
    pub optional: Vec<String>,
}

impl TypeSpec {
    fn new(fields: &[&str], optional: &[&str]) -> Self {
        TypeSpec {
            fields: fields.iter().map(|s| s.to_string()).collect(),
            optional: optional.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vocabulary(BTreeMap<String, TypeSpec>);

impl Vocabulary {
    pub fn builtin() -> Self {
        let table: &[(&str, &[&str], &[&str])] = &[
            (
                "spawn",
                &["agent"],
                &[
                    "model",
                    "runtime",
                    "tab",
                    "pane",
                    "workspace",
                    "role",
                    "kind",
                ],
            ),
            ("prompt", &["agent", "ref"], &["origin", "kind"]),
            ("message", &["from", "to"], &["subject", "ref"]),
            ("drain", &["agent"], &["ref"]),
            (
                "result",
                &["agent", "ref"],
                &[
                    "paths", "summary", "verdict", "detail", "pr", "branch", "worktree",
                ],
            ),
            ("decision", &["key", "value"], &["ref", "mode"]),
            ("escalate", &[], &["agent", "subject", "msg", "ref"]),
            ("approval", &["subject", "decision"], &["by", "ref"]),
            ("retire", &["agent"], &["disposition", "ref", "detail"]),
            ("claim", &["agent", "paths"], &["ref"]),
            ("progress", &["msg"], &["agent", "ref"]),
            ("seam", &["agents"], &["subject", "ref"]),
            ("violation", &["agent", "paths"], &["ref", "detail"]),
            ("ack", &["seq_done", "outcome"], &["ref", "detail"]),
            ("note", &["msg"], &["agent", "ref"]),
            ("intent", &[], &["agent", "paths", "msg", "for", "ref"]),
            ("veto", &["for"], &["role", "reason", "ref"]),
        ];
        Vocabulary(
            table
                .iter()
                .map(|(ty, req, opt)| (ty.to_string(), TypeSpec::new(req, opt)))
                .collect(),
        )
    }

    /// Merge a `[vocabulary]` table from the config file. New types are added;
    /// known types gain any field the file names. Nothing is removed.
    pub fn merge_file(&mut self, extra: BTreeMap<String, TypeSpec>) {
        for (ty, spec) in extra {
            let entry = self.0.entry(ty).or_default();
            for f in spec.fields {
                if !entry.fields.contains(&f) {
                    entry.fields.push(f);
                }
            }
            for f in spec.optional {
                if !entry.optional.contains(&f) && !entry.fields.contains(&f) {
                    entry.optional.push(f);
                }
            }
        }
    }

    pub fn get(&self, ty: &str) -> Option<&TypeSpec> {
        self.0.get(ty)
    }

    /// Every known type, sorted.
    pub fn types(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self::builtin()
    }
}
