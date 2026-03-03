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

    if let Err(e) = commands::run(cli).await {
        eprintln!("{} {}", colored::Colorize::red("error:"), e);
        std::process::exit(1);
    }
}
