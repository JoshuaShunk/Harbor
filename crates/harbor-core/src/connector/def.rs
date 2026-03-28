use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Declarative definition of a host connector — pure data, no logic.
///
/// This is the core of Harbor's scalable connector system. The common case
/// (JSON/TOML file with a servers key) is described entirely as data.
/// Only genuinely unique connectors need custom `Connector` trait impls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDef {
    /// Machine-readable identifier used as lookup key (e.g., "claude", "cursor")
    pub id: String,

    /// Human-readable display name (e.g., "Claude Code", "Cursor")
    pub display_name: String,

    /// Config file format
    #[serde(default)]
    pub format: ConfigFormat,

    /// The key that holds the servers map (e.g., "mcpServers", "servers", "mcp_servers")
    pub servers_key: String,

    /// How to resolve the config file path
    pub config_path: ConfigPathDef,

    /// Extra static fields to inject into each server entry on write.
    /// e.g., {"type": "stdio"} for VS Code, {"enabled": true} for Codex.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub write_extra_fields: BTreeMap<String, serde_json::Value>,

    /// Filter predicate for read: only import entries where ALL these field=value pairs match.
    /// e.g., {"type": "stdio"} for VS Code — only reads stdio entries.
    /// Entries missing a filter key are excluded (except for bool fields which default true).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub read_filter: BTreeMap<String, serde_json::Value>,

    /// When true, preserve unknown fields on existing server entries during write.
    /// Used by Cline/Roo Code to keep `disabled`, `alwaysAllow`, etc.
    #[serde(default)]
    pub preserve_unknown_entry_fields: bool,

    /// Whether the `command` field is optional on read (entries without it are skipped).
    /// VS Code has http-type entries with `url` instead of `command`.
    #[serde(default)]
    pub command_optional_on_read: bool,
}

/// Config file format.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigFormat {
    #[default]
    Json,
    Toml,
}

/// How to resolve the config file path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigPathDef {
    /// Path relative to home directory: $HOME/{path}
    HomeRelative { path: String },

    /// Platform-specific paths (each relative to appropriate base dir).
    /// macOS: relative to $HOME
    /// Linux/Windows: relative to dirs::config_dir()
    PlatformSpecific {
        macos: String,
        linux: String,
        windows: String,
    },

    /// Relative to current workspace directory: $CWD/{path}
    WorkspaceRelative { path: String },
}

impl ConnectorDef {
    /// Validate a connector definition for correctness.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.id.is_empty() {
            return Err("Connector id must not be empty".into());
        }
        if !self
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!(
                "Connector id '{}' must be lowercase alphanumeric with hyphens only",
                self.id
            ));
        }
        if self.display_name.is_empty() {
            return Err("Connector display_name must not be empty".into());
        }
        if self.servers_key.is_empty() {
            return Err("Connector servers_key must not be empty".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_good_def() {
        let def = ConnectorDef {
            id: "my-host".into(),
            display_name: "My Host".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".my-host/mcp.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        assert!(def.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_id() {
        let def = ConnectorDef {
            id: "".into(),
            display_name: "X".into(),
            format: ConfigFormat::Json,
            servers_key: "s".into(),
            config_path: ConfigPathDef::HomeRelative { path: "x".into() },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        assert!(def.validate().is_err());
    }

    #[test]
    fn test_validate_uppercase_id() {
        let def = ConnectorDef {
            id: "MyHost".into(),
            display_name: "X".into(),
            format: ConfigFormat::Json,
            servers_key: "s".into(),
            config_path: ConfigPathDef::HomeRelative { path: "x".into() },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        assert!(def.validate().is_err());
    }

    #[test]
    fn test_serde_roundtrip_json() {
        let def = ConnectorDef {
            id: "test".into(),
            display_name: "Test Host".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".test.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        let toml_str = toml::to_string_pretty(&def).unwrap();
        let back: ConnectorDef = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.id, "test");
        assert_eq!(back.display_name, "Test Host");
    }

    #[test]
    fn test_validate_empty_display_name() {
        let def = ConnectorDef {
            id: "valid-id".into(),
            display_name: "".into(),
            format: ConfigFormat::Json,
            servers_key: "s".into(),
            config_path: ConfigPathDef::HomeRelative { path: "x".into() },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        let result = def.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("display_name"));
    }

    #[test]
    fn test_validate_empty_servers_key() {
        let def = ConnectorDef {
            id: "valid-id".into(),
            display_name: "Valid Name".into(),
            format: ConfigFormat::Json,
            servers_key: "".into(),
            config_path: ConfigPathDef::HomeRelative { path: "x".into() },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        let result = def.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("servers_key"));
    }

    #[test]
    fn test_validate_id_with_special_chars() {
        let def = ConnectorDef {
            id: "host_name".into(), // underscore not allowed
            display_name: "Host".into(),
            format: ConfigFormat::Json,
            servers_key: "servers".into(),
            config_path: ConfigPathDef::HomeRelative { path: "x".into() },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        assert!(def.validate().is_err());
    }

    #[test]
    fn test_validate_id_with_numbers() {
        let def = ConnectorDef {
            id: "host-123".into(), // numbers allowed
            display_name: "Host 123".into(),
            format: ConfigFormat::Json,
            servers_key: "servers".into(),
            config_path: ConfigPathDef::HomeRelative { path: "x".into() },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };
        assert!(def.validate().is_ok());
    }

    #[test]
    fn test_config_format_default() {
        let format = ConfigFormat::default();
        assert_eq!(format, ConfigFormat::Json);
    }

    #[test]
    fn test_config_format_serialization() {
        assert_eq!(
            serde_json::to_string(&ConfigFormat::Json).unwrap(),
            "\"json\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigFormat::Toml).unwrap(),
            "\"toml\""
        );
    }

    #[test]
    fn test_config_path_def_home_relative() {
        let path_def = ConfigPathDef::HomeRelative {
            path: ".config/app.json".into(),
        };
        let json = serde_json::to_string(&path_def).unwrap();
        assert!(json.contains("home_relative"));
        assert!(json.contains(".config/app.json"));
    }

    #[test]
    fn test_config_path_def_platform_specific() {
        let path_def = ConfigPathDef::PlatformSpecific {
            macos: "Library/App/config.json".into(),
            linux: "app/config.json".into(),
            windows: "App/config.json".into(),
        };
        let json = serde_json::to_string(&path_def).unwrap();
        assert!(json.contains("platform_specific"));
        assert!(json.contains("macos"));
        assert!(json.contains("linux"));
        assert!(json.contains("windows"));
    }

    #[test]
    fn test_config_path_def_workspace_relative() {
        let path_def = ConfigPathDef::WorkspaceRelative {
            path: ".vscode/settings.json".into(),
        };
        let json = serde_json::to_string(&path_def).unwrap();
        assert!(json.contains("workspace_relative"));
        assert!(json.contains(".vscode/settings.json"));
    }

    #[test]
    fn test_connector_def_with_write_extra_fields() {
        let mut extra = BTreeMap::new();
        extra.insert("type".to_string(), serde_json::json!("stdio"));
        extra.insert("enabled".to_string(), serde_json::json!(true));

        let def = ConnectorDef {
            id: "test-host".into(),
            display_name: "Test Host".into(),
            format: ConfigFormat::Json,
            servers_key: "servers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".test.json".into(),
            },
            write_extra_fields: extra,
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };

        assert!(def.validate().is_ok());
        assert_eq!(def.write_extra_fields.len(), 2);
    }

    #[test]
    fn test_connector_def_with_read_filter() {
        let mut filter = BTreeMap::new();
        filter.insert("type".to_string(), serde_json::json!("stdio"));

        let def = ConnectorDef {
            id: "filtered-host".into(),
            display_name: "Filtered Host".into(),
            format: ConfigFormat::Json,
            servers_key: "servers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".test.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: filter,
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };

        assert!(def.validate().is_ok());
        assert_eq!(def.read_filter.len(), 1);
    }

    #[test]
    fn test_connector_def_toml_format() {
        let def = ConnectorDef {
            id: "toml-host".into(),
            display_name: "TOML Host".into(),
            format: ConfigFormat::Toml,
            servers_key: "mcp_servers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".config.toml".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        };

        assert!(def.validate().is_ok());
        assert_eq!(def.format, ConfigFormat::Toml);
    }

    #[test]
    fn test_connector_def_clone() {
        let def = ConnectorDef {
            id: "clone-test".into(),
            display_name: "Clone Test".into(),
            format: ConfigFormat::Json,
            servers_key: "servers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".test.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: true,
            command_optional_on_read: true,
        };

        let cloned = def.clone();
        assert_eq!(cloned.id, def.id);
        assert_eq!(cloned.display_name, def.display_name);
        assert_eq!(cloned.preserve_unknown_entry_fields, true);
        assert_eq!(cloned.command_optional_on_read, true);
    }
}
