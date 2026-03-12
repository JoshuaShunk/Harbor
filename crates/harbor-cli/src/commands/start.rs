use clap::Args;
use colored::Colorize;
use harbor_core::server::{ManagedProcess, PidStore, ServerManager};
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct StartArgs {
    /// Name of the ship to launch (omit to launch the whole fleet)
    pub name: Option<String>,

    /// Run in the background — write a PID file and return immediately
    #[arg(long, short = 'd')]
    pub detach: bool,
}

pub async fn run(args: StartArgs) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;

    if args.detach {
        return run_detached(args.name, &config);
    }

    let mut manager = ServerManager::new();

    match args.name {
        Some(name) => {
            let server_config = config.get_server(&name)?;
            manager.start(&name, server_config).await?;
            println!("{} Server '{}' launched", "ok:".green().bold(), name.cyan());

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
                    Ok(()) => println!("{} Launched '{}'", "ok:".green().bold(), name.cyan()),
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

fn run_detached(name: Option<String>, config: &HarborConfig) -> Result<(), HarborError> {
    let targets: Vec<(&String, _)> = match &name {
        Some(n) => {
            let server_config = config.get_server(n)?;
            vec![(n, server_config)]
        }
        None => config
            .servers
            .iter()
            .filter(|(_, s)| s.enabled && s.auto_start)
            .collect(),
    };

    if targets.is_empty() {
        println!("No servers configured with auto-start.");
        println!(
            "  Use {} to launch a specific server",
            "harbor launch --detach <name>".yellow()
        );
        return Ok(());
    }

    let resolved_envs: Vec<_> = targets
        .iter()
        .map(|(_, sc)| harbor_core::auth::vault::Vault::resolve_env(&sc.env))
        .collect();

    for ((server_name, server_config), resolved_env) in targets.iter().zip(resolved_envs.iter()) {
        match ManagedProcess::spawn_detached(server_name, server_config, resolved_env) {
            Ok(pid) => {
                PidStore::write(server_name, pid)?;
                println!(
                    "{} '{}' running in background (PID {})",
                    "ok:".green().bold(),
                    server_name.cyan(),
                    pid.to_string().dimmed()
                );
                println!(
                    "  Stop with: {}",
                    format!("harbor anchor {}", server_name).yellow()
                );
            }
            Err(e) => eprintln!(
                "{} Failed to launch '{}': {}",
                "err:".red().bold(),
                server_name.cyan(),
                e
            ),
        }
    }

    Ok(())
}
