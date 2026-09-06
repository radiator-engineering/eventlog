//! Reading the log file: whole-file read with malformed-line reporting, and a
//! cheap tail check for the writer. Spec section 3.

pub mod append;
pub mod lock;
pub mod verify;

use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::model::event::Event;

/// The log at `path`. Does not require the file to exist yet.
pub struct Log {
    pub path: PathBuf,
}

/// The result of reading every line in the log.
pub struct ReadReport {
    pub events: Vec<Event>,
    /// `(line number, reason)`, one-indexed.
    pub malformed: Vec<(usize, String)>,
}

/// The last line of the log, read without parsing the rest of the file.
pub struct Tail {
    /// `seq` of the last line, or 0 when the file is empty or the last line
    /// does not parse.
    pub last_seq: u64,
    /// Raw bytes of the last line (trailing newline stripped), or `None` when
    /// the file is empty.
    pub last_line: Option<Vec<u8>>,
    /// Whether the last line carries a `prev` field.
    pub chained: bool,
    /// The one-indexed line number of the last line, if it did not parse.
    pub torn: Option<usize>,
}

impl Log {
    pub fn open(path: impl Into<PathBuf>) -> Log {
        Log { path: path.into() }
    }

    /// Read and parse every line. Malformed lines are reported, not fatal.
    pub fn read(&self) -> anyhow::Result<ReadReport> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };

        let mut events = Vec::new();
        let mut malformed = Vec::new();
        for (i, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match Event::parse_line(line) {
                Ok(event) => events.push(event),
                Err(e) => malformed.push((i + 1, e.to_string())),
            }
        }
        Ok(ReadReport { events, malformed })
    }

    /// Read only the last line, without parsing the rest of the file.
    pub fn tail(&self) -> anyhow::Result<Tail> {
        let bytes = match std::fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };

        if bytes.is_empty() {
            return Ok(Tail {
                last_seq: 0,
                last_line: None,
                chained: false,
                torn: None,
            });
        }

        let mut parts: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
        if parts.last() == Some(&&b""[..]) {
            parts.pop();
        }
        let line_no = parts.len();
        let last = parts[line_no - 1];
        let last_line = Some(last.to_vec());

        let parsed = std::str::from_utf8(last)
            .ok()
            .and_then(|s| Event::parse_line(s).ok());

        match parsed {
            Some(event) => Ok(Tail {
                last_seq: event.seq,
                last_line,
                chained: event.prev.is_some(),
                torn: None,
            }),
            None => Ok(Tail {
                last_seq: 0,
                last_line,
                chained: false,
                torn: Some(line_no),
            }),
        }
    }

    /// The hex sha256 of `bytes` with any trailing `\r`/`\n` stripped.
    pub fn hash_line(bytes: &[u8]) -> String {
        let mut end = bytes.len();
        while end > 0 && (bytes[end - 1] == b'\n' || bytes[end - 1] == b'\r') {
            end -= 1;
        }
        let mut hasher = Sha256::new();
        hasher.update(&bytes[..end]);
        hex::encode(hasher.finalize())
    }
}
