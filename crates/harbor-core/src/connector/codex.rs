use crate::connector::{Connector, HostServerEntry};
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Connector for OpenAI Codex.
///
/// Config format (TOML):
/// ```toml
/// [mcp_servers.server-name]
/// command = "npx"
/// args = ["-y", "package"]
/// enabled = true
///
/// [mcp_servers.server-name.env]
/// KEY = "value"
/// ```
///
/// File location: ~/.codex/config.toml
pub struct CodexConnector;

#[derive(Debug, Serialize, Deserialize, Default)]
struct CodexConfig {
    #[serde(default)]
    mcp_servers: BTreeMap<String, CodexServerEntry>,

    /// Preserve all other fields
    #[serde(flatten)]
    other: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CodexServerEntry {
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

impl CodexConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Connector for CodexConnector {
    fn host_name(&self) -> &str {
        "Codex"
    }

    fn config_path(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| HarborError::ConnectorError {
            host: "codex".to_string(),
            reason: "Could not determine home directory".to_string(),
        })?;
        Ok(home.join(".codex").join("config.toml"))
    }

    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let config: CodexConfig = toml::from_str(&content).map_err(|e| {
            HarborError::ConnectorError {
                host: "codex".to_string(),
                reason: format!("Failed to parse {}: {}", path.display(), e),
            }
        })?;

        let servers = config
            .mcp_servers
            .into_iter()
            .filter(|(_, entry)| entry.enabled)
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

    fn write_servers(&self, servers: &BTreeMap<String, HostServerEntry>) -> Result<()> {
        let path = self.config_path()?;

        // Load existing config to preserve non-MCP fields
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
            toml::from_str::<CodexConfig>(&content).unwrap_or_default()
        } else {
            CodexConfig::default()
        };

        // Merge Harbor-managed servers
        for (name, entry) in servers {
            config.mcp_servers.insert(
                name.clone(),
                CodexServerEntry {
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    env: entry.env.clone(),
                    enabled: true,
                },
            );
        }

        // Write back
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(HarborError::Io)?;
        }
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(&path, content).map_err(HarborError::Io)?;

        Ok(())
    }
}
