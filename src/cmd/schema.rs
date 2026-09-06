//! JSON Schema for log lines and `--json` view rows.

use std::collections::BTreeMap;

use schemars::{
    JsonSchema,
    schema::{InstanceType, RootSchema, Schema, SchemaObject},
    schema_for,
};
use serde::Serialize;

use crate::cli::{Args, Command};

/// On-disk event line: fixed fields plus arbitrary string keys.
#[derive(JsonSchema, Serialize)]
struct EventLineSchema {
    seq: u64,
    ts: String,
    #[serde(rename = "type")]
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, String>,
}

/// One `eventlog view --json` row: the event fields plus `"v": 1`.
#[derive(JsonSchema, Serialize)]
struct JsonRowSchema {
    v: u32,
    seq: u64,
    ts: String,
    #[serde(rename = "type")]
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, String>,
}

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Schema(schema_args) = &args.command else {
        anyhow::bail!("schema run called with wrong command");
    };

    let mut schema = if schema_args.output {
        schema_for!(JsonRowSchema)
    } else {
        schema_for!(EventLineSchema)
    };

    finish_log_schema(&mut schema);

    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(0)
}

fn finish_log_schema(schema: &mut RootSchema) {
    let object = schema.schema.object.get_or_insert_with(Box::default);

    object.additional_properties = Some(Box::new(Schema::Object(SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        ..Default::default()
    })));

    if let Some(Schema::Object(ts_obj)) = object.properties.get_mut("ts") {
        ts_obj.format = Some("date-time".to_string());
    }
}
