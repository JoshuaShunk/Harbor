use clap::Args;
use colored::Colorize;
use harbor_core::gateway::{Gateway, RequestLogger};
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct GatewayArgs {
    /// Port to shine the light from (overrides config)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Host/IP to bind to (default: 127.0.0.1, use 0.0.0.0 to expose)
    #[arg(long)]
    pub host: Option<String>,

    /// Expose to network (shorthand for --host 0.0.0.0)
    #[arg(long)]
    pub expose: bool,

    /// Bearer token required for remote access (overrides config)
    #[arg(long)]
    pub token: Option<String>,
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

    if args.expose {
        config.harbor.gateway_host = "0.0.0.0".to_string();
    } else if let Some(host) = args.host {
        config.harbor.gateway_host = host;
    }

    if let Some(token) = args.token {
        config.harbor.gateway_token = Some(token);
    }

    let enabled_count = config.servers.values().filter(|s| s.enabled).count();
    println!(
        "{} Lighting the lighthouse with {} ship(s) in the fleet...",
        "info:".blue().bold(),
        enabled_count.to_string().cyan()
    );

    if config.harbor.gateway_host == "0.0.0.0" {
        if config.harbor.gateway_token.is_some() {
            println!(
                "{} Exposed to network (bearer token required)",
                "info:".blue().bold()
            );
        } else {
            println!(
                "{} Exposed to network WITHOUT authentication",
                "warn:".yellow().bold()
            );
        }
    }

    println!();

    let gateway = Gateway::new(config, std::sync::Arc::new(RequestLogger::new()));

    // Wire ctrl_c to the shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = shutdown_tx.send(());
    });

    gateway.run(shutdown_rx).await?;

    Ok(())
}
