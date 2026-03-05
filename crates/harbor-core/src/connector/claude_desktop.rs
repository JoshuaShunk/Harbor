use crate::connector::{Connector, HostServerEntry};
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Connector for the Claude Desktop app.
///
/// Config format (JSON) — same `mcpServers` structure as Claude Code:
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
/// File location:
/// - macOS: ~/Library/Application Support/Claude/claude_desktop_config.json
/// - Windows: %APPDATA%/Claude/claude_desktop_config.json
/// - Linux: ~/.config/Claude/claude_desktop_config.json
pub struct ClaudeDesktopConnector;

#[derive(Debug, Serialize, Deserialize, Default)]
struct ClaudeDesktopConfig {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, ClaudeDesktopServerEntry>,

    /// Preserve all other fields (preferences, etc.)
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeDesktopServerEntry {
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
}

impl Default for ClaudeDesktopConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeDesktopConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Connector for ClaudeDesktopConnector {
    fn host_name(&self) -> &str {
        "Claude Desktop"
    }

    fn config_path(&self) -> Result<PathBuf> {
        let path = if cfg!(target_os = "macos") {
            let home = dirs::home_dir().ok_or_else(|| HarborError::ConnectorError {
                host: "claude-desktop".to_string(),
                reason: "Could not determine home directory".to_string(),
            })?;
            home.join("Library")
                .join("Application Support")
                .join("Claude")
                .join("claude_desktop_config.json")
        } else if cfg!(target_os = "windows") {
            let appdata =
                dirs::config_dir().ok_or_else(|| HarborError::ConnectorError {
                    host: "claude-desktop".to_string(),
                    reason: "Could not determine APPDATA directory".to_string(),
                })?;
            appdata.join("Claude").join("claude_desktop_config.json")
        } else {
            // Linux
            let config =
                dirs::config_dir().ok_or_else(|| HarborError::ConnectorError {
                    host: "claude-desktop".to_string(),
                    reason: "Could not determine config directory".to_string(),
                })?;
            config.join("Claude").join("claude_desktop_config.json")
        };
        Ok(path)
    }

    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let config: ClaudeDesktopConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "claude-desktop".to_string(),
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
        let mut config: ClaudeDesktopConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "claude-desktop".to_string(),
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

        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
            serde_json::from_str::<ClaudeDesktopConfig>(&content).unwrap_or_default()
        } else {
            ClaudeDesktopConfig::default()
        };

        for (name, entry) in servers {
            config.mcp_servers.insert(
                name.clone(),
                ClaudeDesktopServerEntry {
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    env: entry.env.clone(),
                },
            );
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(HarborError::Io)?;
        }
        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&path, content).map_err(HarborError::Io)?;

        Ok(())
    }
}
