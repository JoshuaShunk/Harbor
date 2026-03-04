use clap::{Args, Subcommand};
use colored::Colorize;
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct ToolsArgs {
    #[command(subcommand)]
    pub action: ToolsAction,
}

#[derive(Subcommand)]
pub enum ToolsAction {
    /// Show tool filters for a server
    Show {
        /// Server name
        server: String,
    },

    /// Set the tool allowlist (only these tools will be exposed)
    Allow {
        /// Server name
        server: String,

        /// Tool names to allow (comma-separated)
        #[arg(value_delimiter = ',')]
        tools: Vec<String>,

        /// Apply only to a specific host
        #[arg(long)]
        host: Option<String>,
    },

    /// Add tools to the blocklist
    Block {
        /// Server name
        server: String,

        /// Tool names to block (comma-separated)
        #[arg(value_delimiter = ',')]
        tools: Vec<String>,
    },

    /// Clear all tool filters for a server
    Reset {
        /// Server name
        server: String,

        /// Clear only filters for a specific host
        #[arg(long)]
        host: Option<String>,
    },
}

pub async fn run(args: ToolsArgs) -> Result<(), HarborError> {
    match args.action {
        ToolsAction::Show { server } => show(&server),
        ToolsAction::Allow {
            server,
            tools,
            host,
        } => allow(&server, tools, host.as_deref()),
        ToolsAction::Block { server, tools } => block(&server, tools),
        ToolsAction::Reset { server, host } => reset(&server, host.as_deref()),
    }
}

fn show(server_name: &str) -> Result<(), HarborError> {
    let config = HarborConfig::load()?;
    let server = config.get_server(server_name)?;

    println!(
        "{} {}",
        "Tool filters for".bold(),
        server_name.cyan().bold()
    );
    println!();

    match &server.tool_allowlist {
        Some(list) if !list.is_empty() => {
            println!("  {} {}", "allowlist:".green(), list.join(", "));
        }
        _ => {
            println!(
                "  {} {}",
                "allowlist:".dimmed(),
                "(all tools exposed)".dimmed()
            );
        }
    }

    match &server.tool_blocklist {
        Some(list) if !list.is_empty() => {
            println!("  {} {}", "blocklist:".red(), list.join(", "));
        }
        _ => {
            println!("  {} {}", "blocklist:".dimmed(), "(none)".dimmed());
        }
    }

    if !server.tool_hosts.is_empty() {
        println!();
        println!("  {}", "Host overrides:".bold());
        for (host, tools) in &server.tool_hosts {
            println!("    {}: {}", host.yellow(), tools.join(", "));
        }
    }

    Ok(())
}

fn allow(server_name: &str, tools: Vec<String>, host: Option<&str>) -> Result<(), HarborError> {
    let mut config = HarborConfig::load()?;
    let server =
        config
            .servers
            .get_mut(server_name)
            .ok_or_else(|| HarborError::ServerNotFound {
                name: server_name.to_string(),
            })?;

    if let Some(host_name) = host {
        server
            .tool_hosts
            .insert(host_name.to_string(), tools.clone());
        config.save()?;
        println!(
            "{} Set {} tool(s) for {} on host {}",
            "ok:".green().bold(),
            tools.len().to_string().cyan(),
            server_name.cyan(),
            host_name.yellow()
        );
    } else {
        let count = tools.len();
        server.tool_allowlist = Some(tools);
        config.save()?;
        println!(
            "{} Set allowlist to {} tool(s) for {}",
            "ok:".green().bold(),
            count.to_string().cyan(),
            server_name.cyan()
        );
    }

    Ok(())
}

fn block(server_name: &str, tools: Vec<String>) -> Result<(), HarborError> {
    let mut config = HarborConfig::load()?;
    let server =
        config
            .servers
            .get_mut(server_name)
            .ok_or_else(|| HarborError::ServerNotFound {
                name: server_name.to_string(),
            })?;

    let count = tools.len();
    let existing = server.tool_blocklist.get_or_insert_with(Vec::new);
    for tool in tools {
        if !existing.contains(&tool) {
            existing.push(tool);
        }
    }

    config.save()?;
    println!(
        "{} Added {} tool(s) to blocklist for {}",
        "ok:".green().bold(),
        count.to_string().cyan(),
        server_name.cyan()
    );

    Ok(())
}

fn reset(server_name: &str, host: Option<&str>) -> Result<(), HarborError> {
    let mut config = HarborConfig::load()?;
    let server =
        config
            .servers
            .get_mut(server_name)
            .ok_or_else(|| HarborError::ServerNotFound {
                name: server_name.to_string(),
            })?;

    if let Some(host_name) = host {
        server.tool_hosts.remove(host_name);
        config.save()?;
        println!(
            "{} Cleared tool filter for {} on host {}",
            "ok:".green().bold(),
            server_name.cyan(),
            host_name.yellow()
        );
    } else {
        server.tool_allowlist = None;
        server.tool_blocklist = None;
        server.tool_hosts.clear();
        config.save()?;
        println!(
            "{} Cleared all tool filters for {}",
            "ok:".green().bold(),
            server_name.cyan()
        );
    }

    Ok(())
}
