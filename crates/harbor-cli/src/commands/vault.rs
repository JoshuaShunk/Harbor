use clap::{Args, Subcommand};
use colored::Colorize;
use harbor_core::auth::vault::Vault;
use harbor_core::HarborError;

#[derive(Args)]
pub struct VaultArgs {
    #[command(subcommand)]
    pub command: VaultCommand,
}

#[derive(Subcommand)]
pub enum VaultCommand {
    /// Stow a secret in the chest
    #[command(alias = "set")]
    Stow {
        /// Secret key name
        key: String,
        /// Secret value
        value: String,
    },
    /// Retrieve a secret from the chest
    #[command(alias = "get")]
    Retrieve {
        /// Secret key name
        key: String,
    },
    /// Toss a secret overboard
    #[command(alias = "delete")]
    Toss {
        /// Secret key name
        key: String,
    },
    /// Take inventory of the chest
    #[command(alias = "list")]
    Inventory,
}

pub async fn run(args: VaultArgs) -> Result<(), HarborError> {
    match args.command {
        VaultCommand::Stow { key, value } => {
            Vault::set(&key, &value)?;
            println!("{} Secret '{}' stowed in the chest", "ok:".green().bold(), key);
        }
        VaultCommand::Retrieve { key } => match Vault::get(&key) {
            Ok(value) => println!("{}", value),
            Err(e) => {
                eprintln!("{} {}", "err:".red().bold(), e);
                std::process::exit(1);
            }
        },
        VaultCommand::Toss { key } => {
            Vault::delete(&key)?;
            println!("{} Secret '{}' tossed overboard", "ok:".green().bold(), key);
        }
        VaultCommand::Inventory => {
            let keys = Vault::list_keys()?;
            if keys.is_empty() {
                println!("{}", "The chest is empty.".dimmed());
            } else {
                println!("{}", "Treasure chest:".bold());
                for key in &keys {
                    println!("  {}", key);
                }
                println!(
                    "\n{} secret(s) stowed",
                    keys.len().to_string().cyan()
                );
            }
        }
    }
    Ok(())
}
