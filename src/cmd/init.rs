use crate::cli::{Args, Command};

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Init(_) = &args.command else {
        unreachable!("cmd::init::run dispatched for a non-Init command")
    };
    let repo_root = std::env::current_dir()?;
    crate::scaffold::init(&repo_root)?;
    Ok(0)
}
