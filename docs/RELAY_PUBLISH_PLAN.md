# Harbor Relay & Publish — Production Implementation Plan

## Overview

Securely publish your local Harbor gateway to the internet so off-network devices can
connect to your MCP servers. Three deployment modes:

1. **`harbor publish`** — zero-config via managed relay at harbormcp.ai
2. **`harbor publish --relay my-server.com`** — self-hosted relay (same binary)
3. **`harbor publish --transport cloudflare`** — pluggable transport (Cloudflare Tunnel, etc.)

The relay is MCP-aware: it understands tool listings, enforces tool-level ACL, and uses
E2E encryption so the relay cannot read JSON-RPC payloads.

---

## Architecture

```
Remote MCP Client (Claude Code, phone, laptop on another network)
        |
        | HTTPS (Streamable HTTP / JSON-RPC)
        v
+---------------------------------------------------------------+
|              Harbor Relay                                      |
|              (harbormcp.ai  OR  self-hosted `harbor relay`)    |
|                                                                |
|  TLS Termination -> Auth (JWT/Bearer) -> Tool ACL -> Rate Lim |
|                                                                |
|  Tunnel Registry: maps subdomain -> active QUIC tunnel         |
+---------------------------------------------------------------+
        |
        | QUIC + Noise Protocol (outbound-only from client)
        | E2E encrypted — relay is a blind pipe for payloads
        v
+---------------------------------------------------------------+
|              Harbor Gateway (user's machine, localhost:3100)   |
|                                                                |
|  BridgeManager -> StdioBridge / HttpBridge -> MCP Servers      |
+---------------------------------------------------------------+
```

### Envelope Protocol

The relay sees a thin envelope but NOT the encrypted payload:

```
+----------------------------------+
| Envelope (plaintext to relay)    |
|   tunnel_id: "abc123"           |
|   session_id: "mcp-sess-xyz"   | <- for session affinity
|   method: "tools/call"          | <- for ACL decisions
|   tool_name: "get_issues"       | <- for tool-level ACL
+----------------------------------+
| Payload (Noise-encrypted)        |
|   Full JSON-RPC request body     |
|   Tool arguments, results        |
+----------------------------------+
```

The relay can:
- Route by tunnel_id/subdomain
- Enforce tool-level access control (which tools are exposed remotely)
- Rate limit per tunnel/tool
- Track session affinity

The relay CANNOT:
- Read tool arguments or results
- See vault-resolved secrets
- Modify payloads without detection

---

## Crate Structure

All new code lives in `harbor-core`. No new crates needed.

```
crates/harbor-core/src/
  relay/
    mod.rs          — Public API: RelayServer, RelayClient
    server.rs       — Relay server (axum HTTP frontend + quinn QUIC backend)
    client.rs       — Tunnel client (outbound QUIC to relay)
    tunnel.rs       — Tunnel lifecycle: register, heartbeat, teardown
    envelope.rs     — Envelope protocol: serialize/deserialize, encrypt/decrypt
    crypto.rs       — Noise protocol wrapper (snow crate)
    acl.rs          — Tool-level access control for remote clients
    token.rs        — JWT/Bearer token generation, validation, scoping
    transport.rs    — Transport trait + implementations
    cloudflare.rs   — CloudflareTransport (shells to cloudflared)
```

---

## Phase 1: Core Tunnel Protocol

### 1.1 Noise Crypto Layer (`relay/crypto.rs`)

Wraps the `snow` crate for Noise_NK_25519_ChaChaPoly_BLAKE2s.

```rust
pub struct NoiseCrypto {
    // ...
}

impl NoiseCrypto {
    /// Generate a new static keypair for the relay or client.
    pub fn generate_keypair() -> Keypair;

    /// Initiator (client) handshake — knows the relay's public key.
    pub fn handshake_initiator(
        relay_public_key: &[u8; 32],
        client_keypair: &Keypair,
    ) -> Result<HandshakeState>;

    /// Responder (relay) handshake.
    pub fn handshake_responder(
        relay_keypair: &Keypair,
    ) -> Result<HandshakeState>;

    /// Complete handshake, return transport-mode cipher.
    pub fn into_transport(state: HandshakeState) -> Result<TransportCipher>;
}

pub struct TransportCipher { /* snow TransportState */ }

impl TransportCipher {
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>>;
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>>;
}
```

**Dependencies:** `snow = "0.9"`

### 1.2 Envelope Protocol (`relay/envelope.rs`)

Defines the wire format between relay and client over QUIC streams.

```rust
/// Metadata visible to the relay (NOT encrypted).
#[derive(Serialize, Deserialize)]
pub struct Envelope {
    /// Unique tunnel identifier
    pub tunnel_id: String,
    /// MCP session ID for affinity routing
    pub session_id: Option<String>,
    /// JSON-RPC method (tools/list, tools/call, etc.)
    pub method: String,
    /// Tool name (for ACL — only present on tools/call)
    pub tool_name: Option<String>,
    /// Unique request ID for correlating response
    pub request_id: String,
    /// Direction: Request or Response
    pub direction: Direction,
}

#[derive(Serialize, Deserialize)]
pub enum Direction { Request, Response }

/// A complete message: envelope + encrypted payload.
pub struct RelayMessage {
    pub envelope: Envelope,
    /// Noise-encrypted JSON-RPC body
    pub payload: Vec<u8>,
}

impl RelayMessage {
    /// Serialize for sending over QUIC stream.
    /// Format: [4-byte envelope length][envelope JSON][encrypted payload]
    pub fn encode(&self) -> Vec<u8>;

    /// Deserialize from QUIC stream bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self>;
}
```

### 1.3 Transport Trait (`relay/transport.rs`)

Pluggable transport abstraction.

```rust
/// Info returned after successfully publishing.
#[derive(Debug, Clone, Serialize)]
pub struct PublishInfo {
    /// Public URL for remote MCP clients
    pub url: String,
    /// Bearer token for authentication
    pub token: String,
    /// Transport type used
    pub transport: String,
}

/// Trait for tunnel transports.
#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Establish outbound tunnel, return public URL info.
    async fn connect(&mut self, config: &TransportConfig) -> Result<PublishInfo>;

    /// Shut down the tunnel gracefully.
    async fn disconnect(&mut self) -> Result<()>;

    /// Whether the transport is currently connected.
    fn is_connected(&self) -> bool;

    /// Human-readable transport name.
    fn name(&self) -> &str;

    /// Handle an incoming request from the relay and forward to local gateway.
    /// This is called in a loop by the publish runtime.
    async fn next_request(&mut self) -> Result<Option<RelayMessage>>;

    /// Send a response back through the tunnel.
    async fn send_response(&mut self, msg: RelayMessage) -> Result<()>;
}

pub struct TransportConfig {
    /// Local gateway address (default: http://127.0.0.1:3100)
    pub gateway_addr: String,
    /// Relay server address (for QUIC transport)
    pub relay_addr: Option<String>,
    /// Authentication token for the relay
    pub auth_token: Option<String>,
    /// Requested subdomain (None = auto-assigned)
    pub subdomain: Option<String>,
    /// Relay's public key (for Noise handshake, self-hosted mode)
    pub relay_public_key: Option<[u8; 32]>,
}
```

### 1.4 QUIC Transport (`relay/client.rs`)

The built-in transport using QUIC + Noise.

```rust
pub struct QuicTransport {
    connection: Option<quinn::Connection>,
    cipher: Option<TransportCipher>,
    tunnel_id: String,
    connected: bool,
}

impl Transport for QuicTransport {
    async fn connect(&mut self, config: &TransportConfig) -> Result<PublishInfo> {
        // 1. Resolve relay address
        // 2. Establish QUIC connection (quinn)
        // 3. Open control stream
        // 4. Perform Noise NK handshake
        // 5. Send RegisterTunnel message (auth token, requested subdomain)
        // 6. Receive RegisterResponse (assigned subdomain, public URL)
        // 7. Start heartbeat task (30s interval)
        // 8. Return PublishInfo
    }

    async fn next_request(&mut self) -> Result<Option<RelayMessage>> {
        // Accept incoming bidirectional QUIC stream from relay
        // Read RelayMessage from stream
        // Decrypt payload using Noise cipher
        // Return decrypted message
    }

    async fn send_response(&mut self, msg: RelayMessage) -> Result<()> {
        // Encrypt payload using Noise cipher
        // Write RelayMessage to the stream
    }

    async fn disconnect(&mut self) -> Result<()> {
        // Send Disconnect on control stream
        // Close QUIC connection
    }
}
```

**Dependencies:** `quinn = "0.11"`, `rustls = "0.23"`

### 1.5 Relay Server (`relay/server.rs`)

The self-hostable relay — a standalone axum server with QUIC backend.

```rust
pub struct RelayServer {
    /// QUIC listener for tunnel clients (port 7800 default)
    quic_port: u16,
    /// HTTPS listener for remote MCP clients (port 443 default)
    https_port: u16,
    /// TLS config for HTTPS frontend
    tls_config: Option<TlsConfig>,
    /// Active tunnels: subdomain -> tunnel handle
    tunnels: Arc<RwLock<HashMap<String, TunnelHandle>>>,
    /// Server keypair for Noise handshake
    keypair: Keypair,
    /// ACL rules (loaded from config or API)
    acl: Arc<RwLock<AclRules>>,
}

struct TunnelHandle {
    tunnel_id: String,
    subdomain: String,
    connection: quinn::Connection,
    cipher: TransportCipher,
    created_at: Instant,
    last_heartbeat: Instant,
    /// Tool-level ACL for this tunnel
    allowed_tools: Option<Vec<String>>,
}

impl RelayServer {
    pub fn new(config: RelayConfig) -> Self;

    /// Start the relay server (both QUIC + HTTPS listeners).
    pub async fn run(self, shutdown: oneshot::Receiver<()>) -> Result<()> {
        // 1. Start QUIC listener for tunnel clients
        // 2. Start HTTPS server for remote MCP clients
        // 3. Start reaper task (remove dead tunnels every 60s)
        // 4. Wait for shutdown signal
    }
}

// --- QUIC backend (tunnel management) ---

async fn handle_tunnel_connection(
    connection: quinn::Connection,
    tunnels: Arc<RwLock<HashMap<String, TunnelHandle>>>,
    keypair: &Keypair,
) -> Result<()> {
    // 1. Accept control stream
    // 2. Perform Noise responder handshake
    // 3. Read RegisterTunnel message
    // 4. Validate auth token
    // 5. Assign subdomain (requested or random)
    // 6. Insert TunnelHandle into registry
    // 7. Send RegisterResponse with public URL
    // 8. Listen for heartbeats on control stream
}

// --- HTTPS frontend (remote MCP clients) ---

// Routes:
//   POST https://{subdomain}.relay.example.com/mcp    -> forward to tunnel
//   GET  https://{subdomain}.relay.example.com/health  -> tunnel health
//   POST https://relay.example.com/mcp                 -> route by header

async fn handle_mcp_request(
    State(state): State<Arc<RelayState>>,
    host: axum::extract::Host,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 1. Extract subdomain from Host header
    // 2. Lookup tunnel in registry
    // 3. Verify Bearer token
    // 4. Extract method + tool_name from JSON-RPC body
    // 5. Check ACL (is this tool allowed for remote access?)
    // 6. Build Envelope (method, tool_name, session_id)
    // 7. Encrypt JSON-RPC body with tunnel's Noise cipher
    // 8. Open new QUIC stream to tunnel client
    // 9. Send RelayMessage, wait for response
    // 10. Decrypt response payload
    // 11. Return JSON-RPC response to remote client
}
```

### 1.6 Relay CLI Commands

New CLI command: `harbor publish` (alias: `broadcast`)

```rust
// In commands/mod.rs:
/// Broadcast your gateway to the high seas (publish to relay)
#[command(alias = "broadcast")]
Publish(publish::PublishArgs),

// In commands/publish.rs:
#[derive(Args)]
pub struct PublishArgs {
    /// Relay server address (default: harbormcp.ai)
    #[arg(long, default_value = "harbormcp.ai")]
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
    /// If not set, all tools are exposed remotely.
    #[arg(long, value_delimiter = ',')]
    pub tools: Option<Vec<String>>,

    /// Stop publishing
    #[arg(long)]
    pub stop: bool,
}
```

New CLI command: `harbor relay` (repurpose existing hidden command)

The existing `Relay` command (alias `proxy`) is the stdio MCP proxy.
Rename that to `Proxy` and use `Relay` for the relay server:

```rust
/// Run the Harbor relay server (self-hosted)
#[command(alias = "relay-server")]
Relay(relay_cmd::RelayArgs),

/// Run as an MCP stdio proxy through the Harbor gateway
#[command(alias = "proxy", hide = true)]
Proxy(proxy::ProxyArgs),

// In commands/relay_cmd.rs:
#[derive(Args)]
pub struct RelayArgs {
    /// Port for QUIC tunnel listener
    #[arg(long, default_value = "7800")]
    pub quic_port: u16,

    /// Port for HTTPS frontend
    #[arg(long, default_value = "443")]
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

    /// Print the relay's public key (for client pinning)
    #[arg(long)]
    pub print_key: bool,
}
```

### 1.7 Config Changes

Add publish settings to `HarborSettings`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarborSettings {
    // ... existing fields ...

    /// Relay server address for publishing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_relay: Option<String>,

    /// Requested subdomain for publishing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_subdomain: Option<String>,

    /// Auth token for the relay (can use vault: prefix)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_token: Option<String>,

    /// Tools exposed remotely (None = all, Some([]) = none)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_tools: Option<Vec<String>>,

    /// Auto-publish when gateway starts
    #[serde(default)]
    pub publish_auto: bool,

    /// Relay's public key for Noise handshake (hex-encoded, for self-hosted)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_relay_key: Option<String>,
}
```

Example `~/.harbor/config.toml`:
```toml
[harbor]
gateway_port = 3100
gateway_host = "127.0.0.1"
gateway_token = "vault:gateway_bearer_token"

# Publishing (optional)
publish_relay = "harbormcp.ai"          # or "my-relay.example.com"
publish_subdomain = "josh"              # -> josh.harbormcp.ai
publish_token = "vault:relay_auth"
publish_auto = true                     # auto-publish on gateway start
publish_tools = ["get_issues", "search"] # only expose these tools remotely
```

---

## Phase 2: Desktop App Integration

### 2.1 New Tauri Commands

```rust
// In commands.rs:

#[tauri::command]
async fn publish_start(
    state: State<'_, AppState>,
    relay: Option<String>,
    subdomain: Option<String>,
) -> Result<PublishInfo, String>;

#[tauri::command]
async fn publish_stop(
    state: State<'_, AppState>,
) -> Result<(), String>;

#[tauri::command]
async fn publish_status(
    state: State<'_, AppState>,
) -> Result<Option<PublishInfo>, String>;

#[tauri::command]
async fn get_publish_settings(
    state: State<'_, AppState>,
) -> Result<PublishSettings, String>;

#[tauri::command]
async fn set_publish_settings(
    state: State<'_, AppState>,
    settings: PublishSettings,
) -> Result<(), String>;
```

### 2.2 UI Changes (Lighthouse Page)

Add a "Publish" section to the existing Lighthouse page:

```
+--------------------------------------------------+
| Lighthouse                                        |
|                                                   |
| [Gateway Toggle: Running]                         |
| Endpoint: http://127.0.0.1:3100                  |
|                                                   |
| --- Publish to Internet ---                       |
|                                                   |
| [Toggle: Published]                    [Stop]     |
|                                                   |
| Public URL: https://josh.harbormcp.ai             |
|             [Copy]                                 |
|                                                   |
| Bearer Token: hbr_k8x9...  [Copy] [Regenerate]   |
|                                                   |
| Relay: harbormcp.ai (managed)                     |
| Transport: QUIC + Noise (E2E encrypted)           |
|                                                   |
| Remote Tool Access:                               |
|   [x] All tools    ( ) Selected tools only        |
|   If selected: [tool picker from gateway tools]   |
|                                                   |
| --- Connection Log ---                            |
| 12:34:05 Connected to relay                       |
| 12:34:06 Subdomain assigned: josh.harbormcp.ai    |
| 12:35:12 Remote call: get_issues (1.2s)           |
+--------------------------------------------------+
```

### 2.3 Tauri Event: `publish-status-changed`

```rust
#[derive(Serialize, Clone)]
struct PublishStatusEvent {
    published: bool,
    url: Option<String>,
    transport: Option<String>,
}
```

Emitted when publish state changes. Frontend listens and updates UI.

### 2.4 Auto-Publish on Gateway Start

If `publish_auto = true` in config, the desktop app's `start_gateway` command
also starts publishing. The gateway start flow becomes:

1. Start gateway HTTP server
2. Start MCP servers in background
3. If `publish_auto`, start publish transport in background
4. Emit `publish-status-changed` event

### 2.5 Frontend TypeScript Types

```typescript
// In lib/tauri.ts:

interface PublishInfo {
  url: string;
  token: string;
  transport: string;
}

interface PublishSettings {
  relay: string | null;
  subdomain: string | null;
  token: string | null;
  auto_publish: boolean;
  tools: string[] | null;  // null = all tools
}

export async function publishStart(
  relay?: string,
  subdomain?: string
): Promise<PublishInfo>;

export async function publishStop(): Promise<void>;

export async function publishStatus(): Promise<PublishInfo | null>;

export async function getPublishSettings(): Promise<PublishSettings>;

export async function setPublishSettings(
  settings: PublishSettings
): Promise<void>;
```

---

## Phase 3: Cloudflare Transport Plugin

### 3.1 CloudflareTransport (`relay/cloudflare.rs`)

```rust
pub struct CloudflareTransport {
    process: Option<tokio::process::Child>,
    public_url: Option<String>,
    connected: bool,
}

impl Transport for CloudflareTransport {
    async fn connect(&mut self, config: &TransportConfig) -> Result<PublishInfo> {
        // 1. Check if cloudflared is installed
        // 2. Spawn: cloudflared tunnel --url http://127.0.0.1:{port} --no-autoupdate
        // 3. Parse stdout for the assigned URL (*.trycloudflare.com)
        // 4. Generate bearer token if not set
        // 5. Return PublishInfo
    }

    async fn disconnect(&mut self) -> Result<()> {
        // Kill cloudflared process
    }

    // next_request/send_response: N/A for cloudflare transport
    // (cloudflared handles proxying directly — not routed through our code)
}
```

Note: Cloudflare transport is simpler because `cloudflared` handles the full
proxy chain. We just need to ensure the gateway has a bearer token set.

### 3.2 CLI Usage

```sh
# Quick tunnel via Cloudflare (no account needed):
harbor publish --transport cloudflare

# With a managed Cloudflare tunnel (requires account):
harbor publish --transport cloudflare --token <cf-tunnel-token>
```

---

## Phase 4: Relay Server Clustering

For production harbormcp.ai deployment. Not needed for self-hosted single-node.

### 4.1 Gossip Protocol

Use a gossip-based protocol (similar to Piko) for cluster state:

- Each node maintains a local tunnel registry
- Tunnel registrations gossip to all nodes via UDP
- Any node can accept remote MCP requests and route to the correct node
- Consistent hashing for tunnel placement

**Dependencies:** `chitchat = "0.8"` (Quickwit's gossip library, Rust-native)

### 4.2 Session Affinity

For stateful MCP sessions:
- First request with a new `Mcp-Session-Id` gets routed to any node
- That node records the affinity: session_id -> node_id
- Subsequent requests with the same session_id route to the same node
- Affinity entries expire after 30 minutes of inactivity

### 4.3 Relay Deployment

```yaml
# Kubernetes StatefulSet
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: harbor-relay
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: relay
          image: ghcr.io/joshuashunk/harbor-relay:latest
          args:
            - relay
            - --quic-port=7800
            - --https-port=8443
            - --domain=harbormcp.ai
            - --cluster
            - --gossip-port=7801
          ports:
            - containerPort: 7800  # QUIC
              protocol: UDP
            - containerPort: 8443  # HTTPS
            - containerPort: 7801  # Gossip
              protocol: UDP
```

---

## Phase 5: Security Hardening

### 5.1 Token Scoping

```rust
/// JWT claims for relay access tokens.
#[derive(Serialize, Deserialize)]
pub struct RelayTokenClaims {
    /// Subject: tunnel owner identifier
    pub sub: String,
    /// Tunnel ID this token is valid for
    pub tunnel_id: String,
    /// Allowed tools (None = all)
    pub tools: Option<Vec<String>>,
    /// Expiration time (Unix timestamp)
    pub exp: u64,
    /// Issued at
    pub iat: u64,
    /// Token ID (for revocation)
    pub jti: String,
}
```

Users can generate scoped tokens for specific devices:
```sh
# Full access token (default):
harbor publish token generate

# Scoped to specific tools, expires in 24h:
harbor publish token generate --tools get_issues,search --expires 24h

# Revoke a token:
harbor publish token revoke <jti>
```

### 5.2 Rate Limiting

Per-tunnel and per-tool rate limits at the relay:

```rust
pub struct RateLimits {
    /// Max requests per second per tunnel
    pub tunnel_rps: u32,        // default: 100
    /// Max requests per second per tool
    pub tool_rps: u32,          // default: 20
    /// Max concurrent requests per tunnel
    pub tunnel_concurrency: u32, // default: 50
    /// Max request body size (bytes)
    pub max_body_size: usize,    // default: 10MB
}
```

### 5.3 Audit Logging

The relay logs all remote tool calls (without payloads):

```
2026-03-07T12:34:56Z tunnel=josh tool=get_issues latency=1.2s status=ok
2026-03-07T12:35:01Z tunnel=josh tool=create_issue latency=0.8s status=ok
2026-03-07T12:35:05Z tunnel=josh tool=delete_repo latency=0ms status=acl_denied
```

---

## Dependencies to Add

### harbor-core/Cargo.toml

```toml
# Phase 1: Core tunnel
quinn = "0.11"                    # QUIC transport
rustls = { version = "0.23", features = ["ring"] }  # TLS for QUIC
snow = "0.9"                      # Noise Protocol Framework
rcgen = "0.13"                    # Self-signed cert generation (for QUIC)

# Phase 4: Clustering (optional feature)
chitchat = { version = "0.8", optional = true }

# Phase 5: JWT tokens
jsonwebtoken = { version = "9", optional = true }
```

### Workspace Cargo.toml

```toml
[workspace.dependencies]
quinn = "0.11"
snow = "0.9"
```

### Feature Flags

```toml
[features]
default = ["relay"]
relay = ["quinn", "snow", "rcgen"]
relay-cluster = ["relay", "chitchat"]
relay-jwt = ["relay", "jsonwebtoken"]
cloudflare-transport = []  # no extra deps, shells to cloudflared
```

---

## Error Types

Add to `error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum HarborError {
    // ... existing variants ...

    #[error("Relay error: {0}")]
    RelayError(String),

    #[error("Tunnel connection failed: {reason}")]
    TunnelConnectionFailed { reason: String },

    #[error("Tunnel not found: {subdomain}")]
    TunnelNotFound { subdomain: String },

    #[error("Tool not allowed for remote access: {tool}")]
    RemoteToolDenied { tool: String },

    #[error("Noise handshake failed: {0}")]
    NoiseHandshakeFailed(String),

    #[error("Publish not active")]
    PublishNotActive,
}
```

---

## Testing Strategy

### Unit Tests

- `relay/crypto.rs` — Noise handshake roundtrip, encrypt/decrypt
- `relay/envelope.rs` — Serialize/deserialize, encode/decode
- `relay/acl.rs` — Tool ACL logic (mirrors existing `tool_allowed` tests)
- `relay/token.rs` — JWT generation, validation, expiry, scoping

### Integration Tests

- Start relay server + tunnel client in-process
- Verify tool listing through tunnel
- Verify tool call forwarding through tunnel
- Verify E2E encryption (relay can't read payloads)
- Verify ACL enforcement (blocked tools return error)
- Verify reconnection after network drop
- Verify heartbeat timeout (tunnel cleanup)

### End-to-End Tests

- Full flow: `harbor lighthouse` + `harbor publish` + remote `curl` to subdomain
- Cloudflare transport: `harbor publish --transport cloudflare` + remote access
- Self-hosted relay: `harbor relay start` + `harbor publish --relay localhost:7800`

---

## Implementation Order

| Step | What | Files | Est. |
|------|------|-------|------|
| 1 | Crypto layer (Noise) | `relay/crypto.rs` | 1 day |
| 2 | Envelope protocol | `relay/envelope.rs` | 1 day |
| 3 | Transport trait | `relay/transport.rs` | 0.5 day |
| 4 | QUIC tunnel client | `relay/client.rs` | 3 days |
| 5 | Relay server (single-node) | `relay/server.rs` | 4 days |
| 6 | Tunnel lifecycle (register, heartbeat, teardown) | `relay/tunnel.rs` | 2 days |
| 7 | Tool ACL | `relay/acl.rs` | 1 day |
| 8 | Token generation/validation | `relay/token.rs` | 1 day |
| 9 | CLI: `harbor publish` command | `cli/commands/publish.rs` | 1 day |
| 10 | CLI: `harbor relay` command | `cli/commands/relay_cmd.rs` | 1 day |
| 11 | Config changes | `config.rs` | 0.5 day |
| 12 | Error types | `error.rs` | 0.5 day |
| 13 | Integration tests | `tests/` | 2 days |
| 14 | Desktop: Tauri commands | `desktop/commands.rs` | 1 day |
| 15 | Desktop: UI (Lighthouse publish section) | `ui/src/pages/Lighthouse.tsx` | 2 days |
| 16 | Cloudflare transport | `relay/cloudflare.rs` | 1 day |
| 17 | Relay clustering (Phase 4) | `relay/cluster.rs` | 5 days |
| 18 | JWT token scoping (Phase 5) | `relay/token.rs` | 2 days |
| 19 | Rate limiting | `relay/server.rs` | 1 day |
| 20 | Audit logging | `relay/server.rs` | 0.5 day |

**Phase 1 (steps 1-13): ~18 days**
**Phase 2 (steps 14-15): ~3 days**
**Phase 3 (step 16): ~1 day**
**Phase 4 (step 17): ~5 days**
**Phase 5 (steps 18-20): ~3.5 days**

---

## Open Questions

1. **Subdomain allocation for harbormcp.ai**: Random (like bore.pub) or account-based (like ngrok)?
   Recommendation: Random for anonymous, persistent for accounts.

2. **Protocol versioning**: How do we handle breaking changes to the envelope protocol?
   Recommendation: Version field in control stream handshake.

3. **Relay-to-relay forwarding**: If a remote client hits relay node A but the tunnel is on node B,
   should A forward to B or redirect the client?
   Recommendation: Forward (transparent to client).

4. **Binary distribution**: Ship relay as part of the harbor binary or separate?
   Recommendation: Same binary (`harbor relay start`), separate Docker image for production.

5. **SSE streaming through tunnel**: MCP tool results can stream via SSE within HTTP responses.
   The QUIC stream must support streaming (not buffer entire response).
   Recommendation: Use QUIC stream bytes directly, chunk-by-chunk.
