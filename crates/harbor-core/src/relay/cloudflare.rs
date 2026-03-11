//! Cloudflare Tunnel transport — delegates tunneling to `cloudflared`.
//!
//! This is a lightweight transport that spawns the `cloudflared` binary
//! and parses its output for the assigned URL. The actual proxying is
//! handled entirely by `cloudflared` — Harbor just ensures the gateway
//! has a bearer token set.
//!
//! Usage:
//! ```sh
//! harbor publish --transport cloudflare
//! ```
//!
//! Prerequisites:
//! - `cloudflared` must be installed and on PATH
//! - For quick tunnels: no account needed (uses trycloudflare.com)
//! - For persistent tunnels: requires Cloudflare account + tunnel token

use crate::error::{HarborError, Result};
use crate::relay::envelope::RelayMessage;
use crate::relay::token::generate_bearer_token;
use crate::relay::transport::{PublishInfo, Transport, TransportConfig};

use async_trait::async_trait;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tracing::info;

/// Cloudflare Tunnel transport.
pub struct CloudflareTransport {
    process: Option<tokio::process::Child>,
    public_url: Option<String>,
    connected: bool,
}

impl CloudflareTransport {
    pub fn new() -> Self {
        Self {
            process: None,
            public_url: None,
            connected: false,
        }
    }
}

#[async_trait]
impl Transport for CloudflareTransport {
    async fn connect(&mut self, config: &TransportConfig) -> Result<PublishInfo> {
        // Check if cloudflared is installed
        let which = Command::new("which")
            .arg("cloudflared")
            .output()
            .await
            .map_err(HarborError::Io)?;

        if !which.status.success() {
            return Err(HarborError::RelayError(
                "cloudflared not found on PATH. Install it from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/".to_string(),
            ));
        }

        let port = config.gateway_port;
        let local_url = format!("http://127.0.0.1:{port}");

        info!("Starting cloudflared tunnel to {local_url}");

        // Spawn cloudflared
        let mut child = Command::new("cloudflared")
            .args(["tunnel", "--url", &local_url, "--no-autoupdate"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| HarborError::RelayError(format!("Failed to start cloudflared: {e}")))?;

        // Parse stderr for the assigned URL
        // cloudflared prints: "Your quick Tunnel has been created! Visit it at (URL)"
        // or on newer versions: "https://xxx.trycloudflare.com" in the INF log lines
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| HarborError::RelayError("Failed to capture cloudflared stderr".into()))?;

        let mut reader = tokio::io::BufReader::new(stderr).lines();

        // Read lines with a timeout
        let url_result = tokio::time::timeout(Duration::from_secs(30), async {
            while let Ok(Some(line)) = reader.next_line().await {
                // Look for the URL in various formats cloudflared uses
                if let Some(url) = extract_cloudflare_url(&line) {
                    return Some(url);
                }
            }
            None
        })
        .await;

        let url = match url_result {
            Ok(Some(url)) => url,
            Ok(None) => {
                child.kill().await.ok();
                return Err(HarborError::RelayError(
                    "cloudflared exited without providing a URL".to_string(),
                ));
            }
            Err(_) => {
                child.kill().await.ok();
                return Err(HarborError::RelayError(
                    "Timed out waiting for cloudflared URL".to_string(),
                ));
            }
        };
        let token = generate_bearer_token();

        info!("Cloudflare tunnel active: {url}");

        self.process = Some(child);
        self.public_url = Some(url.clone());
        self.connected = true;

        Ok(PublishInfo {
            url,
            token,
            transport: "cloudflare".to_string(),
        })
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            info!("Stopping cloudflared tunnel");
            child.kill().await.map_err(HarborError::Io)?;
        }
        self.connected = false;
        self.public_url = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn name(&self) -> &str {
        "cloudflare"
    }

    /// Not used by CloudflareTransport — cloudflared handles proxying directly.
    async fn next_request(&mut self) -> Result<Option<RelayMessage>> {
        Err(HarborError::RelayError(
            "CloudflareTransport does not handle requests directly".to_string(),
        ))
    }

    /// Not used by CloudflareTransport — cloudflared handles proxying directly.
    async fn send_response(&mut self, _msg: RelayMessage) -> Result<()> {
        Err(HarborError::RelayError(
            "CloudflareTransport does not handle responses directly".to_string(),
        ))
    }
}

use std::time::Duration;

/// Extract a cloudflare tunnel URL from a log line.
fn extract_cloudflare_url(line: &str) -> Option<String> {
    // Pattern 1: "https://xxx.trycloudflare.com"
    if let Some(start) = line.find("https://") {
        let rest = &line[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(rest.len());
        let url = &rest[..end];
        if url.contains("trycloudflare.com") || url.contains("cloudflare") {
            return Some(url.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cloudflare_url() {
        assert_eq!(
            extract_cloudflare_url(
                "2026-03-10 INF +-------------------------------------------------------+"
            ),
            None
        );
        assert_eq!(
            extract_cloudflare_url(
                "2026-03-10 INF |  https://random-words-here.trycloudflare.com          |"
            ),
            Some("https://random-words-here.trycloudflare.com".to_string())
        );
        assert_eq!(
            extract_cloudflare_url("Visit it at https://abc123.trycloudflare.com"),
            Some("https://abc123.trycloudflare.com".to_string())
        );
    }
}
