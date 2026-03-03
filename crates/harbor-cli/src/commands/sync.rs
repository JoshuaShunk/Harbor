use clap::Args;
use colored::Colorize;
use harbor_core::connector::{self, resolve_env_for_host, HostServerEntry};
use harbor_core::{HarborConfig, HarborError};
use std::collections::BTreeMap;

#[derive(Args)]
pub struct SyncArgs {
    /// Signal a specific port only (claude, codex, vscode, cursor)
    #[arg(long)]
    pub host: Option<String>,

    /// Hoist the flags but don't fire — preview without changes
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(args: SyncArgs) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;

    let hosts_to_sync: Vec<String> = if let Some(ref host) = args.host {
        vec![host.clone()]
    } else {
        // Sync to all connected hosts
        config
            .hosts
            .iter()
            .filter(|(_, h)| h.connected)
            .map(|(name, _)| name.clone())
            .collect()
    };

    if hosts_to_sync.is_empty() {
        println!("No ports to signal.");
        println!(
            "  Edit {} to connect ports, or use {}",
            "~/.harbor/config.toml".yellow(),
            "harbor signal --host <name>".yellow()
        );
        return Ok(());
    }

    for host_name in &hosts_to_sync {
        sync_to_host(&config, host_name, args.dry_run)?;
    }

    if args.dry_run {
        println!();
        println!(
            "{} Dry run complete. No signals were sent.",
            "info:".blue().bold()
        );
    }

    Ok(())
}

fn sync_to_host(config: &HarborConfig, host_name: &str, dry_run: bool) -> Result<(), HarborError> {
    let conn = connector::get_connector(host_name)?;

    // Get servers enabled for this host
    let servers = config.servers_for_host(host_name);

    if servers.is_empty() {
        println!(
            "{} No ships bound for {}",
            "skip:".yellow().bold(),
            conn.host_name().cyan()
        );
        return Ok(());
    }

    // Refresh Google Drive credential files if any gdrive server is present
    for (_name, server_config) in &servers {
        if server_config.env.values().any(|v| v.contains("GDRIVE_CREDENTIALS_PATH") || v.contains("gdrive"))
            || server_config.args.iter().any(|a| a.contains("gdrive"))
        {
            let _ = harbor_core::auth::oauth::write_gdrive_credentials();
            break;
        }
    }

    // Build entries with resolved env vars
    let entries: BTreeMap<String, HostServerEntry> = servers
        .iter()
        .map(|(name, server_config)| {
            let resolved_env = resolve_env_for_host(&server_config.env);
            (
                (*name).clone(),
                HostServerEntry {
                    command: server_config.command.clone(),
                    args: server_config.args.clone(),
                    env: resolved_env,
                },
            )
        })
        .collect();

    let config_path = conn.config_path()?;

    if dry_run {
        println!(
            "{} Would signal {} ship(s) to {} ({})",
            "dry:".blue().bold(),
            entries.len(),
            conn.host_name().cyan(),
            config_path.display()
        );
        for name in entries.keys() {
            println!("    {}", name);
        }
    } else {
        conn.write_servers(&entries)?;
        println!(
            "{} Signaled {} ship(s) to {} ({})",
            "ok:".green().bold(),
            entries.len(),
            conn.host_name().cyan(),
            config_path.display()
        );
    }

    Ok(())
}
