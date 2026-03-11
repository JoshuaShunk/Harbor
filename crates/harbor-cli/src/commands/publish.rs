use clap::Args;
use colored::Colorize;
use harbor_core::relay::{CloudflareTransport, PublishClient, TransportConfig};
use harbor_core::HarborError;
use tokio::sync::oneshot;

#[derive(Args)]
pub struct PublishArgs {
    /// Relay server address (default: relay.harbormcp.ai)
    #[arg(long, default_value = "relay.harbormcp.ai")]
    pub relay: String,

    /// Requested subdomain (default: auto-generated)
    #[arg(long)]
    pub subdomain: Option<String>,

    /// Authentication token for the relay
    #[arg(long)]
    pub token: Option<String>,

    /// Transport to use: quic (default), cloudflare
    #[arg(long, default_value = "quic")]
    pub transport: String,

    /// Local gateway port (default: from config)
    #[arg(long)]
    pub port: Option<u16>,

    /// Tool allowlist for remote access (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub tools: Option<Vec<String>>,

    /// Relay's public key (hex, for self-hosted relay verification)
    #[arg(long)]
    pub relay_key: Option<String>,
}

pub async fn run(args: PublishArgs) -> Result<(), HarborError> {
    let config = harbor_core::HarborConfig::load()?;
    let port = args.port.unwrap_or(config.harbor.gateway_port);

    let relay = if args.relay == "relay.harbormcp.ai" {
        config
            .harbor
            .publish_relay
            .unwrap_or_else(|| args.relay.clone())
    } else {
        args.relay.clone()
    };

    let subdomain = args.subdomain.or(config.harbor.publish_subdomain);
    let token = args.token.or(config.harbor.publish_token);
    let tools = args.tools.or(config.harbor.publish_tools);
    let relay_key = args.relay_key.or(config.harbor.publish_relay_key);

    println!(
        "{} Broadcasting your gateway to the high seas...",
        "⚓".bold()
    );

    match args.transport.as_str() {
        "cloudflare" | "cf" => {
            run_cloudflare(port).await?;
        }
        _ => {
            run_quic(relay, subdomain, token, tools, relay_key, port).await?;
        }
    }

    Ok(())
}

async fn run_quic(
    relay: String,
    subdomain: Option<String>,
    token: Option<String>,
    tools: Option<Vec<String>>,
    relay_key: Option<String>,
    port: u16,
) -> Result<(), HarborError> {
    let transport_config = TransportConfig {
        gateway_addr: format!("http://127.0.0.1:{port}"),
        relay_addr: Some(format!("{relay}:7800")),
        auth_token: token,
        subdomain,
        relay_public_key: relay_key,
        tools,
        gateway_port: port,
    };

    let client = PublishClient::new(transport_config);

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Handle Ctrl+C
    let ctrlc_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Stopping publish...", "⚓".bold());
        let _ = shutdown_tx.send(());
    });

    match client.run(shutdown_rx).await {
        Ok(info) => {
            println!();
            println!("{}", "Published successfully!".green().bold());
            println!("  URL:       {}", info.url.cyan());
            println!("  Token:     {}", info.token.dimmed());
            println!("  Transport: {}", info.transport);
            println!();
            println!("Remote clients can connect with:",);
            println!(
                "  curl -X POST {}/mcp -H 'Authorization: Bearer {}' -H 'Content-Type: application/json'",
                info.url, info.token
            );
        }
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);
            return Err(e);
        }
    }

    ctrlc_handle.abort();
    Ok(())
}

async fn run_cloudflare(port: u16) -> Result<(), HarborError> {
    use harbor_core::relay::Transport;

    let mut transport = CloudflareTransport::new();
    let config = TransportConfig {
        gateway_addr: format!("http://127.0.0.1:{port}"),
        gateway_port: port,
        ..TransportConfig::default()
    };

    let info = transport.connect(&config).await?;

    println!();
    println!("{}", "Published via Cloudflare Tunnel!".green().bold());
    println!("  URL:       {}", info.url.cyan());
    println!("  Transport: cloudflare");
    println!();
    println!("Press Ctrl+C to stop");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await.ok();
    println!("\n{} Stopping tunnel...", "⚓".bold());

    transport.disconnect().await?;
    Ok(())
}
