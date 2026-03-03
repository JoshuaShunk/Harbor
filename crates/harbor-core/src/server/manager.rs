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
