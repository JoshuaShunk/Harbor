use crate::config::ServerConfig;
use crate::error::{HarborError, Result};
use std::collections::BTreeMap;
use tokio::process::{Child, Command};
use tracing::{error, info};

/// Represents a running MCP server process
pub struct ManagedProcess {
    pub name: String,
    pub child: Child,
    pub pid: u32,
}

impl ManagedProcess {
    /// Spawn a new MCP server process from a ServerConfig
    pub async fn spawn(
        name: &str,
        config: &ServerConfig,
        resolved_env: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let command = config.command.as_deref().unwrap_or("unknown");
        info!(server = name, command = %command, "Starting MCP server");

        let mut cmd = Command::new(command);
        cmd.args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Inject resolved environment variables
        for (key, value) in resolved_env {
            cmd.env(key, value);
        }

        let child = cmd.spawn().map_err(|e| HarborError::ServerStartFailed {
            name: name.to_string(),
            reason: e.to_string(),
        })?;

        let pid = child.id().unwrap_or(0);
        info!(server = name, pid = pid, "MCP server started");

        Ok(Self {
            name: name.to_string(),
            child,
            pid,
        })
    }

    /// Stop the process gracefully
    pub async fn stop(&mut self) -> Result<()> {
        info!(server = %self.name, pid = self.pid, "Stopping MCP server");

        self.child.kill().await.map_err(|e| {
            error!(server = %self.name, error = %e, "Failed to stop server");
            HarborError::Io(e)
        })?;

        info!(server = %self.name, "MCP server stopped");
        Ok(())
    }

    /// Spawn a detached background process. Returns the child PID.
    /// stdin/stdout/stderr are all null — use this for daemon mode.
    pub fn spawn_detached(
        name: &str,
        config: &ServerConfig,
        resolved_env: &BTreeMap<String, String>,
    ) -> Result<u32> {
        let command = config.command.as_deref().unwrap_or("unknown");
        info!(server = name, command = %command, "Starting MCP server (detached)");

        let mut cmd = std::process::Command::new(command);
        cmd.args(&config.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        for (key, value) in resolved_env {
            cmd.env(key, value);
        }

        let child = cmd.spawn().map_err(|e| HarborError::ServerStartFailed {
            name: name.to_string(),
            reason: e.to_string(),
        })?;

        let pid = child.id();
        info!(server = name, pid = pid, "MCP server started (detached)");

        // Drop the Child handle without waiting — the child is reparented to
        // init and continues running after the CLI exits.
        drop(child);

        Ok(pid)
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,     // Still running
            Ok(Some(_)) => false, // Exited
            Err(_) => false,      // Error checking = assume dead
        }
    }
}
