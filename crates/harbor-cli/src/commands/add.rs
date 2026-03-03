use clap::Args;
use colored::Colorize;
use harbor_core::{HarborConfig, HarborError, ServerConfig};
use std::collections::BTreeMap;

#[derive(Args)]
pub struct AddArgs {
    /// Name to paint on the hull
    #[arg(short, long)]
    pub name: String,

    /// Command to run (e.g., "npx", "node", "python")
    #[arg(short, long)]
    pub command: String,

    /// Cargo to pass to the command (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    pub args: Vec<String>,

    /// Provisions (KEY=VALUE env vars, can be repeated)
    #[arg(short, long, value_parser = parse_env_var)]
    pub env: Vec<(String, String)>,

    /// Launch automatically when Harbor opens
    #[arg(long, default_value = "false")]
    pub auto_start: bool,

    /// Keep moored (disabled) initially
    #[arg(long, default_value = "false")]
    pub disabled: bool,
}

fn parse_env_var(s: &str) -> Result<(String, String), String> {
    let pos = s.find('=').ok_or_else(|| {
        format!("Invalid KEY=VALUE pair: no '=' found in '{s}'")
    })?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

pub async fn run(args: AddArgs) -> Result<(), HarborError> {
    let mut config = HarborConfig::load()?;

    let env: BTreeMap<String, String> = args.env.into_iter().collect();

    let server = ServerConfig {
        source: None,
        command: args.command,
        args: args.args,
        env,
        enabled: !args.disabled,
        auto_start: args.auto_start,
        hosts: BTreeMap::new(),
    };

    let name = args.name.clone();
    config.add_server(args.name, server)?;
    config.save()?;

    println!(
        "{} Server '{}' docked",
        "ok:".green().bold(),
        name.cyan()
    );
    println!(
        "  Run {} to launch it, or {} to signal your hosts",
        format!("harbor launch {name}").yellow(),
        "harbor signal".yellow()
    );

    Ok(())
}
