use assert_cmd::Command;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn init_git_repo(dir: &TempDir) {
    StdCommand::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "eventlog-test@example.invalid"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "eventlog test"])
        .current_dir(dir.path())
        .status()
        .unwrap();
}

fn snapshot(dir: &TempDir) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in [
        ".context/events.jsonl",
        ".context/EVENTLOG.md",
        ".context/eventlog.toml",
        ".gitignore",
        ".gitattributes",
    ] {
        let path = dir.path().join(rel);
        let content = fs::read_to_string(&path).unwrap_or_default();
        out.push((rel.to_string(), content));
    }
    out
}

#[test]
fn init_twice_leaves_identical_files() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    let first = snapshot(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    let second = snapshot(&dir);

    assert_eq!(first, second);
    assert!(dir.path().join(".context/events.jsonl").exists());
    assert!(dir.path().join(".context/EVENTLOG.md").exists());
    assert!(dir.path().join(".context/eventlog.toml").exists());
    let gitignore = fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gitignore.contains(".context/events.jsonl"));
    assert!(gitignore.contains(".context/*.reactor.lock/"));
    let attrs = fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert!(attrs.contains(".context/events.jsonl -text"));
}

#[test]
fn setup_is_repeatable_preserves_customization_and_rejects_malformed_config() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    let mut bin = Command::cargo_bin("eventlog").unwrap();
    bin.current_dir(dir.path())
        .args(["setup", "preview"])
        .assert()
        .success()
        .stdout(predicates::str::contains("eventlog-setup.toml"));
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "apply"])
        .assert()
        .success();
    let config = dir.path().join(".context/eventlog-setup.toml");
    let first = fs::read_to_string(&config).unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "apply"])
        .assert()
        .success()
        .stdout(predicates::str::contains("no changes"));
    assert_eq!(fs::read_to_string(&config).unwrap(), first);

    fs::write(
        &config,
        "version = 2\n[commit]\nidentity = \"commit-custom\"\nmodel = \"composer-custom\"\ntimeout = \"12s\"\n[docs]\nidentity = \"docs-custom\"\nmodel = \"sonnet-custom\"\ntimeout = \"34s\"\nroots = [\"manual\", \"GUIDE.md\"]\ncommand = [\"stub\"]\n[invocation]\neventlog = \"eventlog-custom\"\n",
    )
    .unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "upgrade"])
        .assert()
        .success()
        .stdout(predicates::str::contains("eventlog-reactors.star"));
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        "version = 2\n[commit]\nidentity = \"commit-custom\"\nmodel = \"composer-custom\"\ntimeout = \"12s\"\n[docs]\nidentity = \"docs-custom\"\nmodel = \"sonnet-custom\"\ntimeout = \"34s\"\nroots = [\"manual\", \"GUIDE.md\"]\ncommand = [\"stub\"]\n[invocation]\neventlog = \"eventlog-custom\"\n"
    );
    let reactors = fs::read_to_string(dir.path().join(".context/eventlog-reactors.star")).unwrap();
    for expected in [
        "eventlog-custom",
        "commit-custom",
        "composer-custom",
        "12s",
        "docs-custom",
        "sonnet-custom",
        "34s",
        "by=\" + \"commit-custom\"",
        "manual,GUIDE.md",
    ] {
        assert!(
            reactors.contains(expected),
            "missing {expected} in {reactors}"
        );
    }
    fs::write(
        dir.path().join("Drovefile"),
        "load(\".context/eventlog-reactors.star\", \"eventlog_reactors\")\nmain = workspace(\"main\", panes = eventlog_reactors())\nprofile(\"default\", workspaces = [main])\n",
    )
    .unwrap();
    assert!(
        StdCommand::new("drove")
            .arg("render")
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success()
    );
    fs::write(&config, "[docs\n").unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "upgrade"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("configuration conflict"));
    assert_eq!(fs::read_to_string(&config).unwrap(), "[docs\n");
}

#[test]
fn setup_preview_and_apply_upgrade_the_exact_prior_version() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "apply"])
        .assert()
        .success();
    let config = dir.path().join(".context/eventlog-setup.toml");
    let current = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        current
            .replacen("v2", "v1", 1)
            .replacen("version = 2", "version = 1", 1),
    )
    .unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "preview"])
        .assert()
        .success()
        .stdout(predicates::str::contains("eventlog-setup.toml"));
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "upgrade"])
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&config).unwrap(), current);
}

#[test]
fn invalid_docs_roots_fail_setup_without_mutation_or_panic() {
    for roots in [
        "[]",
        "[\"/absolute\"]",
        "[\"../escape\"]",
        "[\"docs,README.md\"]",
    ] {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);
        fs::create_dir_all(dir.path().join(".context")).unwrap();
        let config = dir.path().join(".context/eventlog-setup.toml");
        let text = format!(
            "version = 2\n[commit]\nidentity = \"committer\"\nmodel = \"composer-2.5-fast\"\ntimeout = \"600s\"\n[docs]\nidentity = \"doc-worker\"\nmodel = \"claude-sonnet\"\ntimeout = \"600s\"\nroots = {roots}\ncommand = []\n[invocation]\neventlog = \"eventlog\"\n"
        );
        fs::write(&config, &text).unwrap();
        for command in [
            ["setup", "preview"].as_slice(),
            ["setup", "apply"].as_slice(),
            ["setup", "upgrade"].as_slice(),
        ] {
            let output = Command::cargo_bin("eventlog")
                .unwrap()
                .current_dir(dir.path())
                .args(command)
                .output()
                .unwrap();
            assert!(!output.status.success(), "{roots} {command:?}");
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            assert!(!stderr.contains("panic"), "{stderr}");
        }
        assert_eq!(fs::read_to_string(&config).unwrap(), text);
        assert!(!dir.path().join(".context/eventlog-reactors.star").exists());
        assert!(!dir.path().join(".context/events.jsonl").exists());
    }
}

#[test]
fn custom_docs_root_claim_authorizes_a_docs_result() {
    use eventlog::model::config::Config;
    use eventlog::model::event::Event;
    let events = [
        r#"{"seq":1,"ts":"2026-09-07T00:00:00Z","type":"spawn","agent":"docs-custom"}"#,
        r#"{"seq":2,"ts":"2026-09-07T00:00:01Z","type":"claim","agent":"docs-custom","paths":"manual/**,GUIDE.md"}"#,
    ]
    .into_iter()
    .map(|line| Event::parse_line(line).unwrap())
    .collect::<Vec<_>>();
    let state = eventlog::query::fold(&events, &Config::default());
    let result = Event::parse_line(
        r#"{"seq":3,"ts":"2026-09-07T00:00:02Z","type":"result","by":"docs-custom","agent":"docs-custom","ref":"HEAD","paths":"manual/page.md,GUIDE.md"}"#,
    )
    .unwrap();
    let authorized = eventlog::react::voter::authorize(&result, &state);
    assert!(authorized.excess.is_empty());
    assert_eq!(authorized.paths.len(), 2);
}

#[test]
fn generated_literal_roots_start_lifecycle_and_authorize_docs_action_paths() {
    use eventlog::model::config::Config;
    use eventlog::model::event::Event;
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::write(dir.path().join("LICENSE"), "before\n").unwrap();
    fs::create_dir_all(dir.path().join("docs.v2")).unwrap();
    fs::write(dir.path().join("docs.v2/page.md"), "before\n").unwrap();
    fs::create_dir_all(dir.path().join("empty")).unwrap();
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        "version = 2\n[commit]\nidentity = \"committer\"\nmodel = \"composer-2.5-fast\"\ntimeout = \"600s\"\n[docs]\nidentity = \"docs-custom\"\nmodel = \"claude-sonnet\"\ntimeout = \"600s\"\nroots = [\"LICENSE\", \"docs.v2\", \"empty\"]\ncommand = [\"sh\", \"-c\", \"printf after > LICENSE; printf after > docs.v2/page.md\"]\n[invocation]\neventlog = \"eventlog\"\n",
    )
    .unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["setup", "apply"])
        .assert()
        .success();
    let helper = fs::read_to_string(dir.path().join(".context/eventlog-reactors.star")).unwrap();
    assert!(helper.contains("\"LICENSE,docs.v2,empty\""));
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "lifecycle",
            "start",
            "docs-custom",
            "--model",
            "claude-sonnet",
            "--paths",
            "LICENSE,docs.v2,empty",
        ])
        .assert()
        .success();
    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["action", "docs"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("paths=LICENSE,docs.v2/page.md"), "{stdout}");
    let log = eventlog::log::Log::open(dir.path().join(".context/events.jsonl"));
    let state = eventlog::query::fold(&log.read().unwrap().events, &Config::default());
    let result = Event::parse_line(
        r#"{"seq":99,"ts":"2026-09-07T00:00:02Z","type":"result","by":"docs-custom","agent":"docs-custom","ref":"HEAD","paths":"LICENSE,docs.v2/page.md"}"#,
    )
    .unwrap();
    let authorized = eventlog::react::voter::authorize(&result, &state);
    assert!(authorized.excess.is_empty());
    assert_eq!(authorized.paths.len(), 2);
}

#[test]
fn lifecycle_start_stop_start_restores_only_its_own_claim() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::write(dir.path().join("owned.txt"), "x\n").unwrap();
    let run = |args: &[&str]| {
        Command::cargo_bin("eventlog")
            .unwrap()
            .current_dir(dir.path())
            .args(args)
            .assert()
            .success()
    };
    run(&["init"]);
    run(&[
        "lifecycle",
        "start",
        "worker",
        "--model",
        "stub",
        "--paths",
        "owned.txt",
    ]);
    run(&[
        "lifecycle",
        "start",
        "worker",
        "--model",
        "stub",
        "--paths",
        "owned.txt",
    ]);
    run(&["lifecycle", "stop", "worker"]);
    run(&[
        "lifecycle",
        "start",
        "worker",
        "--model",
        "stub",
        "--paths",
        "owned.txt",
    ]);
    let log = fs::read_to_string(dir.path().join(".context/events.jsonl")).unwrap();
    assert_eq!(log.matches("\"type\":\"spawn\"").count(), 2);
    assert_eq!(log.matches("\"type\":\"claim\"").count(), 2);
    assert_eq!(log.matches("\"type\":\"retire\"").count(), 1);
}

#[test]
fn lifecycle_rejects_invalid_claim_before_spawning_and_keeps_other_claims() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::write(dir.path().join("first.txt"), "x\n").unwrap();
    fs::write(dir.path().join("second.txt"), "x\n").unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["lifecycle", "start", "first", "--paths", "first.txt"])
        .assert()
        .success();
    let log = dir.path().join(".context/events.jsonl");
    let before = fs::read_to_string(&log).unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["lifecycle", "start", "broken", "--paths", "missing.txt"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("claim-path-missing"));
    assert_eq!(fs::read_to_string(&log).unwrap(), before);
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["lifecycle", "start", "second", "--paths", "second.txt"])
        .assert()
        .success();
    let after = fs::read_to_string(&log).unwrap();
    assert!(after.contains("\"agent\":\"first\""));
    assert!(after.contains("\"agent\":\"second\""));
}

#[test]
fn commit_action_keeps_unrelated_staging_out_of_the_commit() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::write(dir.path().join("target.txt"), "target\n").unwrap();
    fs::write(dir.path().join("unrelated.txt"), "unrelated\n").unwrap();
    StdCommand::new("git")
        .args(["add", "unrelated.txt"])
        .current_dir(dir.path())
        .status()
        .unwrap();

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_PATHS", "target.txt")
        .args(["action", "commit", "--message", "feat: target"])
        .assert()
        .success();

    let show = StdCommand::new("git")
        .args(["show", "--format=", "--name-only", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&show.stdout).trim(), "target.txt");
    let staged = StdCommand::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&staged.stdout).trim(),
        "unrelated.txt"
    );
}

#[test]
fn commit_action_rejects_glob_and_directory_scopes_with_staged_target() {
    for scope in ["src/*.rs", "src"] {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "a\n").unwrap();
        fs::write(dir.path().join("src/b.rs"), "b\n").unwrap();
        StdCommand::new("git")
            .args(["add", "src/b.rs"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        Command::cargo_bin("eventlog")
            .unwrap()
            .current_dir(dir.path())
            .env("EVENTLOG_PATHS", scope)
            .args(["action", "commit"])
            .assert()
            .failure()
            .stdout(predicates::str::contains("already staged"));
        let staged = StdCommand::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&staged.stdout).trim(), "src/b.rs");
    }
}

#[test]
fn docs_action_reports_added_deleted_and_already_dirty_doc_files() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/existing.md"), "base\n").unwrap();
    fs::write(dir.path().join("README.md"), "base\n").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .status()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-qm", "initial"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    // The action must still report this file when its command changes it again.
    fs::write(dir.path().join("docs/existing.md"), "already dirty\n").unwrap();
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        "[docs]\nroots = [\"docs\", \"README.md\"]\ncommand = [\"sh\", \"-c\", \"printf changed > docs/existing.md; rm README.md; touch 'docs/new name.md'\"]\n",
    )
    .unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["action", "docs"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "paths=README.md,docs/existing.md,docs/new name.md",
        ));
}

#[test]
fn docs_action_fails_with_the_exact_out_of_scope_paths() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/page.md"), "before\n").unwrap();
    fs::write(dir.path().join("source.rs"), "before\n").unwrap();
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        "[docs]\nroots = [\"docs\"]\ncommand = [\"sh\", \"-c\", \"printf after > docs/page.md; printf changed > source.rs\"]\n",
    )
    .unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["action", "docs"])
        .assert()
        .failure()
        .stdout(predicates::str::contains(
            "outside configured roots: source.rs",
        ));
}

#[test]
fn docs_action_emits_one_result_and_skips_its_own_result() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/page.md"), "before\n").unwrap();
    StdCommand::new("git")
        .args(["add", "docs/page.md"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-qm", "source"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let source_ref = String::from_utf8(
        StdCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(
        dir.path().join(".context/events.jsonl"),
        format!(
            "{{\"seq\":1,\"ts\":\"2026-09-07T00:00:00Z\",\"type\":\"result\",\"agent\":\"implementation\",\"ref\":\"{source_ref}\"}}\n{{\"seq\":2,\"ts\":\"2026-09-07T00:00:01Z\",\"type\":\"ack\",\"by\":\"committer\",\"seq_done\":\"1\",\"outcome\":\"committed\",\"ref\":\"{source_ref}\"}}\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        "[docs]\nidentity = \"doc-worker\"\nroots = [\"docs\"]\ncommand = [\"sh\", \"-c\", \"printf after > docs/page.md\"]\n",
    )
    .unwrap();
    let log = dir.path().join(".context/events.jsonl");
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_LOG", &log)
        .env("EVENTLOG_TYPE", "ack")
        .env("EVENTLOG_SEQ", "2")
        .args(["action", "docs"])
        .assert()
        .success();
    let once = fs::read_to_string(&log).unwrap();
    assert!(once.contains("\"type\":\"result\""));
    assert!(once.contains("\"agent\":\"doc-worker\""));

    fs::write(
        &log,
        format!(
            "{once}{{\"seq\":4,\"ts\":\"2026-09-07T00:00:03Z\",\"type\":\"ack\",\"by\":\"committer\",\"seq_done\":\"3\",\"outcome\":\"committed\",\"ref\":\"{source_ref}\"}}\n"
        ),
    )
    .unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_LOG", &log)
        .env("EVENTLOG_TYPE", "ack")
        .env("EVENTLOG_SEQ", "4")
        .args(["action", "docs"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        format!(
            "{once}{{\"seq\":4,\"ts\":\"2026-09-07T00:00:03Z\",\"type\":\"ack\",\"by\":\"committer\",\"seq_done\":\"3\",\"outcome\":\"committed\",\"ref\":\"{source_ref}\"}}\n"
        )
    );
}

#[test]
fn docs_action_validates_each_cumulative_ack_ref() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/page.md"), "before\n").unwrap();
    StdCommand::new("git")
        .args(["add", "docs/page.md"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-qm", "source"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    let reference = String::from_utf8(
        StdCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(
        dir.path().join(".context/eventlog-setup.toml"),
        "[docs]\nroots = [\"docs\"]\ncommand = [\"sh\", \"-c\", \"true\"]\n",
    )
    .unwrap();
    let log = dir.path().join(".context/events.jsonl");
    fs::write(
        &log,
        format!("{{\"seq\":1,\"ts\":\"2026-09-07T00:00:00Z\",\"type\":\"ack\",\"seq_done\":\"99\",\"outcome\":\"committed\",\"ref\":\"{reference},not-a-commit\"}}\n"),
    )
    .unwrap();
    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .env("EVENTLOG_LOG", &log)
        .env("EVENTLOG_TYPE", "ack")
        .env("EVENTLOG_SEQ", "1")
        .args(["action", "docs"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not a commit"));
}

fn write_unsanctioned_log(dir: &TempDir) {
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(dir.path().join("foo.rs"), "// fixture\n").unwrap();
    let log = dir.path().join(".context/events.jsonl");
    fs::write(
        &log,
        concat!(
            r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"x"}"#,
            "\n",
            r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"prompt","agent":"x","ref":"x.md"}"#,
            "\n",
            r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"claim","by":"x","agent":"x","paths":"foo.rs"}"#,
            "\n",
        ),
    )
    .unwrap();
}

fn write_sanctioned_log(dir: &TempDir) {
    fs::create_dir_all(dir.path().join(".context")).unwrap();
    fs::write(dir.path().join("foo.rs"), "// fixture\n").unwrap();
    let log = dir.path().join(".context/events.jsonl");
    fs::write(
        &log,
        concat!(
            r#"{"seq":1,"ts":"2026-09-06T00:00:00Z","type":"spawn","agent":"x"}"#,
            "\n",
            r#"{"seq":2,"ts":"2026-09-06T00:00:01Z","type":"decision","key":"log-writers","value":"x:claim"}"#,
            "\n",
            r#"{"seq":3,"ts":"2026-09-06T00:00:02Z","type":"claim","by":"x","agent":"x","paths":"foo.rs"}"#,
            "\n",
        ),
    )
    .unwrap();
}

#[test]
fn doctor_flags_unsanctioned_writer_without_grant() {
    let dir = TempDir::new().unwrap();
    write_unsanctioned_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["doctor"])
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("[FAIL]") && combined.to_lowercase().contains("unsanctioned"),
        "expected unsanctioned FAIL, got:\n{combined}"
    );
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn doctor_ok_when_log_writers_grants_the_writer() {
    let dir = TempDir::new().unwrap();
    write_sanctioned_log(&dir);

    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["doctor"])
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.to_lowercase().contains("unsanctioned"),
        "unexpected unsanctioned row:\n{combined}"
    );
    assert!(
        !combined.contains("[FAIL]"),
        "expected no FAIL rows:\n{combined}"
    );
}

#[test]
#[ignore = "needs chflags/chattr permission; run locally with: cargo test --test scaffold -- --ignored"]
fn protect_status_before_and_after_protect() {
    let dir = TempDir::new().unwrap();
    init_git_repo(&dir);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["protect", "--status"])
        .assert()
        .failure()
        .code(1);

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .arg("protect")
        .assert()
        .success();

    Command::cargo_bin("eventlog")
        .unwrap()
        .current_dir(dir.path())
        .args(["protect", "--status"])
        .assert()
        .success()
        .code(0);
}
