//! `eventlog append` — validate, optionally strict-check against the fold, write one line.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::cli::{AppendArgs, Args as CliArgs, Command};
use crate::log::Log;
use crate::log::append::{AppendError, AppendRequest, StrictContext, append};
use crate::log::lock::LockError;
use crate::model::config::{self, Config};
use crate::query::{self, Phase, State};

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::Append(append_args) = &args.command else {
        anyhow::bail!("append::run called with wrong subcommand");
    };

    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&repo_root)?;
    let log_path = log_path(&repo_root, &cfg, args.log.as_deref());

    let writer = resolve_writer(append_args);
    let fields = parse_fields(&append_args.fields)?;
    let report = Log::open(&log_path).read()?;
    let state = query::fold(&report.events, &cfg);

    let req = AppendRequest {
        r#type: append_args.r#type.clone(),
        fields,
        writer,
        strict: !append_args.no_strict,
        dry_run: append_args.dry_run,
    };

    match append(&Log::open(&log_path), &cfg, req, Some(&FoldContext(&state))) {
        Ok(event) => {
            println!("{}", event.to_line());
            Ok(0)
        }
        Err(err) => {
            eprintln!("{err}");
            Ok(exit_code(&err))
        }
    }
}

/// Static epilogue for `append --help` (builtin vocabulary; `eventlog vocab` uses loaded config).
pub const DEFAULT_AFTER_HELP: &str = "\
Vocabulary (required / optional fields):
  spawn: required=agent optional=model,runtime,tab,pane,workspace,role,kind
  prompt: required=agent,ref optional=origin,kind
  message: required=from,to optional=subject,ref
  drain: required=agent optional=ref
  result: required=agent,ref optional=paths,summary,verdict,detail,pr,branch,worktree
  decision: required=key,value optional=ref,mode
  escalate: required= optional=agent,subject,msg,ref
  approval: required=subject,decision optional=by,ref
  retire: required=agent optional=disposition,ref,detail
  claim: required=agent,paths optional=ref
  progress: required=msg optional=agent,ref
  seam: required=agents optional=subject,ref
  violation: required=agent,paths optional=ref,detail
  ack: required=seq_done,outcome optional=ref,detail
  note: required=msg optional=agent,ref
  intent: required= optional=agent,paths,msg,for,ref
  veto: required=for optional=role,reason,ref
  observed: required=paths optional=for,ref,detail
";

pub fn vocab_epilogue(cfg: &Config) -> String {
    let mut out = String::from("Vocabulary (required / optional fields):\n");
    for ty in cfg.vocabulary.types() {
        let Some(spec) = cfg.vocabulary.get(ty) else {
            continue;
        };
        out.push_str(&format!("  {ty}: {}", format_fields(spec)));
    }
    out
}

fn format_fields(spec: &crate::model::vocab::TypeSpec) -> String {
    let opt = if spec.optional.is_empty() {
        String::new()
    } else {
        format!(" optional={}", spec.optional.join(","))
    };
    format!("required={}{opt}", spec.fields.join(","))
}

struct FoldContext<'a>(&'a State);

impl StrictContext for FoldContext<'_> {
    fn allowlist(&self) -> &crate::model::allow::Allowlist {
        &self.0.allowlist
    }

    fn agent_is_open(&self, agent: &str) -> bool {
        if agent == "controller" {
            return true;
        }
        self.0
            .agents
            .get(agent)
            .is_some_and(|a| a.phase != Phase::Retired)
    }

    fn claim_owner(&self, path: &str) -> Option<String> {
        self.0.claim_owner(path).map(str::to_string)
    }

    fn has_open_escalation(&self, agent: &str) -> bool {
        self.0.escalations.iter().any(|e| {
            e.fields
                .get("agent")
                .is_some_and(|subject| subject == agent)
                || e.fields
                    .get("subject")
                    .is_some_and(|subject| subject == agent)
        })
    }
}

fn resolve_writer(args: &AppendArgs) -> String {
    args.writer
        .clone()
        .or_else(|| std::env::var("EVENTLOG_AS").ok())
        .unwrap_or_else(|| "controller".to_string())
}

fn parse_fields(raw: &[String]) -> anyhow::Result<Vec<(String, String)>> {
    raw.iter()
        .map(|pair| {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("field must be key=value, got: {pair}"))?;
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

fn log_path(repo_root: &Path, cfg: &Config, selector: Option<&str>) -> PathBuf {
    let path = config::resolve_log(cfg, selector);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}

fn exit_code(err: &AppendError) -> i32 {
    match err {
        AppendError::Lock(LockError::Busy { .. }) => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epilogue_includes_result_fields() {
        let cfg = Config::default();
        let text = vocab_epilogue(&cfg);
        assert!(text.contains("result:"));
        assert!(text.contains("agent,ref"));
    }
}
