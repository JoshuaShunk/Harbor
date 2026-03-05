use clap::{Args, Subcommand};
use colored::Colorize;
use harbor_core::connector;
use harbor_core::sync::sync_all_hosts;
use harbor_core::{HarborConfig, HarborError, HostConfig};

#[derive(Args)]
pub struct PortArgs {
    #[command(subcommand)]
    pub action: Option<PortAction>,
}

#[derive(Subcommand)]
pub enum PortAction {
    /// Link a host — start syncing servers to it
    Link(HostArg),
    /// Unlink a host — stop syncing servers to it
    Unlink(HostArg),
}

#[derive(Args)]
pub struct HostArg {
    /// Host name (claude, claude-desktop, codex, vscode, cursor)
    pub host: String,
}

const VALID_HOSTS: &[&str] = &["claude", "claude-desktop", "codex", "vscode", "cursor"];

pub async fn run(args: PortArgs) -> Result<(), HarborError> {
    match args.action {
        None => list_hosts().await,
        Some(PortAction::Link(host_arg)) => link_host(&host_arg.host).await,
        Some(PortAction::Unlink(host_arg)) => unlink_host(&host_arg.host).await,
    }
}

async fn list_hosts() -> Result<(), HarborError> {
    let config = HarborConfig::load()?;

    println!("{}", "⚓ Ports".bold());
    println!();

    let connectors = connector::all_connectors();
    for conn in &connectors {
        let host_key = normalize_host_key(conn.host_name());
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

        println!("  {} [{}]", conn.host_name(), status);
        if let Ok(path) = conn.config_path() {
            println!("    {}", path.display().to_string().dimmed());
        }
    }

    Ok(())
}

async fn link_host(host: &str) -> Result<(), HarborError> {
    validate_host(host)?;

    let mut config = HarborConfig::load()?;
    let entry = config
        .hosts
        .entry(host.to_string())
        .or_insert_with(|| HostConfig {
            connected: false,
            scope: None,
        });
    entry.connected = true;
    config.save()?;

    let conn = connector::get_connector(host)?;
    println!(
        "{} Linked {}",
        "ok:".green().bold(),
        conn.host_name().cyan()
    );

    // Auto-sync
    let results = sync_all_hosts(&config);
    for (_, result) in &results {
        if let Ok(r) = result {
            println!("  {} Synced to {}", "=>".dimmed(), r.display_name.cyan());
        }
    }

    Ok(())
}

async fn unlink_host(host: &str) -> Result<(), HarborError> {
    validate_host(host)?;

    let mut config = HarborConfig::load()?;
    if let Some(entry) = config.hosts.get_mut(host) {
        entry.connected = false;
    }
    config.save()?;

    // Remove harbor-proxy entry from the host's config
    let conn = connector::get_connector(host)?;
    let _ = conn.remove_servers(&["harbor-proxy".to_string()]);

    println!(
        "{} Unlinked {}",
        "ok:".green().bold(),
        conn.host_name().cyan()
    );

    Ok(())
}

fn validate_host(host: &str) -> Result<(), HarborError> {
    if !VALID_HOSTS.contains(&host) {
        return Err(HarborError::ConnectorError {
            host: host.to_string(),
            reason: format!(
                "Unknown host '{}'. Valid hosts: {}",
                host,
                VALID_HOSTS.join(", ")
            ),
        });
    }
    Ok(())
}

fn normalize_host_key(display_name: &str) -> String {
    match display_name {
        "Claude Code" => "claude".to_string(),
        "Claude Desktop" => "claude-desktop".to_string(),
        "Codex" => "codex".to_string(),
        "VS Code" => "vscode".to_string(),
        "Cursor" => "cursor".to_string(),
        other => other.to_lowercase().replace(' ', ""),
    }
}
