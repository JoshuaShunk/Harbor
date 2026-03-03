use clap::Args;
use colored::Colorize;
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct ListArgs {
    /// Show only ships bound for a specific port
    #[arg(long)]
    pub host: Option<String>,
}

pub async fn run(args: ListArgs) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;

    if config.servers.is_empty() {
        println!("No ships in the fleet.");
        println!(
            "  Run {} to dock one",
            "harbor dock --name <name> --command <cmd>".yellow()
        );
        return Ok(());
    }

    let servers: Vec<_> = if let Some(ref host) = args.host {
        config
            .servers_for_host(host)
            .into_iter()
            .map(|(name, cfg)| (name.clone(), cfg.clone()))
            .collect()
    } else {
        config
            .servers
            .iter()
            .map(|(name, cfg)| (name.clone(), cfg.clone()))
            .collect()
    };

    if servers.is_empty() {
        println!("No servers match the filter.");
        return Ok(());
    }

    let header = if let Some(ref host) = args.host {
        format!("Fleet (port: {host})")
    } else {
        "Fleet".to_string()
    };
    println!("{}", header.bold());
    println!();

    for (name, server) in &servers {
        let status = if server.enabled {
            "enabled".green()
        } else {
            "disabled".red()
        };

        println!("  {} [{}]", name.cyan().bold(), status);
        println!(
            "    command: {} {}",
            server.command,
            server.args.join(" ")
        );

        if !server.env.is_empty() {
            let keys: Vec<&String> = server.env.keys().collect();
            println!("    env:     {}", keys.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", "));
        }

        if server.auto_start {
            println!("    auto-start: {}", "yes".green());
        }

        if !server.hosts.is_empty() {
            let host_list: Vec<String> = server
                .hosts
                .iter()
                .map(|(h, enabled)| {
                    if *enabled {
                        h.green().to_string()
                    } else {
                        h.red().to_string()
                    }
                })
                .collect();
            println!("    hosts:   {}", host_list.join(", "));
        }

        println!();
    }

    println!("{} ship(s) in the fleet", servers.len());

    Ok(())
}
