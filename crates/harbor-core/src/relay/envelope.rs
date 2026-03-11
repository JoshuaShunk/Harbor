//! Envelope protocol for Harbor relay messages.
//!
//! Each message through the tunnel consists of:
//! - An **envelope** (plaintext to the relay) containing routing metadata
//! - A **payload** (Noise-encrypted) containing the actual JSON-RPC body
//!
//! The relay can inspect the envelope for routing and ACL decisions
//! but cannot read the encrypted payload.
//!
//! Wire format:
//! ```text
//! [4 bytes: envelope length (big-endian u32)]
//! [N bytes: envelope JSON]
//! [remaining bytes: encrypted payload]
//! ```

use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};

/// Direction of a relay message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Request,
    Response,
}

/// Metadata visible to the relay — NOT encrypted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Unique tunnel identifier (assigned during registration).
    pub tunnel_id: String,

    /// MCP session ID for session affinity routing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// JSON-RPC method (e.g., "tools/list", "tools/call").
    pub method: String,

    /// Tool name — only present for "tools/call" requests.
    /// Used by the relay for tool-level ACL enforcement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// Unique request ID for correlating request/response pairs.
    pub request_id: String,

    /// Whether this is a request or response.
    pub direction: Direction,
}

/// A complete relay message: envelope + encrypted payload.
#[derive(Debug, Clone)]
pub struct RelayMessage {
    pub envelope: Envelope,
    /// Noise-encrypted JSON-RPC body (or plaintext if encryption is disabled).
    pub payload: Vec<u8>,
}

impl RelayMessage {
    /// Create a new request message.
    pub fn request(
        tunnel_id: &str,
        method: &str,
        tool_name: Option<&str>,
        session_id: Option<&str>,
        request_id: &str,
        encrypted_payload: Vec<u8>,
    ) -> Self {
        Self {
            envelope: Envelope {
                tunnel_id: tunnel_id.to_string(),
                session_id: session_id.map(String::from),
                method: method.to_string(),
                tool_name: tool_name.map(String::from),
                request_id: request_id.to_string(),
                direction: Direction::Request,
            },
            payload: encrypted_payload,
        }
    }

    /// Create a response message for a given request.
    pub fn response(request_envelope: &Envelope, encrypted_payload: Vec<u8>) -> Self {
        Self {
            envelope: Envelope {
                tunnel_id: request_envelope.tunnel_id.clone(),
                session_id: request_envelope.session_id.clone(),
                method: request_envelope.method.clone(),
                tool_name: request_envelope.tool_name.clone(),
                request_id: request_envelope.request_id.clone(),
                direction: Direction::Response,
            },
            payload: encrypted_payload,
        }
    }

    /// Encode to wire format for sending over a QUIC stream.
    ///
    /// Format: [4-byte envelope length][envelope JSON][encrypted payload]
    pub fn encode(&self) -> Result<Vec<u8>> {
        let envelope_bytes = serde_json::to_vec(&self.envelope)?;
        let envelope_len = envelope_bytes.len() as u32;

        let mut buf = Vec::with_capacity(4 + envelope_bytes.len() + self.payload.len());
        buf.extend_from_slice(&envelope_len.to_be_bytes());
        buf.extend_from_slice(&envelope_bytes);
        buf.extend_from_slice(&self.payload);

        Ok(buf)
    }

    /// Decode from wire format received from a QUIC stream.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(HarborError::RelayError(
                "Message too short: missing envelope length".to_string(),
            ));
        }

        let envelope_len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        if bytes.len() < 4 + envelope_len {
            return Err(HarborError::RelayError(format!(
                "Message too short: expected {} envelope bytes, got {}",
                envelope_len,
                bytes.len() - 4
            )));
        }

        let envelope: Envelope = serde_json::from_slice(&bytes[4..4 + envelope_len])?;
        let payload = bytes[4 + envelope_len..].to_vec();

        Ok(Self { envelope, payload })
    }
}

/// Control messages sent on the QUIC control stream (stream 0).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlMessage {
    /// Client -> Relay: Register a new tunnel.
    #[serde(rename = "register")]
    Register {
        /// Auth token for the relay.
        auth_token: String,
        /// Requested subdomain (None = auto-assign).
        subdomain: Option<String>,
        /// Protocol version.
        version: u32,
        /// Tools this tunnel exposes (None = all).
        tools: Option<Vec<String>>,
    },

    /// Relay -> Client: Registration accepted.
    #[serde(rename = "registered")]
    Registered {
        /// Assigned tunnel ID.
        tunnel_id: String,
        /// Assigned subdomain.
        subdomain: String,
        /// Full public URL for remote access.
        public_url: String,
        /// Bearer token for remote MCP clients.
        bearer_token: String,
    },

    /// Relay -> Client: Registration rejected.
    #[serde(rename = "rejected")]
    Rejected {
        /// Reason for rejection.
        reason: String,
    },

    /// Bidirectional: Heartbeat (keep tunnel alive).
    #[serde(rename = "heartbeat")]
    Heartbeat {
        /// Timestamp (milliseconds since epoch).
        timestamp: u64,
    },

    /// Client -> Relay: Graceful disconnect.
    #[serde(rename = "disconnect")]
    Disconnect,
}

impl ControlMessage {
    /// Serialize to JSON bytes for sending on control stream.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec(self)?;
        bytes.push(b'\n'); // newline-delimited for easy parsing
        Ok(bytes)
    }

    /// Deserialize from JSON bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let trimmed = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        let msg: Self = serde_json::from_slice(trimmed)?;
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_message_roundtrip() {
        let msg = RelayMessage::request(
            "tunnel-123",
            "tools/call",
            Some("get_issues"),
            Some("session-abc"),
            "req-1",
            b"encrypted payload here".to_vec(),
        );

        let encoded = msg.encode().unwrap();
        let decoded = RelayMessage::decode(&encoded).unwrap();

        assert_eq!(decoded.envelope.tunnel_id, "tunnel-123");
        assert_eq!(decoded.envelope.method, "tools/call");
        assert_eq!(decoded.envelope.tool_name.as_deref(), Some("get_issues"));
        assert_eq!(decoded.envelope.session_id.as_deref(), Some("session-abc"));
        assert_eq!(decoded.envelope.request_id, "req-1");
        assert_eq!(decoded.envelope.direction, Direction::Request);
        assert_eq!(decoded.payload, b"encrypted payload here");
    }

    #[test]
    fn test_response_from_request() {
        let request = RelayMessage::request("t1", "tools/call", Some("search"), None, "r1", vec![]);

        let response = RelayMessage::response(&request.envelope, b"response data".to_vec());
        assert_eq!(response.envelope.direction, Direction::Response);
        assert_eq!(response.envelope.request_id, "r1");
        assert_eq!(response.envelope.tunnel_id, "t1");
    }

    #[test]
    fn test_control_message_roundtrip() {
        let msg = ControlMessage::Register {
            auth_token: "tok123".to_string(),
            subdomain: Some("josh".to_string()),
            version: 1,
            tools: Some(vec!["get_issues".to_string()]),
        };

        let encoded = msg.encode().unwrap();
        let decoded = ControlMessage::decode(&encoded).unwrap();

        match decoded {
            ControlMessage::Register {
                auth_token,
                subdomain,
                version,
                tools,
            } => {
                assert_eq!(auth_token, "tok123");
                assert_eq!(subdomain.as_deref(), Some("josh"));
                assert_eq!(version, 1);
                assert_eq!(tools.unwrap(), vec!["get_issues"]);
            }
            _ => panic!("Expected Register message"),
        }
    }

    #[test]
    fn test_decode_too_short() {
        assert!(RelayMessage::decode(&[0, 0]).is_err());
    }
}
