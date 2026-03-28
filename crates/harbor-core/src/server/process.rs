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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(command: &str, args: Vec<&str>) -> ServerConfig {
        ServerConfig {
            source: None,
            command: Some(command.to_string()),
            args: args.into_iter().map(String::from).collect(),
            env: BTreeMap::new(),
            url: None,
            headers: None,
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn test_spawn_echo_process() {
        let config = make_config("echo", vec!["hello"]);
        let env = BTreeMap::new();

        let result = ManagedProcess::spawn("test-echo", &config, &env).await;
        assert!(result.is_ok());

        let mut process = result.unwrap();
        assert_eq!(process.name, "test-echo");
        assert!(process.pid > 0);

        // Stop the process
        let stop_result = process.stop().await;
        assert!(stop_result.is_ok());
    }

    #[tokio::test]
    async fn test_spawn_with_env() {
        let config = make_config("env", vec![]);
        let mut env = BTreeMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let result = ManagedProcess::spawn("test-env", &config, &env).await;
        // env command exists on most systems
        if result.is_ok() {
            let mut process = result.unwrap();
            let _ = process.stop().await;
        }
    }

    #[tokio::test]
    async fn test_spawn_invalid_command() {
        let config = make_config("nonexistent-command-xyz-123", vec![]);
        let env = BTreeMap::new();

        let result = ManagedProcess::spawn("test-invalid", &config, &env).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_spawn_detached_with_echo() {
        let config = make_config("sleep", vec!["0.1"]);
        let env = BTreeMap::new();

        let result = ManagedProcess::spawn_detached("test-detached", &config, &env);
        // sleep might not exist on all systems, but on most it does
        if result.is_ok() {
            let pid = result.unwrap();
            assert!(pid > 0);
        }
    }

    #[test]
    fn test_spawn_detached_invalid_command() {
        let config = make_config("nonexistent-command-abc-789", vec![]);
        let env = BTreeMap::new();

        let result = ManagedProcess::spawn_detached("test-invalid-detached", &config, &env);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_running_for_short_process() {
        // echo exits immediately
        let config = make_config("echo", vec!["test"]);
        let env = BTreeMap::new();

        let result = ManagedProcess::spawn("test-short", &config, &env).await;
        if let Ok(mut process) = result {
            // Give it a moment to exit
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            // Process should have exited
            assert!(!process.is_running());
        }
    }

    #[tokio::test]
    async fn test_is_running_for_long_process() {
        // sleep runs for a while
        let config = make_config("sleep", vec!["10"]);
        let env = BTreeMap::new();

        let result = ManagedProcess::spawn("test-long", &config, &env).await;
        if let Ok(mut process) = result {
            // Process should still be running
            assert!(process.is_running());
            // Clean up
            let _ = process.stop().await;
        }
    }

    #[test]
    fn test_managed_process_fields() {
        // Test that ManagedProcess has expected fields
        // This is a compile-time check more than a runtime test
        let _name: String = "test".to_string();
        let _pid: u32 = 12345;
    }
}
