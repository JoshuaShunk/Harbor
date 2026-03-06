//! Publish client — establishes an outbound QUIC tunnel to a relay server
//! and forwards incoming MCP requests to the local Harbor gateway.
//!
//! This is the main runtime for `harbor publish`. It:
//! 1. Connects to the relay via QUIC
//! 2. Performs Noise NK handshake
//! 3. Registers the tunnel (auth, subdomain, ACL)
//! 4. Runs a request loop: receive request -> forward to gateway -> send response
//! 5. Maintains heartbeats to keep the tunnel alive

// TODO: Implement in Phase 1, Step 4
//
// pub struct PublishClient { ... }
//
// impl PublishClient {
//     pub fn new(config: TransportConfig) -> Self;
//     pub async fn run(&mut self, shutdown: oneshot::Receiver<()>) -> Result<PublishInfo>;
// }
//
// The run loop:
// 1. Connect to relay (QUIC)
// 2. Open control stream, perform Noise handshake
// 3. Send ControlMessage::Register
// 4. Receive ControlMessage::Registered (or Rejected)
// 5. Spawn heartbeat task (sends ControlMessage::Heartbeat every 30s)
// 6. Loop:
//    a. Accept bidirectional QUIC stream from relay
//    b. Read RelayMessage (envelope + encrypted payload)
//    c. Decrypt payload with Noise cipher
//    d. Forward JSON-RPC request to http://127.0.0.1:{port}/mcp
//    e. Encrypt response with Noise cipher
//    f. Send RelayMessage response on same QUIC stream
// 7. On shutdown: send ControlMessage::Disconnect, close connection

pub struct PublishClient;
