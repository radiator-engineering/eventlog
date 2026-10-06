use assert_cmd::Command;

#[test]
fn every_reactor_command_is_recognized() {
    for c in ["react", "action", "setup", "doctor"] {
        Command::cargo_bin("eventlog-reactors")
            .unwrap()
            .arg(c)
            .arg("--help")
            .assert()
            .success();
    }
}
