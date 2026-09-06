use crate::cli::{Args, Command};
use clap::CommandFactory;
use clap_complete::generate;

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Completions(completions_args) = &args.command else {
        unreachable!("cmd::completions::run dispatched for a non-Completions command")
    };

    let mut cmd = Args::command();
    let name = cmd.get_name().to_string();
    generate(
        completions_args.shell,
        &mut cmd,
        name,
        &mut std::io::stdout(),
    );
    Ok(0)
}
