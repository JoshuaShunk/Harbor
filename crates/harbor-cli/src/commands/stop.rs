use clap::Args;
use colored::Colorize;
use harbor_core::HarborError;

#[derive(Args)]
pub struct StopArgs {
    /// Name of the ship to anchor
    pub name: String,
}

pub async fn run(args: StopArgs) -> Result<(), HarborError> {
    // In the current CLI model, `harbor start` holds the process.
    // `harbor stop` will be more useful once we have a daemon mode (Phase 3).
    // For now, inform the user.
    println!(
        "{} To anchor '{}', press Ctrl+C in the terminal where it was launched.",
        "info:".blue().bold(),
        args.name.cyan()
    );
    println!("  Daemon mode (background fleet management) is coming in a future release.");

    Ok(())
}
