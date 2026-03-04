use crate::connector::{Connector, HostServerEntry};
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Connector for VS Code / GitHub Copilot.
///
/// Config format (JSON):
/// ```json
/// {
///   "servers": {
///     "server-name": {
///       "type": "stdio",
///       "command": "npx",
///       "args": ["-y", "package"],
///       "env": { "KEY": "value" }
///     }
///   }
/// }
/// ```
///
/// File location: .vscode/mcp.json (workspace scope)
pub struct VsCodeConnector {
    workspace_root: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct VsCodeMcpConfig {
    #[serde(default)]
    servers: BTreeMap<String, VsCodeServerEntry>,

    /// Preserve inputs and other fields
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VsCodeServerEntry {
    #[serde(rename = "type")]
    transport_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
    /// For http type servers
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,

    /// Preserve unknown fields
    #[serde(flatten)]
    other: BTreeMap<String, serde_json::Value>,
}

impl Default for VsCodeConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl VsCodeConnector {
    pub fn new() -> Self {
        Self {
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    #[allow(dead_code)]
    pub fn with_workspace(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl Connector for VsCodeConnector {
    fn host_name(&self) -> &str {
        "VS Code"
    }

    fn config_path(&self) -> Result<PathBuf> {
        Ok(self.workspace_root.join(".vscode").join("mcp.json"))
    }

    fn read_servers(&self) -> Result<BTreeMap<String, HostServerEntry>> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
        let config: VsCodeMcpConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "vscode".to_string(),
                reason: format!("Failed to parse {}: {}", path.display(), e),
            })?;

        let servers = config
            .servers
            .into_iter()
            .filter_map(|(name, entry)| {
                // Only import stdio servers (http servers can't be represented as command+args)
                if entry.transport_type == "stdio" {
                    entry.command.map(|cmd| {
                        (
                            name,
                            HostServerEntry {
                                command: cmd,
                                args: entry.args,
                                env: entry.env,
                            },
                        )
                    })
                } else {
                    None
                }
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
        let mut config: VsCodeMcpConfig =
            serde_json::from_str(&content).map_err(|e| HarborError::ConnectorError {
                host: "vscode".to_string(),
                reason: format!("Failed to parse {}: {}", path.display(), e),
            })?;

        for name in names {
            config.servers.remove(name);
        }

        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&path, content).map_err(HarborError::Io)?;
        Ok(())
    }

    fn write_servers(&self, servers: &BTreeMap<String, HostServerEntry>) -> Result<()> {
        let path = self.config_path()?;

        // Load existing config to preserve non-stdio entries and inputs
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
            serde_json::from_str::<VsCodeMcpConfig>(&content).unwrap_or_default()
        } else {
            VsCodeMcpConfig::default()
        };

        // Merge Harbor-managed servers as stdio type
        for (name, entry) in servers {
            config.servers.insert(
                name.clone(),
                VsCodeServerEntry {
                    transport_type: "stdio".to_string(),
                    command: Some(entry.command.clone()),
                    args: entry.args.clone(),
                    env: entry.env.clone(),
                    url: None,
                    other: BTreeMap::new(),
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
