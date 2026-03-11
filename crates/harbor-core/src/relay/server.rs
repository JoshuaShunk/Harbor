//! Relay server — accepts QUIC tunnels and proxies HTTPS requests to them.
//!
//! The relay server has two listeners:
//! - **QUIC backend** (default port 7800): Accepts outbound connections from
//!   Harbor publish clients. Manages tunnel registration, heartbeats, and
//!   bidirectional request forwarding.
//! - **HTTPS frontend** (default port 443): Accepts requests from remote MCP
//!   clients. Routes by subdomain to the correct tunnel.
//!
//! Self-hostable as a single binary: `harbor relay start`
//! Managed instance runs at relay.harbormcp.ai.

use crate::error::{HarborError, Result};
use crate::relay::acl::AclRules;
use crate::relay::crypto::{HandshakeState, Keypair, TransportCipher};
use crate::relay::envelope::{ControlMessage, RelayMessage};
use crate::relay::token::generate_bearer_token;
use crate::relay::tunnel::{generate_subdomain, TunnelConfig, TunnelState};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, RwLock};
use tracing::{error, info, warn};

/// Configuration for the relay server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// Port for QUIC tunnel listener (default: 7800).
    #[serde(default = "default_quic_port")]
    pub quic_port: u16,

    /// Port for HTTPS frontend (default: 8443).
    #[serde(default = "default_https_port")]
    pub https_port: u16,

    /// Domain for subdomain routing (e.g., "relay.harbormcp.ai").
    pub domain: Option<String>,

    /// TLS certificate file (PEM) for HTTPS frontend.
    pub tls_cert: Option<String>,

    /// TLS key file (PEM) for HTTPS frontend.
    pub tls_key: Option<String>,

    /// Auth token required from tunnel clients (None = open relay).
    pub auth_token: Option<String>,

    /// Tunnel management config.
    #[serde(default)]
    pub tunnel: TunnelConfig,
}

fn default_quic_port() -> u16 {
    7800
}
fn default_https_port() -> u16 {
    8443
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            quic_port: default_quic_port(),
            https_port: default_https_port(),
            domain: None,
            tls_cert: None,
            tls_key: None,
            auth_token: None,
            tunnel: TunnelConfig::default(),
        }
    }
}

/// A registered tunnel on the relay server.
struct TunnelHandle {
    state: TunnelState,
    /// QUIC connection to the tunnel client.
    connection: quinn::Connection,
    /// Noise cipher for encrypting/decrypting payloads.
    cipher: Arc<tokio::sync::Mutex<TransportCipher>>,
}

/// Shared state for the relay server.
struct RelayState {
    tunnels: RwLock<HashMap<String, TunnelHandle>>,
    keypair: Keypair,
    config: RelayConfig,
}

/// The relay server.
pub struct RelayServer {
    config: RelayConfig,
    keypair: Keypair,
}

impl RelayServer {
    /// Create a new relay server, generating a keypair.
    pub fn new(config: RelayConfig) -> Result<Self> {
        let keypair = Keypair::generate()?;
        info!("Relay public key: {}", keypair.public_hex());
        Ok(Self { config, keypair })
    }

    /// Create a relay server with an existing keypair.
    pub fn with_keypair(config: RelayConfig, keypair: Keypair) -> Self {
        Self { config, keypair }
    }

    /// Get the relay's public key (hex-encoded).
    pub fn public_key_hex(&self) -> String {
        self.keypair.public_hex()
    }

    /// Run the relay server (both QUIC + HTTPS listeners).
    pub async fn run(self, shutdown: oneshot::Receiver<()>) -> Result<()> {
        let state = Arc::new(RelayState {
            tunnels: RwLock::new(HashMap::new()),
            keypair: self.keypair,
            config: self.config.clone(),
        });

        // Start QUIC listener
        let quic_state = Arc::clone(&state);
        let quic_addr: SocketAddr = format!("0.0.0.0:{}", self.config.quic_port)
            .parse()
            .map_err(|e| HarborError::RelayError(format!("Invalid QUIC address: {e}")))?;

        let quic_endpoint = create_quic_endpoint(quic_addr)?;
        info!("QUIC listener on {}", quic_addr);

        let quic_handle = tokio::spawn(async move {
            run_quic_listener(quic_endpoint, quic_state).await;
        });

        // Start HTTPS frontend
        let https_state = Arc::clone(&state);
        let https_addr: SocketAddr = format!("0.0.0.0:{}", self.config.https_port)
            .parse()
            .map_err(|e| HarborError::RelayError(format!("Invalid HTTPS address: {e}")))?;

        let app = create_router(https_state);

        info!("HTTPS frontend on {}", https_addr);
        let listener = tokio::net::TcpListener::bind(https_addr)
            .await
            .map_err(HarborError::Io)?;

        let https_handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("HTTPS server error: {e}");
            }
        });

        // Start reaper task
        let reaper_state = Arc::clone(&state);
        let reaper_handle = tokio::spawn(async move {
            run_reaper(reaper_state).await;
        });

        info!("Relay server running");

        // Wait for shutdown
        let _ = shutdown.await;
        info!("Relay server shutting down");

        quic_handle.abort();
        https_handle.abort();
        reaper_handle.abort();

        Ok(())
    }
}

/// Create a self-signed QUIC endpoint for the relay server.
fn create_quic_endpoint(addr: SocketAddr) -> Result<quinn::Endpoint> {
    // Rustls 0.23 requires an explicit crypto provider; install ring if not already set
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Generate a self-signed cert for QUIC transport layer
    // (Noise protocol handles actual authentication, QUIC TLS is just for transport)
    let cert = rcgen::generate_simple_self_signed(vec!["harbor-relay".to_string()])
        .map_err(|e| HarborError::RelayError(format!("Failed to generate cert: {e}")))?;

    let cert_der = cert.cert.der().clone();
    let key_der = cert.key_pair.serialize_der();

    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert_der],
            rustls::pki_types::PrivateKeyDer::Pkcs8(key_der.into()),
        )
        .map_err(|e| HarborError::RelayError(format!("TLS config error: {e}")))?;

    server_crypto.alpn_protocols = vec![b"harbor-relay".to_vec()];

    let server_config = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
            .map_err(|e| HarborError::RelayError(format!("QUIC config error: {e}")))?,
    ));

    let endpoint = quinn::Endpoint::server(server_config, addr)
        .map_err(|e| HarborError::RelayError(format!("Failed to bind QUIC endpoint: {e}")))?;

    Ok(endpoint)
}

/// Run the QUIC listener, accepting tunnel connections.
async fn run_quic_listener(endpoint: quinn::Endpoint, state: Arc<RelayState>) {
    while let Some(incoming) = endpoint.accept().await {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let connecting = match incoming.accept() {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to accept QUIC connection: {e}");
                    return;
                }
            };
            match connecting.await {
                Ok(connection) => {
                    if let Err(e) = handle_tunnel_connection(connection, state).await {
                        warn!("Tunnel connection error: {e}");
                    }
                }
                Err(e) => {
                    warn!("QUIC handshake failed: {e}");
                }
            }
        });
    }
}

/// Handle a new tunnel client connection.
async fn handle_tunnel_connection(
    connection: quinn::Connection,
    state: Arc<RelayState>,
) -> Result<()> {
    let remote = connection.remote_address();
    info!("New tunnel connection from {remote}");

    // Accept the control stream (first bidirectional stream)
    let (mut send, mut recv) =
        connection
            .accept_bi()
            .await
            .map_err(|e| HarborError::TunnelConnectionFailed {
                reason: format!("Failed to accept control stream: {e}"),
            })?;

    // Perform Noise responder handshake
    let mut hs = HandshakeState::responder(&state.keypair)?;

    // Read initiator's first message (-> e, es)
    let mut hs_buf = vec![0u8; 65535];
    let hs_msg = recv
        .read(&mut hs_buf)
        .await
        .map_err(|e| HarborError::NoiseHandshakeFailed(format!("Read error: {e}")))?
        .ok_or_else(|| {
            HarborError::NoiseHandshakeFailed("Stream closed during handshake".into())
        })?;

    hs.read_message(&hs_buf[..hs_msg])?;

    // Write responder message (-> e, ee)
    let resp_msg = hs.write_message(b"")?;
    send.write_all(&resp_msg)
        .await
        .map_err(|e| HarborError::NoiseHandshakeFailed(format!("Write error: {e}")))?;

    let cipher = hs.into_transport()?;
    let cipher = Arc::new(tokio::sync::Mutex::new(cipher));

    // Read registration message
    let mut reg_buf = vec![0u8; 65535];
    let reg_len = recv
        .read(&mut reg_buf)
        .await
        .map_err(|e| HarborError::TunnelConnectionFailed {
            reason: format!("Failed to read registration: {e}"),
        })?
        .ok_or_else(|| HarborError::TunnelConnectionFailed {
            reason: "Stream closed before registration".into(),
        })?;

    let control_msg = ControlMessage::decode(&reg_buf[..reg_len])?;

    let (auth_token, subdomain_req, _version, tools) = match control_msg {
        ControlMessage::Register {
            auth_token,
            subdomain,
            version,
            tools,
        } => (auth_token, subdomain, version, tools),
        _ => {
            return Err(HarborError::TunnelConnectionFailed {
                reason: "Expected Register message".into(),
            });
        }
    };

    // Validate auth token if relay requires one
    if let Some(ref required_token) = state.config.auth_token {
        if auth_token != *required_token {
            let reject = ControlMessage::Rejected {
                reason: "Invalid auth token".into(),
            };
            let reject_bytes = reject.encode()?;
            let _ = send.write_all(&reject_bytes).await;
            return Err(HarborError::TunnelConnectionFailed {
                reason: "Invalid auth token".into(),
            });
        }
    }

    // Assign subdomain
    let subdomain = subdomain_req.unwrap_or_else(generate_subdomain);

    // Check if subdomain is already taken
    {
        let tunnels = state.tunnels.read().await;
        if tunnels.contains_key(&subdomain) {
            let reject = ControlMessage::Rejected {
                reason: format!("Subdomain '{subdomain}' is already in use"),
            };
            let reject_bytes = reject.encode()?;
            let _ = send.write_all(&reject_bytes).await;
            return Err(HarborError::TunnelConnectionFailed {
                reason: format!("Subdomain '{subdomain}' already in use"),
            });
        }
    }

    let tunnel_id = uuid::Uuid::new_v4().to_string();
    let bearer_token = generate_bearer_token();

    let domain = state
        .config
        .domain
        .as_deref()
        .unwrap_or("relay.harbormcp.ai");
    let public_url = format!("https://{subdomain}.{domain}");

    // Build ACL
    let acl = match tools {
        Some(t) => AclRules::allow_only(t),
        None => AclRules::allow_all(),
    };

    // Send Registered response
    let registered = ControlMessage::Registered {
        tunnel_id: tunnel_id.clone(),
        subdomain: subdomain.clone(),
        public_url: public_url.clone(),
        bearer_token: bearer_token.clone(),
    };
    let reg_bytes = registered.encode()?;
    send.write_all(&reg_bytes)
        .await
        .map_err(|e| HarborError::TunnelConnectionFailed {
            reason: format!("Failed to send registration response: {e}"),
        })?;

    info!("Tunnel registered: {subdomain} (id: {tunnel_id}) from {remote}");

    // Insert tunnel into registry
    let tunnel_state = TunnelState {
        tunnel_id: tunnel_id.clone(),
        subdomain: subdomain.clone(),
        created_at: Instant::now(),
        last_heartbeat: Instant::now(),
        acl,
        bearer_token,
    };

    let handle = TunnelHandle {
        state: tunnel_state,
        connection: connection.clone(),
        cipher: Arc::clone(&cipher),
    };

    {
        let mut tunnels = state.tunnels.write().await;
        tunnels.insert(subdomain.clone(), handle);
    }

    // Listen for heartbeats and disconnect on the control stream
    let heartbeat_state = Arc::clone(&state);
    let heartbeat_subdomain = subdomain.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match recv.read(&mut buf).await {
                Ok(Some(n)) => {
                    if let Ok(msg) = ControlMessage::decode(&buf[..n]) {
                        match msg {
                            ControlMessage::Heartbeat { .. } => {
                                let mut tunnels = heartbeat_state.tunnels.write().await;
                                if let Some(handle) = tunnels.get_mut(&heartbeat_subdomain) {
                                    handle.state.heartbeat();
                                }
                            }
                            ControlMessage::Disconnect => {
                                info!("Tunnel {heartbeat_subdomain} disconnected gracefully");
                                let mut tunnels = heartbeat_state.tunnels.write().await;
                                tunnels.remove(&heartbeat_subdomain);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    info!("Tunnel {heartbeat_subdomain} control stream closed");
                    let mut tunnels = heartbeat_state.tunnels.write().await;
                    tunnels.remove(&heartbeat_subdomain);
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Create the HTTPS frontend router.
fn create_router(state: Arc<RelayState>) -> Router {
    Router::new()
        .route("/mcp", post(handle_mcp_request))
        .route("/health", get(handle_health))
        .route("/", get(handle_root))
        .with_state(state)
}

/// Health check endpoint.
async fn handle_health(State(state): State<Arc<RelayState>>) -> Json<serde_json::Value> {
    let tunnels = state.tunnels.read().await;
    Json(serde_json::json!({
        "status": "ok",
        "tunnels": tunnels.len(),
    }))
}

/// Root endpoint — relay info.
async fn handle_root(State(state): State<Arc<RelayState>>) -> Json<serde_json::Value> {
    let tunnels = state.tunnels.read().await;
    Json(serde_json::json!({
        "service": "harbor-relay",
        "version": env!("CARGO_PKG_VERSION"),
        "public_key": state.keypair.public_hex(),
        "active_tunnels": tunnels.len(),
    }))
}

/// Handle an MCP request from a remote client.
///
/// Routing: subdomain extracted from Host header or X-Harbor-Tunnel header.
async fn handle_mcp_request(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    // Extract subdomain from Host header or X-Harbor-Tunnel header
    let host_value = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let subdomain = headers
        .get("x-harbor-tunnel")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .or_else(|| extract_subdomain(host_value, state.config.domain.as_deref()))
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Extract JSON-RPC method and tool name
    let method = body
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");
    let tool_name = body
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str());

    // Look up the tunnel, verify auth, check ACL, encrypt, open stream
    let (connection, msg_bytes) = {
        let tunnels = state.tunnels.read().await;
        let handle = tunnels.get(&subdomain).ok_or(StatusCode::NOT_FOUND)?;

        // Verify bearer token
        let auth_header = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let provided_token = auth_header.strip_prefix("Bearer ").unwrap_or("");

        if provided_token != handle.state.bearer_token {
            return Err(StatusCode::UNAUTHORIZED);
        }

        // Check ACL
        if !handle.state.acl.is_method_allowed(method, tool_name) {
            return Err(StatusCode::FORBIDDEN);
        }

        // Build envelope and encrypt
        let request_id = uuid::Uuid::new_v4().to_string();
        let session_id = headers.get("mcp-session-id").and_then(|v| v.to_str().ok());

        let body_bytes =
            serde_json::to_vec(&body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let encrypted = {
            let mut cipher = handle.cipher.lock().await;
            cipher
                .encrypt(&body_bytes)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        };

        let relay_msg = RelayMessage::request(
            &handle.state.tunnel_id,
            method,
            tool_name,
            session_id,
            &request_id,
            encrypted,
        );

        let msg_bytes = relay_msg
            .encode()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        (handle.connection.clone(), msg_bytes)
    };

    // Open a new bidirectional QUIC stream to the tunnel client
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Send the request
    send.write_all(&msg_bytes)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    send.finish().ok();

    // Read the response
    let response_bytes = recv
        .read_to_end(16 * 1024 * 1024) // 16MB max
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let response_msg =
        RelayMessage::decode(&response_bytes).map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Decrypt the response
    let decrypted = {
        let tunnels = state.tunnels.read().await;
        let handle = tunnels.get(&subdomain).ok_or(StatusCode::NOT_FOUND)?;
        let mut cipher = handle.cipher.lock().await;
        cipher
            .decrypt(&response_msg.payload)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let response_body: serde_json::Value =
        serde_json::from_slice(&decrypted).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(
        "tunnel={subdomain} tool={} status=ok",
        tool_name.unwrap_or(method)
    );

    Ok(Json(response_body))
}

/// Extract subdomain from a host string.
/// e.g., "josh.relay.harbormcp.ai" with domain "relay.harbormcp.ai" -> "josh"
fn extract_subdomain(host: &str, domain: Option<&str>) -> Option<String> {
    let domain = domain?;
    // Remove port if present
    let host = host.split(':').next().unwrap_or(host);
    let suffix = format!(".{domain}");
    if host.ends_with(&suffix) {
        let subdomain = &host[..host.len() - suffix.len()];
        if !subdomain.is_empty() && !subdomain.contains('.') {
            return Some(subdomain.to_string());
        }
    }
    None
}

/// Reaper task — removes expired tunnels.
async fn run_reaper(state: Arc<RelayState>) {
    let timeout = state.config.tunnel.heartbeat_timeout_secs;
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut tunnels = state.tunnels.write().await;
        let expired: Vec<String> = tunnels
            .iter()
            .filter(|(_, h)| h.state.is_expired(timeout))
            .map(|(k, _)| k.clone())
            .collect();

        for subdomain in &expired {
            if let Some(handle) = tunnels.remove(subdomain) {
                info!(
                    "Reaped expired tunnel: {} (id: {})",
                    subdomain, handle.state.tunnel_id
                );
                handle
                    .connection
                    .close(quinn::VarInt::from_u32(0), b"expired");
            }
        }

        if !expired.is_empty() {
            info!("Reaped {} expired tunnel(s)", expired.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_subdomain() {
        assert_eq!(
            extract_subdomain("josh.relay.harbormcp.ai", Some("relay.harbormcp.ai")),
            Some("josh".to_string())
        );
        assert_eq!(
            extract_subdomain("relay.harbormcp.ai", Some("relay.harbormcp.ai")),
            None
        );
        assert_eq!(
            extract_subdomain("josh.relay.harbormcp.ai:8443", Some("relay.harbormcp.ai")),
            Some("josh".to_string())
        );
        assert_eq!(
            extract_subdomain("unrelated.com", Some("relay.harbormcp.ai")),
            None
        );
    }

    #[test]
    fn test_default_config() {
        let config = RelayConfig::default();
        assert_eq!(config.quic_port, 7800);
        assert_eq!(config.https_port, 8443);
        assert!(config.domain.is_none());
    }
}
