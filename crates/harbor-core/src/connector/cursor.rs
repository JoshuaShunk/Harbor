use crate::connector::{Connector, HostServerEntry};
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Connector for Cursor IDE.
///
/// Config format (JSON) — same structure as Claude Code:
/// ```json
/// {
///   "mcpServers": {
///     "server-name": {
///       "command": "npx",
///       "args": ["-y", "package"],
///       "env": { "KEY": "value" }
///     }
///   }
/// }
/// ```
///
/// File location: ~/.cursor/mcp.json
pub struct CursorConnector;

#[derive(Debug, Serialize, Deserialize, Default)]
struct CursorConfig {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, CursorServerEntry>,

    /// Preserve all other fields
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorServerEntry {
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
}

impl Default for CursorConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Connector for CursorConnector {
    fn host_name(&self) -> &str {
        "Cursor"
    }

    fn config_path(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| HarborError::ConnectorError {
            host: "cursor".to_string(),
            reason: "Could not determine home directory".to_string(),
        })?;
        Ok(home.join(".cursor").join("mcp.json"))
    }

    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let config: CursorConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "cursor".to_string(),
                reason: format!("Failed to parse {}: {}", path.display(), e),
            })?;

        let servers = config
            .mcp_servers
            .into_iter()
            .map(|(name, entry)| {
                (
                    name,
                    HostServerEntry {
                        command: entry.command,
                        args: entry.args,
                        env: entry.env,
                    },
                )
            })
            .collect();

        Ok(servers)
    }

    fn remove_servers(&self, names: &[String]) -> Result<()> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let mut config: CursorConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "cursor".to_string(),
                reason: format!("Failed to parse {}: {}", path.display(), e),
            })?;

        for name in names {
            config.mcp_servers.remove(name);
        }

        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&path, content).map_err(HarborError::Io)?;
        Ok(())
    }

    fn write_servers(&self, servers: &BTreeMap<String, HostServerEntry>) -> Result<()> {
        let path = self.config_path()?;

        // Load existing config to preserve non-MCP fields
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
            serde_json::from_str::<CursorConfig>(&content).unwrap_or_default()
        } else {
            CursorConfig::default()
        };

        // Merge Harbor-managed servers
        for (name, entry) in servers {
            config.mcp_servers.insert(
                name.clone(),
                CursorServerEntry {
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    env: entry.env.clone(),
                },
            );
        }

        // Write back
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(HarborError::Io)?;
        }
        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&path, content).map_err(HarborError::Io)?;

        Ok(())
    }
}
