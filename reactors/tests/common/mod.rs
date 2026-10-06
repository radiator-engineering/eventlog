//! Helpers shared by the reactor integration tests.

use std::path::PathBuf;

/// The `eventlog` binary the reactors run beside. Cargo builds it into the
/// same target directory when the workspace is tested from its root, which is
/// the default (`default-members` names both packages).
#[allow(dead_code)]
pub fn core_bin() -> PathBuf {
    let reactors = PathBuf::from(env!("CARGO_BIN_EXE_eventlog-reactors"));
    let core = reactors.with_file_name(format!("eventlog{}", std::env::consts::EXE_SUFFIX));
    assert!(
        core.is_file(),
        "{} is missing; test from the workspace root so cargo builds the eventlog binary too",
        core.display()
    );
    core
}

/// The `eventlog-reactors` binary under test.
#[allow(dead_code)]
pub fn reactors_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_eventlog-reactors"))
}
