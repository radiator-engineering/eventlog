use eventlog::log::Log;
use eventlog::log::verify::{VerifyReport, verify};

fn strip_prev(line: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
    if let Some(obj) = value.as_object_mut() {
        obj.remove("prev");
    }
    value.to_string()
}

#[test]
fn chain_ok_fixture_passes() {
    let log = Log::open("tests/fixtures/chain_ok.jsonl");
    let report = verify(&log).unwrap();
    assert_eq!(
        report,
        VerifyReport {
            checked: 4,
            last_good: 4,
            failure: None,
        }
    );
}

#[test]
fn chain_break_last_good_is_seq_before_break() {
    let log = Log::open("tests/fixtures/chain_break.jsonl");
    let report = verify(&log).unwrap();
    assert!(report.failure.is_some());
    assert_eq!(report.last_good, 3);
}

#[test]
fn chain_unchained_after_names_the_seq() {
    let log = Log::open("tests/fixtures/chain_unchained_after.jsonl");
    let report = verify(&log).unwrap();
    let failure = report.failure.unwrap();
    assert!(failure.contains("unchained"));
    assert!(failure.contains("4"));
    assert_eq!(report.last_good, 3);
}

#[test]
fn stripping_every_prev_from_chain_ok_fails_with_unchained() {
    let content = std::fs::read_to_string("tests/fixtures/chain_ok.jsonl").unwrap();
    // Line 1 keeps `prev=genesis` so the chain anchor remains; every other line
    // loses `prev`, producing an unchained line at seq 2.
    let mutated = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                line.to_string()
            } else {
                strip_prev(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stripped.jsonl");
    std::fs::write(&path, mutated).unwrap();

    let report = verify(&Log::open(&path)).unwrap();
    let failure = report.failure.expect("expected verify failure");
    assert!(
        failure.contains("unchained"),
        "expected unchained failure, got: {failure}"
    );
}
