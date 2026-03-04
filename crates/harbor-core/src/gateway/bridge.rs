use crate::config::{HarborConfig, ServerConfig};
use crate::error::{HarborError, Result};
use crate::gateway::stdio::{JsonRpcResponse, StdioBridge};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Info about a tool exposed by an MCP server
#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
    /// Which MCP server provides this tool
    pub server: String,
}

/// Manages multiple StdioBridges — one per running MCP server.
/// Provides a unified tool directory and request routing.
pub struct BridgeManager {
    bridges: Arc<Mutex<HashMap<String, StdioBridge>>>,
    tool_cache: Arc<Mutex<Vec<ToolInfo>>>,
}

impl BridgeManager {
    pub fn new() -> Self {
        Self {
            bridges: Arc::new(Mutex::new(HashMap::new())),
            tool_cache: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start a bridge for a server, initialize it, and cache its tools.
    /// Tools are filtered according to the server's `tool_allowlist`, `tool_blocklist`,
    /// and `tool_hosts` configuration.
    pub async fn start_server(
        &self,
        name: &str,
        config: &ServerConfig,
        resolved_env: &BTreeMap<String, String>,
    ) -> Result<()> {
        let mut bridges = self.bridges.lock().await;
        if bridges.contains_key(name) {
            return Err(HarborError::ServerAlreadyRunning {
                name: name.to_string(),
            });
        }

        let bridge = StdioBridge::spawn(name, config, resolved_env).await?;

        // Initialize the MCP handshake
        bridge.initialize().await?;

        // Discover tools
        let tools_response = bridge.list_tools().await?;
        if let Some(result) = &tools_response.result {
            if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
                let mut cache = self.tool_cache.lock().await;
                let mut added = 0usize;
                let total = tools.len();

                for tool in tools {
                    let tool_name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unknown");

                    // Apply global tool filters (host-specific filtering happens at query time)
                    if !config.tool_allowed(tool_name, None) {
                        continue;
                    }

                    let description = tool
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(String::from);
                    let input_schema = tool.get("inputSchema").cloned();

                    cache.push(ToolInfo {
                        name: tool_name.to_string(),
                        description,
                        input_schema,
                        server: name.to_string(),
                    });
                    added += 1;
                }

                if added < total {
                    info!(
                        server = name,
                        discovered = total,
                        exposed = added,
                        filtered = total - added,
                        "Discovered tools (filtered)"
                    );
                } else {
                    info!(server = name, tool_count = total, "Discovered tools");
                }
            }
        }

        bridges.insert(name.to_string(), bridge);
        Ok(())
    }

    /// Stop a server bridge.
    pub async fn stop_server(&self, name: &str) -> Result<()> {
        let mut bridges = self.bridges.lock().await;
        let bridge = bridges
            .remove(name)
            .ok_or_else(|| HarborError::ServerNotRunning {
                name: name.to_string(),
            })?;

        bridge.shutdown().await?;

        // Remove tools from cache
        let mut cache = self.tool_cache.lock().await;
        cache.retain(|t| t.server != name);

        Ok(())
    }

    /// Start all enabled servers from config.
    pub async fn start_all(&self, config: &HarborConfig) -> Result<()> {
        for (name, server_config) in &config.servers {
            if !server_config.enabled {
                continue;
            }

            let resolved_env = resolve_env(&server_config.env);
            match self.start_server(name, server_config, &resolved_env).await {
                Ok(()) => info!(server = %name, "Server started via gateway"),
                Err(e) => warn!(server = %name, error = %e, "Failed to start server"),
            }
        }
        Ok(())
    }

    /// Stop all running servers.
    pub async fn stop_all(&self) -> Result<()> {
        let mut bridges = self.bridges.lock().await;
        let names: Vec<String> = bridges.keys().cloned().collect();
        for name in names {
            if let Some(bridge) = bridges.remove(&name) {
                if let Err(e) = bridge.shutdown().await {
                    warn!(server = %name, error = %e, "Failed to stop server");
                }
            }
        }
        self.tool_cache.lock().await.clear();
        Ok(())
    }

    /// Get the full tool directory.
    pub async fn list_tools(&self) -> Vec<ToolInfo> {
        self.tool_cache.lock().await.clone()
    }

    /// Get tools filtered for a specific host.
    /// Applies host-specific `tool_hosts` overrides from config.
    pub async fn list_tools_for_host(&self, host: &str, config: &HarborConfig) -> Vec<ToolInfo> {
        let cache = self.tool_cache.lock().await;
        cache
            .iter()
            .filter(|tool| config.tool_allowed(&tool.server, &tool.name, Some(host)))
            .cloned()
            .collect()
    }

    /// Call a tool, routing to the correct server bridge.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<JsonRpcResponse> {
        // Find which server owns this tool
        let server_name = {
            let cache = self.tool_cache.lock().await;
            cache
                .iter()
                .find(|t| t.name == tool_name)
                .map(|t| t.server.clone())
        };

        let server_name = server_name.ok_or_else(|| HarborError::ConnectorError {
            host: "gateway".to_string(),
            reason: format!("Tool '{tool_name}' not found in any running server"),
        })?;

        let bridges = self.bridges.lock().await;
        let bridge = bridges
            .get(&server_name)
            .ok_or_else(|| HarborError::ServerNotRunning {
                name: server_name.clone(),
            })?;

        bridge.call_tool(tool_name, arguments).await
    }

    /// Forward a raw JSON-RPC request to a specific server.
    pub async fn forward_to_server(
        &self,
        server_name: &str,
        request: crate::gateway::stdio::JsonRpcRequest,
    ) -> Result<JsonRpcResponse> {
        let bridges = self.bridges.lock().await;
        let bridge = bridges
            .get(server_name)
            .ok_or_else(|| HarborError::ServerNotRunning {
                name: server_name.to_string(),
            })?;

        bridge.send(request).await
    }

    /// Reload: re-read config, start new servers, stop removed/disabled ones.
    /// Returns the names of servers that were started or stopped.
    pub async fn reload(&self, config: &HarborConfig) -> Result<(Vec<String>, Vec<String>)> {
        let running: Vec<String> = self.bridges.lock().await.keys().cloned().collect();
        let desired: Vec<String> = config
            .servers
            .iter()
            .filter(|(_, sc)| sc.enabled)
            .map(|(name, _)| name.clone())
            .collect();

        // Stop servers no longer in config or disabled
        let mut stopped = Vec::new();
        for name in &running {
            if !desired.contains(name) {
                if let Err(e) = self.stop_server(name).await {
                    warn!(server = %name, error = %e, "Failed to stop server during reload");
                } else {
                    info!(server = %name, "Server stopped during reload");
                    stopped.push(name.clone());
                }
            }
        }

        // Start new servers not yet running
        let mut started = Vec::new();
        for name in &desired {
            if !running.contains(name) {
                let server_config = &config.servers[name];
                let resolved_env = resolve_env(&server_config.env);
                match self.start_server(name, server_config, &resolved_env).await {
                    Ok(()) => {
                        info!(server = %name, "Server started during reload");
                        started.push(name.clone());
                    }
                    Err(e) => {
                        warn!(server = %name, error = %e, "Failed to start server during reload")
                    }
                }
            }
        }

        Ok((started, stopped))
    }

    /// Get names of all running servers.
    pub async fn running_servers(&self) -> Vec<String> {
        self.bridges.lock().await.keys().cloned().collect()
    }
}

impl Default for BridgeManager {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_env(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    crate::auth::vault::Vault::resolve_env(env)
}
