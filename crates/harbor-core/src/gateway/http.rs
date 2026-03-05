use crate::auth::vault::Vault;
use crate::error::{HarborError, Result};
use crate::gateway::stdio::{JsonRpcRequest, JsonRpcResponse};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Manages communication with a remote MCP server over HTTP (Streamable-HTTP protocol).
///
/// Headers containing `vault:` references are resolved fresh on each request,
/// so token refreshes in the vault are picked up automatically.
pub struct HttpBridge {
    pub name: String,
    url: String,
    client: reqwest::Client,
    /// Raw (unresolved) headers — `vault:` references resolved per-request.
    raw_headers: BTreeMap<String, String>,
    /// OAuth provider ID for automatic token refresh on 401.
    oauth_provider: Option<String>,
    session_id: Arc<Mutex<Option<String>>>,
    initialized: Arc<Mutex<bool>>,
}

impl HttpBridge {
    /// Create a new HTTP bridge for a remote MCP server.
    ///
    /// `raw_headers` may contain `vault:` references — they are resolved per-request.
    /// `oauth_provider` enables automatic token refresh on 401 responses.
    pub fn new(
        name: &str,
        url: &str,
        raw_headers: BTreeMap<String, String>,
        oauth_provider: Option<String>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| HarborError::ServerStartFailed {
                name: name.to_string(),
                reason: format!("Failed to create HTTP client: {e}"),
            })?;

        info!(server = name, url = url, oauth = ?oauth_provider, "HTTP bridge created");

        Ok(Self {
            name: name.to_string(),
            url: url.to_string(),
            client,
            raw_headers,
            oauth_provider,
            session_id: Arc::new(Mutex::new(None)),
            initialized: Arc::new(Mutex::new(false)),
        })
    }

    /// Resolve raw headers (vault: references) and build a request.
    fn build_request(&self, request: &JsonRpcRequest) -> Result<reqwest::RequestBuilder> {
        let resolved = Vault::resolve_env(&self.raw_headers);

        let mut req_builder = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(request);

        // Inject resolved headers
        for (key, value) in &resolved {
            if value.is_empty() {
                continue;
            }
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    HarborError::ServerStartFailed {
                        name: self.name.clone(),
                        reason: format!("Invalid header name '{key}': {e}"),
                    }
                })?;
            let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                HarborError::ServerStartFailed {
                    name: self.name.clone(),
                    reason: format!("Invalid header value for '{key}': {e}"),
                }
            })?;
            req_builder = req_builder.header(header_name, header_value);
        }

        Ok(req_builder)
    }

    /// Send a JSON-RPC request to the remote server over HTTP POST.
    pub async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let is_notification = request.id.is_none();

        let mut req_builder = self.build_request(&request)?;

        // Include session ID if we have one
        let session_id = self.session_id.lock().await.clone();
        if let Some(ref sid) = session_id {
            req_builder = req_builder.header("Mcp-Session-Id", sid);
        }

        debug!(server = %self.name, method = %request.method, "HTTP POST");

        let resp = req_builder
            .send()
            .await
            .map_err(|e| HarborError::ServerStartFailed {
                name: self.name.clone(),
                reason: format!("HTTP request failed: {e}"),
            })?;

        // Capture session ID from response headers
        if let Some(sid) = resp.headers().get("mcp-session-id") {
            if let Ok(sid_str) = sid.to_str() {
                let mut current = self.session_id.lock().await;
                if current.is_none() {
                    info!(server = %self.name, session_id = sid_str, "Session ID received");
                    *current = Some(sid_str.to_string());
                }
            }
        }

        let status = resp.status();

        // Notifications get 202 Accepted with no body
        if is_notification {
            return Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: Some(serde_json::Value::Null),
                error: None,
            });
        }

        // On 401, try refreshing the OAuth token and retry once
        if status == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(ref provider_id) = self.oauth_provider {
                warn!(server = %self.name, provider = %provider_id, "Got 401 — attempting token refresh");
                match crate::auth::oauth::refresh_access_token(provider_id).await {
                    Ok(_) => {
                        info!(server = %self.name, "Token refreshed, retrying request");
                        return self.send_inner(request, is_notification).await;
                    }
                    Err(e) => {
                        warn!(server = %self.name, error = %e, "Token refresh failed");
                    }
                }
            }
            let body = resp.text().await.unwrap_or_default();
            return Ok(JsonRpcResponse::error(
                request.id,
                -32603,
                format!("HTTP 401 Unauthorized from remote server: {body}"),
            ));
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Ok(JsonRpcResponse::error(
                request.id,
                -32603,
                format!("HTTP {status} from remote server: {body}"),
            ));
        }

        self.parse_response(request.id, resp).await
    }

    /// Inner send — used for the retry after token refresh (avoids infinite recursion).
    async fn send_inner(
        &self,
        request: JsonRpcRequest,
        is_notification: bool,
    ) -> Result<JsonRpcResponse> {
        let mut req_builder = self.build_request(&request)?;

        let session_id = self.session_id.lock().await.clone();
        if let Some(ref sid) = session_id {
            req_builder = req_builder.header("Mcp-Session-Id", sid);
        }

        let resp = req_builder
            .send()
            .await
            .map_err(|e| HarborError::ServerStartFailed {
                name: self.name.clone(),
                reason: format!("HTTP request failed on retry: {e}"),
            })?;

        if let Some(sid) = resp.headers().get("mcp-session-id") {
            if let Ok(sid_str) = sid.to_str() {
                let mut current = self.session_id.lock().await;
                if current.is_none() {
                    *current = Some(sid_str.to_string());
                }
            }
        }

        let status = resp.status();

        if is_notification {
            return Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: Some(serde_json::Value::Null),
                error: None,
            });
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Ok(JsonRpcResponse::error(
                request.id,
                -32603,
                format!("HTTP {status} from remote server (after retry): {body}"),
            ));
        }

        self.parse_response(request.id, resp).await
    }

    /// Parse a successful response — handles both JSON and SSE.
    async fn parse_response(
        &self,
        request_id: Option<serde_json::Value>,
        resp: reqwest::Response,
    ) -> Result<JsonRpcResponse> {
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if content_type.contains("text/event-stream") {
            self.parse_sse_response(request_id, resp).await
        } else {
            let response: JsonRpcResponse =
                resp.json()
                    .await
                    .map_err(|e| HarborError::ServerStartFailed {
                        name: self.name.clone(),
                        reason: format!("Failed to parse JSON-RPC response: {e}"),
                    })?;
            Ok(response)
        }
    }

    /// Parse an SSE response stream to extract the JSON-RPC response.
    async fn parse_sse_response(
        &self,
        request_id: Option<serde_json::Value>,
        resp: reqwest::Response,
    ) -> Result<JsonRpcResponse> {
        let body = resp
            .text()
            .await
            .map_err(|e| HarborError::ServerStartFailed {
                name: self.name.clone(),
                reason: format!("Failed to read SSE response body: {e}"),
            })?;

        // Parse SSE: look for "data:" lines containing JSON-RPC responses
        for line in body.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(data) {
                    return Ok(response);
                }
            }
        }

        Ok(JsonRpcResponse::error(
            request_id,
            -32603,
            "No valid JSON-RPC response found in SSE stream".to_string(),
        ))
    }

    /// Initialize the MCP handshake with the remote server.
    pub async fn initialize(&self) -> Result<JsonRpcResponse> {
        let mut initialized = self.initialized.lock().await;
        if *initialized {
            return Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::Value::Number(0.into())),
                result: Some(serde_json::json!({"already": "initialized"})),
                error: None,
            });
        }

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(0)),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "harbor-gateway",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        };

        let response = self.send(request).await?;

        // Send initialized notification
        let notification = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let _ = self.send(notification).await;

        *initialized = true;
        info!(server = %self.name, "Remote MCP server initialized");
        Ok(response)
    }

    /// List tools from the remote MCP server.
    pub async fn list_tools(&self) -> Result<JsonRpcResponse> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(uuid::Uuid::new_v4().to_string())),
            method: "tools/list".to_string(),
            params: None,
        };
        self.send(request).await
    }

    /// Call a tool on the remote MCP server.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<JsonRpcResponse> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(uuid::Uuid::new_v4().to_string())),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            })),
        };
        self.send(request).await
    }

    /// Shut down the HTTP bridge (clears state).
    pub async fn shutdown(&self) -> Result<()> {
        info!(server = %self.name, "Shutting down HTTP bridge");
        *self.session_id.lock().await = None;
        *self.initialized.lock().await = false;
        Ok(())
    }
}
