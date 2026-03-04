pub mod add;
pub mod gateway;
pub mod list;
pub mod remove;
pub mod search;
pub mod start;
pub mod status;
pub mod stop;
pub mod sync;
pub mod vault;

use clap::{Parser, Subcommand};
use harbor_core::HarborError;

#[derive(Parser)]
#[command(
    name = "harbor",
    about = "⚓ Harbor — your fleet commander for MCP servers",
    version,
    after_help = "Set sail with `harbor dock` to bring your first server into port."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Dock a new MCP server into the harbor
    #[command(alias = "add")]
    Dock(add::AddArgs),

    /// Undock an MCP server and cast it off
    #[command(alias = "remove")]
    Undock(remove::RemoveArgs),

    /// Review your fleet of docked servers
    #[command(alias = "list")]
    Fleet(list::ListArgs),

    /// Launch a server out to sea
    #[command(alias = "start")]
    Launch(start::StartArgs),

    /// Drop anchor on a running server (requires daemon mode — coming soon)
    #[command(alias = "stop", hide = true)]
    Anchor(stop::StopArgs),

    /// Read the harbor manifest
    #[command(alias = "status")]
    Manifest(status::StatusArgs),

    /// Signal connected ports to update their charts
    #[command(alias = "sync")]
    Signal(sync::SyncArgs),

    /// Light the lighthouse (HTTP/SSE gateway)
    #[command(alias = "gateway")]
    Lighthouse(gateway::GatewayArgs),

    /// Scout the seas for new MCP servers
    #[command(alias = "search")]
    Scout(search::SearchArgs),

    /// Open the treasure chest (secret vault)
    #[command(alias = "vault")]
    Chest(vault::VaultArgs),
}

pub async fn run(cli: Cli) -> Result<(), HarborError> {
    match cli.command {
        Commands::Dock(args) => add::run(args).await,
        Commands::Undock(args) => remove::run(args).await,
        Commands::Fleet(args) => list::run(args).await,
        Commands::Launch(args) => start::run(args).await,
        Commands::Anchor(args) => stop::run(args).await,
        Commands::Manifest(args) => status::run(args).await,
        Commands::Signal(args) => sync::run(args).await,
        Commands::Lighthouse(args) => gateway::run(args).await,
        Commands::Scout(args) => search::run(args).await,
        Commands::Chest(args) => vault::run(args).await,
    }
}
