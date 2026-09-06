use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "eventlog", version, about = "Append-only coordination log")]
pub struct Args {
    /// Named or explicit log to operate on.
    #[arg(long, global = true)]
    pub log: Option<String>,

    /// Emit machine-readable JSON rows instead of formatted text.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Validate and append one event.
    Append(AppendArgs),
    /// Show required and optional fields per event type.
    Vocab(VocabArgs),
    /// Walk the hash chain and report the first break.
    Verify(VerifyArgs),
    /// Print log rows, optionally following new ones.
    View(ViewArgs),
    /// Per-agent lifecycle table.
    Agents(AgentsArgs),
    /// Folded state as of a sequence number.
    State(StateArgs),
    /// Explain how one event was acted on.
    Why(WhyArgs),
    /// Report files a worker's claim does not cover.
    #[command(alias = "check-claims")]
    Claims(ClaimsArgs),
    /// Open an event's ref in $EDITOR or $PAGER.
    Open(OpenArgs),
    /// Interactive terminal UI over the log.
    Tui(TuiArgs),
    /// Reactor runtime.
    React(ReactArgs),
    /// Hook guard for agent tool calls.
    Guard(GuardArgs),
    /// Create a new coordination log and scaffold.
    Init(InitArgs),
    /// Diagnose common setup problems.
    Doctor(DoctorArgs),
    /// Toggle or report OS-level append-only protection.
    Protect(ProtectArgs),
    /// Print the JSON Schema for log types.
    Schema(SchemaArgs),
    /// Manage the embedded coordination skill.
    Skill(SkillArgs),
    /// Generate shell completions.
    Completions(CompletionsArgs),
}

#[derive(ClapArgs, Debug)]
#[command(after_help = crate::cmd::append::DEFAULT_AFTER_HELP)]
pub struct AppendArgs {
    /// Event type (lowercase slug).
    pub r#type: String,

    /// Fields as key=value pairs.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub fields: Vec<String>,

    /// Writer identity (default: controller, or $EVENTLOG_AS).
    #[arg(long = "as")]
    pub writer: Option<String>,

    /// Validate and print the line without writing.
    #[arg(long)]
    pub dry_run: bool,

    /// Skip strict fold checks (for repair work).
    #[arg(long)]
    pub no_strict: bool,
}

#[derive(ClapArgs, Debug)]
pub struct VocabArgs {
    /// Event type to describe (omit for all types).
    pub r#type: Option<String>,
}

#[derive(ClapArgs, Debug)]
pub struct VerifyArgs {}

#[derive(ClapArgs, Debug, Clone)]
pub struct ViewArgs {
    #[arg(short, long)]
    pub follow: bool,
    #[arg(long = "type")]
    pub types: Option<String>,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long)]
    pub by: Option<String>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub last: Option<usize>,
    #[arg(long)]
    pub grep: Option<String>,
    #[arg(long, value_enum, default_value_t = crate::cmd::view::ColorChoice::Auto)]
    pub color: crate::cmd::view::ColorChoice,
}

#[derive(ClapArgs, Debug)]
pub struct AgentsArgs {
    /// Fold state as of this sequence number (default: tip).
    #[arg(long)]
    pub at: Option<u64>,
}

#[derive(ClapArgs, Debug)]
pub struct StateArgs {
    /// Fold state as of this sequence number (default: tip).
    #[arg(long)]
    pub at: Option<u64>,
}

#[derive(ClapArgs, Debug)]
pub struct WhyArgs {
    /// Sequence number to explain.
    pub seq: u64,
}

#[derive(ClapArgs, Debug)]
pub struct ClaimsArgs {
    /// Agent whose live claims to check against changed files.
    pub agent: String,
    /// Base git ref for the diff.
    pub base: String,
    /// Head git ref (defaults to HEAD).
    pub head: Option<String>,
}

#[derive(ClapArgs, Debug)]
pub struct OpenArgs {
    /// Sequence number of the event whose ref to open.
    pub seq: u64,
    /// Open in $PAGER (default less) instead of $EDITOR.
    #[arg(long)]
    pub pager: bool,
}

#[derive(ClapArgs, Debug)]
pub struct TuiArgs {}

#[derive(ClapArgs, Debug)]
pub struct ReactArgs {
    #[command(subcommand)]
    pub inner: Option<ReactInner>,
}

#[derive(Subcommand, Debug)]
pub enum ReactInner {
    /// Dry-run one reaction against a real sequence number.
    Test,
}

#[derive(ClapArgs, Debug)]
pub struct GuardArgs {
    #[command(subcommand)]
    pub inner: Option<GuardInner>,
}

#[derive(Subcommand, Debug)]
pub enum GuardInner {
    /// Install the guard hook for an agent.
    Install,
}

#[derive(ClapArgs, Debug)]
pub struct InitArgs {}

#[derive(ClapArgs, Debug)]
pub struct DoctorArgs {}

#[derive(ClapArgs, Debug)]
pub struct ProtectArgs {}

#[derive(ClapArgs, Debug)]
pub struct SchemaArgs {
    /// JSON Schema for a raw event line (default).
    #[arg(long, conflicts_with = "output")]
    pub events: bool,

    /// JSON Schema for a `--json` view row (event fields plus `v`).
    #[arg(long, conflicts_with = "events")]
    pub output: bool,
}

#[derive(ClapArgs, Debug)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub inner: Option<SkillInner>,
}

#[derive(Subcommand, Debug)]
pub enum SkillInner {
    /// Write the embedded skill to disk.
    Install,
}

#[derive(ClapArgs, Debug)]
pub struct CompletionsArgs {}

pub fn run() -> i32 {
    let args = Args::parse();
    let result = match &args.command {
        Command::Append(_) => crate::cmd::append::run(&args),
        Command::Vocab(_) => crate::cmd::vocab::run(&args),
        Command::Verify(_) => crate::cmd::verify::run(&args),
        Command::View(_) => crate::cmd::view::run(&args),
        Command::Agents(_) => crate::cmd::agents::run(&args),
        Command::State(_) => crate::cmd::state::run(&args),
        Command::Why(_) => crate::cmd::why::run(&args),
        Command::Claims(_) => crate::cmd::claims::run(&args),
        Command::Open(_) => crate::cmd::open::run(&args),
        Command::Tui(_) => crate::cmd::tui::run(&args),
        Command::React(_) => crate::cmd::react::run(&args),
        Command::Guard(_) => crate::cmd::guard::run(&args),
        Command::Init(_) => crate::cmd::init::run(&args),
        Command::Doctor(_) => crate::cmd::doctor::run(&args),
        Command::Protect(_) => crate::cmd::protect::run(&args),
        Command::Schema(_) => crate::cmd::schema::run(&args),
        Command::Skill(_) => crate::cmd::skill::run(&args),
        Command::Completions(_) => crate::cmd::completions::run(&args),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("eventlog: {err}");
            1
        }
    }
}
