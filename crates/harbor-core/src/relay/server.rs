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
//! Managed instance runs at harbormcp.ai.

// TODO: Implement in Phase 1, Step 5
//
// pub struct RelayServer { ... }
//
// impl RelayServer {
//     pub fn new(config: RelayConfig) -> Self;
//     pub async fn run(self, shutdown: oneshot::Receiver<()>) -> Result<()>;
// }
//
// The QUIC backend:
// 1. Bind quinn::Endpoint on quic_port
// 2. Accept incoming QUIC connections
// 3. For each connection:
//    a. Accept control stream (stream 0)
//    b. Perform Noise responder handshake
//    c. Read ControlMessage::Register
//    d. Validate auth token
//    e. Assign subdomain (requested or random via generate_subdomain())
//    f. Insert TunnelHandle into registry
//    g. Send ControlMessage::Registered
//    h. Spawn heartbeat listener task
//
// The HTTPS frontend (axum):
// Routes:
//   POST /mcp — forward to tunnel (subdomain from Host header or X-Harbor-Tunnel)
//   GET /health — relay health
//   GET / — landing page / docs
//
// Request flow:
// 1. Extract subdomain from Host header (e.g., "josh" from "josh.harbormcp.ai")
// 2. Lookup tunnel in registry
// 3. Verify Bearer token matches tunnel's bearer_token
// 4. Extract method + tool_name from JSON-RPC body
// 5. Check ACL: acl.is_method_allowed(method, tool_name)
// 6. Build Envelope with routing metadata
// 7. Encrypt JSON-RPC body with tunnel's Noise cipher
// 8. Open bidirectional QUIC stream to tunnel client
// 9. Send RelayMessage, read response RelayMessage
// 10. Decrypt response payload
// 11. Return JSON-RPC response to remote client
//
// Reaper task:
// - Every 60s, iterate tunnels and remove expired ones
// - Close QUIC connection for expired tunnels

pub struct RelayServer;
