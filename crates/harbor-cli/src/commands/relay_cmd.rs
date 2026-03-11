use clap::Args;
use colored::Colorize;
use harbor_core::relay::crypto::Keypair;
use harbor_core::relay::{RelayConfig, RelayServer};
use harbor_core::HarborError;
use std::path::PathBuf;
use tokio::sync::oneshot;

#[derive(Args)]
pub struct RelayCmdArgs {
    /// Port for QUIC tunnel listener
    #[arg(long, default_value = "7800")]
    pub quic_port: u16,

    /// Port for HTTPS frontend
    #[arg(long, default_value = "8443")]
    pub https_port: u16,

    /// TLS certificate file (PEM)
    #[arg(long)]
    pub tls_cert: Option<PathBuf>,

    /// TLS key file (PEM)
    #[arg(long)]
    pub tls_key: Option<PathBuf>,

    /// Domain for subdomain routing (e.g., relay.example.com)
    #[arg(long)]
    pub domain: Option<String>,

    /// Auth token required from tunnel clients
    #[arg(long)]
    pub auth_token: Option<String>,

    /// Print the relay's public key and exit
    #[arg(long)]
    pub print_key: bool,

    /// File to persist the relay keypair (load if exists, generate+save if not)
    #[arg(long, default_value = "/data/harbor-relay/keypair")]
    pub keypair_file: PathBuf,
}

fn load_or_generate_keypair(path: &std::path::Path) -> Result<Keypair, HarborError> {
    if path.exists() {
        let s = std::fs::read_to_string(path)
            .map_err(|e| HarborError::RelayError(format!("Failed to read keypair file: {e}")))?;
        Keypair::from_file_format(&s)
    } else {
        let kp = Keypair::generate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                HarborError::RelayError(format!("Failed to create keypair directory: {e}"))
            })?;
        }
        std::fs::write(path, kp.to_file_format())
            .map_err(|e| HarborError::RelayError(format!("Failed to save keypair: {e}")))?;
        Ok(kp)
    }
}

pub async fn run(args: RelayCmdArgs) -> Result<(), HarborError> {
    let keypair = load_or_generate_keypair(&args.keypair_file)?;

    let config = RelayConfig {
        quic_port: args.quic_port,
        https_port: args.https_port,
        domain: args.domain.clone(),
        tls_cert: args.tls_cert.map(|p| p.to_string_lossy().to_string()),
        tls_key: args.tls_key.map(|p| p.to_string_lossy().to_string()),
        auth_token: args.auth_token,
        ..RelayConfig::default()
    };

    let server = RelayServer::with_keypair(config, keypair);

    if args.print_key {
        println!("{}", server.public_key_hex());
        return Ok(());
    }

    let domain = args.domain.as_deref().unwrap_or("relay.harbormcp.ai");

    println!("{}", "⚓ Harbor Relay Server".bold());
    println!("  QUIC:   0.0.0.0:{}", args.quic_port);
    println!("  HTTPS:  0.0.0.0:{}", args.https_port);
    println!("  Domain: {}", domain.cyan());
    println!("  Key:    {}", server.public_key_hex().dimmed());
    println!();
    println!("Tunnel clients connect with:");
    println!(
        "  harbor publish --relay {}:{} --relay-key {}",
        domain,
        args.quic_port,
        server.public_key_hex()
    );
    println!();

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Handle Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n{} Shutting down relay...", "⚓".bold());
        let _ = shutdown_tx.send(());
    });

    server.run(shutdown_rx).await?;

    Ok(())
}
