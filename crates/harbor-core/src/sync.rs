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
