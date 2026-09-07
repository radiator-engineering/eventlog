use anyhow::Context;

use crate::cli::{Args, Command, SetupInner};

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Setup(setup) = &args.command else {
        anyhow::bail!("setup::run called with wrong subcommand");
    };
    let root = std::env::current_dir().context("current directory")?;
    let changes = crate::scaffold::setup_changes(&root)?;
    match setup.inner {
        SetupInner::Preview => {
            if changes.is_empty() {
                println!("setup: no changes");
            } else {
                for change in &changes {
                    println!(
                        "create {}",
                        change
                            .path
                            .strip_prefix(&root)
                            .unwrap_or(&change.path)
                            .display()
                    );
                }
                println!("ensure .context/events.jsonl and non-overwriting init assets");
                print_example();
            }
        }
        SetupInner::Apply | SetupInner::Upgrade => {
            // Validation above happened before this non-overwriting bootstrap,
            // so a customized owned config can never leave a partial setup.
            crate::scaffold::init(&root)?;
            crate::scaffold::apply_setup(&changes)?;
            if changes.is_empty() {
                println!("setup: no changes");
            } else {
                for change in &changes {
                    println!(
                        "wrote {}",
                        change
                            .path
                            .strip_prefix(&root)
                            .unwrap_or(&change.path)
                            .display()
                    );
                }
                print_example();
            }
        }
    }
    Ok(0)
}

fn print_example() {
    println!(
        "Drove hook argv example: [\"eventlog\", \"lifecycle\", \"start\", agent, \"--model\", model, \"--paths\", paths]"
    );
    println!(
        "Reactor argv example: [\"eventlog\", \"react\", \"--as\", \"committer\", \"--on\", \"result\", \"--git\", \"--\", \"eventlog\", \"action\", \"commit\"]"
    );
}
