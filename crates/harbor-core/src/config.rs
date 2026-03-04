use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Top-level Harbor configuration stored at ~/.harbor/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HarborConfig {
    #[serde(default)]
    pub harbor: HarborSettings,

    #[serde(default)]
    pub servers: BTreeMap<String, ServerConfig>,

    #[serde(default)]
    pub hosts: BTreeMap<String, HostConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarborSettings {
    #[serde(default = "default_gateway_port")]
    pub gateway_port: u16,
}

impl Default for HarborSettings {
    fn default() -> Self {
        Self {
            gateway_port: default_gateway_port(),
        }
    }
}

fn default_gateway_port() -> u16 {
    3100
}

/// Configuration for a single MCP server managed by Harbor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// How the server was sourced (e.g., "npm:@mcp/server-github", manual)
    #[serde(default)]
    pub source: Option<String>,

    /// Command to execute
    pub command: String,

    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables (values may use "vault:key_name" references)
    #[serde(default)]
    pub env: BTreeMap<String, String>,

    /// Whether this server is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether to start this server automatically with Harbor
    #[serde(default)]
    pub auto_start: bool,

    /// Per-host enable/disable overrides
    #[serde(default)]
    pub hosts: BTreeMap<String, bool>,

    /// Allowlist of tool names to expose (None = all tools exposed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_allowlist: Option<Vec<String>>,

    /// Blocklist of tool names to hide (applied after allowlist)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_blocklist: Option<Vec<String>>,

    /// Per-host tool allowlist overrides (overrides global allowlist/blocklist for that host)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_hosts: BTreeMap<String, Vec<String>>,
}

fn default_true() -> bool {
    true
}

impl ServerConfig {
    /// Check if a tool is allowed by this server's filter rules.
    ///
    /// Resolution order:
    /// 1. If a host-specific override exists in `tool_hosts`, only those tools are allowed.
    /// 2. Otherwise, apply global `tool_allowlist` (if set, tool must be in it).
    /// 3. Then apply global `tool_blocklist` (if set, tool must NOT be in it).
    pub fn tool_allowed(&self, tool_name: &str, host: Option<&str>) -> bool {
        // Check host-specific override first
        if let Some(host_name) = host {
            if let Some(host_tools) = self.tool_hosts.get(host_name) {
                return host_tools.iter().any(|t| t == tool_name);
            }
        }

        // Apply global allowlist
        if let Some(ref allowlist) = self.tool_allowlist {
            if !allowlist.iter().any(|t| t == tool_name) {
                return false;
            }
        }

        // Apply global blocklist
        if let Some(ref blocklist) = self.tool_blocklist {
            if blocklist.iter().any(|t| t == tool_name) {
                return false;
            }
        }

        true
    }
}

/// Configuration for a connected host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    /// Whether this host connector is active
    #[serde(default)]
    pub connected: bool,

    /// Scope for hosts that support it (e.g., "user" or "project" for Claude Code)
    #[serde(default)]
    pub scope: Option<String>,

    /// When true, sync writes a Harbor proxy entry instead of direct server entries.
    /// The proxy routes through the Harbor gateway for runtime tool filtering.
    #[serde(default)]
    pub proxy_mode: bool,
}

impl HarborConfig {
    /// Returns the default config directory: ~/.harbor/
    pub fn default_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| HarborError::ConfigNotFound {
            path: PathBuf::from("~/.harbor"),
        })?;
        Ok(home.join(".harbor"))
    }

    /// Returns the default config file path: ~/.harbor/config.toml
    pub fn default_path() -> Result<PathBuf> {
        Ok(Self::default_dir()?.join("config.toml"))
    }

    /// Load config from the default path, creating a default if it doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::default_path()?;
        Self::load_from(&path)
    }

    /// Load config from a specific path
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path).map_err(HarborError::Io)?;
        let config: HarborConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to the default path
    pub fn save(&self) -> Result<()> {
        let path = Self::default_path()?;
        self.save_to(&path)
    }

    /// Save config to a specific path
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(HarborError::Io)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content).map_err(HarborError::Io)?;
        Ok(())
    }

    /// Add a new server to the config
    pub fn add_server(&mut self, name: String, config: ServerConfig) -> Result<()> {
        if self.servers.contains_key(&name) {
            return Err(HarborError::ServerAlreadyExists { name });
        }
        self.servers.insert(name, config);
        Ok(())
    }

    /// Remove a server from the config
    pub fn remove_server(&mut self, name: &str) -> Result<ServerConfig> {
        self.servers
            .remove(name)
            .ok_or_else(|| HarborError::ServerNotFound {
                name: name.to_string(),
            })
    }

    /// Get a server config by name
    pub fn get_server(&self, name: &str) -> Result<&ServerConfig> {
        self.servers
            .get(name)
            .ok_or_else(|| HarborError::ServerNotFound {
                name: name.to_string(),
            })
    }

    /// Check if a server is enabled for a specific host
    pub fn server_enabled_for_host(&self, server_name: &str, host_name: &str) -> bool {
        if let Some(server) = self.servers.get(server_name) {
            if !server.enabled {
                return false;
            }
            // If host-specific override exists, use it; otherwise default to enabled
            server.hosts.get(host_name).copied().unwrap_or(true)
        } else {
            false
        }
    }

    /// Check if a specific tool is allowed for a server, optionally scoped to a host.
    /// Returns true if the tool passes the server's filter rules.
    pub fn tool_allowed(&self, server_name: &str, tool_name: &str, host: Option<&str>) -> bool {
        if let Some(server) = self.servers.get(server_name) {
            server.tool_allowed(tool_name, host)
        } else {
            false
        }
    }

    /// Get all servers enabled for a specific host
    pub fn servers_for_host(&self, host_name: &str) -> Vec<(&String, &ServerConfig)> {
        self.servers
            .iter()
            .filter(|(name, _)| self.server_enabled_for_host(name, host_name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HarborConfig::default();
        assert_eq!(config.harbor.gateway_port, 3100);
        assert!(config.servers.is_empty());
        assert!(config.hosts.is_empty());
    }

    #[test]
    fn test_add_remove_server() {
        let mut config = HarborConfig::default();
        let server = ServerConfig {
            source: None,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "test-server".to_string()],
            env: BTreeMap::new(),
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        };

        config.add_server("test".to_string(), server).unwrap();
        assert!(config.servers.contains_key("test"));

        // Duplicate should fail
        let server2 = ServerConfig {
            source: None,
            command: "node".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        };
        assert!(config.add_server("test".to_string(), server2).is_err());

        config.remove_server("test").unwrap();
        assert!(!config.servers.contains_key("test"));
    }

    #[test]
    fn test_server_host_filtering() {
        let mut config = HarborConfig::default();
        let mut hosts = BTreeMap::new();
        hosts.insert("claude".to_string(), true);
        hosts.insert("codex".to_string(), false);

        let server = ServerConfig {
            source: None,
            command: "npx".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            enabled: true,
            auto_start: false,
            hosts,
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        };

        config.add_server("test".to_string(), server).unwrap();

        assert!(config.server_enabled_for_host("test", "claude"));
        assert!(!config.server_enabled_for_host("test", "codex"));
        assert!(config.server_enabled_for_host("test", "vscode")); // default: enabled
    }

    /// Helper to create a basic ServerConfig for tests
    fn test_server(command: &str) -> ServerConfig {
        ServerConfig {
            source: None,
            command: command.to_string(),
            args: vec![],
            env: BTreeMap::new(),
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        }
    }

    #[test]
    fn test_roundtrip_toml() {
        let mut config = HarborConfig::default();
        let mut env = BTreeMap::new();
        env.insert("API_KEY".to_string(), "vault:my_key".to_string());

        let mut server = test_server("npx");
        server.source = Some("npm:@mcp/server-github".to_string());
        server.args = vec!["-y".to_string(), "@mcp/server-github".to_string()];
        server.env = env;

        config.add_server("github".to_string(), server).unwrap();

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: HarborConfig = toml::from_str(&serialized).unwrap();
        assert!(deserialized.servers.contains_key("github"));
        assert_eq!(
            deserialized.servers["github"].env["API_KEY"],
            "vault:my_key"
        );
    }

    #[test]
    fn test_save_and_load_from_file() {
        let dir = std::env::temp_dir().join("harbor_test_config");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let mut server = test_server("echo");
        server.args = vec!["hello".to_string()];
        server.auto_start = true;

        let mut config = HarborConfig::default();
        config
            .add_server("test-server".to_string(), server)
            .unwrap();

        config.save_to(&path).unwrap();
        assert!(path.exists());

        let loaded = HarborConfig::load_from(&path).unwrap();
        assert!(loaded.servers.contains_key("test-server"));
        assert_eq!(loaded.servers["test-server"].command, "echo");
        assert!(loaded.servers["test-server"].auto_start);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_from_nonexistent_returns_default() {
        let path = std::env::temp_dir().join("harbor_nonexistent_config.toml");
        let _ = std::fs::remove_file(&path); // ensure it doesn't exist
        let config = HarborConfig::load_from(&path).unwrap();
        assert!(config.servers.is_empty());
        assert_eq!(config.harbor.gateway_port, 3100);
    }

    #[test]
    fn test_get_server() {
        let mut config = HarborConfig::default();
        let mut server = test_server("node");
        server.args = vec!["server.js".to_string()];

        config.add_server("my-server".to_string(), server).unwrap();

        let server = config.get_server("my-server").unwrap();
        assert_eq!(server.command, "node");

        assert!(config.get_server("nonexistent").is_err());
    }

    #[test]
    fn test_remove_nonexistent_server_errors() {
        let mut config = HarborConfig::default();
        let result = config.remove_server("does-not-exist");
        assert!(result.is_err());
    }

    #[test]
    fn test_servers_for_host() {
        let mut config = HarborConfig::default();

        // Server enabled for claude, disabled for codex
        let mut server_a = test_server("cmd-a");
        server_a.hosts.insert("claude".to_string(), true);
        server_a.hosts.insert("codex".to_string(), false);
        config.add_server("server-a".to_string(), server_a).unwrap();

        // Server with no host overrides (enabled for all)
        config
            .add_server("server-b".to_string(), test_server("cmd-b"))
            .unwrap();

        // Globally disabled server
        let mut server_c = test_server("cmd-c");
        server_c.enabled = false;
        config.add_server("server-c".to_string(), server_c).unwrap();

        let claude_servers = config.servers_for_host("claude");
        let names: Vec<&str> = claude_servers.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"server-a"));
        assert!(names.contains(&"server-b"));
        assert!(!names.contains(&"server-c"));

        let codex_servers = config.servers_for_host("codex");
        let names: Vec<&str> = codex_servers.iter().map(|(n, _)| n.as_str()).collect();
        assert!(!names.contains(&"server-a")); // disabled for codex
        assert!(names.contains(&"server-b"));
        assert!(!names.contains(&"server-c")); // globally disabled
    }

    #[test]
    fn test_tool_allowed_no_filters() {
        let server = test_server("cmd");
        // No filters = all tools allowed
        assert!(server.tool_allowed("any_tool", None));
        assert!(server.tool_allowed("any_tool", Some("claude")));
    }

    #[test]
    fn test_tool_allowed_allowlist() {
        let mut server = test_server("cmd");
        server.tool_allowlist = Some(vec!["get_issues".to_string(), "create_issue".to_string()]);

        assert!(server.tool_allowed("get_issues", None));
        assert!(server.tool_allowed("create_issue", None));
        assert!(!server.tool_allowed("delete_repo", None));
    }

    #[test]
    fn test_tool_allowed_blocklist() {
        let mut server = test_server("cmd");
        server.tool_blocklist = Some(vec!["dangerous_tool".to_string()]);

        assert!(server.tool_allowed("safe_tool", None));
        assert!(!server.tool_allowed("dangerous_tool", None));
    }

    #[test]
    fn test_tool_allowed_allowlist_and_blocklist() {
        let mut server = test_server("cmd");
        server.tool_allowlist = Some(vec![
            "tool_a".to_string(),
            "tool_b".to_string(),
            "tool_c".to_string(),
        ]);
        server.tool_blocklist = Some(vec!["tool_c".to_string()]);

        assert!(server.tool_allowed("tool_a", None));
        assert!(server.tool_allowed("tool_b", None));
        assert!(!server.tool_allowed("tool_c", None)); // blocked even though in allowlist
        assert!(!server.tool_allowed("tool_d", None)); // not in allowlist
    }

    #[test]
    fn test_tool_allowed_host_override() {
        let mut server = test_server("cmd");
        server.tool_allowlist = Some(vec![
            "tool_a".to_string(),
            "tool_b".to_string(),
            "tool_c".to_string(),
        ]);
        server
            .tool_hosts
            .insert("claude".to_string(), vec!["tool_a".to_string()]);

        // Global: tool_a, tool_b, tool_c allowed
        assert!(server.tool_allowed("tool_a", None));
        assert!(server.tool_allowed("tool_b", None));
        assert!(server.tool_allowed("tool_c", None));

        // Claude host override: only tool_a
        assert!(server.tool_allowed("tool_a", Some("claude")));
        assert!(!server.tool_allowed("tool_b", Some("claude")));
        assert!(!server.tool_allowed("tool_c", Some("claude")));

        // Cursor: no override, falls back to global
        assert!(server.tool_allowed("tool_a", Some("cursor")));
        assert!(server.tool_allowed("tool_b", Some("cursor")));
    }

    #[test]
    fn test_tool_allowed_via_config() {
        let mut config = HarborConfig::default();
        let mut server = test_server("cmd");
        server.tool_allowlist = Some(vec!["get_issues".to_string()]);
        config.add_server("github".to_string(), server).unwrap();

        assert!(config.tool_allowed("github", "get_issues", None));
        assert!(!config.tool_allowed("github", "delete_repo", None));
        assert!(!config.tool_allowed("nonexistent", "anything", None));
    }

    #[test]
    fn test_tool_filter_roundtrip_toml() {
        let mut config = HarborConfig::default();
        let mut server = test_server("npx");
        server.tool_allowlist = Some(vec!["tool_a".to_string(), "tool_b".to_string()]);
        server.tool_blocklist = Some(vec!["tool_c".to_string()]);
        server
            .tool_hosts
            .insert("claude".to_string(), vec!["tool_a".to_string()]);

        config.add_server("test".to_string(), server).unwrap();

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: HarborConfig = toml::from_str(&serialized).unwrap();

        let s = &deserialized.servers["test"];
        assert_eq!(
            s.tool_allowlist.as_ref().unwrap(),
            &vec!["tool_a".to_string(), "tool_b".to_string()]
        );
        assert_eq!(
            s.tool_blocklist.as_ref().unwrap(),
            &vec!["tool_c".to_string()]
        );
        assert_eq!(s.tool_hosts["claude"], vec!["tool_a".to_string()]);
    }
}
