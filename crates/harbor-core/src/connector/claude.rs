use crate::connector::{Connector, HostServerEntry};
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Connector for Claude Code.
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
/// File locations:
/// - User scope: ~/.claude.json
/// - Project scope: .mcp.json
pub struct ClaudeConnector {
    scope: ClaudeScope,
}

enum ClaudeScope {
    User,
    #[allow(dead_code)]
    Project(PathBuf),
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ClaudeConfig {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, ClaudeServerEntry>,

    /// Preserve all other fields
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeServerEntry {
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
}

impl Default for ClaudeConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeConnector {
    pub fn new() -> Self {
        Self {
            scope: ClaudeScope::User,
        }
    }

    #[allow(dead_code)]
    pub fn with_project_scope(project_path: PathBuf) -> Self {
        Self {
            scope: ClaudeScope::Project(project_path),
        }
    }
}

impl Connector for ClaudeConnector {
    fn host_name(&self) -> &str {
        "Claude Code"
    }

    fn config_path(&self) -> Result<PathBuf> {
        match &self.scope {
            ClaudeScope::User => {
                let home = dirs::home_dir().ok_or_else(|| HarborError::ConnectorError {
                    host: "claude".to_string(),
                    reason: "Could not determine home directory".to_string(),
                })?;
                Ok(home.join(".claude.json"))
            }
            ClaudeScope::Project(path) => Ok(path.join(".mcp.json")),
        }
    }

    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let config: ClaudeConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "claude".to_string(),
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

    fn write_servers(&self, servers: &BTreeMap<String, HostServerEntry>) -> Result<()> {
        let path = self.config_path()?;

        // Load existing config to preserve non-MCP fields
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
            serde_json::from_str::<ClaudeConfig>(&content).unwrap_or_default()
        } else {
            ClaudeConfig::default()
        };

        // Merge Harbor-managed servers (overwrite matching names, preserve others)
        for (name, entry) in servers {
            config.mcp_servers.insert(
                name.clone(),
                ClaudeServerEntry {
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
