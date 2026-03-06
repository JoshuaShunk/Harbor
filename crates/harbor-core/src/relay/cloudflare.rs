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

// TODO: Implement in Phase 3
//
// pub struct CloudflareTransport { ... }
//
// impl Transport for CloudflareTransport {
//     async fn connect(&mut self, config: &TransportConfig) -> Result<PublishInfo> {
//         // 1. Check if `cloudflared` is on PATH
//         // 2. Spawn: cloudflared tunnel --url http://127.0.0.1:{port} --no-autoupdate
//         // 3. Read stdout line by line, looking for URL:
//         //    "Your quick Tunnel has been created! Visit it at: https://xxx.trycloudflare.com"
//         // 4. Ensure gateway has a bearer token (generate if missing)
//         // 5. Return PublishInfo { url, token, transport: "cloudflare" }
//     }
//
//     async fn disconnect(&mut self) -> Result<()> {
//         // Kill the cloudflared child process
//     }
//
//     // next_request / send_response: Not used by CloudflareTransport
//     // because cloudflared handles proxying directly to the gateway.
//     // The Transport trait methods return NotSupported errors.
// }

pub struct CloudflareTransport;
