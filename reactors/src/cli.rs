use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "eventlog-reactors",
    version,
    about = "Optional reactors for an eventlog coordination log"
)]
pub struct Args {
    /// Named or explicit log to operate on.
    #[arg(long, global = true)]
    pub log: Option<String>,

    /// Emit machine-readable JSON instead of formatted text.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Reactor runtime.
    React(Box<ReactArgs>),
    /// Run a packaged reactor action.
    Action(ActionArgs),
    /// Preview or apply the reactor policy and its Drove helper.
    Setup(SetupArgs),
    /// Check reactor locks and the generated helper.
    Doctor(DoctorArgs),
}

#[derive(ClapArgs, Debug)]
pub struct ReactArgs {
    #[command(subcommand)]
    pub inner: Option<ReactInner>,

    #[command(flatten)]
    pub opts: ReactOpts,
}

#[derive(Subcommand, Debug)]
pub enum ReactInner {
    /// Dry-run one reaction against a real sequence number.
    Test(ReactTestArgs),
}

#[derive(ClapArgs, Debug)]
pub struct ReactTestArgs {
    /// The sequence number to react to.
    pub seq: u64,

    #[command(flatten)]
    pub opts: ReactOpts,
}

/// Options shared by the live loop and its dry run.
#[derive(ClapArgs, Clone, Debug)]
pub struct ReactOpts {
    /// Reactor name: the `by=` on every line it writes.
    #[arg(long = "as")]
    pub name: Option<String>,

    /// Event types to react to, comma-separated.
    #[arg(long, value_delimiter = ',')]
    pub on: Vec<String>,

    /// Extra `k=v` condition the driving event must meet (repeatable).
    #[arg(long)]
    pub filter: Vec<String>,

    /// How long to wait for a veto after declaring intent (for example `2s`).
    #[arg(long, default_value = "0s")]
    pub window: String,

    /// Snapshot git before and after, and report writes outside the claim.
    #[arg(long)]
    pub git: bool,

    /// How long the action command may run (for example `300s`).
    #[arg(long, default_value = "600s")]
    pub timeout: String,

    /// The action command, after `--`.
    #[arg(last = true)]
    pub command: Vec<String>,
}

#[derive(ClapArgs, Debug)]
pub struct ActionArgs {
    #[command(subcommand)]
    pub inner: ActionInner,
}

#[derive(Subcommand, Debug)]
pub enum ActionInner {
    /// Commit exactly EVENTLOG_PATHS without consuming unrelated staging.
    Commit {
        /// Direct-mode message; also passed as EVENTLOG_COMMIT_MESSAGE to a configured command.
        #[arg(long, default_value = "chore(eventlog): apply reactor result")]
        message: String,
    },
    /// Run the configured documentation command and report changed doc paths.
    Docs,
}

#[derive(ClapArgs, Debug)]
pub struct SetupArgs {
    #[command(subcommand)]
    pub inner: SetupInner,
}

#[derive(Subcommand, Debug)]
pub enum SetupInner {
    /// Show the exact reactor configuration setup would create or upgrade.
    Preview,
    /// Create the missing reactor configuration.
    Apply,
    /// Validate and preserve the reactor policy before upgrading its helper.
    Upgrade,
}

#[derive(ClapArgs, Debug)]
pub struct DoctorArgs {}

pub fn run() -> i32 {
    let args = Args::parse();
    let result = match &args.command {
        Command::React(_) => crate::cmd::react::run(&args),
        Command::Action(_) => crate::cmd::action::run(&args),
        Command::Setup(_) => crate::cmd::setup::run(&args),
        Command::Doctor(_) => crate::cmd::doctor::run(&args),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("eventlog-reactors: {err}");
            1
        }
    }
}
