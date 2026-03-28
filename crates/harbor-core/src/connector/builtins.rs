use crate::connector::def::{ConfigFormat, ConfigPathDef, ConnectorDef};
use std::collections::BTreeMap;

/// Returns all built-in connector definitions.
///
/// Each definition is pure data — no logic, no per-host code.
/// The [`GenericConnector`] turns these into full [`Connector`] implementations.
pub fn builtin_defs() -> Vec<ConnectorDef> {
    vec![
        // ── Claude Code ──
        ConnectorDef {
            id: "claude".into(),
            display_name: "Claude Code".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".claude.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        },
        // ── Claude Desktop ──
        ConnectorDef {
            id: "claude-desktop".into(),
            display_name: "Claude Desktop".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::PlatformSpecific {
                macos: "Library/Application Support/Claude/claude_desktop_config.json".into(),
                linux: "Claude/claude_desktop_config.json".into(),
                windows: "Claude/claude_desktop_config.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        },
        // ── Codex (TOML) ──
        ConnectorDef {
            id: "codex".into(),
            display_name: "Codex".into(),
            format: ConfigFormat::Toml,
            servers_key: "mcp_servers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".codex/config.toml".into(),
            },
            write_extra_fields: {
                let mut m = BTreeMap::new();
                m.insert("enabled".into(), serde_json::json!(true));
                m
            },
            read_filter: {
                let mut m = BTreeMap::new();
                m.insert("enabled".into(), serde_json::json!(true));
                m
            },
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        },
        // ── VS Code ──
        ConnectorDef {
            id: "vscode".into(),
            display_name: "VS Code".into(),
            format: ConfigFormat::Json,
            servers_key: "servers".into(),
            config_path: ConfigPathDef::WorkspaceRelative {
                path: ".vscode/mcp.json".into(),
            },
            write_extra_fields: {
                let mut m = BTreeMap::new();
                m.insert("type".into(), serde_json::json!("stdio"));
                m
            },
            read_filter: {
                let mut m = BTreeMap::new();
                m.insert("type".into(), serde_json::json!("stdio"));
                m
            },
            preserve_unknown_entry_fields: true,
            command_optional_on_read: true,
        },
        // ── Cursor ──
        ConnectorDef {
            id: "cursor".into(),
            display_name: "Cursor".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".cursor/mcp.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        },
        // ── Cline ──
        ConnectorDef {
            id: "cline".into(),
            display_name: "Cline".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::PlatformSpecific {
                macos: "Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json".into(),
                linux: "Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json".into(),
                windows: "Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: true,
            command_optional_on_read: false,
        },
        // ── Roo Code ──
        ConnectorDef {
            id: "roo-code".into(),
            display_name: "Roo Code".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::PlatformSpecific {
                macos: "Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json".into(),
                linux: "Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json".into(),
                windows: "Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: true,
            command_optional_on_read: false,
        },
        // ── Windsurf ──
        ConnectorDef {
            id: "windsurf".into(),
            display_name: "Windsurf".into(),
            format: ConfigFormat::Json,
            servers_key: "mcpServers".into(),
            config_path: ConfigPathDef::HomeRelative {
                path: ".codeium/windsurf/mcp_config.json".into(),
            },
            write_extra_fields: BTreeMap::new(),
            read_filter: BTreeMap::new(),
            preserve_unknown_entry_fields: false,
            command_optional_on_read: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_builtins_validate() {
        for def in builtin_defs() {
            assert!(
                def.validate().is_ok(),
                "Built-in '{}' failed validation: {:?}",
                def.id,
                def.validate()
            );
        }
    }

    #[test]
    fn test_builtin_count() {
        assert_eq!(builtin_defs().len(), 8);
    }

    #[test]
    fn test_builtin_ids_unique() {
        let defs = builtin_defs();
        let mut ids: Vec<&str> = defs.iter().map(|d| d.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), defs.len(), "Duplicate built-in connector IDs");
    }

    #[test]
    fn test_claude_connector_def() {
        let defs = builtin_defs();
        let claude = defs.iter().find(|d| d.id == "claude").unwrap();

        assert_eq!(claude.display_name, "Claude Code");
        assert_eq!(claude.format, ConfigFormat::Json);
        assert_eq!(claude.servers_key, "mcpServers");
        assert!(claude.write_extra_fields.is_empty());
        assert!(claude.read_filter.is_empty());
        assert!(!claude.preserve_unknown_entry_fields);
        assert!(!claude.command_optional_on_read);
    }

    #[test]
    fn test_claude_desktop_connector_def() {
        let defs = builtin_defs();
        let desktop = defs.iter().find(|d| d.id == "claude-desktop").unwrap();

        assert_eq!(desktop.display_name, "Claude Desktop");
        assert_eq!(desktop.format, ConfigFormat::Json);
        assert_eq!(desktop.servers_key, "mcpServers");

        // Should have platform-specific paths
        match &desktop.config_path {
            ConfigPathDef::PlatformSpecific {
                macos,
                linux,
                windows,
            } => {
                assert!(macos.contains("Library/Application Support"));
                assert!(linux.contains("Claude"));
                assert!(windows.contains("Claude"));
            }
            _ => panic!("Expected PlatformSpecific config path"),
        }
    }

    #[test]
    fn test_codex_connector_def() {
        let defs = builtin_defs();
        let codex = defs.iter().find(|d| d.id == "codex").unwrap();

        assert_eq!(codex.display_name, "Codex");
        assert_eq!(codex.format, ConfigFormat::Toml);
        assert_eq!(codex.servers_key, "mcp_servers");

        // Should write enabled = true
        assert_eq!(
            codex.write_extra_fields.get("enabled"),
            Some(&serde_json::json!(true))
        );

        // Should filter by enabled = true
        assert_eq!(
            codex.read_filter.get("enabled"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn test_vscode_connector_def() {
        let defs = builtin_defs();
        let vscode = defs.iter().find(|d| d.id == "vscode").unwrap();

        assert_eq!(vscode.display_name, "VS Code");
        assert_eq!(vscode.format, ConfigFormat::Json);
        assert_eq!(vscode.servers_key, "servers");

        // Should write type = stdio
        assert_eq!(
            vscode.write_extra_fields.get("type"),
            Some(&serde_json::json!("stdio"))
        );

        // Should filter by type = stdio
        assert_eq!(
            vscode.read_filter.get("type"),
            Some(&serde_json::json!("stdio"))
        );

        // VS Code preserves unknown fields and has optional command
        assert!(vscode.preserve_unknown_entry_fields);
        assert!(vscode.command_optional_on_read);

        // Should be workspace relative
        match &vscode.config_path {
            ConfigPathDef::WorkspaceRelative { path } => {
                assert_eq!(path, ".vscode/mcp.json");
            }
            _ => panic!("Expected WorkspaceRelative config path"),
        }
    }

    #[test]
    fn test_cursor_connector_def() {
        let defs = builtin_defs();
        let cursor = defs.iter().find(|d| d.id == "cursor").unwrap();

        assert_eq!(cursor.display_name, "Cursor");
        assert_eq!(cursor.format, ConfigFormat::Json);
        assert_eq!(cursor.servers_key, "mcpServers");

        // Should be home relative
        match &cursor.config_path {
            ConfigPathDef::HomeRelative { path } => {
                assert_eq!(path, ".cursor/mcp.json");
            }
            _ => panic!("Expected HomeRelative config path"),
        }
    }

    #[test]
    fn test_cline_connector_def() {
        let defs = builtin_defs();
        let cline = defs.iter().find(|d| d.id == "cline").unwrap();

        assert_eq!(cline.display_name, "Cline");
        assert_eq!(cline.format, ConfigFormat::Json);
        assert_eq!(cline.servers_key, "mcpServers");

        // Cline preserves unknown entry fields
        assert!(cline.preserve_unknown_entry_fields);
        assert!(!cline.command_optional_on_read);

        // Should have platform-specific paths
        match &cline.config_path {
            ConfigPathDef::PlatformSpecific { macos, linux, .. } => {
                assert!(macos.contains("saoudrizwan.claude-dev"));
                assert!(linux.contains("saoudrizwan.claude-dev"));
            }
            _ => panic!("Expected PlatformSpecific config path"),
        }
    }

    #[test]
    fn test_roo_code_connector_def() {
        let defs = builtin_defs();
        let roo = defs.iter().find(|d| d.id == "roo-code").unwrap();

        assert_eq!(roo.display_name, "Roo Code");
        assert_eq!(roo.format, ConfigFormat::Json);
        assert!(roo.preserve_unknown_entry_fields);

        // Should have platform-specific paths
        match &roo.config_path {
            ConfigPathDef::PlatformSpecific { macos, linux, .. } => {
                assert!(macos.contains("rooveterinaryinc.roo-cline"));
                assert!(linux.contains("rooveterinaryinc.roo-cline"));
            }
            _ => panic!("Expected PlatformSpecific config path"),
        }
    }

    #[test]
    fn test_windsurf_connector_def() {
        let defs = builtin_defs();
        let windsurf = defs.iter().find(|d| d.id == "windsurf").unwrap();

        assert_eq!(windsurf.display_name, "Windsurf");
        assert_eq!(windsurf.format, ConfigFormat::Json);
        assert_eq!(windsurf.servers_key, "mcpServers");
        assert!(!windsurf.preserve_unknown_entry_fields);

        // Should be home relative
        match &windsurf.config_path {
            ConfigPathDef::HomeRelative { path } => {
                assert_eq!(path, ".codeium/windsurf/mcp_config.json");
            }
            _ => panic!("Expected HomeRelative config path"),
        }
    }

    #[test]
    fn test_all_builtins_have_display_names() {
        for def in builtin_defs() {
            assert!(
                !def.display_name.is_empty(),
                "Built-in '{}' has empty display_name",
                def.id
            );
        }
    }

    #[test]
    fn test_all_builtins_have_servers_key() {
        for def in builtin_defs() {
            assert!(
                !def.servers_key.is_empty(),
                "Built-in '{}' has empty servers_key",
                def.id
            );
        }
    }

    #[test]
    fn test_builtin_ids_are_lowercase() {
        for def in builtin_defs() {
            assert_eq!(
                def.id,
                def.id.to_lowercase(),
                "Built-in '{}' should have lowercase id",
                def.id
            );
        }
    }
}
