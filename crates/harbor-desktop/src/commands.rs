use harbor_core::connector;
use harbor_core::gateway::Gateway;
use harbor_core::{HarborConfig, ServerConfig};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

#[derive(Clone)]
pub struct AppState {
    config: Arc<Mutex<HarborConfig>>,
    /// Holds the shutdown sender while the gateway is running.
    /// `Some(tx)` = running, `None` = stopped.
    /// Wrapped in Arc so the background task can clear it on exit.
    gateway_shutdown: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl AppState {
    pub fn new(config: HarborConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            gateway_shutdown: Arc::new(Mutex::new(None)),
        }
    }

    pub fn gateway_running(&self) -> bool {
        self.gateway_shutdown
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    fn with_config_mut<T>(&self, f: impl FnOnce(&mut HarborConfig) -> T) -> Result<T, String> {
        let mut config = self
            .config
            .lock()
            .map_err(|_| "Config lock poisoned — restart the application".to_string())?;
        let result = f(&mut config);
        config.save().map_err(|e| e.to_string())?;
        Ok(result)
    }
}

// -- Response types --

#[derive(Serialize)]
pub struct ServerStatus {
    name: String,
    enabled: bool,
    running: bool,
    pid: Option<u32>,
    command: String,
}

#[derive(Serialize)]
pub struct HostStatus {
    name: String,
    display_name: String,
    connected: bool,
    config_exists: bool,
    config_path: String,
    server_count: usize,
}

#[derive(Serialize)]
pub struct HarborStatusResponse {
    servers: Vec<ServerStatus>,
    hosts: Vec<HostStatus>,
    gateway_port: u16,
}

// -- Commands --

#[tauri::command]
pub fn get_status(state: tauri::State<AppState>) -> Result<HarborStatusResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned — restart the application".to_string())?;

    let servers: Vec<ServerStatus> = config
        .servers
        .iter()
        .map(|(name, sc)| ServerStatus {
            name: name.clone(),
            enabled: sc.enabled,
            running: false,
            pid: None,
            command: format!("{} {}", sc.command, sc.args.join(" ")),
        })
        .collect();

    let connectors = connector::all_connectors();
    let hosts: Vec<HostStatus> = connectors
        .iter()
        .map(|conn| {
            let host_key = normalize_host_key(conn.host_name());
            let connected = config
                .hosts
                .get(&host_key)
                .map(|h| h.connected)
                .unwrap_or(false);
            let config_exists = conn.config_exists();
            let config_path = conn
                .config_path()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let server_count = config.servers_for_host(&host_key).len();

            HostStatus {
                name: host_key,
                display_name: conn.host_name().to_string(),
                connected,
                config_exists,
                config_path,
                server_count,
            }
        })
        .collect();

    Ok(HarborStatusResponse {
        servers,
        hosts,
        gateway_port: config.harbor.gateway_port,
    })
}

#[tauri::command]
pub fn add_server(
    state: tauri::State<AppState>,
    name: String,
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        let server = ServerConfig {
            source: None,
            command,
            args,
            env,
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        };
        config.add_server(name, server).map_err(|e| e.to_string())
    })??;
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    Ok(())
}

#[tauri::command]
pub fn remove_server(state: tauri::State<AppState>, name: String) -> Result<(), String> {
    state.with_config_mut(|config| config.remove_server(&name).map_err(|e| e.to_string()))??;
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    Ok(())
}

#[tauri::command]
pub fn toggle_server(
    state: tauri::State<AppState>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        if let Some(server) = config.servers.get_mut(&name) {
            server.enabled = enabled;
            Ok(())
        } else {
            Err(format!("Server '{name}' not found"))
        }
    })??;
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    Ok(())
}

#[tauri::command]
pub fn sync_host(state: tauri::State<AppState>, host: String) -> Result<String, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned — restart the application".to_string())?;

    let result = harbor_core::sync::sync_to_host(&config, &host).map_err(|e| e.to_string())?;
    Ok(format!(
        "Synced {} server(s) to {}",
        result.server_count, result.display_name
    ))
}

#[tauri::command]
pub fn sync_all(state: tauri::State<AppState>) -> Result<String, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned — restart the application".to_string())?;

    let results = harbor_core::sync::sync_all_hosts(&config);
    if results.is_empty() {
        return Ok("No connected hosts. Connect a host first.".to_string());
    }

    let lines: Vec<String> = results
        .into_iter()
        .map(|(host_name, result)| match result {
            Ok(r) => {
                format!("{}: synced {} server(s)", r.display_name, r.server_count)
            }
            Err(e) => format!("{}: error ({})", host_name, e),
        })
        .collect();

    Ok(lines.join("\n"))
}

#[tauri::command]
pub fn connect_host(state: tauri::State<AppState>, host: String) -> Result<(), String> {
    state.with_config_mut(|config| {
        let entry = config
            .hosts
            .entry(host)
            .or_insert_with(|| harbor_core::HostConfig {
                connected: false,
                scope: None,
            });
        entry.connected = true;
    })?;
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    Ok(())
}

#[tauri::command]
pub fn disconnect_host(state: tauri::State<AppState>, host: String) -> Result<(), String> {
    state.with_config_mut(|config| {
        if let Some(entry) = config.hosts.get_mut(&host) {
            entry.connected = false;
        }
    })?;
    // Remove the harbor-proxy entry from the host's config file
    if let Ok(conn) = connector::get_connector(&host) {
        let _ = conn.remove_servers(&["harbor-proxy".to_string()]);
    }
    Ok(())
}

// -- Vault commands --

#[tauri::command]
pub fn vault_set(key: String, value: String) -> Result<(), String> {
    harbor_core::Vault::set(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn vault_get(key: String) -> Result<String, String> {
    harbor_core::Vault::get(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn vault_delete(key: String) -> Result<(), String> {
    harbor_core::Vault::delete(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn vault_list() -> Result<Vec<String>, String> {
    harbor_core::Vault::list_keys().map_err(|e| e.to_string())
}

// -- Marketplace commands --

#[derive(Serialize)]
pub struct MarketplaceEnvVar {
    name: String,
    description: Option<String>,
    is_required: bool,
    is_secret: bool,
    default: Option<String>,
}

#[derive(Serialize)]
pub struct MarketplacePackage {
    registry_type: String,
    identifier: String,
    version: Option<String>,
    runtime_hint: Option<String>,
    environment_variables: Vec<MarketplaceEnvVar>,
}

#[derive(Serialize)]
pub struct MarketplaceServer {
    name: String,
    title: Option<String>,
    description: String,
    website_url: Option<String>,
    is_official: bool,
    repository_url: Option<String>,
    package: Option<MarketplacePackage>,
}

#[derive(Serialize)]
pub struct MarketplaceSearchResult {
    servers: Vec<MarketplaceServer>,
    next_cursor: Option<String>,
}

#[tauri::command]
pub async fn marketplace_search(
    query: String,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<MarketplaceSearchResult, String> {
    let client = harbor_core::marketplace::registry::RegistryClient::new();

    let result = client
        .search(&query, cursor.as_deref(), limit)
        .await
        .map_err(|e| e.to_string())?;

    Ok(MarketplaceSearchResult {
        servers: result
            .servers
            .into_iter()
            .map(|s| MarketplaceServer {
                name: s.name,
                title: s.title,
                description: s.description,
                website_url: s.website_url,
                is_official: s.is_official,
                repository_url: s.repository_url,
                package: s.package.map(|p| MarketplacePackage {
                    registry_type: p.registry_type,
                    identifier: p.identifier,
                    version: p.version,
                    runtime_hint: p.runtime_hint,
                    environment_variables: p
                        .environment_variables
                        .into_iter()
                        .map(|e| MarketplaceEnvVar {
                            name: e.name,
                            description: e.description,
                            is_required: e.is_required,
                            is_secret: e.is_secret,
                            default: e.default,
                        })
                        .collect(),
                }),
            })
            .collect(),
        next_cursor: result.next_cursor,
    })
}

// -- OAuth commands --

#[derive(Serialize)]
pub struct OAuthProviderInfo {
    id: String,
    display_name: String,
    has_token: bool,
    token_expired: bool,
    scopes: Vec<String>,
}

#[tauri::command]
pub fn oauth_list_providers() -> Vec<OAuthProviderInfo> {
    use harbor_core::auth::oauth;
    oauth::builtin_providers()
        .into_iter()
        .map(|p| {
            let has_valid = oauth::has_valid_token(&p.id);
            let has_any = oauth::get_access_token(&p.id).is_ok();
            OAuthProviderInfo {
                id: p.id,
                display_name: p.display_name,
                has_token: has_valid,
                token_expired: has_any && !has_valid,
                scopes: p.scopes,
            }
        })
        .collect()
}

#[tauri::command]
pub fn oauth_provider_for_server(qualified_name: String) -> Option<String> {
    harbor_core::auth::oauth::provider_for_server(&qualified_name).map(String::from)
}

#[tauri::command]
pub async fn oauth_start_charter(
    app_handle: tauri::AppHandle,
    provider_id: String,
) -> Result<(), String> {
    use harbor_core::auth::oauth;

    let (auth_url, callback_server, pkce) = oauth::start_oauth_flow(&provider_id)
        .await
        .map_err(|e| e.to_string())?;

    // Open system browser
    tauri_plugin_opener::OpenerExt::opener(&app_handle)
        .open_url(&auth_url, None::<&str>)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    // Wait for callback (5 minute timeout)
    let port = callback_server.port;
    let code = tokio::time::timeout(std::time::Duration::from_secs(300), callback_server.code_rx)
        .await
        .map_err(|_| "Charter timed out. The authorization window was open too long.".to_string())?
        .map_err(|_| "Charter cancelled.".to_string())?;

    // Exchange code for tokens and store in vault (shared with CLI)
    oauth::complete_oauth_flow(&provider_id, &code, port, pkce.as_ref())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn oauth_get_status(provider_id: String) -> Result<OAuthProviderInfo, String> {
    use harbor_core::auth::oauth;
    let provider = oauth::builtin_providers()
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Unknown provider: {provider_id}"))?;

    let has_valid = oauth::has_valid_token(&provider_id);
    let has_any = oauth::get_access_token(&provider_id).is_ok();

    Ok(OAuthProviderInfo {
        id: provider.id,
        display_name: provider.display_name,
        has_token: has_valid,
        token_expired: has_any && !has_valid,
        scopes: provider.scopes,
    })
}

#[tauri::command]
pub fn oauth_revoke_charter(provider_id: String) -> Result<(), String> {
    harbor_core::auth::oauth::clear_tokens(&provider_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn oauth_set_custom_credentials(
    provider_id: String,
    client_id: String,
    client_secret: Option<String>,
) -> Result<(), String> {
    harbor_core::Vault::set(&format!("oauth:{provider_id}:client_id"), &client_id)
        .map_err(|e| e.to_string())?;
    if let Some(secret) = client_secret {
        harbor_core::Vault::set(&format!("oauth:{provider_id}:client_secret"), &secret)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn gdrive_credential_paths() -> Result<(String, String), String> {
    harbor_core::auth::oauth::gdrive_credential_paths().ok_or_else(|| {
        "Google Drive credentials not found. Complete the Charter flow first.".into()
    })
}

// -- Native catalog commands --

#[derive(Serialize)]
pub struct NativeServerInfo {
    id: String,
    display_name: String,
    description: String,
    auth_kind: String,
    has_auth: bool,
    /// For manual-token servers: the vault key to store the API key under.
    manual_vault_key: Option<String>,
}

#[tauri::command]
pub fn catalog_list() -> Vec<NativeServerInfo> {
    harbor_core::catalog::catalog()
        .into_iter()
        .map(|s| {
            let (auth_kind, manual_vault_key) = match &s.auth {
                harbor_core::AuthKind::None => ("none".to_string(), None),
                harbor_core::AuthKind::OAuth(p) => (format!("oauth:{p}"), None),
                harbor_core::AuthKind::ManualToken { env_var, .. } => {
                    ("manual".to_string(), Some(env_var.to_lowercase()))
                }
            };
            let has_auth = harbor_core::catalog::has_auth(&s);
            NativeServerInfo {
                id: s.id.to_string(),
                display_name: s.display_name.to_string(),
                description: s.description.to_string(),
                auth_kind,
                has_auth,
                manual_vault_key,
            }
        })
        .collect()
}

#[tauri::command]
pub async fn dock_native(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    id: String,
    name: Option<String>,
) -> Result<(), String> {
    let native =
        harbor_core::catalog::lookup(&id).ok_or_else(|| format!("Unknown native server: {id}"))?;

    let server_name = name.unwrap_or_else(|| native.id.to_string());

    // Handle OAuth if needed
    if let harbor_core::AuthKind::OAuth(ref provider_id) = native.auth {
        if !harbor_core::catalog::has_auth(&native) {
            oauth_start_charter(app_handle, provider_id.clone()).await?;
        }
    }

    // Reject manual-token servers that are missing their key
    if let harbor_core::AuthKind::ManualToken {
        env_var,
        description,
    } = &native.auth
    {
        if !harbor_core::catalog::has_auth(&native) {
            return Err(format!(
                "{} requires {}.\nStore it with: harbor chest set {} <value>",
                native.display_name,
                description,
                env_var.to_lowercase(),
            ));
        }
    }

    let env = harbor_core::catalog::build_env(&native).map_err(|e| e.to_string())?;

    state.with_config_mut(|config| {
        let server = ServerConfig {
            source: Some(format!("native:{}", native.id)),
            command: native.command.to_string(),
            args: native.args.iter().map(|s| s.to_string()).collect(),
            env,
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        };
        config
            .add_server(server_name, server)
            .map_err(|e| e.to_string())
    })??;

    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });

    Ok(())
}

// -- Tool discovery & filter commands --

#[derive(Serialize)]
pub struct DiscoveredTool {
    name: String,
    description: Option<String>,
}

/// Discover tools by temporarily spawning the MCP server, sending initialize + tools/list,
/// then shutting it down. Works without the gateway running.
#[tauri::command]
pub async fn discover_tools(
    state: tauri::State<'_, AppState>,
    server: String,
) -> Result<Vec<DiscoveredTool>, String> {
    let server_config = {
        let config = state
            .config
            .lock()
            .map_err(|_| "Config lock poisoned — restart the application".to_string())?;
        config
            .get_server(&server)
            .map_err(|e| e.to_string())?
            .clone()
    };

    // Resolve env vars (vault: references → actual values)
    let resolved_env = connector::resolve_env_for_host(&server_config.env);

    // Spawn a temporary bridge to discover tools
    let bridge =
        harbor_core::gateway::stdio::StdioBridge::spawn(&server, &server_config, &resolved_env)
            .await
            .map_err(|e| format!("Failed to start server: {e}"))?;

    // Initialize the MCP handshake
    bridge
        .initialize()
        .await
        .map_err(|e| format!("MCP initialize failed: {e}"))?;

    // Query tools
    let tools_response = bridge
        .list_tools()
        .await
        .map_err(|e| format!("tools/list failed: {e}"))?;

    // Shut down the temporary process
    let _ = bridge.shutdown().await;

    // Parse the response
    let tools = tools_response
        .result
        .as_ref()
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(tools
        .into_iter()
        .filter_map(|t| {
            let name = t.get("name")?.as_str()?.to_string();
            let description = t
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from);
            Some(DiscoveredTool { name, description })
        })
        .collect())
}

#[derive(Serialize)]
pub struct ToolFilterInfo {
    tool_allowlist: Option<Vec<String>>,
    tool_blocklist: Option<Vec<String>>,
    tool_hosts: BTreeMap<String, Vec<String>>,
}

#[tauri::command]
pub fn get_tool_filters(
    state: tauri::State<AppState>,
    server: String,
) -> Result<ToolFilterInfo, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned — restart the application".to_string())?;
    let sc = config.get_server(&server).map_err(|e| e.to_string())?;
    Ok(ToolFilterInfo {
        tool_allowlist: sc.tool_allowlist.clone(),
        tool_blocklist: sc.tool_blocklist.clone(),
        tool_hosts: sc.tool_hosts.clone(),
    })
}

#[tauri::command]
pub fn set_tool_allowlist(
    state: tauri::State<AppState>,
    server: String,
    tools: Option<Vec<String>>,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        if let Some(sc) = config.servers.get_mut(&server) {
            sc.tool_allowlist = tools;
            Ok(())
        } else {
            Err(format!("Server '{server}' not found"))
        }
    })??;
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    Ok(())
}

#[tauri::command]
pub fn set_tool_blocklist(
    state: tauri::State<AppState>,
    server: String,
    tools: Option<Vec<String>>,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        if let Some(sc) = config.servers.get_mut(&server) {
            sc.tool_blocklist = tools;
            Ok(())
        } else {
            Err(format!("Server '{server}' not found"))
        }
    })??;
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    Ok(())
}

#[tauri::command]
pub fn set_tool_host_override(
    state: tauri::State<AppState>,
    server: String,
    host: String,
    tools: Option<Vec<String>>,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        if let Some(sc) = config.servers.get_mut(&server) {
            match tools {
                Some(t) => {
                    sc.tool_hosts.insert(host, t);
                }
                None => {
                    sc.tool_hosts.remove(&host);
                }
            }
            Ok(())
        } else {
            Err(format!("Server '{server}' not found"))
        }
    })??;
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    Ok(())
}

/// Sync all connected hosts and reload the gateway if running.
async fn auto_sync_and_reload(state: &AppState) {
    let config = match state.config.lock() {
        Ok(c) => c.clone(),
        Err(_) => return,
    };
    let results = harbor_core::sync::sync_all_hosts(&config);
    for (host, result) in &results {
        match result {
            Ok(r) => tracing::info!(host = %host, servers = r.server_count, "Auto-synced"),
            Err(e) => tracing::warn!(host = %host, error = %e, "Auto-sync failed"),
        }
    }
    reload_gateway_if_running(state).await;
}

/// If the gateway is running, tell it to reload config (start new servers, stop removed ones).
async fn reload_gateway_if_running(state: &AppState) {
    if !state.gateway_running() {
        return;
    }
    let port = state
        .config
        .lock()
        .ok()
        .map(|c| c.harbor.gateway_port)
        .unwrap_or(3100);
    let url = format!("http://127.0.0.1:{port}/reload");
    let client = reqwest::Client::new();
    match client.post(&url).send().await {
        Ok(resp) => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                tracing::info!(result = %body, "Gateway reloaded");
            }
        }
        Err(e) => tracing::warn!(error = %e, "Failed to reload gateway"),
    }
}

// -- Gateway commands --

/// Inner implementation shared by the Tauri command and the tray menu handler.
pub async fn start_gateway_inner(
    app_handle: tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    // Check if already running
    {
        let guard = state
            .gateway_shutdown
            .lock()
            .map_err(|_| "Lock poisoned".to_string())?;
        if guard.is_some() {
            return Err("Lighthouse is already running".to_string());
        }
    }

    let config = {
        state
            .config
            .lock()
            .map_err(|_| "Config lock poisoned".to_string())?
            .clone()
    };

    let port = config.harbor.gateway_port;
    let gateway = Gateway::new(config);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let shutdown_ref = Arc::clone(&state.gateway_shutdown);
    {
        let mut guard = shutdown_ref
            .lock()
            .map_err(|_| "Lock poisoned".to_string())?;
        *guard = Some(shutdown_tx);
    }

    let app_handle_bg = app_handle.clone();
    tokio::spawn(async move {
        if let Err(e) = gateway.run(shutdown_rx).await {
            tracing::error!(error = %e, "Gateway exited with error");
        }
        // Clear the shutdown sender so status reflects reality
        if let Ok(mut guard) = shutdown_ref.lock() {
            *guard = None;
        }
        let _ = app_handle_bg.emit("gateway-status-changed", false);
        tracing::info!("Gateway stopped");
    });

    let _ = app_handle.emit("gateway-status-changed", true);
    Ok(format!("Lighthouse lit on port {}", port))
}

#[tauri::command]
pub async fn start_gateway(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    start_gateway_inner(app_handle, &state).await
}

/// Inner implementation shared by the Tauri command and the tray menu handler.
pub fn stop_gateway_inner(
    app_handle: tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    let tx = {
        let mut guard = state
            .gateway_shutdown
            .lock()
            .map_err(|_| "Lock poisoned".to_string())?;
        guard.take()
    };

    match tx {
        Some(tx) => {
            let _ = tx.send(());
            let _ = app_handle.emit("gateway-status-changed", false);
            Ok("Lighthouse extinguished".to_string())
        }
        None => Err("Lighthouse is not running".to_string()),
    }
}

#[tauri::command]
pub fn stop_gateway(
    app_handle: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<String, String> {
    stop_gateway_inner(app_handle, &state)
}

#[tauri::command]
pub fn gateway_status(state: tauri::State<AppState>) -> Result<bool, String> {
    let guard = state
        .gateway_shutdown
        .lock()
        .map_err(|_| "Lock poisoned".to_string())?;
    Ok(guard.is_some())
}

fn normalize_host_key(display_name: &str) -> String {
    match display_name {
        "Claude Code" => "claude".to_string(),
        "Codex" => "codex".to_string(),
        "VS Code" => "vscode".to_string(),
        "Cursor" => "cursor".to_string(),
        other => other.to_lowercase().replace(' ', ""),
    }
}
