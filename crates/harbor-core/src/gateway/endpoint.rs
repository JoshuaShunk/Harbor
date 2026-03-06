use crate::auth::vault::Vault;
use crate::config::HarborConfig;
use crate::error::Result;
use crate::gateway::bridge::{stdio_servers_with_oauth, BridgeManager, ToolInfo};
use crate::gateway::stdio::{JsonRpcRequest, JsonRpcResponse};
use axum::extract::{ConnectInfo, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

/// Shared gateway state
pub struct GatewayState {
    pub bridge_manager: BridgeManager,
    pub config: Mutex<HarborConfig>,
    /// Broadcast channel for SSE events (tools_changed, etc.)
    pub events_tx: tokio::sync::broadcast::Sender<GatewayEvent>,
    /// Resolved bearer token for auth (None = no auth required).
    /// Behind RwLock so it can be hot-swapped without restarting the gateway.
    pub token: RwLock<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum GatewayEvent {
    #[serde(rename = "tools_changed")]
    ToolsChanged { tool_count: usize },
}

/// The Harbor Gateway — an HTTP server that bridges MCP clients to stdio servers.
pub struct Gateway {
    host: String,
    port: u16,
    state: Arc<GatewayState>,
}

impl Gateway {
    /// Create a new gateway from Harbor config.
    pub fn new(config: HarborConfig) -> Self {
        let host = config.harbor.gateway_host.clone();
        let port = config.harbor.gateway_port;

        // Resolve vault: references in the token
        let token = config.harbor.gateway_token.as_ref().map(|t| {
            if let Some(key) = t.strip_prefix("vault:") {
                Vault::get(key).unwrap_or_else(|_| t.clone())
            } else {
                t.clone()
            }
        });

        let (events_tx, _) = tokio::sync::broadcast::channel(16);
        Self {
            host,
            port,
            state: Arc::new(GatewayState {
                bridge_manager: BridgeManager::new(),
                config: Mutex::new(config),
                events_tx,
                token: RwLock::new(token),
            }),
        }
    }

    /// Launch the HTTP gateway, then start MCP servers in the background.
    ///
    /// The `shutdown_rx` oneshot is used to trigger graceful shutdown.
    /// Drop the corresponding `Sender` or send `()` to stop the gateway.
    pub async fn run(self, shutdown_rx: tokio::sync::oneshot::Receiver<()>) -> Result<()> {
        // Health is always accessible (no auth)
        let health_router = Router::new().route("/health", get(health));

        // Protected routes (auth required if token is set)
        let protected = Router::new()
            .route("/tools", get(list_tools))
            .route("/servers", get(list_servers))
            .route("/mcp", post(handle_mcp_request))
            .route("/reload", post(handle_reload))
            .route("/sse", get(handle_sse))
            .route_layer(middleware::from_fn_with_state(
                self.state.clone(),
                auth_middleware,
            ));

        let app = health_router
            .merge(protected)
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone());

        let addr = format!("{}:{}", self.host, self.port);
        info!(addr = %addr, "Harbor Gateway starting");

        if self.host == "0.0.0.0" {
            if self.state.token.read().await.is_some() {
                info!("Gateway exposed to network (bearer token required)");
            } else {
                warn!("Gateway exposed to network WITHOUT authentication — consider setting gateway_token");
            }
        }

        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            crate::error::HarborError::ServerStartFailed {
                name: "gateway".to_string(),
                reason: format!("Failed to bind to {addr}: {e}"),
            }
        })?;

        info!(addr = %addr, "Harbor Gateway running");
        info!(
            "Endpoints: POST /mcp, POST /reload, GET /sse, GET /tools, GET /servers, GET /health"
        );

        // Start MCP servers in the background (don't block the HTTP server)
        let bg_state = self.state.clone();
        tokio::spawn(async move {
            let config = bg_state.config.lock().await.clone();
            let enabled = config.servers.values().filter(|s| s.enabled).count();
            info!(count = enabled, "Starting MCP servers in background...");

            bg_state.bridge_manager.start_all(&config).await.ok();

            let tools = bg_state.bridge_manager.list_tools().await;
            if tools.is_empty() {
                info!("No tools discovered (servers may not support tools/list)");
            } else {
                info!(tool_count = tools.len(), "Tool directory ready");
                for tool in &tools {
                    info!(
                        tool = %tool.name,
                        server = %tool.server,
                        "  {}",
                        tool.description.as_deref().unwrap_or("(no description)")
                    );
                }
            }
        });

        // Background task: periodically check OAuth tokens for stdio servers and restart
        // them when tokens expire. Stdio processes receive tokens as env vars at startup,
        // so they can't pick up refreshed tokens without a restart.
        let refresh_state = self.state.clone();
        tokio::spawn(async move {
            // Wait for initial server startup to complete
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
            loop {
                interval.tick().await;

                let config = refresh_state.config.lock().await.clone();
                let oauth_servers = stdio_servers_with_oauth(&config);

                for (server_name, provider_id) in &oauth_servers {
                    // Proactively restart 5 minutes before expiry to avoid downtime
                    if crate::auth::oauth::token_valid_for(provider_id, 300) {
                        continue;
                    }

                    info!(
                        server = %server_name,
                        provider = %provider_id,
                        "OAuth token expiring soon for stdio server, restarting with refreshed token"
                    );

                    match refresh_state
                        .bridge_manager
                        .restart_server(server_name, &config)
                        .await
                    {
                        Ok(true) => {
                            info!(server = %server_name, "Stdio server restarted with fresh token");
                            // Notify SSE subscribers
                            let tools = refresh_state.bridge_manager.list_tools().await;
                            let _ = refresh_state.events_tx.send(GatewayEvent::ToolsChanged {
                                tool_count: tools.len(),
                            });
                        }
                        Ok(false) => {
                            warn!(server = %server_name, "Server was not running, skipping restart");
                        }
                        Err(e) => {
                            warn!(
                                server = %server_name,
                                error = %e,
                                "Failed to restart stdio server after token refresh"
                            );
                        }
                    }
                }
            }
        });

        // Graceful shutdown when the oneshot fires
        let state = self.state.clone();
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
            info!("Shutting down gateway...");
            if let Err(e) = state.bridge_manager.stop_all().await {
                error!(error = %e, "Error stopping servers during shutdown");
            }
        })
        .await
        .map_err(|e| crate::error::HarborError::ServerStartFailed {
            name: "gateway".to_string(),
            reason: format!("Gateway server error: {e}"),
        })?;

        Ok(())
    }
}

// --- Auth Middleware ---

async fn auth_middleware(
    State(state): State<Arc<GatewayState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    // Localhost connections are always trusted — no auth needed
    let is_local = addr.ip().is_loopback();

    if !is_local {
        let token_guard = state.token.read().await;
        if let Some(ref expected_token) = *token_guard {
            let auth_header = request
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok());

            match auth_header {
                Some(header) if header.starts_with("Bearer ") => {
                    let provided = &header[7..];
                    if provided != expected_token.as_str() {
                        return Err(StatusCode::UNAUTHORIZED);
                    }
                }
                _ => return Err(StatusCode::UNAUTHORIZED),
            }
        }
        drop(token_guard);
    }

    Ok(next.run(request).await)
}

// --- HTTP Handlers ---

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
struct ToolsResponse {
    tools: Vec<ToolInfo>,
    count: usize,
}

#[derive(Deserialize, Default)]
struct ToolsQuery {
    /// Filter tools for a specific host (applies host-specific tool overrides)
    host: Option<String>,
    /// Filter tools from a specific server
    server: Option<String>,
}

async fn list_tools(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<ToolsQuery>,
) -> Json<ToolsResponse> {
    let config = state.config.lock().await;
    let mut tools = if let Some(ref host) = query.host {
        state
            .bridge_manager
            .list_tools_for_host(host, &config)
            .await
    } else {
        state.bridge_manager.list_tools_global(&config).await
    };

    if let Some(ref server) = query.server {
        tools.retain(|t| t.server == *server);
    }

    let count = tools.len();
    Json(ToolsResponse { tools, count })
}

#[derive(Serialize)]
struct ServersResponse {
    servers: Vec<String>,
}

async fn list_servers(State(state): State<Arc<GatewayState>>) -> Json<ServersResponse> {
    let servers = state.bridge_manager.running_servers().await;
    Json(ServersResponse { servers })
}

/// Handle an incoming MCP JSON-RPC request over HTTP.
///
/// Supports two modes:
/// 1. `tools/call` with `name` — routes to the correct server automatically
/// 2. Any method with `_harbor_server` param — routes to a specific server
/// 3. `tools/list` — returns the unified tool directory
async fn handle_mcp_request(
    State(state): State<Arc<GatewayState>>,
    Json(request): Json<JsonRpcRequest>,
) -> std::result::Result<Json<JsonRpcResponse>, StatusCode> {
    match request.method.as_str() {
        "tools/list" => {
            // Unified tool directory, optionally filtered by host
            let host = request
                .params
                .as_ref()
                .and_then(|p| p.get("_harbor_host"))
                .and_then(|h| h.as_str())
                .map(String::from);

            let config = state.config.lock().await;
            let tools = if let Some(ref host) = host {
                state
                    .bridge_manager
                    .list_tools_for_host(host, &config)
                    .await
            } else {
                state.bridge_manager.list_tools_global(&config).await
            };
            let mcp_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    let mut obj = serde_json::json!({
                        "name": t.name,
                    });
                    if let Some(ref desc) = t.description {
                        obj["description"] = serde_json::json!(desc);
                    }
                    if let Some(ref schema) = t.input_schema {
                        obj["inputSchema"] = schema.clone();
                    }
                    obj
                })
                .collect();

            Ok(Json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({ "tools": mcp_tools })),
                error: None,
            }))
        }
        "tools/call" => {
            // Route to correct server based on tool name
            let params = request.params.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
            let tool_name = params
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or(StatusCode::BAD_REQUEST)?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            info!(tool = %tool_name, "tools/call");

            match state.bridge_manager.call_tool(tool_name, arguments).await {
                Ok(response) => {
                    info!(tool = %tool_name, "tools/call succeeded");
                    // Use the original request id
                    Ok(Json(JsonRpcResponse {
                        id: request.id,
                        ..response
                    }))
                }
                Err(e) => {
                    warn!(tool = %tool_name, error = %e, "tools/call failed");
                    Ok(Json(JsonRpcResponse::error(
                        request.id,
                        -32602,
                        e.to_string(),
                    )))
                }
            }
        }
        _ => {
            // Try to extract target server from params
            let target_server = request
                .params
                .as_ref()
                .and_then(|p| p.get("_harbor_server"))
                .and_then(|s| s.as_str())
                .map(String::from);

            if let Some(server) = target_server {
                // Strip _harbor_server from params before forwarding
                let mut clean_request = request.clone();
                if let Some(ref mut params) = clean_request.params {
                    if let Some(obj) = params.as_object_mut() {
                        obj.remove("_harbor_server");
                    }
                }

                match state
                    .bridge_manager
                    .forward_to_server(&server, clean_request)
                    .await
                {
                    Ok(response) => Ok(Json(JsonRpcResponse {
                        id: request.id,
                        ..response
                    })),
                    Err(e) => Ok(Json(JsonRpcResponse::error(
                        request.id,
                        -32603,
                        e.to_string(),
                    ))),
                }
            } else {
                Ok(Json(JsonRpcResponse::error(
                    request.id,
                    -32601,
                    format!("Unknown method '{}'. Use tools/list, tools/call, or specify _harbor_server param.", request.method),
                )))
            }
        }
    }
}

/// Reload: re-read config from disk, start new servers, stop removed ones.
async fn handle_reload(
    State(state): State<Arc<GatewayState>>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    // Re-read config from disk
    let config = HarborConfig::load().map_err(|e| {
        error!(error = %e, "Failed to reload config");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Hot-swap the bearer token (no restart needed)
    let new_token = config.harbor.gateway_token.as_ref().map(|t| {
        if let Some(key) = t.strip_prefix("vault:") {
            Vault::get(key).unwrap_or_else(|_| t.clone())
        } else {
            t.clone()
        }
    });
    *state.token.write().await = new_token;

    // Update the gateway's config
    *state.config.lock().await = config.clone();

    // Reload servers
    let (started, stopped) = state.bridge_manager.reload(&config).await.map_err(|e| {
        error!(error = %e, "Failed to reload servers");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let tools = state.bridge_manager.list_tools().await;
    let tool_count = tools.len();
    info!(
        started = started.len(),
        stopped = stopped.len(),
        total_tools = tool_count,
        "Gateway reloaded"
    );

    // Notify SSE subscribers that tools changed
    if !started.is_empty() || !stopped.is_empty() {
        let _ = state
            .events_tx
            .send(GatewayEvent::ToolsChanged { tool_count });
    }

    Ok(Json(serde_json::json!({
        "started": started,
        "stopped": stopped,
        "total_tools": tool_count,
    })))
}

/// SSE endpoint — streams real-time gateway events (tools_changed, etc.).
async fn handle_sse(
    State(state): State<Arc<GatewayState>>,
) -> Sse<impl futures::Stream<Item = std::result::Result<Event, std::convert::Infallible>>> {
    let servers = state.bridge_manager.running_servers().await;
    let tools = state.bridge_manager.list_tools().await;

    // Send initial state as first event
    let initial = serde_json::json!({
        "type": "gateway_status",
        "servers": servers,
        "tool_count": tools.len(),
    });
    let initial_event = Event::default().event("status").data(initial.to_string());

    // Subscribe to broadcast channel for future events
    let mut rx = state.events_tx.subscribe();

    let event_stream = async_stream::stream! {
        yield Ok(initial_event);
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event).unwrap_or_default();
                    yield Ok(Event::default().event("tools_changed").data(data));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}
