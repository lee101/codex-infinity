use clap::Args;
use clap::Parser;
use codex_common::CliConfigOverrides;

#[derive(Parser, Debug, Default)]
#[command(version)]
pub struct Cli {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Launch a new codex agent in the cloud
    Launch(LaunchCommand),
    /// List running agents
    List(ListCommand),
    /// Get agent status
    Status(StatusCommand),
    /// Stream agent logs
    Logs(LogsCommand),
    /// SSH into an agent
    Attach(AttachCommand),
    /// Stop and delete an agent
    Cancel(CancelCommand),
}

#[derive(Debug, Args)]
pub struct LaunchCommand {
    /// Repository URL to clone
    #[arg(long = "repo")]
    pub repo: Option<String>,

    /// Agent name
    #[arg(long = "name")]
    pub name: Option<String>,

    /// Server type (cx22, cx32, cx52, gx11, etc.)
    #[arg(long = "server-type", default_value = "cx22")]
    pub server_type: String,

    /// Datacenter location (nbg1, fsn1, hel1)
    #[arg(long = "location", default_value = "nbg1")]
    pub location: String,

    /// Enable GPU
    #[arg(long = "gpu")]
    pub with_gpu: bool,

    /// Custom setup script
    #[arg(long = "setup")]
    pub setup_script: Option<String>,

    /// Enable auto-next-steps mode
    #[arg(long = "auto-next-steps")]
    pub auto_next_steps: bool,

    /// Enable auto-next-idea mode
    #[arg(long = "auto-next-idea")]
    pub auto_next_idea: bool,

    /// Pack size (concurrent agents)
    #[arg(long = "pack-size", default_value = "1")]
    pub pack_size: usize,

    /// Emit JSON output
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ListCommand {
    /// Emit JSON instead of plain text
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Agent identifier
    #[arg(value_name = "AGENT_ID")]
    pub agent_id: i64,

    /// Emit JSON output
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct LogsCommand {
    /// Agent identifier
    #[arg(value_name = "AGENT_ID")]
    pub agent_id: i64,

    /// Follow logs (not yet implemented)
    #[arg(long = "follow", short = 'f', default_value_t = false)]
    pub follow: bool,
}

#[derive(Debug, Args)]
pub struct AttachCommand {
    /// Agent identifier
    #[arg(value_name = "AGENT_ID")]
    pub agent_id: i64,
}

#[derive(Debug, Args)]
pub struct CancelCommand {
    /// Agent identifier
    #[arg(value_name = "AGENT_ID")]
    pub agent_id: i64,

    /// Skip confirmation prompt
    #[arg(long = "force", short = 'f', default_value_t = false)]
    pub force: bool,
}
