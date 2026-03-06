use crate::config::{HarborConfig, ServerConfig};
use crate::error::{HarborError, Result};
use crate::gateway::http::HttpBridge;
use crate::gateway::stdio::{JsonRpcRequest, JsonRpcResponse, StdioBridge};
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

/// Unified bridge type — wraps either a stdio or HTTP MCP server connection.
pub enum Bridge {
    Stdio(StdioBridge),
    Http(HttpBridge),
}

impl Bridge {
    pub async fn initialize(&self) -> Result<JsonRpcResponse> {
        match self {
            Bridge::Stdio(b) => b.initialize().await,
            Bridge::Http(b) => b.initialize().await,
        }
    }

    pub async fn list_tools(&self) -> Result<JsonRpcResponse> {
        match self {
            Bridge::Stdio(b) => b.list_tools().await,
            Bridge::Http(b) => b.list_tools().await,
        }
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<JsonRpcResponse> {
        match self {
            Bridge::Stdio(b) => b.call_tool(tool_name, arguments).await,
            Bridge::Http(b) => b.call_tool(tool_name, arguments).await,
        }
    }

    pub async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        match self {
            Bridge::Stdio(b) => b.send(request).await,
            Bridge::Http(b) => b.send(request).await,
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        match self {
            Bridge::Stdio(b) => b.shutdown().await,
            Bridge::Http(b) => b.shutdown().await,
        }
    }
}

/// Manages multiple MCP server bridges — one per running server.
/// Provides a unified tool directory and request routing.
pub struct BridgeManager {
    bridges: Arc<Mutex<HashMap<String, Bridge>>>,
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
    /// All discovered tools are cached; filtering is applied at query time
    /// via `list_tools_for_host` to support host-specific `tool_hosts` overrides.
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

        let bridge = if config.is_remote() {
            let raw_headers = config.headers.clone().unwrap_or_default();
            let oauth_provider = detect_oauth_provider(&raw_headers);
            let url = config.url.as_deref().unwrap_or_default();
            let http_bridge = HttpBridge::new(name, url, raw_headers, oauth_provider)?;
            Bridge::Http(http_bridge)
        } else {
            let stdio_bridge = StdioBridge::spawn(name, config, resolved_env).await?;
            Bridge::Stdio(stdio_bridge)
        };

        // Initialize the MCP handshake
        bridge.initialize().await?;

        // Discover and cache all tools (filtering applied at query time)
        let tools_response = bridge.list_tools().await?;
        if let Some(ref error) = tools_response.error {
            warn!(server = name, error = ?error, "tools/list returned error");
        }
        if let Some(result) = &tools_response.result {
            if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
                let mut cache = self.tool_cache.lock().await;

                for tool in tools {
                    let tool_name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unknown");

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
                }

                info!(server = name, tool_count = tools.len(), "Discovered tools");
            } else {
                warn!(server = name, result = ?result, "tools/list response missing 'tools' array");
            }
        } else if tools_response.error.is_none() {
            warn!(server = name, "tools/list returned no result and no error");
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

            let resolved_env = resolve_env_with_refresh(&server_config.env).await;
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

    /// Get all discovered tools (unfiltered cache).
    pub async fn list_tools(&self) -> Vec<ToolInfo> {
        self.tool_cache.lock().await.clone()
    }

    /// Get tools filtered by global allowlist/blocklist (no host-specific overrides).
    pub async fn list_tools_global(&self, config: &HarborConfig) -> Vec<ToolInfo> {
        let cache = self.tool_cache.lock().await;
        cache
            .iter()
            .filter(|tool| config.tool_allowed(&tool.server, &tool.name, None))
            .cloned()
            .collect()
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

    /// Restart a single server: stop it, refresh env (including token refresh), and start again.
    /// Returns Ok(true) if the server was restarted, Ok(false) if it wasn't running.
    pub async fn restart_server(&self, name: &str, config: &HarborConfig) -> Result<bool> {
        let server_config = config
            .servers
            .get(name)
            .ok_or_else(|| HarborError::ServerNotRunning {
                name: name.to_string(),
            })?;

        // Only restart if it's currently running
        {
            let bridges = self.bridges.lock().await;
            if !bridges.contains_key(name) {
                return Ok(false);
            }
        }

        // Stop the server
        self.stop_server(name).await?;

        // Resolve env with fresh tokens
        let resolved_env = resolve_env_with_refresh(&server_config.env).await;

        // Start it again
        self.start_server(name, server_config, &resolved_env)
            .await?;

        info!(server = %name, "Server restarted with refreshed credentials");
        Ok(true)
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
                let resolved_env = resolve_env_with_refresh(&server_config.env).await;
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

/// Find stdio servers that use OAuth tokens in their env vars.
/// Returns a list of (server_name, provider_id) pairs.
pub fn stdio_servers_with_oauth(config: &HarborConfig) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for (name, server_config) in &config.servers {
        if !server_config.enabled || server_config.is_remote() {
            continue;
        }
        for value in server_config.env.values() {
            if let Some(provider_id) = extract_oauth_provider_from_env(value) {
                result.push((name.clone(), provider_id));
                break; // one provider per server is enough
            }
        }
    }
    result
}

/// Refresh any expired OAuth tokens referenced in env vars, then resolve vault references.
async fn resolve_env_with_refresh(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    // Detect OAuth providers referenced in env values and refresh if expired
    for value in env.values() {
        if let Some(provider_id) = extract_oauth_provider_from_env(value) {
            if !crate::auth::oauth::has_valid_token(&provider_id) {
                info!(provider = %provider_id, "OAuth token expired, refreshing before server start");
                match crate::auth::oauth::refresh_access_token(&provider_id).await {
                    Ok(_) => info!(provider = %provider_id, "OAuth token refreshed"),
                    Err(e) => {
                        warn!(provider = %provider_id, error = %e, "Failed to refresh OAuth token")
                    }
                }
            }
        }
    }
    crate::auth::vault::Vault::resolve_env(env)
}

/// Extract an OAuth provider ID from a vault reference like `vault:oauth:google:access_token`.
fn extract_oauth_provider_from_env(value: &str) -> Option<String> {
    let rest = value.strip_prefix("vault:oauth:")?;
    let provider = rest.strip_suffix(":access_token")?;
    Some(provider.to_string())
}

/// Detect an OAuth provider from vault references in headers.
/// Looks for `vault:oauth:<provider>:access_token` patterns.
fn detect_oauth_provider(headers: &BTreeMap<String, String>) -> Option<String> {
    for value in headers.values() {
        if let Some(rest) = value.strip_prefix("vault:oauth:") {
            if let Some(provider) = rest.strip_suffix(":access_token") {
                return Some(provider.to_string());
            }
        }
        // Also handle "Bearer vault:oauth:..." format
        if let Some(rest) = value.strip_prefix("Bearer vault:oauth:") {
            if let Some(provider) = rest.strip_suffix(":access_token") {
                return Some(provider.to_string());
            }
        }
    }
    None
}
