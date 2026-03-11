use clap::{Args, Subcommand};
use colored::Colorize;
use harbor_core::auth::vault::Vault;
use harbor_core::fleet::{self, FleetGit, FleetServerDef, FleetState, FLEET_SOURCE};
use harbor_core::sync::sync_all_hosts;
use harbor_core::{HarborConfig, HarborError};

// ─── CLI types ────────────────────────────────────────────────────────────────

#[derive(Args)]
pub struct CrewArgs {
    #[command(subcommand)]
    pub command: CrewCommand,
}

#[derive(Subcommand)]
pub enum CrewCommand {
    /// Initialize a fleet repository for team sync
    ///
    /// Creates ~/.harbor/fleet/ as a git repo. Optionally links a remote so
    /// teammates can clone the same fleet.
    #[command(alias = "setup")]
    Init {
        /// Git remote URL (SSH or HTTPS) to push/pull the fleet config
        #[arg(long, value_name = "URL")]
        git: Option<String>,
    },

    /// Join an existing team fleet by cloning its git repository
    Join {
        /// Git URL of the fleet repository (SSH or HTTPS)
        git_url: String,
    },

    /// Push local fleet-managed servers to the shared repository
    ///
    /// Only servers that are fleet-managed (docked with `--fleet` or pulled
    /// from the fleet) are included. Pass server names to add local servers
    /// to the fleet before pushing.
    Push {
        /// Mark these local servers as fleet-managed and include them in the push
        #[arg(value_name = "SERVER")]
        share: Vec<String>,

        /// Commit message (defaults to "Update fleet config")
        #[arg(short, long, value_name = "MSG")]
        message: Option<String>,
    },

    /// Pull server definitions from the fleet and merge into local config
    Pull {
        /// Preview what would change without applying anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Show local vs fleet drift (git sync status + per-server state)
    Status,

    /// Prompt for vault keys required by fleet servers that aren't yet provisioned
    Provision {
        /// Show missing keys without prompting for values
        #[arg(long)]
        dry_run: bool,
    },
}

// ─── Dispatcher ───────────────────────────────────────────────────────────────

pub async fn run(args: CrewArgs) -> Result<(), HarborError> {
    match args.command {
        CrewCommand::Init { git } => cmd_init(git).await,
        CrewCommand::Join { git_url } => cmd_join(git_url).await,
        CrewCommand::Push { share, message } => cmd_push(share, message).await,
        CrewCommand::Pull { dry_run } => cmd_pull(dry_run).await,
        CrewCommand::Status => cmd_status().await,
        CrewCommand::Provision { dry_run } => cmd_provision(dry_run).await,
    }
}

// ─── init ─────────────────────────────────────────────────────────────────────

async fn cmd_init(git_url: Option<String>) -> Result<(), HarborError> {
    let dir = fleet::fleet_dir()?;

    if FleetGit::is_repo(&dir) {
        println!(
            "{} Fleet already initialized at {}",
            "info:".blue().bold(),
            dir.display()
        );
    } else {
        FleetGit::init(&dir)?;
        println!(
            "{} Fleet repository initialized at {}",
            "ok:".green().bold(),
            dir.display()
        );
    }

    let git = FleetGit::new(dir);

    if let Some(url) = &git_url {
        git.set_remote(url)?;
        println!("{} Remote set to {}", "ok:".green().bold(), url.cyan());
    }

    // Create an empty fleet file and make the first commit.
    let file = fleet::fleet_file()?;
    if !file.exists() {
        let config = fleet::FleetConfig::default();
        fleet::save(&config)?;
        git.commit_local("Initialize Harbor fleet")?;
        println!("{} Created harbor-fleet.toml", "ok:".green().bold());
    }

    println!();
    if git_url.is_some() {
        println!("Next steps:");
        println!(
            "  Add servers to the fleet: {}",
            "harbor crew push <server>".cyan()
        );
        println!(
            "  Teammates join with:      {}",
            "harbor crew join <git-url>".cyan()
        );
    } else {
        println!(
            "{}",
            "hint: Link a remote later with `harbor crew init --git <url>`.".dimmed()
        );
        println!("  Add servers: {}", "harbor crew push <server>".cyan());
    }

    Ok(())
}

// ─── join ─────────────────────────────────────────────────────────────────────

async fn cmd_join(git_url: String) -> Result<(), HarborError> {
    let dir = fleet::fleet_dir()?;

    if FleetGit::is_repo(&dir) {
        return Err(HarborError::FleetGitError(
            "Fleet directory already exists. Remove ~/.harbor/fleet/ to re-join.".to_string(),
        ));
    }

    println!("Cloning fleet from {}...", git_url.cyan());
    FleetGit::clone_from(&git_url, &dir)?;
    println!("{} Fleet cloned", "ok:".green().bold());

    // Auto-merge after cloning.
    println!();
    do_pull(false).await?;

    Ok(())
}

// ─── push ─────────────────────────────────────────────────────────────────────

async fn cmd_push(share: Vec<String>, message: Option<String>) -> Result<(), HarborError> {
    require_initialized()?;

    let mut local = HarborConfig::load()?;

    // Mark any explicitly listed servers as fleet-managed.
    for name in &share {
        let server = local
            .servers
            .get_mut(name)
            .ok_or_else(|| HarborError::ServerNotFound { name: name.clone() })?;
        server.source = Some(FLEET_SOURCE.to_string());
    }
    if !share.is_empty() {
        local.save()?;
    }

    // Collect all fleet-managed servers.
    let fleet_servers: Vec<(&String, &harbor_core::config::ServerConfig)> = local
        .servers
        .iter()
        .filter(|(_, s)| s.source.as_deref() == Some(FLEET_SOURCE))
        .collect();

    if fleet_servers.is_empty() {
        println!("{}", "No fleet-managed servers to push.".dimmed());
        println!(
            "  Pass server names to add them: {}",
            "harbor crew push github linear".cyan()
        );
        return Ok(());
    }

    // Rebuild the fleet TOML from the local fleet-managed servers.
    let mut fleet_config = fleet::load().unwrap_or_default();
    for (name, server) in &fleet_servers {
        fleet_config
            .servers
            .insert(name.to_string(), FleetServerDef::from_server_config(server));
    }
    fleet::save(&fleet_config)?;

    let dir = fleet::fleet_dir()?;
    let git = FleetGit::new(dir);
    let msg = message.unwrap_or_else(|| "Update fleet config".to_string());

    match git.commit_and_push(&msg)? {
        true if git.has_remote() => println!(
            "{} Fleet pushed ({} server(s))",
            "ok:".green().bold(),
            fleet_servers.len()
        ),
        true => {
            println!(
                "{} Fleet committed locally ({} server(s))",
                "ok:".green().bold(),
                fleet_servers.len()
            );
            println!(
                "{}",
                "hint: Set a remote with `harbor crew init --git <url>` to enable team sync."
                    .dimmed()
            );
        }
        false => println!("{} Fleet is already up to date", "ok:".green().bold()),
    }

    Ok(())
}

// ─── pull ─────────────────────────────────────────────────────────────────────

async fn cmd_pull(dry_run: bool) -> Result<(), HarborError> {
    require_initialized()?;
    do_pull(dry_run).await
}

async fn do_pull(dry_run: bool) -> Result<(), HarborError> {
    let dir = fleet::fleet_dir()?;
    let git = FleetGit::new(dir);

    if git.has_remote() {
        println!("Fetching latest fleet config...");
        git.pull()?;
    }

    let fleet_config = fleet::load()?;

    if fleet_config.servers.is_empty() {
        println!("{}", "No servers defined in the fleet yet.".dimmed());
        return Ok(());
    }

    let mut local = HarborConfig::load()?;
    let mut state = FleetState::load();
    let result = fleet::merge(&mut local, &fleet_config, &mut state, dry_run);

    if dry_run {
        println!("{}", "Dry run — no changes will be applied.".bold());
        println!();
    }

    // Report added servers.
    let added = result.added();
    if !added.is_empty() {
        let label = if dry_run { "would add:" } else { "added:" };
        println!("{} {} server(s)", label.green().bold(), added.len());
        for name in &added {
            println!("  {} {}", "+".green(), name);
        }
    }

    // Report updated servers.
    let updated = result.updated();
    if !updated.is_empty() {
        println!("{} {} server(s)", "updated:".cyan().bold(), updated.len());
        for name in &updated {
            println!("  {} {}", "~".cyan(), name);
        }
    }

    // Report unchanged servers.
    let unchanged = result.unchanged();
    if !unchanged.is_empty() {
        println!(
            "{} {} server(s) already up to date",
            "ok:".green().bold(),
            unchanged.len()
        );
    }

    // Report locally modified (user edited since last pull).
    let locally_modified = result.locally_modified();
    if !locally_modified.is_empty() {
        println!(
            "{} {} server(s) skipped (locally modified since last pull):",
            "warn:".yellow().bold(),
            locally_modified.len()
        );
        for name in &locally_modified {
            println!("  {} {}", "≠".yellow(), name);
        }
        println!(
            "{}",
            "hint: To accept upstream changes: `harbor undock <name>` then `harbor crew pull`."
                .dimmed()
        );
        println!(
            "{}",
            "      To share your version with the team: `harbor crew push <name>`.".dimmed()
        );
    }

    // Report servers with a non-fleet source (existing behavior).
    let conflicts = result.conflicts();
    if !conflicts.is_empty() {
        println!(
            "{} {} server(s) skipped (non-fleet source):",
            "warn:".yellow().bold(),
            conflicts.len()
        );
        for (name, reason) in &conflicts {
            println!("  {} {} — {}", "!".yellow(), name, reason);
        }
        println!(
            "{}",
            "hint: To accept the fleet version run `harbor undock <name>` then `harbor crew pull`."
                .dimmed()
        );
    }

    if !dry_run && result.has_changes() {
        local.save()?;
        state.save()?;
        sync_all_hosts(&local);
        println!();
        println!(
            "{} Run {} to provision any missing secrets",
            "→".dimmed(),
            "harbor crew provision".cyan()
        );
    }

    Ok(())
}

// ─── status ───────────────────────────────────────────────────────────────────

async fn cmd_status() -> Result<(), HarborError> {
    require_initialized()?;

    let dir = fleet::fleet_dir()?;
    let git = FleetGit::new(dir);
    let local = HarborConfig::load()?;
    let fleet_config = fleet::load()?;

    println!("{}", "Fleet status".bold());
    println!();

    // ── Git sync status ──
    if git.has_remote() {
        match git.divergence() {
            Some((0, 0)) => println!("  {} In sync with remote", "✓".green()),
            Some((ahead, 0)) => println!(
                "  {} {} commit(s) ahead — run {}",
                "↑".yellow(),
                ahead,
                "harbor crew push".cyan()
            ),
            Some((0, behind)) => println!(
                "  {} {} commit(s) behind — run {}",
                "↓".cyan(),
                behind,
                "harbor crew pull".cyan()
            ),
            Some((ahead, behind)) => {
                println!(
                    "  {} {} ahead, {} behind remote",
                    "↕".yellow(),
                    ahead,
                    behind
                );
            }
            None => println!("  {} Could not determine remote sync status", "?".dimmed()),
        }
        if let Some(url) = git.remote_url() {
            println!("  {} remote: {}", "→".dimmed(), url.dimmed());
        }
    } else {
        println!("  {} Local-only fleet (no remote configured)", "ℹ".dimmed());
        println!(
            "     Set a remote: {}",
            "harbor crew init --git <url>".cyan()
        );
    }

    println!();
    println!("{}", "Servers:".bold());

    for name in fleet_config.servers.keys() {
        match local.servers.get(name) {
            Some(s) if s.source.as_deref() == Some(FLEET_SOURCE) => {
                println!("  {} {}  {}", "✓".green(), name, "(fleet-managed)".dimmed());
            }
            Some(_) => {
                println!(
                    "  {} {}  {}",
                    "!".yellow(),
                    name,
                    "(local override — fleet definition skipped on pull)".yellow()
                );
            }
            None => {
                println!(
                    "  {} {}  {}",
                    "–".dimmed(),
                    name,
                    "(not pulled yet — run `harbor crew pull`)".dimmed()
                );
            }
        }
    }

    // Fleet-managed servers that no longer exist in the fleet file (orphans).
    let orphans: Vec<&str> = local
        .servers
        .iter()
        .filter(|(name, s)| {
            s.source.as_deref() == Some(FLEET_SOURCE) && !fleet_config.servers.contains_key(*name)
        })
        .map(|(name, _)| name.as_str())
        .collect();

    if !orphans.is_empty() {
        println!();
        println!(
            "{} Orphaned fleet servers (removed from fleet upstream):",
            "warn:".yellow().bold()
        );
        for name in &orphans {
            println!(
                "  {} {}  {}",
                "×".red(),
                name,
                "(run `harbor undock` to clean up)".dimmed()
            );
        }
    }

    if fleet_config.servers.is_empty() && orphans.is_empty() {
        println!("{}", "  No servers in this fleet yet.".dimmed());
    }

    Ok(())
}

// ─── provision ────────────────────────────────────────────────────────────────

async fn cmd_provision(dry_run: bool) -> Result<(), HarborError> {
    let fleet_config = fleet::load()?;
    let report = fleet::find_missing_keys(&fleet_config);

    if report.is_complete() {
        println!("{} All vault keys are provisioned", "ok:".green().bold());
        return Ok(());
    }

    println!(
        "{} {} vault key(s) missing:",
        "warn:".yellow().bold(),
        report.missing.len()
    );
    println!();

    for mk in &report.missing {
        println!(
            "  {} {}  {}",
            "vault:".dimmed(),
            mk.key.cyan(),
            format!("(used by: {})", mk.used_by).dimmed()
        );
    }

    if dry_run {
        println!();
        println!("{}", "Dry run — no keys provisioned.".dimmed());
        return Ok(());
    }

    println!();
    println!("Enter values for each missing key (input is hidden):");
    println!();

    let mut provisioned = 0usize;
    for mk in &report.missing {
        let prompt = format!("  {}: ", mk.key.cyan());
        let value = read_secret(&prompt)?;
        if value.is_empty() {
            println!("  {} Skipped {}", "–".dimmed(), mk.key);
        } else {
            Vault::set(&mk.key, &value)?;
            println!("  {} Stowed {}", "✓".green(), mk.key);
            provisioned += 1;
        }
    }

    println!();
    println!(
        "{} {}/{} key(s) provisioned",
        "ok:".green().bold(),
        provisioned,
        report.missing.len()
    );

    Ok(())
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn require_initialized() -> Result<(), HarborError> {
    if !fleet::is_initialized() {
        Err(HarborError::FleetNotInitialized)
    } else {
        Ok(())
    }
}

fn read_secret(prompt: &str) -> Result<String, HarborError> {
    rpassword::prompt_password(prompt).map_err(HarborError::Io)
}
