use crate::connector::builtins::builtin_defs;
use crate::connector::def::ConnectorDef;
use crate::connector::generic::GenericConnector;
use crate::connector::Connector;
use crate::error::{HarborError, Result};
use std::path::PathBuf;

/// Central registry of all available connectors (built-in + user-defined).
///
/// This is Harbor's equivalent of Terraform's provider registry or Homebrew's
/// tap system. Built-in connectors ship as embedded data. User-defined
/// connectors are loaded from `~/.harbor/connectors/*.toml` at runtime.
pub struct ConnectorRegistry {
    defs: Vec<ConnectorDef>,
}

impl ConnectorRegistry {
    /// Build registry from built-ins + user-defined connectors.
    pub fn load() -> Result<Self> {
        let mut defs = builtin_defs();

        // Load user-defined connectors (warn on errors, don't crash)
        match load_user_connectors() {
            Ok(user_defs) => {
                for user_def in user_defs {
                    // Skip if ID collides with a built-in
                    if defs.iter().any(|d| d.id == user_def.id) {
                        tracing::warn!(
                            "User connector '{}' skipped: ID collides with built-in",
                            user_def.id
                        );
                        continue;
                    }
                    defs.push(user_def);
                }
            }
            Err(e) => {
                tracing::debug!("Could not load user connectors: {e}");
            }
        }

        Ok(Self { defs })
    }

    /// Build registry from built-ins only (no filesystem access).
    pub fn builtins_only() -> Self {
        Self {
            defs: builtin_defs(),
        }
    }

    /// Get a connector by its machine-readable ID.
    pub fn get(&self, id: &str) -> Result<Box<dyn Connector>> {
        let def =
            self.defs
                .iter()
                .find(|d| d.id == id)
                .ok_or_else(|| HarborError::ConnectorError {
                    host: id.to_string(),
                    reason: format!("Unknown host: {id}"),
                })?;
        Ok(Box::new(GenericConnector::new(def.clone())))
    }

    /// Get all available connectors.
    pub fn all(&self) -> Vec<Box<dyn Connector>> {
        self.defs
            .iter()
            .map(|def| Box::new(GenericConnector::new(def.clone())) as Box<dyn Connector>)
            .collect()
    }

    /// Get all known connector IDs.
    pub fn all_ids(&self) -> Vec<&str> {
        self.defs.iter().map(|d| d.id.as_str()).collect()
    }

    /// Check if a host ID is valid (registered).
    pub fn is_valid_host(&self, id: &str) -> bool {
        self.defs.iter().any(|d| d.id == id)
    }
}

/// Load user-defined connector definitions from `~/.harbor/connectors/*.toml`.
fn load_user_connectors() -> Result<Vec<ConnectorDef>> {
    let harbor_dir = crate::config::HarborConfig::default_dir()?;
    let connectors_dir = harbor_dir.join("connectors");

    if !connectors_dir.exists() {
        return Ok(vec![]);
    }

    let mut defs = Vec::new();
    let entries = std::fs::read_dir(&connectors_dir).map_err(HarborError::Io)?;

    for entry in entries {
        let entry = entry.map_err(HarborError::Io)?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        match load_single_connector(&path) {
            Ok(def) => defs.push(def),
            Err(e) => {
                tracing::warn!(
                    "Skipping invalid connector definition {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    Ok(defs)
}

fn load_single_connector(path: &PathBuf) -> Result<ConnectorDef> {
    let content = std::fs::read_to_string(path).map_err(HarborError::Io)?;
    let def: ConnectorDef = toml::from_str(&content).map_err(|e| HarborError::ConnectorError {
        host: path.display().to_string(),
        reason: format!("Failed to parse connector definition: {e}"),
    })?;

    def.validate().map_err(|e| HarborError::ConnectorError {
        host: def.id.clone(),
        reason: e,
    })?;

    Ok(def)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtins_only_returns_eight() {
        let registry = ConnectorRegistry::builtins_only();
        assert_eq!(registry.all_ids().len(), 8);
    }

    #[test]
    fn test_get_known_hosts() {
        let registry = ConnectorRegistry::builtins_only();
        for id in &[
            "claude",
            "claude-desktop",
            "codex",
            "vscode",
            "cursor",
            "cline",
            "roo-code",
            "windsurf",
        ] {
            assert!(
                registry.get(id).is_ok(),
                "Failed to get connector for '{id}'"
            );
        }
    }

    #[test]
    fn test_get_unknown_host_errors() {
        let registry = ConnectorRegistry::builtins_only();
        assert!(registry.get("unknown").is_err());
    }

    #[test]
    fn test_is_valid_host() {
        let registry = ConnectorRegistry::builtins_only();
        assert!(registry.is_valid_host("claude"));
        assert!(registry.is_valid_host("vscode"));
        assert!(!registry.is_valid_host("unknown"));
    }

    #[test]
    fn test_all_connectors_have_correct_names() {
        let registry = ConnectorRegistry::builtins_only();

        let claude = registry.get("claude").unwrap();
        assert_eq!(claude.host_name(), "Claude Code");
        assert_eq!(claude.host_id(), "claude");

        let desktop = registry.get("claude-desktop").unwrap();
        assert_eq!(desktop.host_name(), "Claude Desktop");

        let codex = registry.get("codex").unwrap();
        assert_eq!(codex.host_name(), "Codex");

        let vscode = registry.get("vscode").unwrap();
        assert_eq!(vscode.host_name(), "VS Code");

        let cursor = registry.get("cursor").unwrap();
        assert_eq!(cursor.host_name(), "Cursor");

        let cline = registry.get("cline").unwrap();
        assert_eq!(cline.host_name(), "Cline");

        let roo = registry.get("roo-code").unwrap();
        assert_eq!(roo.host_name(), "Roo Code");

        let windsurf = registry.get("windsurf").unwrap();
        assert_eq!(windsurf.host_name(), "Windsurf");
    }

    #[test]
    fn test_all_config_paths_are_absolute() {
        let registry = ConnectorRegistry::builtins_only();
        for id in registry.all_ids() {
            // VS Code is workspace-relative, skip it
            if id == "vscode" {
                continue;
            }
            let conn = registry.get(id).unwrap();
            let path = conn.config_path();
            assert!(
                path.is_ok(),
                "config_path() failed for '{id}': {:?}",
                path.err()
            );
            assert!(
                path.as_ref().unwrap().is_absolute(),
                "config_path() for '{id}' is not absolute: {:?}",
                path.unwrap()
            );
        }
    }

    #[test]
    fn test_all_returns_all_connectors() {
        let registry = ConnectorRegistry::builtins_only();
        let connectors = registry.all();
        assert_eq!(connectors.len(), 8);
    }

    #[test]
    fn test_all_ids_returns_all_ids() {
        let registry = ConnectorRegistry::builtins_only();
        let ids = registry.all_ids();
        assert_eq!(ids.len(), 8);
        assert!(ids.contains(&"claude"));
        assert!(ids.contains(&"vscode"));
    }

    #[test]
    fn test_get_returns_correct_connector() {
        let registry = ConnectorRegistry::builtins_only();

        let claude = registry.get("claude").unwrap();
        assert_eq!(claude.host_id(), "claude");
        assert_eq!(claude.host_name(), "Claude Code");

        let vscode = registry.get("vscode").unwrap();
        assert_eq!(vscode.host_id(), "vscode");
        assert_eq!(vscode.host_name(), "VS Code");
    }

    #[test]
    fn test_get_error_message_contains_host() {
        let registry = ConnectorRegistry::builtins_only();
        let result = registry.get("invalid-host");

        assert!(result.is_err());
        match result {
            Err(HarborError::ConnectorError { host, reason }) => {
                assert_eq!(host, "invalid-host");
                assert!(reason.contains("Unknown host"));
            }
            _ => panic!("Expected ConnectorError"),
        }
    }

    #[test]
    fn test_is_valid_host_for_all_builtins() {
        let registry = ConnectorRegistry::builtins_only();
        let ids = registry.all_ids();

        for id in ids {
            assert!(
                registry.is_valid_host(id),
                "is_valid_host should return true for '{}'",
                id
            );
        }
    }

    #[test]
    fn test_is_valid_host_case_sensitive() {
        let registry = ConnectorRegistry::builtins_only();
        assert!(registry.is_valid_host("claude"));
        assert!(!registry.is_valid_host("Claude")); // uppercase
        assert!(!registry.is_valid_host("CLAUDE")); // all caps
    }

    #[test]
    fn test_connectors_return_dyn_connector() {
        let registry = ConnectorRegistry::builtins_only();
        let connectors = registry.all();

        for conn in connectors {
            // All connectors should implement the trait correctly
            assert!(!conn.host_id().is_empty());
            assert!(!conn.host_name().is_empty());
        }
    }

    #[test]
    fn test_load_returns_at_least_builtins() {
        // Load may add user connectors, but should always have builtins
        if let Ok(registry) = ConnectorRegistry::load() {
            assert!(registry.all_ids().len() >= 8);
        }
    }
}
