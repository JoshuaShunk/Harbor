use clap::Args;
use colored::Colorize;
use harbor_core::updater;
use harbor_core::HarborError;
use std::io::{self, Write};

#[derive(Args)]
pub struct UpdateArgs {
    /// Just check for updates, don't install
    #[arg(long)]
    pub check: bool,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

pub async fn run(args: UpdateArgs) -> Result<(), HarborError> {
    println!("{} Checking for updates...", "info:".blue().bold());

    let update = updater::check_for_update().await?;

    // Update the cache regardless of outcome
    let _ = updater::write_cache(&update);

    if !update.update_available {
        println!(
            "{} Harbor v{} is the latest version",
            "ok:".green().bold(),
            update.current_version
        );
        return Ok(());
    }

    println!(
        "{} New version available: {} (current: v{})",
        "info:".blue().bold(),
        format!("v{}", update.latest_version).cyan(),
        update.current_version
    );

    if args.check {
        println!("\nRun {} to install the update.", "harbor update".yellow());
        return Ok(());
    }

    if update.download_url.is_none() {
        println!(
            "{} No pre-built binary available for this platform ({}).",
            "warn:".yellow().bold(),
            updater::current_target()
        );
        println!("  Build from source or check GitHub releases manually.");
        return Ok(());
    }

    // Confirm
    if !args.yes {
        print!("Install v{}? [y/N] ", update.latest_version);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Find current binary location
    let binary_path = updater::which_harbor().ok_or_else(|| HarborError::ConnectorError {
        host: "update".into(),
        reason: "Could not locate the harbor binary in PATH. Install manually from GitHub.".into(),
    })?;

    println!(
        "{} Downloading v{}...",
        "info:".blue().bold(),
        update.latest_version
    );

    let tarball_path = updater::download_and_verify(&update).await?;

    println!("{} Checksum verified. Installing...", "ok:".green().bold());

    match updater::extract_and_replace(&tarball_path, &binary_path) {
        Ok(()) => {
            let _ = updater::clear_cache();
            println!(
                "\n{} Harbor updated to v{}",
                "ok:".green().bold(),
                update.latest_version
            );
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.to_lowercase().contains("permission denied") {
                println!(
                    "{} Permission denied writing to {}",
                    "err:".red().bold(),
                    binary_path.display()
                );
                println!("\n  Try: {}", "sudo harbor update --yes".yellow());
                return Ok(());
            }
            return Err(e);
        }
    }

    Ok(())
}
