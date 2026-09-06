use crate::cli::{Args, Command, SkillInner};

pub fn run(args: &Args) -> anyhow::Result<i32> {
    let Command::Skill(skill_args) = &args.command else {
        unreachable!("cmd::skill::run dispatched for a non-Skill command")
    };

    match &skill_args.inner {
        Some(SkillInner::Install { dir, force }) => {
            let dest_root = dir.clone().unwrap_or_else(crate::skill::default_dir);
            match crate::skill::install(&dest_root, *force) {
                Ok(dest) => {
                    println!("installed skill to {}", dest.display());
                    Ok(0)
                }
                Err(err) => {
                    eprintln!("eventlog skill install: {err}");
                    Ok(1)
                }
            }
        }
        None => {
            eprintln!("eventlog skill: run `eventlog skill install`");
            Ok(1)
        }
    }
}
