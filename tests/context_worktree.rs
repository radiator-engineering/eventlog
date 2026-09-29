use eventlog::context::worktree::{collect, parse_numstat, parse_porcelain_z};
use eventlog::model::config::Config;
use eventlog::model::event::Event;
use eventlog::query;
use std::process::Command;

#[test]
fn parse_porcelain_z_plain_and_untracked() {
    let raw = b" M src/a.rs\0?? notes.md\0";
    assert_eq!(
        parse_porcelain_z(raw),
        vec![
            ("M".into(), "src/a.rs".into()),
            ("??".into(), "notes.md".into())
        ]
    );
}

#[test]
fn parse_porcelain_z_spaces_are_unquoted() {
    let raw = b"?? my file.md\0";
    assert_eq!(
        parse_porcelain_z(raw),
        vec![("??".into(), "my file.md".into())]
    );
}

#[test]
fn parse_porcelain_z_rename_keeps_new_name() {
    // -z rename: "R  new\0old\0"
    let raw = b"R  src/new.rs\0src/old.rs\0 M x.rs\0";
    assert_eq!(
        parse_porcelain_z(raw),
        vec![
            ("R".into(), "src/new.rs".into()),
            ("M".into(), "x.rs".into())
        ]
    );
}

#[test]
fn parse_numstat_reads_counts_and_binary() {
    let map = parse_numstat("3\t1\tsrc/a.rs\0-\t-\timg.png\0");
    assert_eq!(map["src/a.rs"], "+3 -1");
    assert_eq!(map["img.png"], "binary");
}

#[test]
fn parse_numstat_rename_and_space_use_new_name() {
    // -z rename record: "<add>\t<del>\t\0<old>\0<new>\0"
    let map = parse_numstat("2\t0\t\0src/old name.rs\0src/new name.rs\0");
    assert_eq!(map["src/new name.rs"], "+2 -0");
    assert!(!map.contains_key("src/old name.rs"));
}

#[test]
fn collect_outside_git_is_unavailable() {
    let dir = tempfile::TempDir::new().unwrap();
    let state = query::fold(&[], &Config::default());
    let wt = collect(dir.path(), &state);
    assert!(!wt.available);
    assert!(wt.changes.is_empty());
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn collect_skips_context_dir_and_marks_claim_owner() {
    let dir = tempfile::TempDir::new().unwrap();
    let d = dir.path();
    git(d, &["init", "-q"]);
    git(d, &["config", "user.email", "t@t"]);
    git(d, &["config", "user.name", "t"]);
    std::fs::create_dir_all(d.join("src/a")).unwrap();
    std::fs::write(d.join("src/a/x.rs"), "one\n").unwrap();
    git(d, &["add", "."]);
    git(d, &["commit", "-qm", "init"]);
    std::fs::write(d.join("src/a/x.rs"), "one\ntwo\n").unwrap();
    std::fs::create_dir_all(d.join(".context")).unwrap();
    std::fs::write(d.join(".context/scratch.md"), "x").unwrap();
    std::fs::write(d.join("new file.md"), "x").unwrap();

    let events = vec![
        Event::parse_line(r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"spawn","agent":"w1"}"#)
            .unwrap(),
        Event::parse_line(
            r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"claim","agent":"w1","paths":"src/a"}"#,
        )
        .unwrap(),
    ];
    let state = query::fold(&events, &Config::default());
    let wt = collect(d, &state);
    assert!(wt.available);
    let paths: Vec<&str> = wt.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["new file.md", "src/a/x.rs"]);
    let x = wt.changes.iter().find(|c| c.path == "src/a/x.rs").unwrap();
    assert_eq!(x.status, "M");
    assert_eq!(x.stat, "+1 -0");
    assert_eq!(x.owner.as_deref(), Some("w1"));
    assert!(x.mtime > 0);
    let n = wt.changes.iter().find(|c| c.path == "new file.md").unwrap();
    assert_eq!(
        (n.status.as_str(), n.stat.as_str(), n.owner.as_deref()),
        ("??", "new", None)
    );
}

#[test]
fn collect_finds_rename_stat_under_new_name_with_space() {
    let dir = tempfile::TempDir::new().unwrap();
    let d = dir.path();
    git(d, &["init", "-q"]);
    git(d, &["config", "user.email", "t@t"]);
    git(d, &["config", "user.name", "t"]);
    std::fs::write(d.join("old name.rs"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
    git(d, &["add", "."]);
    git(d, &["commit", "-qm", "init"]);
    std::fs::rename(d.join("old name.rs"), d.join("new name.rs")).unwrap();
    std::fs::write(d.join("new name.rs"), "one\ntwo\nthree\nfour\nfive\nsix\n").unwrap();
    git(d, &["add", "-A"]);

    let state = query::fold(&[], &Config::default());
    let wt = collect(d, &state);
    assert!(wt.available);
    let renamed = wt.changes.iter().find(|c| c.path == "new name.rs");
    assert!(
        renamed.is_some(),
        "expected renamed path in {:?}",
        wt.changes
    );
    let renamed = renamed.unwrap();
    assert_eq!(renamed.status, "R");
    assert_eq!(renamed.stat, "+1 -0");
}

#[test]
fn narrower_worker_claim_wins_over_a_wider_controller_claim() {
    let dir = tempfile::TempDir::new().unwrap();
    let d = dir.path();
    git(d, &["init", "-q"]);
    git(d, &["config", "user.email", "t@t"]);
    git(d, &["config", "user.name", "t"]);
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(d.join("src/a.rs"), "one\n").unwrap();
    git(d, &["add", "."]);
    git(d, &["commit", "-qm", "init"]);
    std::fs::write(d.join("src/a.rs"), "one\ntwo\n").unwrap();

    // The controller claims the whole `src` directory first; `w1` then
    // claims the narrower `src/a.rs`. `claim_owner("src/a.rs")` would find
    // the controller's claim first and get filtered to None; the narrower
    // non-controller claim should win instead.
    let events = vec![
        Event::parse_line(
            r#"{"seq":1,"ts":"2026-09-01T00:00:00Z","type":"claim","agent":"controller","paths":"src"}"#,
        )
        .unwrap(),
        Event::parse_line(r#"{"seq":2,"ts":"2026-09-01T00:00:01Z","type":"spawn","agent":"w1"}"#)
            .unwrap(),
        Event::parse_line(
            r#"{"seq":3,"ts":"2026-09-01T00:00:02Z","type":"claim","agent":"w1","paths":"src/a.rs"}"#,
        )
        .unwrap(),
    ];
    let state = query::fold(&events, &Config::default());
    let wt = collect(d, &state);
    let a = wt.changes.iter().find(|c| c.path == "src/a.rs").unwrap();
    assert_eq!(a.owner.as_deref(), Some("w1"));
}
