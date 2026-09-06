use assert_cmd::Command;
use serde_json::Value;

#[test]
fn events_schema_has_core_fields() {
    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["schema", "--events"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema["properties"]["seq"]["type"], "integer");
    assert_eq!(schema["additionalProperties"]["type"], "string");
}

#[test]
fn default_schema_is_events() {
    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .arg("schema")
        .output()
        .unwrap();
    assert!(output.status.success());
    let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema["properties"]["seq"]["type"], "integer");
}

#[test]
fn output_schema_includes_v() {
    let output = Command::cargo_bin("eventlog")
        .unwrap()
        .args(["schema", "--output"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema["properties"]["v"]["type"], "integer");
    assert_eq!(schema["additionalProperties"]["type"], "string");
}
