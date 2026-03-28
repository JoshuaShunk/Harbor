use crate::config::ServerConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The source tag written into a local `ServerConfig` when it was pulled from a fleet.
pub const FLEET_SOURCE: &str = "fleet";

/// Shareable team fleet configuration stored in a git repository.
///
/// Only contains server definitions — no per-machine state (enabled,
/// auto_start, host connections). Safe to commit publicly because all
/// secret values use `vault:key_name` references, never raw values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetConfig {
    #[serde(default)]
    pub fleet: FleetMeta,

    /// Servers shared across the team, keyed by server name.
    #[serde(default)]
    pub servers: BTreeMap<String, FleetServerDef>,
}

/// Metadata about the fleet itself.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetMeta {
    /// Human-readable name for this fleet (e.g., "acme-team").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Optional description shown in `harbor crew status`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A server definition suitable for team sharing.
///
/// Mirrors the core fields of `ServerConfig` but omits per-machine state
/// (`enabled`, `auto_start`, `hosts`, `tool_hosts`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetServerDef {
    /// Optional human-readable description (fleet-only metadata).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Command to execute (for stdio servers). Mutually exclusive with `url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Arguments passed to the command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// Environment variables. Values may use `vault:key_name` references.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,

    /// URL for remote HTTP MCP servers. Mutually exclusive with `command`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Custom HTTP headers. Values may use `vault:key_name` references.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,

    /// Team-recommended tool allowlist (None = expose all tools).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_allowlist: Option<Vec<String>>,

    /// Team-recommended tool blocklist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_blocklist: Option<Vec<String>>,
}

impl FleetServerDef {
    /// Convert to a `ServerConfig` for a freshly pulled server.
    ///
    /// Per-machine state is set to conservative defaults:
    /// - `enabled: true` (the server exists in the fleet, so enable it)
    /// - `auto_start: false` (let users opt in)
    /// - `hosts`, `tool_hosts`: empty (no per-host overrides)
    pub fn to_server_config(&self) -> ServerConfig {
        ServerConfig {
            source: Some(FLEET_SOURCE.to_string()),
            command: self.command.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            url: self.url.clone(),
            headers: self.headers.clone(),
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: self.tool_allowlist.clone(),
            tool_blocklist: self.tool_blocklist.clone(),
            tool_hosts: BTreeMap::new(),
        }
    }

    /// Convert to a `ServerConfig`, preserving per-machine state from an existing local entry.
    ///
    /// Updates the canonical definition (command, args, env, url, headers, tool filters)
    /// while keeping the user's local choices (enabled, auto_start, hosts, tool_hosts).
    pub fn to_server_config_preserving(&self, existing: &ServerConfig) -> ServerConfig {
        ServerConfig {
            source: Some(FLEET_SOURCE.to_string()),
            command: self.command.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            url: self.url.clone(),
            headers: self.headers.clone(),
            // Preserve per-machine state
            enabled: existing.enabled,
            auto_start: existing.auto_start,
            hosts: existing.hosts.clone(),
            // Use team tool filters; keep per-host overrides
            tool_allowlist: self.tool_allowlist.clone(),
            tool_blocklist: self.tool_blocklist.clone(),
            tool_hosts: existing.tool_hosts.clone(),
        }
    }

    /// Build a `FleetServerDef` from a local `ServerConfig` for pushing to the fleet.
    ///
    /// Per-machine state is intentionally dropped.
    pub fn from_server_config(server: &ServerConfig) -> Self {
        Self {
            description: None,
            command: server.command.clone(),
            args: server.args.clone(),
            env: server.env.clone(),
            url: server.url.clone(),
            headers: server.headers.clone(),
            tool_allowlist: server.tool_allowlist.clone(),
            tool_blocklist: server.tool_blocklist.clone(),
        }
    }

    /// Returns true if this fleet definition is equivalent to an existing `ServerConfig`.
    ///
    /// Only compares the shareable fields — ignores per-machine state.
    pub fn is_equivalent_to(&self, server: &ServerConfig) -> bool {
        self.command == server.command
            && self.args == server.args
            && self.env == server.env
            && self.url == server.url
            && self.headers == server.headers
            && self.tool_allowlist == server.tool_allowlist
            && self.tool_blocklist == server.tool_blocklist
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;

    fn github_def() -> FleetServerDef {
        let mut env = BTreeMap::new();
        env.insert("GITHUB_TOKEN".to_string(), "vault:github_token".to_string());
        FleetServerDef {
            description: Some("GitHub MCP server".to_string()),
            command: Some("npx".to_string()),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-github".to_string(),
            ],
            env,
            url: None,
            headers: None,
            tool_allowlist: None,
            tool_blocklist: None,
        }
    }

    fn make_server_config(enabled: bool, auto_start: bool) -> ServerConfig {
        let mut hosts = BTreeMap::new();
        hosts.insert("claude".to_string(), true);
        ServerConfig {
            source: Some(FLEET_SOURCE.to_string()),
            command: Some("npx".to_string()),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-github".to_string(),
            ],
            env: {
                let mut m = BTreeMap::new();
                m.insert("GITHUB_TOKEN".to_string(), "vault:github_token".to_string());
                m
            },
            url: None,
            headers: None,
            enabled,
            auto_start,
            hosts,
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        }
    }

    // ── TOML round-trip ───────────────────────────────────────────────────────

    #[test]
    fn empty_fleet_config_round_trips() {
        let config = FleetConfig::default();
        let toml = toml::to_string_pretty(&config).unwrap();
        let back: FleetConfig = toml::from_str(&toml).unwrap();
        assert!(back.servers.is_empty());
        assert!(back.fleet.name.is_none());
    }

    #[test]
    fn fleet_config_with_server_round_trips() {
        let mut config = FleetConfig::default();
        config.fleet.name = Some("acme-fleet".to_string());
        config.fleet.description = Some("Acme team servers".to_string());
        config.servers.insert("github".to_string(), github_def());

        let toml = toml::to_string_pretty(&config).unwrap();
        let back: FleetConfig = toml::from_str(&toml).unwrap();

        assert_eq!(back.fleet.name.as_deref(), Some("acme-fleet"));
        let def = back.servers.get("github").unwrap();
        assert_eq!(def.command.as_deref(), Some("npx"));
        assert_eq!(def.args, ["-y", "@modelcontextprotocol/server-github"]);
        assert_eq!(def.env.get("GITHUB_TOKEN").unwrap(), "vault:github_token");
        assert_eq!(def.description.as_deref(), Some("GitHub MCP server"));
    }

    #[test]
    fn remote_server_def_round_trips() {
        let mut headers = BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer vault:slack_token".to_string(),
        );
        let def = FleetServerDef {
            description: None,
            command: None,
            args: vec![],
            env: BTreeMap::new(),
            url: Some("https://mcp.example.com/slack".to_string()),
            headers: Some(headers),
            tool_allowlist: Some(vec!["send_message".to_string()]),
            tool_blocklist: None,
        };
        let mut config = FleetConfig::default();
        config.servers.insert("slack".to_string(), def);

        let toml = toml::to_string_pretty(&config).unwrap();
        let back: FleetConfig = toml::from_str(&toml).unwrap();
        let back_def = back.servers.get("slack").unwrap();

        assert_eq!(
            back_def.url.as_deref(),
            Some("https://mcp.example.com/slack")
        );
        assert_eq!(
            back_def
                .headers
                .as_ref()
                .unwrap()
                .get("Authorization")
                .unwrap(),
            "Bearer vault:slack_token"
        );
        assert_eq!(
            back_def.tool_allowlist.as_ref().map(|v| v.as_slice()),
            Some(["send_message".to_string()].as_slice())
        );
    }

    // ── from_server_config ────────────────────────────────────────────────────

    #[test]
    fn from_server_config_captures_shareable_fields() {
        let sc = make_server_config(false, true);
        let def = FleetServerDef::from_server_config(&sc);
        assert_eq!(def.command, sc.command);
        assert_eq!(def.args, sc.args);
        assert_eq!(def.env, sc.env);
    }

    #[test]
    fn from_server_config_drops_description() {
        // description is fleet-only metadata; from_server_config doesn't invent one
        let sc = make_server_config(true, false);
        let def = FleetServerDef::from_server_config(&sc);
        assert!(def.description.is_none());
    }

    // ── to_server_config ──────────────────────────────────────────────────────

    #[test]
    fn to_server_config_sets_fleet_source_and_defaults() {
        let def = github_def();
        let sc = def.to_server_config();
        assert_eq!(sc.source.as_deref(), Some(FLEET_SOURCE));
        assert_eq!(sc.command, def.command);
        assert_eq!(sc.args, def.args);
        assert_eq!(sc.env, def.env);
        assert!(sc.enabled);
        assert!(!sc.auto_start);
        assert!(sc.hosts.is_empty());
        assert!(sc.tool_hosts.is_empty());
    }

    #[test]
    fn to_server_config_preserving_keeps_per_machine_state() {
        let def = github_def();
        let existing = make_server_config(false, true); // user disabled, auto_start on

        let sc = def.to_server_config_preserving(&existing);

        // Canonical fields updated from fleet def.
        assert_eq!(sc.command, def.command);
        assert_eq!(sc.env, def.env);
        // Per-machine state preserved from existing.
        assert!(!sc.enabled);
        assert!(sc.auto_start);
        assert_eq!(sc.hosts, existing.hosts);
    }

    // ── is_equivalent_to ─────────────────────────────────────────────────────

    #[test]
    fn is_equivalent_to_returns_true_for_matching_config() {
        let def = github_def();
        let sc = def.to_server_config();
        assert!(def.is_equivalent_to(&sc));
    }

    #[test]
    fn is_equivalent_to_ignores_per_machine_state() {
        let def = github_def();
        let mut sc = def.to_server_config();
        sc.enabled = false;
        sc.auto_start = true;
        sc.hosts.insert("cursor".to_string(), true);
        // Per-machine differences must not affect equivalence.
        assert!(def.is_equivalent_to(&sc));
    }

    #[test]
    fn is_equivalent_to_returns_false_when_command_differs() {
        let def = github_def();
        let mut sc = def.to_server_config();
        sc.command = Some("uvx".to_string());
        assert!(!def.is_equivalent_to(&sc));
    }

    #[test]
    fn is_equivalent_to_returns_false_when_env_differs() {
        let def = github_def();
        let mut sc = def.to_server_config();
        sc.env.insert("EXTRA".to_string(), "value".to_string());
        assert!(!def.is_equivalent_to(&sc));
    }

    #[test]
    fn is_equivalent_to_returns_false_when_args_differ() {
        let def = github_def();
        let mut sc = def.to_server_config();
        sc.args.push("--extra-flag".to_string());
        assert!(!def.is_equivalent_to(&sc));
    }

    #[test]
    fn is_equivalent_to_returns_false_when_url_differs() {
        let mut def = FleetServerDef {
            description: None,
            command: None,
            args: vec![],
            env: BTreeMap::new(),
            url: Some("https://original.com".to_string()),
            headers: None,
            tool_allowlist: None,
            tool_blocklist: None,
        };
        let sc = def.to_server_config();
        def.url = Some("https://different.com".to_string());
        assert!(!def.is_equivalent_to(&sc));
    }

    #[test]
    fn is_equivalent_to_returns_false_when_tool_allowlist_differs() {
        let mut def = github_def();
        def.tool_allowlist = Some(vec!["tool_a".to_string()]);
        let sc = def.to_server_config();

        def.tool_allowlist = Some(vec!["tool_b".to_string()]);
        assert!(!def.is_equivalent_to(&sc));
    }

    #[test]
    fn is_equivalent_to_returns_false_when_tool_blocklist_differs() {
        let mut def = github_def();
        def.tool_blocklist = Some(vec!["blocked".to_string()]);
        let sc = def.to_server_config();

        def.tool_blocklist = None;
        assert!(!def.is_equivalent_to(&sc));
    }

    #[test]
    fn fleet_source_constant_value() {
        assert_eq!(FLEET_SOURCE, "fleet");
    }

    #[test]
    fn fleet_meta_default() {
        let meta = FleetMeta::default();
        assert!(meta.name.is_none());
        assert!(meta.description.is_none());
    }

    #[test]
    fn fleet_config_default() {
        let config = FleetConfig::default();
        assert!(config.servers.is_empty());
        assert!(config.fleet.name.is_none());
    }

    #[test]
    fn fleet_server_def_with_tool_filters() {
        let def = FleetServerDef {
            description: Some("Test server".to_string()),
            command: Some("node".to_string()),
            args: vec!["server.js".to_string()],
            env: BTreeMap::new(),
            url: None,
            headers: None,
            tool_allowlist: Some(vec!["tool_a".to_string(), "tool_b".to_string()]),
            tool_blocklist: Some(vec!["dangerous".to_string()]),
        };

        let sc = def.to_server_config();
        assert_eq!(sc.tool_allowlist, def.tool_allowlist);
        assert_eq!(sc.tool_blocklist, def.tool_blocklist);
    }

    #[test]
    fn to_server_config_preserving_updates_tool_filters() {
        let mut def = github_def();
        def.tool_allowlist = Some(vec!["new_tool".to_string()]);

        let existing = make_server_config(true, false);
        let sc = def.to_server_config_preserving(&existing);

        // Tool filters should be updated from fleet def
        assert_eq!(sc.tool_allowlist, Some(vec!["new_tool".to_string()]));
        // But tool_hosts should be preserved from existing
        assert_eq!(sc.tool_hosts, existing.tool_hosts);
    }

    #[test]
    fn from_server_config_captures_tool_filters() {
        let mut sc = make_server_config(true, false);
        sc.tool_allowlist = Some(vec!["allowed".to_string()]);
        sc.tool_blocklist = Some(vec!["blocked".to_string()]);

        let def = FleetServerDef::from_server_config(&sc);
        assert_eq!(def.tool_allowlist, Some(vec!["allowed".to_string()]));
        assert_eq!(def.tool_blocklist, Some(vec!["blocked".to_string()]));
    }

    #[test]
    fn fleet_config_clone() {
        let mut config = FleetConfig::default();
        config.fleet.name = Some("my-fleet".to_string());
        config.servers.insert("test".to_string(), github_def());

        let cloned = config.clone();
        assert_eq!(cloned.fleet.name, config.fleet.name);
        assert!(cloned.servers.contains_key("test"));
    }

    #[test]
    fn fleet_server_def_clone() {
        let def = github_def();
        let cloned = def.clone();
        assert_eq!(cloned.command, def.command);
        assert_eq!(cloned.args, def.args);
        assert_eq!(cloned.env, def.env);
    }

    #[test]
    fn remote_server_to_config() {
        let mut headers = BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer vault:token".to_string(),
        );

        let def = FleetServerDef {
            description: None,
            command: None,
            args: vec![],
            env: BTreeMap::new(),
            url: Some("https://api.example.com/mcp".to_string()),
            headers: Some(headers.clone()),
            tool_allowlist: None,
            tool_blocklist: None,
        };

        let sc = def.to_server_config();
        assert!(sc.command.is_none());
        assert_eq!(sc.url, Some("https://api.example.com/mcp".to_string()));
        assert_eq!(sc.headers, Some(headers));
    }

    #[test]
    fn is_equivalent_to_returns_false_when_headers_differ() {
        let mut headers = BTreeMap::new();
        headers.insert("X-Custom".to_string(), "value1".to_string());

        let def = FleetServerDef {
            description: None,
            command: None,
            args: vec![],
            env: BTreeMap::new(),
            url: Some("https://api.example.com".to_string()),
            headers: Some(headers),
            tool_allowlist: None,
            tool_blocklist: None,
        };

        let mut sc = def.to_server_config();
        sc.headers
            .as_mut()
            .unwrap()
            .insert("X-Custom".to_string(), "value2".to_string());

        assert!(!def.is_equivalent_to(&sc));
    }
}
