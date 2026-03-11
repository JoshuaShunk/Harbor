use crate::connector::{Connector, HostServerEntry};
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Connector for Windsurf (Codeium) IDE.
///
/// Config format (JSON):
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
/// File location: ~/.codeium/windsurf/mcp_config.json
pub struct WindsurfConnector;

#[derive(Debug, Serialize, Deserialize, Default)]
struct WindsurfConfig {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, WindsurfServerEntry>,

    /// Preserve all other fields
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WindsurfServerEntry {
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
}

impl Default for WindsurfConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl WindsurfConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Connector for WindsurfConnector {
    fn host_name(&self) -> &str {
        "Windsurf"
    }

    fn config_path(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| HarborError::ConnectorError {
            host: "windsurf".to_string(),
            reason: "Could not determine home directory".to_string(),
        })?;
        Ok(home
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json"))
    }

    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let config: WindsurfConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "windsurf".to_string(),
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
        let mut config: WindsurfConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "windsurf".to_string(),
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
            serde_json::from_str::<WindsurfConfig>(&content).unwrap_or_default()
        } else {
            WindsurfConfig::default()
        };

        // Merge Harbor-managed servers
        for (name, entry) in servers {
            config.mcp_servers.insert(
                name.clone(),
                WindsurfServerEntry {
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
