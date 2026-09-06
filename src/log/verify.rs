//! Hash-chain verification (spec section 3).

use crate::log::Log;
use crate::model::event::Event;

/// Outcome of walking the hash chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyReport {
    pub checked: u64,
    pub last_good: u64,
    pub failure: Option<String>,
}

/// Walk `log`, checking `prev` links and reporting the first failure.
pub fn verify(log: &Log) -> anyhow::Result<VerifyReport> {
    let content = match std::fs::read_to_string(&log.path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };

    let mut checked = 0u64;
    let mut last_good = 0u64;
    let mut chain_started = false;
    let mut prev_line_bytes: Option<Vec<u8>> = None;

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let line_no = i + 1;
        let line_bytes = line.as_bytes().to_vec();

        let event = match Event::parse_line(line) {
            Ok(e) => e,
            Err(e) => {
                return Ok(VerifyReport {
                    checked,
                    last_good,
                    failure: Some(format!("malformed line {line_no}: {e}")),
                });
            }
        };

        match &event.prev {
            None => {
                if chain_started {
                    return Ok(VerifyReport {
                        checked,
                        last_good,
                        failure: Some(format!("unchained line at seq {}", event.seq)),
                    });
                }
            }
            Some(prev) => {
                let expected = if chain_started {
                    Log::hash_line(prev_line_bytes.as_ref().expect("prior line"))
                } else if prev_line_bytes.is_none() {
                    "genesis".to_string()
                } else {
                    Log::hash_line(prev_line_bytes.as_ref().expect("prior line"))
                };

                if prev != &expected {
                    return Ok(VerifyReport {
                        checked,
                        last_good,
                        failure: Some(format!("chain break at seq {}", event.seq)),
                    });
                }
                chain_started = true;
            }
        }

        prev_line_bytes = Some(line_bytes);
        checked += 1;
        last_good = event.seq;
    }

    Ok(VerifyReport {
        checked,
        last_good,
        failure: None,
    })
}
