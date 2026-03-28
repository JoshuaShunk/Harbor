use crate::config::HarborConfig;
use crate::connector::{self, HostServerEntry};
use crate::error::Result;
use std::collections::BTreeMap;

/// Result of syncing servers to a single host.
pub struct SyncHostResult {
    pub host_name: String,
    pub display_name: String,
    pub server_count: usize,
}

/// Sync servers to a single connected host.
///
/// Always writes a proxy entry that routes through the Harbor gateway.
/// The gateway handles vault resolution, tool filtering, and hot reload at runtime.
pub fn sync_to_host(config: &HarborConfig, host_name: &str) -> Result<SyncHostResult> {
    let conn = connector::get_connector(host_name)?;
    let server_count = config.servers_for_host(host_name).len();

    let entries = build_proxy_entry(host_name, config.harbor.gateway_port);

    conn.write_servers(&entries)?;

    Ok(SyncHostResult {
        host_name: host_name.to_string(),
        display_name: conn.host_name().to_string(),
        server_count,
    })
}

/// Sync servers to all connected hosts. Returns results for each host.
pub fn sync_all_hosts(config: &HarborConfig) -> Vec<(String, Result<SyncHostResult>)> {
    let connected: Vec<String> = config
        .hosts
        .iter()
        .filter(|(_, h)| h.connected)
        .map(|(name, _)| name.clone())
        .collect();

    connected
        .into_iter()
        .map(|host_name| {
            let result = sync_to_host(config, &host_name);
            (host_name, result)
        })
        .collect()
}

fn build_proxy_entry(host_name: &str, gateway_port: u16) -> BTreeMap<String, HostServerEntry> {
    let mut args = vec![
        "relay".to_string(),
        "--host".to_string(),
        host_name.to_string(),
    ];
    if gateway_port != 3100 {
        args.push("--port".to_string());
        args.push(gateway_port.to_string());
    }
    let mut entries = BTreeMap::new();
    entries.insert(
        "harbor-proxy".to_string(),
        HostServerEntry {
            command: "harbor".to_string(),
            args,
            env: BTreeMap::new(),
        },
    );
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_proxy_entry_default_port() {
        let entries = build_proxy_entry("claude", 3100);
        assert_eq!(entries.len(), 1);

        let entry = entries.get("harbor-proxy").unwrap();
        assert_eq!(entry.command, "harbor");
        assert_eq!(entry.args, vec!["relay", "--host", "claude"]);
        assert!(entry.env.is_empty());
    }

    #[test]
    fn test_build_proxy_entry_custom_port() {
        let entries = build_proxy_entry("vscode", 4200);
        let entry = entries.get("harbor-proxy").unwrap();

        assert_eq!(
            entry.args,
            vec!["relay", "--host", "vscode", "--port", "4200"]
        );
    }

    #[test]
    fn test_build_proxy_entry_different_hosts() {
        for host in &["claude", "codex", "vscode", "cursor"] {
            let entries = build_proxy_entry(host, 3100);
            let entry = entries.get("harbor-proxy").unwrap();
            assert_eq!(entry.args[2], *host);
        }
    }

    #[test]
    fn test_sync_host_result_fields() {
        let result = SyncHostResult {
            host_name: "claude".to_string(),
            display_name: "Claude Code".to_string(),
            server_count: 5,
        };
        assert_eq!(result.host_name, "claude");
        assert_eq!(result.display_name, "Claude Code");
        assert_eq!(result.server_count, 5);
    }

    #[test]
    fn test_build_proxy_entry_contains_harbor_proxy() {
        let entries = build_proxy_entry("test-host", 3100);
        assert!(entries.contains_key("harbor-proxy"));
    }

    #[test]
    fn test_build_proxy_entry_empty_env() {
        let entries = build_proxy_entry("claude", 3100);
        let entry = entries.get("harbor-proxy").unwrap();
        assert!(entry.env.is_empty());
    }

    #[test]
    fn test_build_proxy_entry_port_threshold() {
        // Port 3100 should not include port arg
        let entries_3100 = build_proxy_entry("host", 3100);
        let entry_3100 = entries_3100.get("harbor-proxy").unwrap();
        assert!(!entry_3100.args.contains(&"--port".to_string()));

        // Port 3101 should include port arg
        let entries_3101 = build_proxy_entry("host", 3101);
        let entry_3101 = entries_3101.get("harbor-proxy").unwrap();
        assert!(entry_3101.args.contains(&"--port".to_string()));
        assert!(entry_3101.args.contains(&"3101".to_string()));
    }

    #[test]
    fn test_build_proxy_entry_relay_command() {
        let entries = build_proxy_entry("any-host", 3100);
        let entry = entries.get("harbor-proxy").unwrap();
        assert!(entry.args.contains(&"relay".to_string()));
    }

    #[test]
    fn test_build_proxy_entry_host_argument() {
        let entries = build_proxy_entry("cursor", 3100);
        let entry = entries.get("harbor-proxy").unwrap();

        // Should have --host cursor
        let host_idx = entry.args.iter().position(|a| a == "--host").unwrap();
        assert_eq!(entry.args[host_idx + 1], "cursor");
    }

    #[test]
    fn test_sync_host_result_zero_servers() {
        let result = SyncHostResult {
            host_name: "empty-host".to_string(),
            display_name: "Empty Host".to_string(),
            server_count: 0,
        };
        assert_eq!(result.server_count, 0);
    }

    #[test]
    fn test_sync_host_result_many_servers() {
        let result = SyncHostResult {
            host_name: "full-host".to_string(),
            display_name: "Full Host".to_string(),
            server_count: 100,
        };
        assert_eq!(result.server_count, 100);
    }
}
