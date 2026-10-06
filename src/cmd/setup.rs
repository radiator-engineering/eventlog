use anyhow::Context;

use crate::cli::{Args, Command, SetupInner};

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Setup(setup) = &args.command else {
        anyhow::bail!("setup::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    let plan = crate::scaffold::setup_plan(&root);
    let applying = matches!(setup.inner, SetupInner::Apply | SetupInner::Upgrade);
    if applying {
        // init never overwrites a file, so an edited template survives.
        crate::scaffold::init(&root)?;
    }
    if plan.is_empty() {
        println!("setup: no changes");
    }
    for line in &plan {
        if applying {
            // "create x" becomes "created x", and likewise for each verb.
            let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
            println!("{}d {rest}", verb.trim_end_matches('e'));
        } else {
            println!("{line}");
        }
    }
    Ok(0)
}
