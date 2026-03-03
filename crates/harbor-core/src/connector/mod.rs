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
