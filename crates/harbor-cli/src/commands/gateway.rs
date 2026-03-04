use clap::Args;
use colored::Colorize;
use harbor_core::gateway::Gateway;
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct GatewayArgs {
    /// Port to shine the light from (overrides config)
    #[arg(short, long)]
    pub port: Option<u16>,
}

pub async fn run(args: GatewayArgs) -> Result<(), HarborError> {
    let mut config = HarborConfig::load()?;

    if config.servers.is_empty() {
        println!(
            "{} No ships in the fleet. Dock some first with {}",
            "warn:".yellow().bold(),
            "harbor dock".yellow()
        );
        return Ok(());
    }

    if let Some(port) = args.port {
        config.harbor.gateway_port = port;
    }

    let enabled_count = config.servers.values().filter(|s| s.enabled).count();
    println!(
        "{} Lighting the lighthouse with {} ship(s) in the fleet...",
        "info:".blue().bold(),
        enabled_count.to_string().cyan()
    );
    println!();

    let gateway = Gateway::new(config);

    // Wire ctrl_c to the shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = shutdown_tx.send(());
    });

    gateway.run(shutdown_rx).await?;

    Ok(())
}
