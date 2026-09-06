//! `eventlog vocab` — list required and optional fields per event type.

use anyhow::Context;

use crate::cli::{Args as CliArgs, Command, VocabArgs};
use crate::model::config::{self, Config};
use crate::model::vocab::TypeSpec;

pub fn run(args: &CliArgs) -> anyhow::Result<i32> {
    let Command::Vocab(vocab_args) = &args.command else {
        anyhow::bail!("vocab::run called with wrong subcommand");
    };

    let repo_root = std::env::current_dir().context("current directory")?;
    let cfg = config::load(&repo_root)?;

    if args.json {
        print_json(vocab_args, &cfg)?;
    } else {
        print_text(vocab_args, &cfg);
    }
    Ok(0)
}

fn print_json(vocab_args: &VocabArgs, cfg: &Config) -> anyhow::Result<()> {
    if let Some(ty) = &vocab_args.r#type {
        let spec = cfg
            .vocabulary
            .get(ty)
            .ok_or_else(|| anyhow::anyhow!("unknown type: {ty}"))?;
        let row = type_json(ty, spec);
        println!("{}", serde_json::to_string(&row)?);
        return Ok(());
    }

    let mut rows = Vec::new();
    for ty in cfg.vocabulary.types() {
        let Some(spec) = cfg.vocabulary.get(ty) else {
            continue;
        };
        rows.push(type_json(ty, spec));
    }
    println!("{}", serde_json::to_string(&rows)?);
    Ok(())
}

fn type_json(ty: &str, spec: &TypeSpec) -> serde_json::Value {
    serde_json::json!({
        "v": 1,
        "type": ty,
        "fields": spec.fields,
        "optional": spec.optional,
    })
}

fn print_text(vocab_args: &VocabArgs, cfg: &Config) {
    if let Some(ty) = &vocab_args.r#type {
        if let Some(spec) = cfg.vocabulary.get(ty) {
            println!("{ty}: {}", format_fields(spec));
        } else {
            eprintln!("eventlog vocab: unknown type: {ty}");
        }
        return;
    }

    for ty in cfg.vocabulary.types() {
        let Some(spec) = cfg.vocabulary.get(ty) else {
            continue;
        };
        println!("{ty}: {}", format_fields(spec));
    }
}

fn format_fields(spec: &TypeSpec) -> String {
    let opt = if spec.optional.is_empty() {
        String::new()
    } else {
        format!(" optional=[{}]", spec.optional.join(", "))
    };
    format!("required=[{}]{opt}", spec.fields.join(", "))
}
