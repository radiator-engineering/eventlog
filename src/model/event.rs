//! One log line: [`Event`], its parser and its serializer.
//!
//! The on-disk shape is spec section 3. `seq` is a JSON integer; every other
//! field is a JSON string, including the reference fields (`seq_done`, `for`,
//! `for_ack`, `intent`) that name another line's `seq`. Those are parsed as
//! integers in memory by [`Event::seq_ref`] and compared numerically, never as
//! strings.

use indexmap::IndexMap;
use std::fmt;

/// A line that did not parse. `line_hint` is a short prefix of the offending
/// text, so a caller can name the line without echoing 4 KB of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line_hint: String,
    pub reason: String,
}

impl ParseError {
    fn new(line: &str, reason: impl Into<String>) -> Self {
        ParseError {
            line_hint: hint(line),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason, self.line_hint)
    }
}

impl std::error::Error for ParseError {}

/// The first 80 characters of a line, with control characters stripped.
fn hint(line: &str) -> String {
    let cleaned: String = line
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "<empty line>".to_string()
    } else {
        cleaned
    }
}

/// The fields the spec names, in the order it names them. Everything else
/// lands in [`Event::fields`], in the order it appeared in the file.
const NAMED: [&str; 6] = ["seq", "ts", "type", "prev", "by", "agent"];

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    pub r#type: String,
    pub prev: Option<String>,
    pub by: Option<String>,
    pub agent: Option<String>,
    /// Every other field, in file order.
    pub fields: IndexMap<String, String>,
}

impl Event {
    /// Parse one JSON line. Rejects anything that is not a flat object with an
    /// integer `seq`, a string `ts` and a string `type`; never panics.
    pub fn parse_line(line: &str) -> Result<Event, ParseError> {
        let raw: IndexMap<String, serde_json::Value> = serde_json::from_str(line)
            .map_err(|e| ParseError::new(line, format!("not a JSON object: {e}")))?;

        let seq = match raw.get("seq") {
            Some(serde_json::Value::Number(n)) => n
                .as_u64()
                .ok_or_else(|| ParseError::new(line, "seq is not a non-negative integer"))?,
            Some(serde_json::Value::String(s)) => s
                .parse::<u64>()
                .map_err(|_| ParseError::new(line, "seq is not a non-negative integer"))?,
            Some(_) => return Err(ParseError::new(line, "seq is not a number")),
            None => return Err(ParseError::new(line, "missing seq")),
        };

        let string_at = |key: &str| -> Result<Option<String>, ParseError> {
            match raw.get(key) {
                Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(ParseError::new(line, format!("{key} is not a string"))),
                None => Ok(None),
            }
        };

        let ts = string_at("ts")?.ok_or_else(|| ParseError::new(line, "missing ts"))?;
        let ty = string_at("type")?.ok_or_else(|| ParseError::new(line, "missing type"))?;
        let prev = string_at("prev")?;
        let by = string_at("by")?;
        let agent = string_at("agent")?;

        let mut fields = IndexMap::new();
        for (k, v) in raw.iter() {
            if NAMED.contains(&k.as_str()) {
                continue;
            }
            match v {
                serde_json::Value::String(s) => {
                    fields.insert(k.clone(), s.clone());
                }
                _ => return Err(ParseError::new(line, format!("{k} is not a string"))),
            }
        }

        Ok(Event {
            seq,
            ts,
            r#type: ty,
            prev,
            by,
            agent,
            fields,
        })
    }

    /// Serialize in spec order — `seq`, `ts`, `type`, `prev`, `by`, `agent`,
    /// then the remaining fields in the order they were read. No trailing
    /// newline.
    pub fn to_line(&self) -> String {
        let mut out = String::from("{");
        out.push_str(&format!("\"seq\":{}", self.seq));
        push_str_field(&mut out, "ts", &self.ts);
        push_str_field(&mut out, "type", &self.r#type);
        for (key, value) in [
            ("prev", &self.prev),
            ("by", &self.by),
            ("agent", &self.agent),
        ] {
            if let Some(v) = value {
                push_str_field(&mut out, key, v);
            }
        }
        for (k, v) in &self.fields {
            if matches!(k.as_str(), "seq" | "ts" | "type" | "prev" | "by" | "agent") {
                continue;
            }
            push_str_field(&mut out, k, v);
        }
        out.push('}');
        out
    }

    /// A reference field read as the `seq` it names. `None` when the field is
    /// absent or not a non-negative integer.
    pub fn seq_ref(&self, field: &str) -> Option<u64> {
        self.fields.get(field)?.parse::<u64>().ok()
    }

    /// The `paths` field split on `,`. Empty when there is no `paths`.
    pub fn paths(&self) -> Vec<String> {
        match self.fields.get("paths") {
            Some(p) => p
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            None => Vec::new(),
        }
    }

    /// Who wrote the line. Absent `by` means the controller wrote it.
    pub fn writer(&self) -> &str {
        self.by.as_deref().unwrap_or("controller")
    }

    /// Whose work the line is about. Absent `agent` means the controller's own.
    pub fn subject(&self) -> &str {
        self.agent.as_deref().unwrap_or("controller")
    }
}

fn push_str_field(out: &mut String, key: &str, value: &str) {
    if !out.ends_with('{') {
        out.push(',');
    }
    out.push_str(&json_string(key));
    out.push(':');
    out.push_str(&json_string(value));
}

fn json_string(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}
