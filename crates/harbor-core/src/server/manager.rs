use crate::config::{HarborConfig, ServerConfig};
use crate::error::{HarborError, Result};
use crate::server::process::ManagedProcess;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use tracing::{info, warn};

/// Status of a managed server
#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    pub name: String,
    pub enabled: bool,
    pub running: bool,
    pub pid: Option<u32>,
    pub command: String,
}

/// Manages the lifecycle of MCP server processes
pub struct ServerManager {
    processes: HashMap<String, ManagedProcess>,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    /// Start a server by name using the provided config
    pub async fn start(&mut self, name: &str, config: &ServerConfig) -> Result<()> {
        if self.processes.contains_key(name) {
            return Err(HarborError::ServerAlreadyRunning {
                name: name.to_string(),
            });
        }

        if !config.enabled {
            warn!(server = name, "Server is disabled, not starting");
            return Ok(());
        }

        // Resolve environment variables (replace vault: references with actual values later)
        let resolved_env = self.resolve_env(&config.env);

        let process = ManagedProcess::spawn(name, config, &resolved_env).await?;
        self.processes.insert(name.to_string(), process);
        Ok(())
    }

    /// Stop a server by name
    pub async fn stop(&mut self, name: &str) -> Result<()> {
        let mut process = self.processes.remove(name).ok_or_else(|| {
            HarborError::ServerNotRunning {
                name: name.to_string(),
            }
        })?;

        process.stop().await?;
        Ok(())
    }

    /// Restart a server
    pub async fn restart(&mut self, name: &str, config: &ServerConfig) -> Result<()> {
        if self.processes.contains_key(name) {
            self.stop(name).await?;
        }
        self.start(name, config).await
    }

    /// Get status of a specific server
    pub fn status(&mut self, name: &str, config: &ServerConfig) -> ServerStatus {
        let (running, pid) = if let Some(process) = self.processes.get_mut(name) {
            let running = process.is_running();
            if !running {
                // Process exited, clean up
                let pid = process.pid;
                info!(server = name, pid = pid, "Server process exited, cleaning up");
                (false, Some(pid))
            } else {
                (true, Some(process.pid))
            }
        } else {
            (false, None)
        };

        // Clean up dead processes
        if !running {
            self.processes.remove(name);
        }

        ServerStatus {
            name: name.to_string(),
            enabled: config.enabled,
            running,
            pid,
            command: format!("{} {}", config.command, config.args.join(" ")),
        }
    }

    /// Get status of all servers in the config
    pub fn status_all(&mut self, config: &HarborConfig) -> Vec<ServerStatus> {
        config
            .servers
            .iter()
            .map(|(name, server_config)| self.status(name, server_config))
            .collect()
    }

    /// Stop all running servers
    pub async fn stop_all(&mut self) -> Result<()> {
        let names: Vec<String> = self.processes.keys().cloned().collect();
        for name in names {
            if let Err(e) = self.stop(&name).await {
                warn!(server = %name, error = %e, "Failed to stop server during shutdown");
            }
        }
        Ok(())
    }

    /// Resolve environment variables, replacing vault: references using the vault.
    fn resolve_env(&self, env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        crate::auth::vault::Vault::resolve_env(env)
    }
}

impl Default for ServerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Helper to create a simple ServerConfig for testing
    fn make_server_config(command: &str, enabled: bool) -> ServerConfig {
        ServerConfig {
            source: None,
            command: command.to_string(),
            args: vec![],
            env: BTreeMap::new(),
            enabled,
            auto_start: false,
            hosts: BTreeMap::new(),
        }
    }

    #[test]
    fn test_new_server_manager_has_no_processes() {
        let manager = ServerManager::new();
        assert!(manager.processes.is_empty());
    }

    #[test]
    fn test_default_server_manager() {
        let manager = ServerManager::default();
        assert!(manager.processes.is_empty());
    }

    #[test]
    fn test_status_for_untracked_server() {
        let mut manager = ServerManager::new();
        let config = make_server_config("echo", true);
        let status = manager.status("my-server", &config);

        assert_eq!(status.name, "my-server");
        assert!(status.enabled);
        assert!(!status.running);
        assert!(status.pid.is_none());
        assert_eq!(status.command, "echo ");
    }

    #[test]
    fn test_status_disabled_server() {
        let mut manager = ServerManager::new();
        let config = make_server_config("echo", false);
        let status = manager.status("disabled-server", &config);

        assert_eq!(status.name, "disabled-server");
        assert!(!status.enabled);
        assert!(!status.running);
    }

    #[test]
    fn test_status_all_with_multiple_servers() {
        let mut manager = ServerManager::new();
        let mut harbor_config = HarborConfig::default();

        harbor_config
            .add_server("server-a".to_string(), make_server_config("cmd-a", true))
            .unwrap();
        harbor_config
            .add_server("server-b".to_string(), make_server_config("cmd-b", false))
            .unwrap();

        let statuses = manager.status_all(&harbor_config);
        assert_eq!(statuses.len(), 2);

        let names: Vec<&str> = statuses.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"server-a"));
        assert!(names.contains(&"server-b"));
    }

    #[tokio::test]
    async fn test_stop_nonexistent_server_errors() {
        let mut manager = ServerManager::new();
        let result = manager.stop("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_disabled_server_is_noop() {
        let mut manager = ServerManager::new();
        let config = make_server_config("echo", false);
        // Starting a disabled server should succeed but not add a process
        let result = manager.start("disabled", &config).await;
        assert!(result.is_ok());
        assert!(manager.processes.is_empty());
    }

    #[tokio::test]
    async fn test_stop_all_with_no_processes() {
        let mut manager = ServerManager::new();
        let result = manager.stop_all().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_env_passes_through_plain_values() {
        let manager = ServerManager::new();
        let mut env = BTreeMap::new();
        env.insert("PLAIN_KEY".to_string(), "plain_value".to_string());

        let resolved = manager.resolve_env(&env);
        assert_eq!(resolved.get("PLAIN_KEY").unwrap(), "plain_value");
    }

    #[test]
    fn test_server_status_command_format() {
        let mut manager = ServerManager::new();
        let config = ServerConfig {
            source: None,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@mcp/server".to_string()],
            env: BTreeMap::new(),
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
        };
        let status = manager.status("test", &config);
        assert_eq!(status.command, "npx -y @mcp/server");
    }
}
