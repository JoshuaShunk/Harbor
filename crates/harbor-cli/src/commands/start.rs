use clap::Args;
use colored::Colorize;
use harbor_core::server::ServerManager;
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct StartArgs {
    /// Name of the ship to launch (omit to launch the whole fleet)
    pub name: Option<String>,
}

pub async fn run(args: StartArgs) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;
    let mut manager = ServerManager::new();

    match args.name {
        Some(name) => {
            let server_config = config.get_server(&name)?;
            manager.start(&name, server_config).await?;
            println!(
                "{} Server '{}' launched",
                "ok:".green().bold(),
                name.cyan()
            );

            // Keep the process alive until Ctrl+C
            println!("Press Ctrl+C to drop anchor");
            tokio::signal::ctrl_c().await.ok();
            println!("\nDropping anchor...");
            manager.stop_all().await?;
        }
        None => {
            let auto_start: Vec<_> = config
                .servers
                .iter()
                .filter(|(_, s)| s.enabled && s.auto_start)
                .collect();

            if auto_start.is_empty() {
                println!("No servers configured with auto-start.");
                println!(
                    "  Use {} to launch a specific server",
                    "harbor launch <name>".yellow()
                );
                return Ok(());
            }

            for (name, server_config) in &auto_start {
                match manager.start(name, server_config).await {
                    Ok(()) => println!(
                        "{} Launched '{}'",
                        "ok:".green().bold(),
                        name.cyan()
                    ),
                    Err(e) => eprintln!(
                        "{} Failed to launch '{}': {}",
                        "err:".red().bold(),
                        name.cyan(),
                        e
                    ),
                }
            }

            println!("\nPress Ctrl+C to anchor all servers");
            tokio::signal::ctrl_c().await.ok();
            println!("\nDropping anchor on all servers...");
            manager.stop_all().await?;
        }
    }

    Ok(())
}
