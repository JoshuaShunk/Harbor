use crate::connector::{Connector, HostServerEntry};
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Connector for the Cline VS Code extension (saoudrizwan.claude-dev).
///
/// Config format (JSON):
/// ```json
/// {
///   "mcpServers": {
///     "server-name": {
///       "command": "npx",
///       "args": ["-y", "package"],
///       "env": { "KEY": "value" },
///       "disabled": false,
///       "alwaysAllow": []
///     }
///   }
/// }
/// ```
///
/// File location (VS Code global extension storage):
/// - macOS:   ~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json
/// - Linux:   ~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json
/// - Windows: %APPDATA%/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json
pub struct ClineConnector;

#[derive(Debug, Serialize, Deserialize, Default)]
struct ClineConfig {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, ClineServerEntry>,

    /// Preserve all other fields
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClineServerEntry {
    command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,

    /// Preserve Cline-specific fields (disabled, alwaysAllow, etc.)
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

impl Default for ClineConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl ClineConnector {
    pub fn new() -> Self {
        Self
    }
}

impl Connector for ClineConnector {
    fn host_name(&self) -> &str {
        "Cline"
    }

    fn config_path(&self) -> Result<PathBuf> {
        const EXT_ID: &str = "saoudrizwan.claude-dev";
        const SETTINGS_FILE: &str = "cline_mcp_settings.json";

        let base = if cfg!(target_os = "macos") {
            let home = dirs::home_dir().ok_or_else(|| HarborError::ConnectorError {
                host: "cline".to_string(),
                reason: "Could not determine home directory".to_string(),
            })?;
            home.join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("globalStorage")
        } else {
            // Windows: dirs::config_dir() → %APPDATA%/Code/User/globalStorage
            // Linux:   dirs::config_dir() → ~/.config/Code/User/globalStorage
            let config = dirs::config_dir().ok_or_else(|| HarborError::ConnectorError {
                host: "cline".to_string(),
                reason: "Could not determine config directory".to_string(),
            })?;
            config.join("Code").join("User").join("globalStorage")
        };

        Ok(base.join(EXT_ID).join("settings").join(SETTINGS_FILE))
    }

    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let config: ClineConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "cline".to_string(),
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
        let mut config: ClineConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "cline".to_string(),
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

        // Load existing config to preserve non-MCP fields and Cline-specific entry fields
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
            serde_json::from_str::<ClineConfig>(&content).unwrap_or_default()
        } else {
            ClineConfig::default()
        };

        // Merge Harbor-managed servers, preserving any existing Cline-specific fields
        for (name, entry) in servers {
            let existing_extra = config
                .mcp_servers
                .get(name)
                .map(|e| e.extra.clone())
                .unwrap_or_default();

            config.mcp_servers.insert(
                name.clone(),
                ClineServerEntry {
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    env: entry.env.clone(),
                    extra: existing_extra,
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
