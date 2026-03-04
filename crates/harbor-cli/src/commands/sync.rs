use clap::Args;
use colored::Colorize;
use harbor_core::connector;
use harbor_core::sync::sync_all_hosts;
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct SyncArgs {
    /// Preview what would be synced without writing changes
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(args: SyncArgs) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;

    let hosts_to_sync: Vec<String> = config
        .hosts
        .iter()
        .filter(|(_, h)| h.connected)
        .map(|(name, _)| name.clone())
        .collect();

    if hosts_to_sync.is_empty() {
        println!("No linked hosts to sync.");
        println!(
            "  Link a host first with {}",
            "harbor port link <host>".yellow()
        );
        return Ok(());
    }

    if args.dry_run {
        for host_name in &hosts_to_sync {
            preview_sync(&config, host_name)?;
        }
        println!();
        println!(
            "{} Dry run complete. No changes were written.",
            "info:".blue().bold()
        );
    } else {
        let results = sync_all_hosts(&config);
        for (host_name, result) in results {
            match result {
                Ok(r) => {
                    println!(
                        "{} Synced {} server(s) to {}",
                        "ok:".green().bold(),
                        r.server_count,
                        r.display_name.cyan()
                    );
                }
                Err(e) => {
                    println!(
                        "{} Failed to sync to {}: {}",
                        "err:".red().bold(),
                        host_name.cyan(),
                        e
                    );
                }
            }
        }
    }

    Ok(())
}

fn preview_sync(config: &HarborConfig, host_name: &str) -> Result<(), HarborError> {
    let conn = connector::get_connector(host_name)?;
    let servers = config.servers_for_host(host_name);

    if servers.is_empty() {
        println!(
            "{} No servers for {}",
            "skip:".yellow().bold(),
            conn.host_name().cyan()
        );
        return Ok(());
    }

    let config_path = conn.config_path()?;

    println!(
        "{} Would sync harbor-proxy to {} ({})",
        "dry:".blue().bold(),
        conn.host_name().cyan(),
        config_path.display()
    );
    println!(
        "    harbor-proxy ({} server(s) behind gateway)",
        servers.len()
    );

    Ok(())
}
