use clap::Args;
use colored::Colorize;
use harbor_core::marketplace::registry::RegistryClient;
use harbor_core::HarborError;

#[derive(Args)]
pub struct SearchArgs {
    /// What waters to scout (e.g. "github", "database", "memory")
    pub query: String,

    /// Max sightings per page
    #[arg(long, default_value = "10")]
    pub limit: u32,

    /// Cursor for next page (from previous search output)
    #[arg(long)]
    pub cursor: Option<String>,
}

pub async fn run(args: SearchArgs) -> Result<(), HarborError> {
    let client = RegistryClient::new();

    println!(
        "{} the seas for \"{}\"...\n",
        "Scouting".cyan().bold(),
        args.query
    );

    let result = client
        .search(&args.query, args.cursor.as_deref(), Some(args.limit))
        .await?;

    if result.servers.is_empty() {
        println!("{}", "No ships spotted on the horizon.".dimmed());
        return Ok(());
    }

    for server in &result.servers {
        let badge = if server.is_official {
            " [official]".green().to_string()
        } else {
            String::new()
        };
        let display = server.title.as_deref().unwrap_or(&server.name);

        println!("  {} {}{}", display.bold(), server.name.dimmed(), badge);
        println!("    {}", server.description.dimmed());

        if let Some(ref pkg) = server.package {
            let runtime = pkg
                .runtime_hint
                .as_deref()
                .unwrap_or(match pkg.registry_type.as_str() {
                    "pypi" => "uvx",
                    _ => "npx",
                });
            println!("    {} {} {}", "pkg:".cyan(), runtime, pkg.identifier);

            let env_vars: Vec<_> = pkg.environment_variables.iter().collect();
            if !env_vars.is_empty() {
                let names: Vec<String> = env_vars
                    .iter()
                    .map(|e| {
                        let mut s = e.name.clone();
                        if e.is_required {
                            s.push('*');
                        }
                        s
                    })
                    .collect();
                println!("    {} {}", "env:".cyan(), names.join(", "));
            }
        }

        if let Some(ref url) = server.website_url {
            println!("    {}", url.dimmed());
        }
        println!();
    }

    println!("{} sightings loaded", result.servers.len());

    if let Some(ref cursor) = result.next_cursor {
        println!(
            "{}",
            format!(
                "Sail ahead: harbor scout \"{}\" --cursor {}",
                args.query, cursor
            )
            .dimmed()
        );
    }

    Ok(())
}
