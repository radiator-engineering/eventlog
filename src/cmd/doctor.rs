use crate::cli::{Args, Command, DoctorArgs};
use crate::model::config;
use crate::scaffold::doctor::{self, Options};

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Doctor(doctor_args) = &args.command else {
        unreachable!("cmd::doctor::run dispatched for a non-Doctor command")
    };
    let repo_root = std::env::current_dir()?;
    let cfg = config::load(&repo_root)?;
    doctor::run(&repo_root, &cfg, &options(doctor_args))
}

fn options(args: &DoctorArgs) -> Options {
    Options {
        fix: args.fix,
        protect: args.protect,
    }
}
