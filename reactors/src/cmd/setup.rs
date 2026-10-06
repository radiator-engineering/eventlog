use anyhow::Context;

use crate::cli::{Args, Command, SetupInner};

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Setup(setup) = &args.command else {
        anyhow::bail!("setup::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    let changes = crate::setup::setup_changes(&root)?;
    let rel = |path: &std::path::Path| {
        path.strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    match setup.inner {
        SetupInner::Preview => {
            if changes.is_empty() {
                println!("setup: no changes");
            } else {
                for change in &changes {
                    println!("create {}", rel(&change.path));
                }
                println!("ensure the log (eventlog setup) and ignore .context/*.reactor.lock/");
                print_example();
            }
        }
        SetupInner::Apply | SetupInner::Upgrade => {
            // Validation above happened before this non-overwriting bootstrap,
            // so a customized owned config can never leave a partial setup.
            eventlog::scaffold::init(&root)?;
            crate::setup::apply_setup(&root, &changes)?;
            if changes.is_empty() {
                println!("setup: no changes");
            } else {
                for change in &changes {
                    println!("wrote {}", rel(&change.path));
                }
                print_example();
            }
        }
    }
    Ok(0)
}

fn print_example() {
    println!(
        "Drove hook argv example: [\"eventlog\", \"lifecycle\", \"start\", agent, \"--role\", \"reactor\", \"--model\", model, \"--paths\", paths]"
    );
    println!(
        "Reactor argv example: [\"eventlog-reactors\", \"react\", \"--as\", \"committer\", \"--on\", \"result\", \"--git\", \"--\", \"eventlog-reactors\", \"action\", \"commit\"]"
    );
}
