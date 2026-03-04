use clap::Args;
use colored::Colorize;
use harbor_core::connector::{self, Connector};
use harbor_core::{HarborConfig, HarborError, Vault};
use std::io::{self, Write};

#[derive(Args)]
pub struct UninstallArgs {
    /// Scuttle everything — remove config directory and vault secrets too
    #[arg(long)]
    pub purge: bool,

    /// Hoist the flags but don't fire — preview what would be removed
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

pub async fn run(args: UninstallArgs) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;
    let harbor_server_names: Vec<String> = config.servers.keys().cloned().collect();

    // Collect what we'll do
    let mut actions: Vec<String> = Vec::new();

    // 1. Harbor-managed entries in host configs
    let hosts = ["claude", "codex", "vscode", "cursor"];
    for host_name in &hosts {
        if let Ok(conn) = connector::get_connector(host_name) {
            if conn.config_exists() {
                if let Ok(existing) = conn.read_servers() {
                    let to_remove: Vec<&String> = harbor_server_names
                        .iter()
                        .filter(|name| existing.contains_key(*name))
                        .collect();
                    if !to_remove.is_empty() {
                        actions.push(format!(
                            "Remove {} Harbor-managed server(s) from {} ({})",
                            to_remove.len(),
                            conn.host_name(),
                            conn.config_path()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default()
                        ));
                    }
                }
            }
        }
    }

    // 2. macOS: Harbor.app
    #[cfg(target_os = "macos")]
    let app_path = std::path::Path::new("/Applications/Harbor.app");
    #[cfg(not(target_os = "macos"))]
    let app_path = std::path::Path::new("/nonexistent");

    if app_path.exists() {
        actions.push("Remove /Applications/Harbor.app".to_string());
    }

    // 3. CLI symlink or binary
    let cli_path = which_harbor();
    if let Some(ref path) = cli_path {
        let is_symlink = std::fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if is_symlink {
            actions.push(format!("Remove CLI symlink at {}", path));
        } else {
            actions.push(format!("Remove CLI binary at {}", path));
        }
    }

    // 4. Purge: config directory + vault
    if args.purge {
        if let Ok(config_dir) = HarborConfig::default_dir() {
            if config_dir.exists() {
                actions.push(format!(
                    "Remove config directory ({})",
                    config_dir.display()
                ));
            }
        }

        if let Ok(keys) = Vault::list_keys() {
            if !keys.is_empty() {
                actions.push(format!(
                    "Remove {} vault secret(s) from OS keychain",
                    keys.len()
                ));
            }
        }
    }

    if actions.is_empty() {
        println!(
            "{} Nothing to remove — Harbor doesn't appear to be installed.",
            "info:".blue().bold()
        );
        return Ok(());
    }

    // Show what will be done
    println!();
    if args.dry_run {
        println!(
            "{} The following would be removed:",
            "dry:".blue().bold()
        );
    } else {
        println!(
            "{} The following will be removed:",
            "warn:".yellow().bold()
        );
    }
    println!();
    for action in &actions {
        println!("  - {}", action);
    }

    if !args.purge {
        println!();
        println!(
            "  {} Config directory (~/.harbor/) will be kept.",
            "note:".dimmed()
        );
        println!(
            "  {} Use {} to remove everything.",
            "note:".dimmed(),
            "--purge".yellow()
        );
    }

    if args.dry_run {
        println!();
        println!(
            "{} Dry run complete. Nothing was removed.",
            "info:".blue().bold()
        );
        return Ok(());
    }

    // Confirm
    if !args.yes {
        println!();
        print!("Continue? [y/N] ");
        io::stdout().flush().map_err(HarborError::Io)?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(HarborError::Io)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!();

    // Execute: remove Harbor-managed entries from host configs
    for host_name in &hosts {
        if let Ok(conn) = connector::get_connector(host_name) {
            if conn.config_exists() {
                if let Ok(existing) = conn.read_servers() {
                    let to_remove: Vec<String> = harbor_server_names
                        .iter()
                        .filter(|name| existing.contains_key(*name))
                        .cloned()
                        .collect();
                    if !to_remove.is_empty() {
                        match remove_servers_from_host(&*conn, &to_remove) {
                            Ok(()) => println!(
                                "{} Removed {} server(s) from {}",
                                "ok:".green().bold(),
                                to_remove.len(),
                                conn.host_name().cyan()
                            ),
                            Err(e) => println!(
                                "{} Failed to clean {}: {}",
                                "err:".red().bold(),
                                conn.host_name(),
                                e
                            ),
                        }
                    }
                }
            }
        }
    }

    // Execute: stop Harbor process (macOS)
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let _ = Command::new("pkill").args(["-x", "Harbor"]).status();
    }

    // Execute: remove Harbor.app
    if app_path.exists() {
        match std::fs::remove_dir_all(app_path) {
            Ok(()) => println!(
                "{} Removed /Applications/Harbor.app",
                "ok:".green().bold()
            ),
            Err(e) => println!(
                "{} Failed to remove Harbor.app: {} (try with sudo)",
                "err:".red().bold(),
                e
            ),
        }
    }

    // Execute: purge config + vault
    if args.purge {
        if let Ok(config_dir) = HarborConfig::default_dir() {
            if config_dir.exists() {
                match std::fs::remove_dir_all(&config_dir) {
                    Ok(()) => println!(
                        "{} Removed {}",
                        "ok:".green().bold(),
                        config_dir.display()
                    ),
                    Err(e) => println!(
                        "{} Failed to remove {}: {}",
                        "err:".red().bold(),
                        config_dir.display(),
                        e
                    ),
                }
            }
        }

        if let Ok(keys) = Vault::list_keys() {
            let count = keys.len();
            for key in &keys {
                let _ = Vault::delete(key);
            }
            if count > 0 {
                println!(
                    "{} Removed {} vault secret(s) from keychain",
                    "ok:".green().bold(),
                    count
                );
            }
        }
    }

    // Execute: remove CLI binary/symlink (do this last since we're running from it)
    if let Some(ref path) = cli_path {
        match std::fs::remove_file(path) {
            Ok(()) => println!("{} Removed {}", "ok:".green().bold(), path),
            Err(e) => println!(
                "{} Failed to remove {}: {} (try with sudo)",
                "err:".red().bold(),
                path,
                e
            ),
        }
    }

    println!();
    println!("{} Harbor has been uninstalled.", "ok:".green().bold());

    if !args.purge {
        println!(
            "  Config at {} was preserved. Remove it manually if desired.",
            "~/.harbor/".dimmed()
        );
    }

    Ok(())
}

/// Find the harbor binary path in PATH
fn which_harbor() -> Option<String> {
    std::process::Command::new("which")
        .arg("harbor")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

/// Remove specific server entries from a host's config file.
/// Reads the config, removes the named servers, writes it back.
fn remove_servers_from_host(
    conn: &dyn Connector,
    server_names: &[String],
) -> Result<(), HarborError> {
    let path = conn.config_path()?;
    if !path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;

    // Parse as generic JSON, remove the keys, write back
    let mut doc: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
            host: conn.host_name().to_string(),
            reason: format!("Failed to parse {}: {}", path.display(), e),
        })?;

    // Determine the servers key for this host type
    let servers_key = match conn.host_name() {
        "Claude Code" => "mcpServers",
        "Cursor" => "mcpServers",
        "VS Code" => "servers",
        "Codex" => return remove_servers_from_codex_toml(&path, server_names),
        _ => return Ok(()),
    };

    if let Some(servers_obj) = doc.get_mut(servers_key).and_then(|v| v.as_object_mut()) {
        for name in server_names {
            servers_obj.remove(name);
        }
    }

    let content = serde_json::to_string_pretty(&doc)?;
    std::fs::write(&path, content).map_err(HarborError::Io)?;

    Ok(())
}

/// Remove server entries from Codex's TOML config
fn remove_servers_from_codex_toml(
    path: &std::path::Path,
    server_names: &[String],
) -> Result<(), HarborError> {
    let content = std::fs::read_to_string(path).map_err(HarborError::Io)?;
    let mut doc: toml::Value =
        content
            .parse()
            .map_err(|e: toml::de::Error| HarborError::ConnectorError {
                host: "codex".to_string(),
                reason: format!("Failed to parse {}: {}", path.display(), e),
            })?;

    if let Some(table) = doc.as_table_mut() {
        if let Some(toml::Value::Table(servers)) = table.get_mut("mcp_servers") {
            for name in server_names {
                servers.remove(name);
            }
        }
    }

    let content = toml::to_string_pretty(&doc).map_err(|e| HarborError::ConnectorError {
        host: "codex".to_string(),
        reason: format!("Failed to serialize TOML: {}", e),
    })?;
    std::fs::write(path, content).map_err(HarborError::Io)?;

    Ok(())
}
