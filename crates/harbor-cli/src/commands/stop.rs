use clap::Args;
use colored::Colorize;
use harbor_core::server::PidStore;
use harbor_core::HarborError;

#[derive(Args)]
pub struct StopArgs {
    /// Name of the ship to anchor
    pub name: String,
}

pub async fn run(args: StopArgs) -> Result<(), HarborError> {
    let name = &args.name;

    let pid = match PidStore::read(name) {
        Some(p) => p,
        None => {
            println!(
                "{} No background process found for '{}'.",
                "info:".blue().bold(),
                name.cyan()
            );
            println!(
                "  Start one with: {}",
                format!("harbor launch --detach {}", name).yellow()
            );
            return Ok(());
        }
    };

    if !PidStore::is_running(pid) {
        println!(
            "{} '{}' (PID {}) is no longer running. Cleaning up stale PID file.",
            "info:".blue().bold(),
            name.cyan(),
            pid.to_string().dimmed()
        );
        PidStore::remove(name);
        return Ok(());
    }

    terminate_process(pid).map_err(|e| HarborError::ConnectorError {
        host: "process".to_string(),
        reason: format!("Failed to terminate '{}' (PID {}): {}", name, pid, e),
    })?;

    PidStore::remove(name);

    println!(
        "{} Anchored '{}' (PID {})",
        "ok:".green().bold(),
        name.cyan(),
        pid.to_string().dimmed()
    );

    Ok(())
}

fn terminate_process(pid: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .args(["-s", "TERM", &pid.to_string()])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "kill returned non-zero for PID {}",
                pid
            )));
        }
    }
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "taskkill returned non-zero for PID {}",
                pid
            )));
        }
    }
    Ok(())
}
