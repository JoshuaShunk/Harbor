use clap::Args;
use colored::Colorize;
use harbor_core::auth::oauth;
use harbor_core::sync::sync_all_hosts;
use harbor_core::{HarborConfig, HarborError, ServerConfig};
use std::collections::BTreeMap;

#[derive(Args)]
pub struct AddArgs {
    /// Native ship name (e.g., github, slack, google-drive, filesystem)
    pub native: Option<String>,

    /// Name to paint on the hull (required for custom servers)
    #[arg(short, long)]
    pub name: Option<String>,

    /// Command to run (e.g., "npx", "node", "python")
    #[arg(short, long)]
    pub command: Option<String>,

    /// Cargo to pass to the command (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    pub args: Vec<String>,

    /// Provisions (KEY=VALUE env vars, can be repeated)
    #[arg(short, long, value_parser = parse_env_var)]
    pub env: Vec<(String, String)>,

    /// Launch automatically when Harbor opens
    #[arg(long, default_value = "false")]
    pub auto_start: bool,

    /// Keep moored (disabled) initially
    #[arg(long, default_value = "false")]
    pub disabled: bool,

    /// Skip OAuth — dock now, charter later
    #[arg(long, default_value = "false")]
    pub skip_auth: bool,

    /// Extra arguments appended after native defaults (use -- to separate)
    #[arg(last = true)]
    pub extra_args: Vec<String>,
}

fn parse_env_var(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid KEY=VALUE pair: no '=' found in '{s}'"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

pub async fn run(args: AddArgs) -> Result<(), HarborError> {
    if let Some(ref native_id) = args.native {
        if let Some(native) = harbor_core::catalog::lookup(native_id) {
            return run_native(native, args).await;
        }
        // Not a known native server — give a helpful error
        let ids = harbor_core::catalog::all_ids();
        return Err(HarborError::ConfigParse(format!(
            "Unknown native server '{}'. Available: {}\n\
             For custom servers, use: harbor dock --name {} --command <cmd> --args <args>",
            native_id,
            ids.join(", "),
            native_id,
        )));
    }

    run_custom(args).await
}

// ---------------------------------------------------------------------------
// Native server path
// ---------------------------------------------------------------------------

async fn run_native(native: harbor_core::NativeServer, args: AddArgs) -> Result<(), HarborError> {
    let mut config = HarborConfig::load()?;

    let server_name = args.name.as_deref().unwrap_or(native.id).to_string();

    println!(
        "{} Docking {} ({})...",
        "=>".dimmed(),
        native.display_name.cyan().bold(),
        native.description.dimmed(),
    );

    // Handle OAuth if needed
    if let harbor_core::AuthKind::OAuth(ref provider_id) = native.auth {
        if !args.skip_auth && !harbor_core::catalog::has_auth(&native) {
            println!(
                "{} {} requires authorization. Opening browser...",
                "auth:".yellow().bold(),
                native.display_name,
            );
            run_cli_oauth(provider_id).await?;
            println!("{} Papers received!", "ok:".green().bold());
        } else if harbor_core::catalog::has_auth(&native) {
            println!(
                "{} {} papers already on file",
                "ok:".green().bold(),
                native.display_name,
            );
        }
    }

    // Handle ManualToken — check if key is in vault
    if let harbor_core::AuthKind::ManualToken {
        env_var,
        description,
    } = &native.auth
    {
        if !harbor_core::catalog::has_auth(&native) {
            return Err(HarborError::ConfigParse(format!(
                "{} requires {}.\nStore it with: harbor chest set {} <value>",
                native.display_name,
                description,
                env_var.to_lowercase(),
            )));
        }
    }

    // Build env map from catalog defaults
    let mut env = harbor_core::catalog::build_env(&native)?;

    // Merge any user-supplied --env overrides
    for (k, v) in args.env {
        env.insert(k, v);
    }

    // Build final args: catalog defaults + any extra trailing args
    let mut final_args: Vec<String> = native.args.iter().map(|s| s.to_string()).collect();
    final_args.extend(args.extra_args);

    let server = ServerConfig {
        source: Some(format!("native:{}", native.id)),
        command: native.command.to_string(),
        args: final_args,
        env,
        enabled: !args.disabled,
        auto_start: args.auto_start,
        hosts: BTreeMap::new(),
        tool_allowlist: None,
        tool_blocklist: None,
        tool_hosts: BTreeMap::new(),
    };

    config.add_server(server_name.clone(), server)?;
    config.save()?;

    println!(
        "{} Server '{}' docked",
        "ok:".green().bold(),
        server_name.cyan(),
    );

    // Auto-sync to all connected hosts
    let results = sync_all_hosts(&config);
    for (_, result) in &results {
        if let Ok(r) = result {
            println!("  {} Synced to {}", "=>".dimmed(), r.display_name.cyan());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CLI OAuth flow — opens browser, waits for callback
// ---------------------------------------------------------------------------

async fn run_cli_oauth(provider_id: &str) -> Result<(), HarborError> {
    let (auth_url, callback_server, pkce) = oauth::start_oauth_flow(provider_id).await?;

    // Open system browser
    if let Err(e) = open::that(&auth_url) {
        eprintln!(
            "{} Could not open browser automatically: {}",
            "warn:".yellow().bold(),
            e,
        );
        println!(
            "{} Visit this URL to authorize:\n  {}",
            "=>".dimmed(),
            auth_url.underline(),
        );
    }

    println!(
        "{} Waiting for authorization (up to 5 minutes)...",
        "=>".dimmed(),
    );

    let port = callback_server.port;
    let code = tokio::time::timeout(std::time::Duration::from_secs(300), callback_server.code_rx)
        .await
        .map_err(|_| {
            HarborError::OAuthError("Authorization timed out after 5 minutes. Try again.".into())
        })?
        .map_err(|_| HarborError::OAuthError("Authorization cancelled.".into()))?;

    oauth::complete_oauth_flow(provider_id, &code, port, pkce.as_ref()).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Custom (manual) server path — backward-compatible
// ---------------------------------------------------------------------------

async fn run_custom(args: AddArgs) -> Result<(), HarborError> {
    let name = args.name.ok_or_else(|| {
        let ids = harbor_core::catalog::all_ids();
        HarborError::ConfigParse(format!(
            "Missing --name. For native servers try: harbor dock <name>\n\
             Available: {}",
            ids.join(", "),
        ))
    })?;
    let command = args
        .command
        .ok_or_else(|| HarborError::ConfigParse("Custom servers require --command".into()))?;

    let mut config = HarborConfig::load()?;

    let env: BTreeMap<String, String> = args.env.into_iter().collect();

    let server = ServerConfig {
        source: None,
        command,
        args: args.args,
        env,
        enabled: !args.disabled,
        auto_start: args.auto_start,
        hosts: BTreeMap::new(),
        tool_allowlist: None,
        tool_blocklist: None,
        tool_hosts: BTreeMap::new(),
    };

    config.add_server(name.clone(), server)?;
    config.save()?;

    println!("{} Server '{}' docked", "ok:".green().bold(), name.cyan());

    // Auto-sync to all connected hosts
    let results = sync_all_hosts(&config);
    for (_, result) in &results {
        if let Ok(r) = result {
            println!("  {} Synced to {}", "=>".dimmed(), r.display_name.cyan());
        }
    }

    Ok(())
}
