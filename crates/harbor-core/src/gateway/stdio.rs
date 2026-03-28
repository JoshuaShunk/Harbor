use crate::config::ServerConfig;
use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, info, warn};

/// A JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    pub fn error(id: Option<serde_json::Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// Key for pending request map — wraps serde_json::Value for hashing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RequestId(String);

impl From<&serde_json::Value> for RequestId {
    fn from(v: &serde_json::Value) -> Self {
        RequestId(v.to_string())
    }
}

/// Manages a single stdio MCP server process and provides JSON-RPC communication.
///
/// Spawns the server, reads responses from stdout in a background task,
/// and correlates them with pending requests via their JSON-RPC `id`.
pub struct StdioBridge {
    pub name: String,
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>>,
    _child: Arc<Mutex<Child>>,
    initialized: Arc<Mutex<bool>>,
}

impl StdioBridge {
    /// Spawn a new MCP server process and set up the stdio bridge.
    pub async fn spawn(
        name: &str,
        config: &ServerConfig,
        resolved_env: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let command = config
            .command
            .as_deref()
            .ok_or_else(|| HarborError::ServerStartFailed {
                name: name.to_string(),
                reason: "No command specified for stdio bridge (is this a remote server?)"
                    .to_string(),
            })?;

        info!(server = name, command = %command, "Spawning stdio bridge");

        let mut cmd = Command::new(command);
        cmd.args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Prevent the user's local gcloud ADC from leaking a wrong
        // quota_project_id into MCP server processes. Setting this to a
        // nonexistent path stops gws (and Google client libraries) from
        // falling back to ~/.config/gcloud/application_default_credentials.json.
        // TODO: switch to GOOGLE_CLOUD_QUOTA_PROJECT once gws supports it
        // (https://github.com/googleworkspace/cli/issues/261)
        if !resolved_env.contains_key("GOOGLE_APPLICATION_CREDENTIALS") {
            cmd.env("GOOGLE_APPLICATION_CREDENTIALS", "/dev/null/harbor-no-adc");
        }

        for (key, value) in resolved_env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| HarborError::ServerStartFailed {
            name: name.to_string(),
            reason: e.to_string(),
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| HarborError::ServerStartFailed {
                name: name.to_string(),
                reason: "Failed to capture stdin".to_string(),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HarborError::ServerStartFailed {
                name: name.to_string(),
                reason: "Failed to capture stdout".to_string(),
            })?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| HarborError::ServerStartFailed {
                name: name.to_string(),
                reason: "Failed to capture stderr".to_string(),
            })?;

        let pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Background task: read stdout line by line, dispatch responses
        let pending_clone = Arc::clone(&pending);
        let server_name = name.to_string();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                debug!(server = %server_name, "stdout: {}", &line);

                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(response) => {
                        if let Some(ref id) = response.id {
                            let key = RequestId::from(id);
                            let mut pending = pending_clone.lock().await;
                            if let Some(sender) = pending.remove(&key) {
                                if sender.send(response).is_err() {
                                    warn!(server = %server_name, "Response receiver dropped");
                                }
                            } else {
                                debug!(server = %server_name, "No pending request for id {:?}", id);
                            }
                        }
                        // Notifications (no id) are logged but not dispatched
                    }
                    Err(e) => {
                        debug!(server = %server_name, error = %e, "Non-JSON-RPC stdout line");
                    }
                }
            }

            info!(server = %server_name, "Stdout reader task ended");
        });

        // Background task: log stderr
        let server_name2 = name.to_string();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(server = %server_name2, "stderr: {}", line);
            }
        });

        info!(server = name, "Stdio bridge ready");

        Ok(Self {
            name: name.to_string(),
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            _child: Arc::new(Mutex::new(child)),
            initialized: Arc::new(Mutex::new(false)),
        })
    }

    /// Send a JSON-RPC request and wait for the matching response.
    pub async fn send(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let id = request.id.clone();

        // Set up the response channel
        let (tx, rx) = oneshot::channel();
        if let Some(ref id_val) = id {
            let key = RequestId::from(id_val);
            let mut pending = self.pending.lock().await;
            pending.insert(key, tx);
        }

        // Serialize and send over stdin
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| HarborError::ServerStartFailed {
                    name: self.name.clone(),
                    reason: format!("Failed to write to stdin: {e}"),
                })?;
            stdin
                .flush()
                .await
                .map_err(|e| HarborError::ServerStartFailed {
                    name: self.name.clone(),
                    reason: format!("Failed to flush stdin: {e}"),
                })?;
        }

        // Wait for response (with timeout)
        if id.is_some() {
            match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(_)) => Ok(JsonRpcResponse::error(
                    id,
                    -32603,
                    "Response channel closed".to_string(),
                )),
                Err(_) => {
                    // Clean up pending entry on timeout
                    if let Some(ref id_val) = request.id {
                        let key = RequestId::from(id_val);
                        self.pending.lock().await.remove(&key);
                    }
                    Ok(JsonRpcResponse::error(
                        request.id,
                        -32603,
                        "Request timed out after 30s".to_string(),
                    ))
                }
            }
        } else {
            // Notification — no response expected
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: Some(serde_json::Value::Null),
                error: None,
            })
        }
    }

    /// Send the MCP `initialize` handshake.
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
        info!(server = %self.name, "MCP server initialized");
        Ok(response)
    }

    /// List tools from this MCP server.
    pub async fn list_tools(&self) -> Result<JsonRpcResponse> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(uuid::Uuid::new_v4().to_string())),
            method: "tools/list".to_string(),
            params: None,
        };
        self.send(request).await
    }

    /// Call a tool on this MCP server.
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

    /// Shut down the bridge and kill the process.
    pub async fn shutdown(&self) -> Result<()> {
        info!(server = %self.name, "Shutting down stdio bridge");
        let mut child = self._child.lock().await;
        child.kill().await.map_err(HarborError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "test/method".to_string(),
            params: Some(serde_json::json!({"key": "value"})),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"test/method\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_json_rpc_request_without_params() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("abc")),
            method: "simple/method".to_string(),
            params: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("params"));
    }

    #[test]
    fn test_json_rpc_request_notification() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn test_json_rpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(serde_json::json!(1)));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let response = JsonRpcResponse::error(
            Some(serde_json::json!(42)),
            -32600,
            "Invalid request".to_string(),
        );

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(serde_json::json!(42)));
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid request");
    }

    #[test]
    fn test_json_rpc_response_error_serialization() {
        let response = JsonRpcResponse::error(Some(serde_json::json!(1)), -32603, "Test".to_string());

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32603"));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_request_id_from_value() {
        let val = serde_json::json!(123);
        let id = RequestId::from(&val);
        assert_eq!(id.0, "123");

        let val_str = serde_json::json!("abc-def");
        let id_str = RequestId::from(&val_str);
        assert_eq!(id_str.0, "\"abc-def\"");
    }

    #[test]
    fn test_request_id_equality() {
        let val1 = serde_json::json!(42);
        let val2 = serde_json::json!(42);
        let val3 = serde_json::json!(43);

        let id1 = RequestId::from(&val1);
        let id2 = RequestId::from(&val2);
        let id3 = RequestId::from(&val3);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_json_rpc_error_with_data() {
        let error = JsonRpcError {
            code: -32000,
            message: "Server error".to_string(),
            data: Some(serde_json::json!({"details": "more info"})),
        };

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"data\""));
        assert!(json.contains("details"));
    }

    #[test]
    fn test_json_rpc_response_roundtrip() {
        let original = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("test-id")),
            result: Some(serde_json::json!({"key": "value", "num": 42})),
            error: None,
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.jsonrpc, original.jsonrpc);
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.result, original.result);
    }
}
