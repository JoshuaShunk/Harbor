use crate::config::HarborConfig;
use crate::error::Result;
use crate::gateway::bridge::{BridgeManager, ToolInfo};
use crate::gateway::stdio::{JsonRpcRequest, JsonRpcResponse};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::{error, info};

/// Shared gateway state
pub struct GatewayState {
    pub bridge_manager: BridgeManager,
    pub config: Mutex<HarborConfig>,
}

/// The Harbor Gateway — an HTTP server that bridges MCP clients to stdio servers.
pub struct Gateway {
    port: u16,
    state: Arc<GatewayState>,
}

impl Gateway {
    /// Create a new gateway from Harbor config.
    pub fn new(config: HarborConfig) -> Self {
        let port = config.harbor.gateway_port;
        Self {
            port,
            state: Arc::new(GatewayState {
                bridge_manager: BridgeManager::new(),
                config: Mutex::new(config),
            }),
        }
    }

    /// Launch the HTTP gateway, then start MCP servers in the background.
    pub async fn run(self) -> Result<()> {
        let app = Router::new()
            .route("/health", get(health))
            .route("/tools", get(list_tools))
            .route("/servers", get(list_servers))
            .route("/mcp", post(handle_mcp_request))
            .route("/sse", get(handle_sse))
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone());

        let addr = format!("127.0.0.1:{}", self.port);
        info!(addr = %addr, "Harbor Gateway starting");

        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            crate::error::HarborError::ServerStartFailed {
                name: "gateway".to_string(),
                reason: format!("Failed to bind to {addr}: {e}"),
            }
        })?;

        println!("Harbor Gateway running at http://{addr}");
        println!();
        println!("Endpoints:");
        println!("  POST /mcp     — Send JSON-RPC requests (MCP Streamable HTTP)");
        println!("  GET  /sse     — SSE stream for server notifications");
        println!("  GET  /tools   — Tool directory (all tools from all servers)");
        println!("  GET  /servers — Running server list");
        println!("  GET  /health  — Health check");
        println!();

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

        // Graceful shutdown on Ctrl+C
        let state = self.state.clone();
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                tokio::signal::ctrl_c().await.ok();
                println!("\nShutting down gateway...");
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

async fn list_tools(State(state): State<Arc<GatewayState>>) -> Json<ToolsResponse> {
    let tools = state.bridge_manager.list_tools().await;
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
            // Unified tool directory
            let tools = state.bridge_manager.list_tools().await;
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

            match state.bridge_manager.call_tool(tool_name, arguments).await {
                Ok(response) => {
                    // Use the original request id
                    Ok(Json(JsonRpcResponse {
                        id: request.id,
                        ..response
                    }))
                }
                Err(e) => Ok(Json(JsonRpcResponse::error(
                    request.id,
                    -32602,
                    e.to_string(),
                ))),
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

/// SSE endpoint — streams server events/notifications.
/// Placeholder for real-time notifications (tool changes, server status).
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

    Sse::new(stream::once(async move { Ok(initial_event) })).keep_alive(KeepAlive::default())
}
