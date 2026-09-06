use eventlog::log::lock::Lock;
use std::time::{Duration, Instant};

#[test]
fn second_acquire_in_one_process_is_busy_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let lock_dir = dir.path().join("events.jsonl.lock");

    let _first = Lock::acquire(&lock_dir, Duration::from_millis(50)).unwrap();

    let start = Instant::now();
    let second = Lock::acquire(&lock_dir, Duration::from_millis(100));
    assert!(second.is_err());
    assert!(start.elapsed() < Duration::from_millis(200));
}

#[test]
fn a_lock_held_by_a_dead_pid_is_reclaimed() {
    let dir = tempfile::tempdir().unwrap();
    let lock_dir = dir.path().join("events.jsonl.lock");

    std::fs::create_dir(&lock_dir).unwrap();
    std::fs::write(lock_dir.join("pid"), "999999").unwrap();

    let lock = Lock::acquire(&lock_dir, Duration::from_millis(200));
    assert!(lock.is_ok());
}
