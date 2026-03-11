pub mod add;
pub mod gateway;
pub mod icons;
pub mod list;
pub mod port;
pub mod proxy;
pub mod publish;
pub mod relay_cmd;
pub mod remove;
pub mod search;
pub mod start;
pub mod status;
pub mod stop;
pub mod sync;
pub mod tools;
pub mod uninstall;
pub mod update;
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

    /// Sync server configs to connected hosts
    #[command(alias = "signal")]
    Sync(sync::SyncArgs),

    /// Light the lighthouse (HTTP/SSE gateway)
    #[command(alias = "gateway")]
    Lighthouse(gateway::GatewayArgs),

    /// Scout the seas for new MCP servers
    #[command(alias = "search")]
    Scout(search::SearchArgs),

    /// Open the treasure chest (secret vault)
    #[command(alias = "vault")]
    Chest(vault::VaultArgs),

    /// Inspect or filter a ship's cargo manifest (tool filters)
    #[command(alias = "filter")]
    Cargo(tools::ToolsArgs),

    /// Manage port connections to hosts
    #[command(alias = "host")]
    Port(port::PortArgs),

    /// Scuttle the ship — uninstall Harbor
    #[command(alias = "uninstall")]
    Scuttle(uninstall::UninstallArgs),

    /// Update Harbor to the latest version
    Update(update::UpdateArgs),

    /// Broadcast your gateway to the high seas (publish to relay)
    #[command(alias = "broadcast")]
    Publish(publish::PublishArgs),

    /// Run the Harbor relay server (self-hosted)
    #[command(alias = "relay-server")]
    Relay(relay_cmd::RelayCmdArgs),

    /// Run as an MCP stdio proxy through the Harbor gateway
    #[command(alias = "proxy", hide = true)]
    Proxy(proxy::ProxyArgs),
}

pub async fn run(cli: Cli) -> Result<(), HarborError> {
    match cli.command {
        Commands::Dock(args) => add::run(args).await,
        Commands::Undock(args) => remove::run(args).await,
        Commands::Fleet(args) => list::run(args).await,
        Commands::Launch(args) => start::run(args).await,
        Commands::Anchor(args) => stop::run(args).await,
        Commands::Manifest(args) => status::run(args).await,
        Commands::Sync(args) => sync::run(args).await,
        Commands::Lighthouse(args) => gateway::run(args).await,
        Commands::Scout(args) => search::run(args).await,
        Commands::Chest(args) => vault::run(args).await,
        Commands::Cargo(args) => tools::run(args).await,
        Commands::Port(args) => port::run(args).await,
        Commands::Scuttle(args) => uninstall::run(args).await,
        Commands::Update(args) => update::run(args).await,
        Commands::Publish(args) => publish::run(args).await,
        Commands::Relay(args) => relay_cmd::run(args).await,
        Commands::Proxy(args) => proxy::run(args).await,
    }
}
