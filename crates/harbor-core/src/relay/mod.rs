//! Harbor Relay — secure tunnel for publishing local MCP gateways to the internet.
//!
//! # Architecture
//!
//! The relay system has two sides:
//!
//! - **Relay Server** (`RelayServer`): Accepts incoming QUIC tunnels from Harbor clients
//!   and proxies HTTPS requests from remote MCP clients through those tunnels.
//!   Can be self-hosted (`harbor relay start`) or managed (harbormcp.ai).
//!
//! - **Publish Client** (`PublishClient`): Runs on the user's machine alongside the
//!   Harbor gateway. Establishes an outbound-only QUIC connection to the relay,
//!   receives forwarded MCP requests, and proxies them to localhost.
//!
//! # Security
//!
//! - **Noise Protocol** (NK pattern): E2E encryption between the publish client and
//!   remote MCP clients. The relay server cannot read JSON-RPC payloads.
//! - **Envelope Protocol**: The relay sees only routing metadata (subdomain, method,
//!   tool name) for ACL enforcement — not arguments or results.
//! - **Tool-Level ACL**: Users control which tools are exposed remotely.
//! - **Bearer Token Auth**: Remote MCP clients authenticate with scoped tokens.
//!
//! # Transports
//!
//! The publish client uses a pluggable `Transport` trait:
//! - `QuicTransport`: Built-in QUIC + Noise tunnel (default)
//! - `CloudflareTransport`: Delegates to `cloudflared` binary
//!
//! # Usage
//!
//! ```sh
//! # Self-hosted relay:
//! harbor relay start --quic-port 7800 --https-port 443 --domain relay.example.com
//!
//! # Publish to managed relay:
//! harbor publish
//!
//! # Publish to self-hosted relay:
//! harbor publish --relay my-relay.example.com
//!
//! # Publish via Cloudflare Tunnel:
//! harbor publish --transport cloudflare
//! ```

pub mod acl;
pub mod client;
pub mod cloudflare;
pub mod crypto;
pub mod envelope;
pub mod server;
pub mod token;
pub mod transport;
pub mod tunnel;

pub use client::PublishClient;
pub use cloudflare::CloudflareTransport;
pub use server::{RelayConfig, RelayServer};
pub use transport::{PublishInfo, Transport, TransportConfig};
