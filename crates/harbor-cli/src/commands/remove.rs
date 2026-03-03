use clap::Args;
use colored::Colorize;
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
    println!("  Run {} to signal your hosts", "harbor signal".yellow());

    Ok(())
}
