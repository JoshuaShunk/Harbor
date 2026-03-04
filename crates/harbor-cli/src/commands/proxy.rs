use clap::Args;
use harbor_core::HarborError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

#[derive(Args)]
pub struct ProxyArgs {
    /// Host this proxy is serving (for tool filtering)
    #[arg(long)]
    pub host: String,

    /// Gateway port (default: from config or 3100)
    #[arg(long)]
    pub port: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<serde_json::Value>, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(serde_json::json!({
                "code": code,
                "message": message,
            })),
        }
    }
}

/// Meta-tool: list all available tools from Harbor's gateway in real-time.
fn meta_tool_harbor_tools() -> serde_json::Value {
    serde_json::json!({
        "name": "harbor_tools",
        "description": "Lists all currently available tools from Harbor's MCP servers in real-time. Use this to discover tools that may have been added or removed since the session started. Returns tool names, descriptions, and which server provides each tool.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "server": {
                    "type": "string",
                    "description": "Optional: filter tools from a specific MCP server by name"
                }
            }
        }
    })
}

/// Meta-tool: call any tool by name, routing through Harbor's gateway.
fn meta_tool_harbor_call() -> serde_json::Value {
    serde_json::json!({
        "name": "harbor_call",
        "description": "Calls any tool by name with arguments, routing through Harbor's gateway. Use this to invoke tools that were added after the session started and may not appear in the standard tool index. First use harbor_tools to discover available tools, then use harbor_call to invoke them.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The exact name of the tool to call (as returned by harbor_tools)"
                },
                "arguments": {
                    "type": "object",
                    "description": "The arguments to pass to the tool, matching its inputSchema",
                    "additionalProperties": true
                }
            },
            "required": ["name"]
        }
    })
}

/// Write a JSON-RPC message to stdout (shared between main loop and poller).
async fn write_stdout(
    stdout: &Arc<Mutex<tokio::io::Stdout>>,
    msg: &str,
) -> Result<(), HarborError> {
    let mut guard = stdout.lock().await;
    guard
        .write_all(msg.as_bytes())
        .await
        .map_err(HarborError::Io)?;
    guard.write_all(b"\n").await.map_err(HarborError::Io)?;
    guard.flush().await.map_err(HarborError::Io)?;
    Ok(())
}

pub async fn run(args: ProxyArgs) -> Result<(), HarborError> {
    let config = harbor_core::HarborConfig::load()?;
    let port = args.port.unwrap_or(config.harbor.gateway_port);
    let base_url = format!("http://127.0.0.1:{port}");
    let host = args.host;

    let client = reqwest::Client::new();

    // Don't exit if gateway is down — stay alive so Claude Code doesn't drop us.
    // tools/list will return empty and tools/call will return errors until the gateway starts.

    let stdin = BufReader::new(tokio::io::stdin());
    let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
    let mut lines = stdin.lines();

    // SSE listener: subscribe to gateway events and emit notifications/tools/list_changed
    let sse_base = base_url.clone();
    let sse_stdout = Arc::clone(&stdout);
    tokio::spawn(async move {
        loop {
            // Connect (or reconnect) to the gateway SSE endpoint
            let url = format!("{sse_base}/sse");
            eprintln!("[harbor-relay] Connecting to SSE: {url}");
            let Ok(resp) = reqwest::get(&url).await else {
                eprintln!("[harbor-relay] Gateway not reachable, retrying in 5s");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            };
            eprintln!("[harbor-relay] SSE connected");

            let mut stream = resp.bytes_stream();
            use futures_util::StreamExt;
            let mut buf = String::new();
            while let Some(Ok(chunk)) = stream.next().await {
                buf.push_str(&String::from_utf8_lossy(&chunk));
                // Normalize \r\n to \n for consistent SSE parsing
                buf = buf.replace("\r\n", "\n");
                // Process complete SSE messages (double newline delimited)
                while let Some(pos) = buf.find("\n\n") {
                    let message = buf[..pos].to_string();
                    buf = buf[pos + 2..].to_string();
                    if message.contains("event: tools_changed") {
                        eprintln!(
                            "[harbor-relay] Received tools_changed event, sending notifications"
                        );
                        let notification = serde_json::json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"});
                        let msg = notification.to_string();
                        // Send the notification multiple times with short delays.
                        // Claude Code's deferred tool index can miss a single notification
                        // due to race conditions or rate-limiting.
                        let _ = write_stdout(&sse_stdout, &msg).await;
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        let _ = write_stdout(&sse_stdout, &msg).await;
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let _ = write_stdout(&sse_stdout, &msg).await;
                        eprintln!("[harbor-relay] Sent 3x notifications/tools/list_changed");
                    }
                }
            }

            // Stream ended — gateway shut down, retry
            eprintln!("[harbor-relay] SSE stream ended, reconnecting in 5s");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Notifications (no id) don't get responses per JSON-RPC spec
        if request.id.is_none() {
            continue;
        }

        let response = handle_request(&client, &base_url, &host, &request).await;

        let out = serde_json::to_string(&response).unwrap_or_default();
        write_stdout(&stdout, &out).await?;
    }

    Ok(())
}

async fn handle_request(
    client: &reqwest::Client,
    base_url: &str,
    host: &str,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            // Respond with MCP server capabilities
            JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": true
                        }
                    },
                    "serverInfo": {
                        "name": format!("harbor-proxy (host: {host})"),
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
            )
        }
        "notifications/initialized" | "initialized" => {
            JsonRpcResponse::success(request.id.clone(), serde_json::json!({}))
        }
        "tools/list" => {
            // Fetch filtered tools from gateway (return empty list if gateway is down)
            let url = format!("{base_url}/tools?host={host}");
            let mut tools_array: Vec<serde_json::Value> = match client.get(&url).send().await {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(body) => {
                        let mut tools = body.get("tools").cloned().unwrap_or(serde_json::json!([]));
                        // Normalize tool entries for MCP spec compliance
                        if let Some(arr) = tools.as_array_mut() {
                            for tool in arr.iter_mut() {
                                if let Some(obj) = tool.as_object_mut() {
                                    // Strip "server" — Harbor routing field, not MCP spec
                                    obj.remove("server");
                                    // Rename "input_schema" → "inputSchema" (MCP spec uses camelCase)
                                    if let Some(schema) = obj.remove("input_schema") {
                                        obj.insert("inputSchema".to_string(), schema);
                                    }
                                }
                            }
                        }
                        tools.as_array().cloned().unwrap_or_default()
                    }
                    Err(_) => Vec::new(),
                },
                Err(_) => Vec::new(),
            };

            // Prepend meta-tools — always present regardless of gateway state.
            // These let the agent discover and call tools in real-time, working
            // around Claude Code's broken notifications/tools/list_changed.
            tools_array.insert(0, meta_tool_harbor_call());
            tools_array.insert(0, meta_tool_harbor_tools());

            JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({ "tools": tools_array }),
            )
        }
        "tools/call" => {
            let tool_name = request
                .params
                .as_ref()
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");

            match tool_name {
                "harbor_tools" => handle_harbor_tools(client, base_url, host, request).await,
                "harbor_call" => handle_harbor_call(client, base_url, request).await,
                _ => {
                    // Forward tool call to gateway
                    let url = format!("{base_url}/mcp");
                    let gateway_request = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": request.id,
                        "method": "tools/call",
                        "params": request.params,
                    });

                    match client.post(&url).json(&gateway_request).send().await {
                        Ok(resp) => match resp.json::<JsonRpcResponse>().await {
                            Ok(mut gw_resp) => {
                                gw_resp.id = request.id.clone();
                                gw_resp
                            }
                            Err(e) => {
                                JsonRpcResponse::error(request.id.clone(), -32603, &e.to_string())
                            }
                        },
                        Err(e) => {
                            JsonRpcResponse::error(request.id.clone(), -32603, &e.to_string())
                        }
                    }
                }
            }
        }
        _ => JsonRpcResponse::error(
            request.id.clone(),
            -32601,
            &format!("Method '{}' not supported by Harbor proxy", request.method),
        ),
    }
}

/// Handle the harbor_tools meta-tool: list all available tools from the gateway in real-time.
async fn handle_harbor_tools(
    client: &reqwest::Client,
    base_url: &str,
    host: &str,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
    let arguments = request
        .params
        .as_ref()
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let mut url = format!("{base_url}/tools?host={host}");
    if let Some(server) = arguments.get("server").and_then(|s| s.as_str()) {
        url.push_str(&format!("&server={server}"));
    }

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(body) => {
                let tools = body
                    .get("tools")
                    .and_then(|t| t.as_array())
                    .cloned()
                    .unwrap_or_default();
                let count = tools.len();

                let mut lines = Vec::new();
                lines.push(format!("Found {count} tool(s) available via Harbor:\n"));
                for tool in &tools {
                    let name = tool
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unknown");
                    let server = tool
                        .get("server")
                        .and_then(|s| s.as_str())
                        .unwrap_or("unknown");
                    let desc = tool
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("(no description)");
                    lines.push(format!("- {name} (server: {server}): {desc}"));
                }
                lines.push(String::new());
                lines.push("Use harbor_call to invoke any of these tools by name.".to_string());

                JsonRpcResponse::success(
                    request.id.clone(),
                    serde_json::json!({
                        "content": [{ "type": "text", "text": lines.join("\n") }]
                    }),
                )
            }
            Err(e) => JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({
                    "content": [{ "type": "text", "text": format!("Error parsing gateway response: {e}") }],
                    "isError": true
                }),
            ),
        },
        Err(e) => JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({
                "content": [{ "type": "text", "text": format!("Harbor gateway is not reachable at {base_url}: {e}\nMake sure the gateway is running (harbor lighthouse).") }],
                "isError": true
            }),
        ),
    }
}

/// Handle the harbor_call meta-tool: forward a tool call to the gateway by name.
async fn handle_harbor_call(
    client: &reqwest::Client,
    base_url: &str,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
    let arguments = request
        .params
        .as_ref()
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let tool_name = match arguments.get("name").and_then(|n| n.as_str()) {
        Some(name) => name.to_string(),
        None => {
            return JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({
                    "content": [{ "type": "text", "text": "Error: 'name' is required. Provide the tool name to call. Use harbor_tools to list available tools." }],
                    "isError": true
                }),
            );
        }
    };

    let tool_arguments = arguments
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let url = format!("{base_url}/mcp");
    let gateway_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request.id,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": tool_arguments
        }
    });

    match client.post(&url).json(&gateway_request).send().await {
        Ok(resp) => match resp.json::<JsonRpcResponse>().await {
            Ok(mut gw_resp) => {
                gw_resp.id = request.id.clone();
                gw_resp
            }
            Err(e) => JsonRpcResponse::success(
                request.id.clone(),
                serde_json::json!({
                    "content": [{ "type": "text", "text": format!("Error parsing gateway response: {e}") }],
                    "isError": true
                }),
            ),
        },
        Err(e) => JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({
                "content": [{ "type": "text", "text": format!("Harbor gateway is not reachable at {base_url}: {e}\nMake sure the gateway is running (harbor lighthouse).") }],
                "isError": true
            }),
        ),
    }
}
