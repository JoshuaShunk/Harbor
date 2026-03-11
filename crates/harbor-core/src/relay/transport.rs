//! Pluggable transport abstraction for Harbor publish.
//!
//! Transports establish and maintain a tunnel between the local Harbor gateway
//! and a relay server, forwarding MCP requests from remote clients.

use crate::error::Result;
use crate::relay::envelope::RelayMessage;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Info returned after successfully publishing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishInfo {
    /// Public URL for remote MCP clients (e.g., "https://josh.harbormcp.ai").
    pub url: String,
    /// Bearer token that remote clients must include.
    pub token: String,
    /// Transport type used ("quic", "cloudflare", etc.).
    pub transport: String,
}

/// Configuration for establishing a transport.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Local gateway address (default: "http://127.0.0.1:3100").
    pub gateway_addr: String,
    /// Relay server address (for QUIC transport).
    pub relay_addr: Option<String>,
    /// Authentication token for the relay.
    pub auth_token: Option<String>,
    /// Requested subdomain (None = auto-assigned).
    pub subdomain: Option<String>,
    /// Relay's Noise public key (hex-encoded, for self-hosted relay verification).
    pub relay_public_key: Option<String>,
    /// Tools to expose remotely (None = all).
    pub tools: Option<Vec<String>>,
    /// Local gateway port.
    pub gateway_port: u16,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            gateway_addr: "http://127.0.0.1:3100".to_string(),
            relay_addr: None,
            auth_token: None,
            subdomain: None,
            relay_public_key: None,
            tools: None,
            gateway_port: 3100,
        }
    }
}

/// Trait for tunnel transports.
///
/// Implementations handle the tunnel lifecycle (connect, receive requests,
/// send responses, disconnect) while the `PublishClient` handles the
/// gateway forwarding logic.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Establish the tunnel to the relay, returning publish info.
    async fn connect(&mut self, config: &TransportConfig) -> Result<PublishInfo>;

    /// Shut down the tunnel gracefully.
    async fn disconnect(&mut self) -> Result<()>;

    /// Whether the transport is currently connected.
    fn is_connected(&self) -> bool;

    /// Human-readable transport name.
    fn name(&self) -> &str;

    /// Receive the next incoming request from the relay.
    /// Returns None if the tunnel has been closed.
    async fn next_request(&mut self) -> Result<Option<RelayMessage>>;

    /// Send a response back through the tunnel.
    async fn send_response(&mut self, msg: RelayMessage) -> Result<()>;
}
