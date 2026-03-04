mod commands;

use clap::Parser;
use commands::Cli;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("harbor=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let is_update_command = matches!(cli.command, commands::Commands::Update(_));

    if let Err(e) = commands::run(cli).await {
        eprintln!("{} {}", colored::Colorize::red("error:"), e);
        std::process::exit(1);
    }

    // Show update notice from cache (no network call, no delay)
    if !is_update_command {
        show_update_notice();
    }
}

fn show_update_notice() {
    use colored::Colorize;
    use harbor_core::updater;

    match updater::read_cache() {
        Some(cache) if cache.update_available => {
            eprintln!(
                "\n{} Harbor v{} available — run {} to update",
                "update:".dimmed(),
                cache.latest_version.dimmed(),
                "harbor update".dimmed(),
            );
        }
        Some(_) => {}
        None => {
            // Cache expired or missing — refresh in background for next time
            tokio::spawn(async {
                if let Ok(update) = updater::check_for_update().await {
                    let _ = updater::write_cache(&update);
                }
            });
        }
    }
}
