pub mod claude;
pub mod codex;
pub mod cursor;
pub mod vscode;

use crate::config::ServerConfig;
use crate::error::Result;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Trait for host-specific MCP config connectors.
///
/// Each connector knows how to read and write MCP server entries
/// into a specific host's configuration format and file location.
pub trait Connector {
    /// Human-readable name of this host (e.g., "Claude Code", "Codex")
    fn host_name(&self) -> &str;

    /// Returns the config file path for this host
    fn config_path(&self) -> Result<PathBuf>;

    /// Read the current MCP servers from the host's config.
    /// Returns a map of server_name -> (command, args, env).
    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>>;

    /// Write/merge Harbor-managed servers into the host's config.
    /// This should preserve any existing non-Harbor entries.
    fn write_servers(&self, servers: &BTreeMap<String, HostServerEntry>) -> Result<()>;

    /// Remove specific server entries from the host's config by name.
    fn remove_servers(&self, names: &[String]) -> Result<()>;

    /// Check if the host's config file exists
    fn config_exists(&self) -> bool {
        self.config_path().map(|p| p.exists()).unwrap_or(false)
    }
}

/// A normalized server entry that connectors translate to/from host-specific formats
#[derive(Debug, Clone)]
pub struct HostServerEntry {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

impl From<&ServerConfig> for HostServerEntry {
    fn from(config: &ServerConfig) -> Self {
        Self {
            command: config.command.clone(),
            args: config.args.clone(),
            env: config.env.clone(),
        }
    }
}

/// Resolve vault: references in env to actual values for writing to host configs.
/// Host configs don't understand vault references, so we must resolve them first.
/// Uses the vault (OS keychain) with env-var fallback.
pub fn resolve_env_for_host(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    crate::auth::vault::Vault::resolve_env(env)
}

/// Get a connector by host name
pub fn get_connector(host: &str) -> Result<Box<dyn Connector>> {
    match host {
        "claude" => Ok(Box::new(claude::ClaudeConnector::new())),
        "codex" => Ok(Box::new(codex::CodexConnector::new())),
        "vscode" => Ok(Box::new(vscode::VsCodeConnector::new())),
        "cursor" => Ok(Box::new(cursor::CursorConnector::new())),
        _ => Err(crate::error::HarborError::ConnectorError {
            host: host.to_string(),
            reason: format!("Unknown host: {host}"),
        }),
    }
}

/// Get all available connectors
pub fn all_connectors() -> Vec<Box<dyn Connector>> {
    vec![
        Box::new(claude::ClaudeConnector::new()),
        Box::new(codex::CodexConnector::new()),
        Box::new(vscode::VsCodeConnector::new()),
        Box::new(cursor::CursorConnector::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_host_server_entry_from_server_config() {
        let config = crate::config::ServerConfig {
            source: Some("npm:@mcp/test".to_string()),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@mcp/test".to_string()],
            env: {
                let mut env = BTreeMap::new();
                env.insert("TOKEN".to_string(), "secret123".to_string());
                env
            },
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        };

        let entry = HostServerEntry::from(&config);
        assert_eq!(entry.command, "npx");
        assert_eq!(entry.args, vec!["-y", "@mcp/test"]);
        assert_eq!(entry.env.get("TOKEN").unwrap(), "secret123");
    }

    #[test]
    fn test_host_server_entry_from_minimal_config() {
        let config = crate::config::ServerConfig {
            source: None,
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        };

        let entry = HostServerEntry::from(&config);
        assert_eq!(entry.command, "echo");
        assert!(entry.args.is_empty());
        assert!(entry.env.is_empty());
    }

    #[test]
    fn test_get_connector_valid_hosts() {
        let hosts = ["claude", "codex", "vscode", "cursor"];
        for host in &hosts {
            let connector = get_connector(host);
            assert!(connector.is_ok(), "Failed to get connector for {}", host);
        }
    }

    #[test]
    fn test_get_connector_unknown_host_errors() {
        let result = get_connector("unknown-host");
        assert!(result.is_err());
    }

    #[test]
    fn test_all_connectors_returns_four() {
        let connectors = all_connectors();
        assert_eq!(connectors.len(), 4);
    }

    #[test]
    fn test_connector_host_names() {
        let claude = get_connector("claude").unwrap();
        assert_eq!(claude.host_name(), "Claude Code");

        let codex = get_connector("codex").unwrap();
        assert_eq!(codex.host_name(), "Codex");

        let vscode = get_connector("vscode").unwrap();
        assert_eq!(vscode.host_name(), "VS Code");

        let cursor = get_connector("cursor").unwrap();
        assert_eq!(cursor.host_name(), "Cursor");
    }

    #[test]
    fn test_connector_config_paths_are_valid() {
        // Each connector should return a valid path (not error)
        let hosts = ["claude", "codex", "vscode", "cursor"];
        for host in &hosts {
            let connector = get_connector(host).unwrap();
            let path = connector.config_path();
            assert!(path.is_ok(), "config_path() failed for {}", host);
            // The path should be an absolute path
            assert!(
                path.as_ref().unwrap().is_absolute(),
                "config_path() for {} is not absolute",
                host
            );
        }
    }
}
