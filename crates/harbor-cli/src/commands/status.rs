use clap::Args;
use colored::Colorize;
use harbor_core::connector;
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct StatusArgs;

pub async fn run(_args: StatusArgs) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;

    println!("{}", "⚓ Harbor Manifest".bold());
    println!();

    if config.servers.is_empty() {
        println!("  The docks are empty. No ships in the fleet.");
    } else {
        println!("  {} Fleet:", "⛵".cyan());
        for (name, server) in &config.servers {
            let status = if server.enabled {
                "rigged".green()
            } else {
                "moored".red()
            };
            println!("    {} [{}]", name, status);
        }
    }

    println!();

    println!("  {} Ports:", "🏴".cyan());
    let connectors = connector::all_connectors();
    for conn in &connectors {
        let host_key = conn.host_name().to_lowercase().replace(' ', "");
        let connected = config
            .hosts
            .get(&host_key)
            .map(|h| h.connected)
            .unwrap_or(false);

        let config_exists = conn.config_exists();

        let status = if connected {
            "linked".green().to_string()
        } else if config_exists {
            "sighted".yellow().to_string()
        } else {
            "uncharted".red().to_string()
        };

        println!("    {} [{}]", conn.host_name(), status);
        if let Ok(path) = conn.config_path() {
            println!("      {}", path.display().to_string().dimmed());
        }
    }

    println!();
    println!(
        "  Charts: {}",
        HarborConfig::default_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
            .dimmed()
    );

    Ok(())
}
