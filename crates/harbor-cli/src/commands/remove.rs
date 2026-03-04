use clap::Args;
use colored::Colorize;
use harbor_core::sync::sync_all_hosts;
use harbor_core::{HarborConfig, HarborError};

#[derive(Args)]
pub struct RemoveArgs {
    /// Name of the ship to cast off
    pub name: String,
}

pub async fn run(args: RemoveArgs) -> Result<(), HarborError> {
    let mut config = HarborConfig::load()?;
    config.remove_server(&args.name)?;
    config.save()?;

    println!(
        "{} Server '{}' undocked",
        "ok:".green().bold(),
        args.name.cyan()
    );

    // Auto-sync to all connected hosts
    let config = HarborConfig::load()?;
    let results = sync_all_hosts(&config);
    for (_, result) in &results {
        if let Ok(r) = result {
            println!("  {} Synced to {}", "=>".dimmed(), r.display_name.cyan());
        }
    }

    Ok(())
}
