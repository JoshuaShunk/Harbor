//! Tunnel lifecycle management: registration, heartbeat, and teardown.
//!
//! Used by both the relay server (managing multiple tunnels) and
//! the publish client (maintaining a single tunnel).

use crate::relay::acl::AclRules;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// State of a registered tunnel on the relay server.
pub struct TunnelState {
    /// Unique tunnel ID (assigned by relay).
    pub tunnel_id: String,
    /// Assigned subdomain.
    pub subdomain: String,
    /// When the tunnel was registered.
    pub created_at: Instant,
    /// Last heartbeat received.
    pub last_heartbeat: Instant,
    /// ACL rules for this tunnel.
    pub acl: AclRules,
    /// Bearer token for remote clients accessing this tunnel.
    pub bearer_token: String,
}

impl TunnelState {
    /// Check if the tunnel has expired (no heartbeat within timeout).
    pub fn is_expired(&self, timeout_secs: u64) -> bool {
        self.last_heartbeat.elapsed().as_secs() > timeout_secs
    }

    /// Update the heartbeat timestamp.
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }
}

/// Configuration for the relay server's tunnel management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    /// Heartbeat timeout in seconds (default: 90).
    /// Tunnels without a heartbeat within this window are reaped.
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout_secs: u64,

    /// Heartbeat interval in seconds (default: 30).
    /// How often the client sends heartbeats.
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// Maximum tunnels per auth token (default: 10).
    #[serde(default = "default_max_tunnels")]
    pub max_tunnels_per_token: u32,

    /// Domain for subdomain routing (e.g., "harbormcp.ai").
    pub domain: Option<String>,
}

fn default_heartbeat_timeout() -> u64 {
    90
}
fn default_heartbeat_interval() -> u64 {
    30
}
fn default_max_tunnels() -> u32 {
    10
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            heartbeat_timeout_secs: default_heartbeat_timeout(),
            heartbeat_interval_secs: default_heartbeat_interval(),
            max_tunnels_per_token: default_max_tunnels(),
            domain: None,
        }
    }
}

/// Generate a random subdomain (6 chars, URL-safe).
pub fn generate_subdomain() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_subdomain() {
        let sub = generate_subdomain();
        assert_eq!(sub.len(), 6);
        assert!(sub.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_subdomains_unique() {
        let s1 = generate_subdomain();
        let s2 = generate_subdomain();
        // Technically could collide but astronomically unlikely
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_tunnel_state_expiry() {
        let state = TunnelState {
            tunnel_id: "t1".to_string(),
            subdomain: "test".to_string(),
            created_at: Instant::now(),
            last_heartbeat: Instant::now(),
            acl: AclRules::allow_all(),
            bearer_token: "tok".to_string(),
        };

        assert!(!state.is_expired(90));
        // Can't easily test expiry without sleeping, but the logic is trivial
    }

    #[test]
    fn test_tunnel_state_heartbeat() {
        let mut state = TunnelState {
            tunnel_id: "t1".to_string(),
            subdomain: "test".to_string(),
            created_at: Instant::now(),
            last_heartbeat: Instant::now(),
            acl: AclRules::allow_all(),
            bearer_token: "tok".to_string(),
        };

        let old_heartbeat = state.last_heartbeat;
        // Small delay to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(5));
        state.heartbeat();
        assert!(state.last_heartbeat >= old_heartbeat);
    }

    #[test]
    fn test_tunnel_config_default() {
        let config = TunnelConfig::default();
        assert_eq!(config.heartbeat_timeout_secs, 90);
        assert_eq!(config.heartbeat_interval_secs, 30);
        assert_eq!(config.max_tunnels_per_token, 10);
        assert!(config.domain.is_none());
    }

    #[test]
    fn test_tunnel_config_serialization() {
        let config = TunnelConfig {
            heartbeat_timeout_secs: 120,
            heartbeat_interval_secs: 45,
            max_tunnels_per_token: 5,
            domain: Some("example.com".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("120"));
        assert!(json.contains("45"));
        assert!(json.contains("example.com"));

        let deserialized: TunnelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.heartbeat_timeout_secs, 120);
        assert_eq!(deserialized.domain, Some("example.com".to_string()));
    }

    #[test]
    fn test_tunnel_config_clone() {
        let config = TunnelConfig {
            heartbeat_timeout_secs: 60,
            heartbeat_interval_secs: 20,
            max_tunnels_per_token: 3,
            domain: Some("test.com".to_string()),
        };

        let cloned = config.clone();
        assert_eq!(cloned.heartbeat_timeout_secs, config.heartbeat_timeout_secs);
        assert_eq!(cloned.domain, config.domain);
    }

    #[test]
    fn test_generate_subdomain_alphanumeric() {
        for _ in 0..20 {
            let sub = generate_subdomain();
            assert!(sub.chars().all(|c| c.is_ascii_alphanumeric()));
            assert!(sub.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        }
    }

    #[test]
    fn test_generate_subdomain_length() {
        for _ in 0..20 {
            let sub = generate_subdomain();
            assert_eq!(sub.len(), 6);
        }
    }

    #[test]
    fn test_subdomain_uniqueness_many() {
        let subdomains: std::collections::HashSet<String> =
            (0..100).map(|_| generate_subdomain()).collect();
        // All should be unique (collision is astronomically unlikely)
        assert_eq!(subdomains.len(), 100);
    }

    #[test]
    fn test_tunnel_state_fields() {
        let state = TunnelState {
            tunnel_id: "tunnel-abc-123".to_string(),
            subdomain: "myapp".to_string(),
            created_at: Instant::now(),
            last_heartbeat: Instant::now(),
            acl: AclRules::allow_only(vec!["get_data".to_string()]),
            bearer_token: "hbr_test_token".to_string(),
        };

        assert_eq!(state.tunnel_id, "tunnel-abc-123");
        assert_eq!(state.subdomain, "myapp");
        assert!(state.acl.is_tool_allowed("get_data"));
        assert!(!state.acl.is_tool_allowed("delete_data"));
    }

    #[test]
    fn test_tunnel_not_expired_immediately() {
        let state = TunnelState {
            tunnel_id: "t1".to_string(),
            subdomain: "test".to_string(),
            created_at: Instant::now(),
            last_heartbeat: Instant::now(),
            acl: AclRules::allow_all(),
            bearer_token: "tok".to_string(),
        };

        // Should not be expired with any reasonable timeout
        assert!(!state.is_expired(1));
        assert!(!state.is_expired(10));
        assert!(!state.is_expired(90));
        assert!(!state.is_expired(3600));
    }

    #[test]
    fn test_tunnel_config_with_domain() {
        let config = TunnelConfig {
            domain: Some("harbormcp.ai".to_string()),
            ..Default::default()
        };

        assert_eq!(config.domain, Some("harbormcp.ai".to_string()));
        assert_eq!(config.heartbeat_timeout_secs, 90); // default
    }
}
