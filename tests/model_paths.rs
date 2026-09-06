use eventlog::model::paths::{canonicalize, is_log_or_lock, validate_paths};
use std::path::Path;

#[test]
fn rejects_absolute_and_escape() {
    assert!(validate_paths("/etc/hosts").is_err());
    assert!(validate_paths("../x").is_err());
    assert!(validate_paths("a/../../x").is_err());
}

#[test]
fn splits_on_comma() {
    assert_eq!(validate_paths("a.md,b/c.rs").unwrap().len(), 2);
}

#[test]
fn rejects_empty_entries() {
    assert!(validate_paths("").is_err());
    assert!(validate_paths("a.md,").is_err());
    assert!(validate_paths("a.md, ,b.rs").is_err());
}

#[test]
fn keeps_interior_dotdot_that_stays_inside() {
    let ps = validate_paths("a/b/../c.rs").unwrap();
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].as_str(), "a/b/../c.rs");
}

#[test]
fn canonicalize_rejects_symlinked_components() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join("real")).unwrap();
    std::fs::write(root.join("real/file.txt"), b"hi").unwrap();
    std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();

    let ok = validate_paths("real/file.txt").unwrap();
    assert!(canonicalize(root, &ok[0]).is_ok());

    let bad = validate_paths("link/file.txt").unwrap();
    assert!(canonicalize(root, &bad[0]).is_err());
}

#[test]
fn spots_the_log_and_its_locks() {
    let log = Path::new(".context/events.jsonl");
    assert!(is_log_or_lock(log, Path::new(".context/events.jsonl")));
    assert!(is_log_or_lock(log, Path::new(".context/events.jsonl.lock")));
    assert!(is_log_or_lock(
        log,
        Path::new(".context/events.jsonl.commit.reactor.lock")
    ));
    assert!(!is_log_or_lock(log, Path::new(".context/events2.jsonl")));
    assert!(!is_log_or_lock(log, Path::new("src/lib.rs")));
}
