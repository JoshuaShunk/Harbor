//! Publish client — establishes an outbound QUIC tunnel to a relay server
//! and forwards incoming MCP requests to the local Harbor gateway.
//!
//! This is the main runtime for `harbor publish`. It:
//! 1. Connects to the relay via QUIC
//! 2. Performs Noise NK handshake
//! 3. Registers the tunnel (auth, subdomain, ACL)
//! 4. Runs a request loop: receive request -> forward to gateway -> send response
//! 5. Maintains heartbeats to keep the tunnel alive

use crate::error::{HarborError, Result};
use crate::relay::crypto::{HandshakeState, Keypair};
use crate::relay::envelope::{ControlMessage, RelayMessage};
use crate::relay::transport::{PublishInfo, TransportConfig};

use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

/// The publish client — connects to a relay and forwards MCP requests.
pub struct PublishClient {
    config: TransportConfig,
}

impl PublishClient {
    pub fn new(config: TransportConfig) -> Self {
        Self { config }
    }

    /// Run the publish client, sending publish info via channel as soon as
    /// registration succeeds. Blocks until shutdown signal is received.
    pub async fn run_with_info(
        &self,
        shutdown: oneshot::Receiver<()>,
        info_tx: oneshot::Sender<PublishInfo>,
    ) -> Result<PublishInfo> {
        self.run_inner(shutdown, Some(info_tx)).await
    }

    /// Run the publish client. Blocks until shutdown signal is received.
    pub async fn run(&self, shutdown: oneshot::Receiver<()>) -> Result<PublishInfo> {
        self.run_inner(shutdown, None).await
    }

    async fn run_inner(
        &self,
        shutdown: oneshot::Receiver<()>,
        info_tx: Option<oneshot::Sender<PublishInfo>>,
    ) -> Result<PublishInfo> {
        let relay_addr = self
            .config
            .relay_addr
            .as_deref()
            .unwrap_or("relay.harbormcp.ai:7800");

        info!("Connecting to relay at {relay_addr}");

        // Parse relay's public key if provided (for self-hosted relays)
        let relay_public_key = match &self.config.relay_public_key {
            Some(hex_key) => Some(Keypair::public_from_hex(hex_key)?),
            None => None,
        };

        // Rustls 0.23 requires an explicit crypto provider
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Create QUIC client endpoint
        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().map_err(|e| {
            HarborError::TunnelConnectionFailed {
                reason: format!("Invalid bind address: {e}"),
            }
        })?)
        .map_err(|e| HarborError::TunnelConnectionFailed {
            reason: format!("Failed to create QUIC endpoint: {e}"),
        })?;

        // Configure client TLS (accept self-signed certs for relay QUIC transport)
        let mut client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();
        client_crypto.alpn_protocols = vec![b"harbor-relay".to_vec()];

        let client_config = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto).map_err(|e| {
                HarborError::TunnelConnectionFailed {
                    reason: format!("QUIC client config error: {e}"),
                }
            })?,
        ));
        endpoint.set_default_client_config(client_config);

        // Resolve relay address (hostname:port or ip:port)
        let addr_str = if relay_addr.contains(':') {
            relay_addr.to_string()
        } else {
            format!("{relay_addr}:7800")
        };
        let relay_socket = tokio::net::lookup_host(&addr_str)
            .await
            .map_err(|e| HarborError::TunnelConnectionFailed {
                reason: format!("Failed to resolve relay address '{addr_str}': {e}"),
            })?
            .next()
            .ok_or_else(|| HarborError::TunnelConnectionFailed {
                reason: format!("No addresses found for '{addr_str}'"),
            })?;

        // Connect QUIC
        let connection = endpoint
            .connect(relay_socket, "harbor-relay")
            .map_err(|e| HarborError::TunnelConnectionFailed {
                reason: format!("QUIC connect error: {e}"),
            })?
            .await
            .map_err(|e| HarborError::TunnelConnectionFailed {
                reason: format!("QUIC connection failed: {e}"),
            })?;

        info!("QUIC connection established to {relay_addr}");

        // Open control stream
        let (mut send, mut recv) =
            connection
                .open_bi()
                .await
                .map_err(|e| HarborError::TunnelConnectionFailed {
                    reason: format!("Failed to open control stream: {e}"),
                })?;

        // Perform Noise initiator handshake.
        // If no key provided, auto-fetch from the relay's HTTPS info endpoint.
        let relay_pk = match relay_public_key {
            Some(pk) => pk,
            None => {
                let host = relay_addr.split(':').next().unwrap_or(relay_addr);
                let info_url = format!("https://{host}/");
                info!("Fetching relay public key from {info_url}");
                let body: serde_json::Value = reqwest::get(&info_url)
                    .await
                    .map_err(|e| HarborError::TunnelConnectionFailed {
                        reason: format!("Failed to fetch relay info from {info_url}: {e}"),
                    })?
                    .json()
                    .await
                    .map_err(|e| HarborError::TunnelConnectionFailed {
                        reason: format!("Failed to parse relay info: {e}"),
                    })?;
                let hex_key = body
                    .get("public_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| HarborError::TunnelConnectionFailed {
                        reason: "Relay info response missing 'public_key' field".into(),
                    })?;
                Keypair::public_from_hex(hex_key)?
            }
        };

        let mut hs = HandshakeState::initiator(&relay_pk)?;

        // NK pattern: client writes first (-> e, es)
        let msg1 = hs.write_message(b"")?;
        send.write_all(&msg1).await.map_err(|e| {
            HarborError::NoiseHandshakeFailed(format!("Failed to send handshake: {e}"))
        })?;

        // Read relay's response (-> e, ee)
        let mut hs_buf = vec![0u8; 65535];
        let hs_len = recv
            .read(&mut hs_buf)
            .await
            .map_err(|e| HarborError::NoiseHandshakeFailed(format!("Read error: {e}")))?
            .ok_or_else(|| {
                HarborError::NoiseHandshakeFailed("Stream closed during handshake".into())
            })?;

        hs.read_message(&hs_buf[..hs_len])?;

        let cipher = hs.into_transport()?;
        let cipher = Arc::new(Mutex::new(cipher));

        info!("Noise handshake complete");

        // Send registration
        let auth_token = self.config.auth_token.clone().unwrap_or_default();
        let register = ControlMessage::Register {
            auth_token,
            subdomain: self.config.subdomain.clone(),
            version: 1,
            tools: self.config.tools.clone(),
        };
        let reg_bytes = register.encode()?;
        send.write_all(&reg_bytes)
            .await
            .map_err(|e| HarborError::TunnelConnectionFailed {
                reason: format!("Failed to send registration: {e}"),
            })?;

        // Read registration response
        let mut resp_buf = vec![0u8; 65535];
        let resp_len = recv
            .read(&mut resp_buf)
            .await
            .map_err(|e| HarborError::TunnelConnectionFailed {
                reason: format!("Failed to read registration response: {e}"),
            })?
            .ok_or_else(|| HarborError::TunnelConnectionFailed {
                reason: "Stream closed before registration response".into(),
            })?;

        let resp_msg = ControlMessage::decode(&resp_buf[..resp_len])?;

        let (_tunnel_id, _subdomain, public_url, bearer_token) = match resp_msg {
            ControlMessage::Registered {
                tunnel_id,
                subdomain,
                public_url,
                bearer_token,
            } => (tunnel_id, subdomain, public_url, bearer_token),
            ControlMessage::Rejected { reason } => {
                return Err(HarborError::TunnelConnectionFailed { reason });
            }
            _ => {
                return Err(HarborError::TunnelConnectionFailed {
                    reason: "Unexpected response to registration".into(),
                });
            }
        };

        let publish_info = PublishInfo {
            url: public_url.clone(),
            token: bearer_token.clone(),
            transport: "quic".to_string(),
        };

        info!("Published at {public_url}");
        info!("Bearer token: {bearer_token}");

        // Send publish info early if a channel was provided
        if let Some(tx) = info_tx {
            let _ = tx.send(publish_info.clone());
        }

        // Spawn heartbeat task
        let heartbeat_send = connection.clone();
        let heartbeat_shutdown = tokio::sync::watch::channel(false);
        let (heartbeat_tx, heartbeat_rx) = heartbeat_shutdown;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                if *heartbeat_rx.borrow() {
                    break;
                }
                // Send heartbeat on a new unidirectional stream
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let hb = ControlMessage::Heartbeat { timestamp };
                if let Ok(bytes) = hb.encode() {
                    if let Ok(mut s) = heartbeat_send.open_uni().await {
                        let _ = s.write_all(&bytes).await;
                        let _ = s.finish();
                    }
                }
            }
        });

        // Request forwarding loop
        let http_client = reqwest::Client::new();
        let gateway_addr = self.config.gateway_addr.clone();

        let conn = connection.clone();
        let cipher_clone = Arc::clone(&cipher);
        let forward_handle = tokio::spawn(async move {
            loop {
                // Accept incoming bidirectional stream from relay
                let (mut resp_send, mut req_recv) = match conn.accept_bi().await {
                    Ok(streams) => streams,
                    Err(e) => {
                        warn!("Tunnel closed: {e}");
                        break;
                    }
                };

                // Read the request
                let request_bytes = match req_recv.read_to_end(16 * 1024 * 1024).await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!("Failed to read request: {e}");
                        continue;
                    }
                };

                let relay_msg = match RelayMessage::decode(&request_bytes) {
                    Ok(msg) => msg,
                    Err(e) => {
                        warn!("Failed to decode relay message: {e}");
                        continue;
                    }
                };

                // Decrypt the payload
                let decrypted = {
                    let mut c = cipher_clone.lock().await;
                    match c.decrypt(&relay_msg.payload) {
                        Ok(d) => d,
                        Err(e) => {
                            warn!("Failed to decrypt payload: {e}");
                            continue;
                        }
                    }
                };

                // Forward to local gateway
                let gateway_url = format!("{gateway_addr}/mcp");
                let response = match http_client
                    .post(&gateway_url)
                    .header("content-type", "application/json")
                    .body(decrypted)
                    .send()
                    .await
                {
                    Ok(resp) => match resp.bytes().await {
                        Ok(bytes) => bytes.to_vec(),
                        Err(e) => {
                            let err_response = serde_json::json!({
                                "jsonrpc": "2.0",
                                "error": {"code": -32603, "message": format!("Gateway error: {e}")},
                            });
                            serde_json::to_vec(&err_response).unwrap_or_default()
                        }
                    },
                    Err(e) => {
                        let err_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32603, "message": format!("Gateway unreachable: {e}")},
                        });
                        serde_json::to_vec(&err_response).unwrap_or_default()
                    }
                };

                // Encrypt the response
                let encrypted = {
                    let mut c = cipher_clone.lock().await;
                    match c.encrypt(&response) {
                        Ok(e) => e,
                        Err(e) => {
                            warn!("Failed to encrypt response: {e}");
                            continue;
                        }
                    }
                };

                // Send response back through tunnel
                let resp_msg = RelayMessage::response(&relay_msg.envelope, encrypted);
                match resp_msg.encode() {
                    Ok(bytes) => {
                        let _ = resp_send.write_all(&bytes).await;
                        let _ = resp_send.finish();
                    }
                    Err(e) => {
                        warn!("Failed to encode response: {e}");
                    }
                }
            }
        });

        // Wait for shutdown
        let _ = shutdown.await;
        info!("Publish client shutting down");

        // Send disconnect
        let disconnect = ControlMessage::Disconnect;
        if let Ok(bytes) = disconnect.encode() {
            let _ = send.write_all(&bytes).await;
        }

        // Stop heartbeat
        let _ = heartbeat_tx.send(true);

        // Abort forward loop
        forward_handle.abort();

        // Close connection
        connection.close(quinn::VarInt::from_u32(0), b"shutdown");

        Ok(publish_info)
    }
}

/// Custom certificate verifier that accepts any server cert.
/// QUIC TLS is only for transport — Noise protocol handles actual auth.
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
