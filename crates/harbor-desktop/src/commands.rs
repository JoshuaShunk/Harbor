use harbor_core::connector;
use harbor_core::fleet;
use harbor_core::gateway::{Gateway, RequestLogger};
use harbor_core::{HarborConfig, ServerConfig};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

#[derive(Clone, Serialize)]
pub struct PublishInfoResponse {
    pub url: String,
    pub token: String,
    pub transport: String,
}

#[derive(Clone)]
pub struct AppState {
    config: Arc<Mutex<HarborConfig>>,
    /// Holds the shutdown sender while the gateway is running.
    /// `Some(tx)` = running, `None` = stopped.
    /// Wrapped in Arc so the background task can clear it on exit.
    gateway_shutdown: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// Holds the shutdown sender while publish is running.
    publish_shutdown: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// Holds the publish info after successful publishing.
    publish_info: Arc<Mutex<Option<PublishInfoResponse>>>,
    /// Ring buffer of recent gateway tool call records.
    /// Persists across gateway restarts — cleared only by the user.
    pub request_logger: Arc<RequestLogger>,
}

impl AppState {
    pub fn new(config: HarborConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            gateway_shutdown: Arc::new(Mutex::new(None)),
            publish_shutdown: Arc::new(Mutex::new(None)),
            publish_info: Arc::new(Mutex::new(None)),
            request_logger: Arc::new(RequestLogger::new()),
        }
    }

    pub fn gateway_running(&self) -> bool {
        self.gateway_shutdown
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    pub fn publish_running(&self) -> bool {
        self.publish_shutdown
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    pub fn hide_on_close(&self) -> bool {
        self.config
            .lock()
            .map(|c| c.harbor.hide_on_close)
            .unwrap_or(true)
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
    is_remote: bool,
    source: Option<String>,
    locally_modified: bool,
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
    gateway_host: String,
    local_ip: Option<String>,
}

// -- Commands --

#[tauri::command]
pub fn get_status(state: tauri::State<AppState>) -> Result<HarborStatusResponse, String> {
    // Load fleet state outside the config lock (file I/O, infallible).
    let fleet_state = fleet::FleetState::load();

    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned — restart the application".to_string())?;

    let servers: Vec<ServerStatus> = config
        .servers
        .iter()
        .map(|(name, sc)| {
            let locally_modified = if sc.source.as_deref() == Some(fleet::FLEET_SOURCE) {
                let def = fleet::FleetServerDef::from_server_config(sc);
                fleet_state.is_locally_clean(name, &def) == Some(false)
            } else {
                false
            };
            ServerStatus {
                name: name.clone(),
                enabled: sc.enabled,
                running: false,
                pid: None,
                command: if let Some(ref url) = sc.url {
                    url.clone()
                } else {
                    format!(
                        "{} {}",
                        sc.command.as_deref().unwrap_or(""),
                        sc.args.join(" ")
                    )
                },
                is_remote: sc.is_remote(),
                source: sc.source.clone(),
                locally_modified,
            }
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

    let local_ip = std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .ok()
        .map(|addr| addr.ip().to_string());

    Ok(HarborStatusResponse {
        servers,
        hosts,
        gateway_port: config.harbor.gateway_port,
        gateway_host: config.harbor.gateway_host.clone(),
        local_ip,
    })
}

#[tauri::command]
pub fn add_server(
    state: tauri::State<AppState>,
    name: String,
    command: Option<String>,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    url: Option<String>,
    headers: Option<BTreeMap<String, String>>,
    source: Option<String>,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        let server = ServerConfig {
            source,
            command,
            args,
            env,
            url,
            headers,
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

    // Reload gateway so running servers pick up the new token
    let state = app_handle.state::<AppState>();
    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });

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
    is_remote: bool,
    /// For manual-token servers: the vault key to store the API key under.
    manual_vault_key: Option<String>,
    /// What kind of extra args the UI should prompt for.
    extra_args_kind: String,
    /// Human-readable label for the extra args prompt.
    extra_args_label: Option<String>,
    /// Placeholder text for text-input extra args.
    extra_args_placeholder: Option<String>,
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
            let (extra_args_kind, extra_args_label, extra_args_placeholder) = match &s.extra_args {
                harbor_core::ExtraArgs::None => ("none".to_string(), None, None),
                harbor_core::ExtraArgs::Directories { label } => {
                    ("directories".to_string(), Some(label.to_string()), None)
                }
                harbor_core::ExtraArgs::FilePath { label, .. } => {
                    ("file".to_string(), Some(label.to_string()), None)
                }
                harbor_core::ExtraArgs::TextInput {
                    label, placeholder, ..
                } => (
                    "text".to_string(),
                    Some(label.to_string()),
                    Some(placeholder.to_string()),
                ),
            };
            NativeServerInfo {
                id: s.id.to_string(),
                display_name: s.display_name.to_string(),
                description: s.description.to_string(),
                auth_kind,
                has_auth,
                is_remote: s.is_remote(),
                manual_vault_key,
                extra_args_kind,
                extra_args_label,
                extra_args_placeholder,
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
    extra_args: Option<Vec<String>>,
) -> Result<(), String> {
    let native =
        harbor_core::catalog::lookup(&id).ok_or_else(|| format!("Unknown native server: {id}"))?;

    let server_name = name.unwrap_or_else(|| native.id.to_string());

    // Check if this server is already docked (re-docking)
    let already_exists = state
        .config
        .lock()
        .map(|c| c.servers.contains_key(&server_name))
        .unwrap_or(false);

    // Handle OAuth if needed.
    // If the server already exists (re-docking), always re-authenticate since
    // the existing token may be invalid/revoked even though has_auth() returns true.
    if let harbor_core::AuthKind::OAuth(ref provider_id) = native.auth {
        if already_exists || !harbor_core::catalog::has_auth(&native) {
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
        let server = if native.is_remote() {
            // Remote HTTP server — use url + headers, no command/args
            let headers = harbor_core::catalog::build_headers(&native);
            ServerConfig {
                source: Some(format!("native:{}", native.id)),
                command: None,
                args: vec![],
                env,
                url: native.url.map(String::from),
                headers: if headers.is_empty() {
                    None
                } else {
                    Some(headers)
                },
                enabled: true,
                auto_start: false,
                hosts: BTreeMap::new(),
                tool_allowlist: None,
                tool_blocklist: None,
                tool_hosts: BTreeMap::new(),
            }
        } else {
            // Stdio server — use command + args
            ServerConfig {
                source: Some(format!("native:{}", native.id)),
                command: native.command.map(String::from),
                args: {
                    let mut args: Vec<String> = native.args.iter().map(|s| s.to_string()).collect();
                    if let Some(extra) = extra_args {
                        args.extend(extra);
                    }
                    args
                },
                env,
                url: None,
                headers: None,
                enabled: true,
                auto_start: false,
                hosts: BTreeMap::new(),
                tool_allowlist: None,
                tool_blocklist: None,
                tool_hosts: BTreeMap::new(),
            }
        };
        config
            .upsert_server(server_name, server)
            .map_err(|e| e.to_string())
    })??;

    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });

    Ok(())
}

// -- Extra args (get/set for native servers) --

#[derive(Serialize)]
pub struct ServerExtraArgsInfo {
    extra_args: Vec<String>,
    extra_args_kind: String,
    extra_args_label: Option<String>,
    extra_args_placeholder: Option<String>,
}

/// Get the extra args for a server (everything after the catalog default args).
#[tauri::command]
pub fn get_server_extra_args(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<ServerExtraArgsInfo, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;

    let sc = config
        .servers
        .get(&name)
        .ok_or_else(|| format!("Server '{name}' not found"))?;

    // Look up the catalog entry via source field
    let native_id = sc.source.as_ref().and_then(|s| s.strip_prefix("native:"));

    if let Some(id) = native_id {
        if let Some(native) = harbor_core::catalog::lookup(id) {
            let default_count = native.args.len();
            let extra: Vec<String> = sc.args.iter().skip(default_count).cloned().collect();
            let (kind, label, placeholder) = match &native.extra_args {
                harbor_core::ExtraArgs::None => ("none".to_string(), None, None),
                harbor_core::ExtraArgs::Directories { label } => {
                    ("directories".to_string(), Some(label.to_string()), None)
                }
                harbor_core::ExtraArgs::FilePath { label, .. } => {
                    ("file".to_string(), Some(label.to_string()), None)
                }
                harbor_core::ExtraArgs::TextInput {
                    label, placeholder, ..
                } => (
                    "text".to_string(),
                    Some(label.to_string()),
                    Some(placeholder.to_string()),
                ),
            };
            return Ok(ServerExtraArgsInfo {
                extra_args: extra,
                extra_args_kind: kind,
                extra_args_label: label,
                extra_args_placeholder: placeholder,
            });
        }
    }

    Ok(ServerExtraArgsInfo {
        extra_args: vec![],
        extra_args_kind: "none".to_string(),
        extra_args_label: None,
        extra_args_placeholder: None,
    })
}

/// Set the extra args for a native server (replaces everything after the catalog default args).
#[tauri::command]
pub async fn set_server_extra_args(
    state: tauri::State<'_, AppState>,
    name: String,
    extra_args: Vec<String>,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        let sc = config
            .servers
            .get_mut(&name)
            .ok_or_else(|| format!("Server '{name}' not found"))?;

        let native_id = sc
            .source
            .as_ref()
            .and_then(|s| s.strip_prefix("native:"))
            .map(|s| s.to_string());

        if let Some(ref id) = native_id {
            if let Some(native) = harbor_core::catalog::lookup(id) {
                let mut new_args: Vec<String> = native.args.iter().map(|s| s.to_string()).collect();
                new_args.extend(extra_args);
                sc.args = new_args;
            }
        }

        config.save().map_err(|e| e.to_string())
    })??;

    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });

    Ok(())
}

// -- General args (any server) --

/// Get the full args array for any server.
#[tauri::command]
pub fn get_server_args(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Vec<String>, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned".to_string())?;

    let sc = config
        .servers
        .get(&name)
        .ok_or_else(|| format!("Server '{name}' not found"))?;

    Ok(sc.args.clone())
}

/// Set the full args array for any server.
#[tauri::command]
pub async fn set_server_args(
    state: tauri::State<'_, AppState>,
    name: String,
    args: Vec<String>,
) -> Result<(), String> {
    state.with_config_mut(|config| -> Result<(), String> {
        let sc = config
            .servers
            .get_mut(&name)
            .ok_or_else(|| format!("Server '{name}' not found"))?;
        sc.args = args;
        Ok(())
    })??;

    let s = (*state).clone();
    tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });

    Ok(())
}

// -- Config schema (from MCP Registry) --

#[derive(Serialize)]
pub struct ConfigSchemaArg {
    arg_type: String,
    name: String,
    description: Option<String>,
    is_required: bool,
    format: String,
    default: Option<String>,
    is_secret: bool,
    is_repeated: bool,
    choices: Option<Vec<String>>,
    placeholder: Option<String>,
    value_hint: Option<String>,
}

#[derive(Serialize)]
pub struct ConfigSchemaEnvVar {
    name: String,
    description: Option<String>,
    is_required: bool,
    is_secret: bool,
    default: Option<String>,
}

#[derive(Serialize)]
pub struct ConfigSchemaResponse {
    args: Option<Vec<ConfigSchemaArg>>,
    env_vars: Option<Vec<ConfigSchemaEnvVar>>,
    registry_name: Option<String>,
}

/// Fetch the config schema for a docked server from the MCP Registry.
#[tauri::command]
pub async fn get_config_schema(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<ConfigSchemaResponse, String> {
    let lookup_key = {
        let config = state
            .config
            .lock()
            .map_err(|_| "Config lock poisoned".to_string())?;

        let sc = config
            .servers
            .get(&name)
            .ok_or_else(|| format!("Server '{name}' not found"))?;

        extract_lookup_key(sc)
    };

    let Some(key) = lookup_key else {
        return Ok(ConfigSchemaResponse {
            args: None,
            env_vars: None,
            registry_name: None,
        });
    };

    match harbor_core::marketplace::schema::lookup_config_schema(&key).await {
        Ok(Some(schema)) => Ok(ConfigSchemaResponse {
            args: Some(
                schema
                    .package_arguments
                    .into_iter()
                    .map(|a| ConfigSchemaArg {
                        arg_type: a.arg_type,
                        name: a.name,
                        description: a.description,
                        is_required: a.is_required,
                        format: a.format,
                        default: a.default,
                        is_secret: a.is_secret,
                        is_repeated: a.is_repeated,
                        choices: a.choices,
                        placeholder: a.placeholder,
                        value_hint: a.value_hint,
                    })
                    .collect(),
            ),
            env_vars: Some(
                schema
                    .environment_variables
                    .into_iter()
                    .map(|e| ConfigSchemaEnvVar {
                        name: e.name,
                        description: e.description,
                        is_required: e.is_required,
                        is_secret: e.is_secret,
                        default: e.default,
                    })
                    .collect(),
            ),
            registry_name: schema.registry_name,
        }),
        _ => Ok(ConfigSchemaResponse {
            args: None,
            env_vars: None,
            registry_name: None,
        }),
    }
}

/// Extract a package identifier to look up from a server's config.
fn extract_lookup_key(sc: &ServerConfig) -> Option<String> {
    // Check source for registry prefix
    if let Some(ref source) = sc.source {
        if let Some(id) = source.strip_prefix("registry:") {
            return Some(id.to_string());
        }
    }

    // Scan args for package identifiers
    for arg in &sc.args {
        // npm scoped packages: @scope/package-name
        if arg.starts_with('@') && arg.contains('/') {
            return Some(arg.clone());
        }
        // pypi packages: mcp-server-* or mcp_server_*
        if arg.starts_with("mcp-server-") || arg.starts_with("mcp_server_") {
            return Some(arg.clone());
        }
    }

    None
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

    // Create the appropriate bridge type
    let bridge: harbor_core::gateway::bridge::Bridge = if server_config.is_remote() {
        let raw_headers = server_config.headers.clone().unwrap_or_default();
        // Detect OAuth provider from vault:oauth:*:access_token patterns in headers
        let oauth_provider = raw_headers.values().find_map(|v| {
            v.strip_prefix("Bearer vault:oauth:")
                .or_else(|| v.strip_prefix("vault:oauth:"))
                .and_then(|rest| rest.strip_suffix(":access_token"))
                .map(String::from)
        });
        let http_bridge = harbor_core::gateway::http::HttpBridge::new(
            &server,
            server_config.url.as_deref().unwrap_or_default(),
            raw_headers,
            oauth_provider,
        )
        .map_err(|e| format!("Failed to create HTTP bridge: {e}"))?;
        harbor_core::gateway::bridge::Bridge::Http(http_bridge)
    } else {
        let resolved_env = connector::resolve_env_for_host(&server_config.env);
        let stdio_bridge =
            harbor_core::gateway::stdio::StdioBridge::spawn(&server, &server_config, &resolved_env)
                .await
                .map_err(|e| format!("Failed to start server: {e}"))?;
        harbor_core::gateway::bridge::Bridge::Stdio(stdio_bridge)
    };

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

    // Shut down the temporary bridge
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
    let gateway = Gateway::new(config, Arc::clone(&state.request_logger));

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

#[tauri::command]
pub fn set_gateway_settings(
    state: tauri::State<AppState>,
    host: String,
    token: Option<String>,
) -> Result<(), String> {
    state.with_config_mut(|config| {
        config.harbor.gateway_host = host;
        config.harbor.gateway_token = token;
    })?;
    Ok(())
}

#[tauri::command]
pub fn get_gateway_settings(state: tauri::State<AppState>) -> Result<GatewaySettings, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Config lock poisoned — restart the application".to_string())?;
    Ok(GatewaySettings {
        host: config.harbor.gateway_host.clone(),
        token: config.harbor.gateway_token.clone(),
    })
}

#[tauri::command]
pub async fn reload_gateway(state: tauri::State<'_, AppState>) -> Result<(), String> {
    reload_gateway_if_running(&state).await;
    Ok(())
}

#[derive(Serialize)]
pub struct GatewaySettings {
    host: String,
    token: Option<String>,
}

// -- Request log commands --

#[tauri::command]
pub fn get_request_logs(
    state: tauri::State<AppState>,
    limit: Option<usize>,
) -> Vec<harbor_core::gateway::logger::RequestLog> {
    let limit = limit.unwrap_or(100).min(500);
    state.request_logger.recent(limit)
}

#[tauri::command]
pub fn clear_request_logs(state: tauri::State<AppState>) {
    state.request_logger.clear();
}

// -- Publish commands --

#[tauri::command]
pub async fn start_publish(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    subdomain: Option<String>,
    relay: Option<String>,
    tools: Option<Vec<String>>,
) -> Result<PublishInfoResponse, String> {
    // Check if already publishing
    {
        let guard = state
            .publish_shutdown
            .lock()
            .map_err(|_| "Lock poisoned".to_string())?;
        if guard.is_some() {
            return Err("Already publishing".to_string());
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
    let relay_addr = relay
        .or(config.harbor.publish_relay.clone())
        .unwrap_or_else(|| "relay.harbormcp.ai".to_string());
    let subdomain = subdomain.or(config.harbor.publish_subdomain.clone());
    let token = config.harbor.publish_token.clone();
    let relay_key = config.harbor.publish_relay_key.clone();
    let tools = tools.or(config.harbor.publish_tools.clone());

    let transport_config = harbor_core::relay::TransportConfig {
        gateway_addr: format!("http://127.0.0.1:{port}"),
        relay_addr: Some(format!("{relay_addr}:7800")),
        auth_token: token,
        subdomain,
        relay_public_key: relay_key,
        tools,
        gateway_port: port,
    };

    let client = harbor_core::relay::PublishClient::new(transport_config);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let shutdown_ref = Arc::clone(&state.publish_shutdown);
    let info_ref = Arc::clone(&state.publish_info);
    {
        let mut guard = shutdown_ref
            .lock()
            .map_err(|_| "Lock poisoned".to_string())?;
        *guard = Some(shutdown_tx);
    }

    // Use run_with_info so we get the PublishInfo as soon as registration succeeds
    let (early_info_tx, early_info_rx) = tokio::sync::oneshot::channel();
    // Separate channel for errors during connection
    let (err_tx, err_rx) = tokio::sync::oneshot::channel::<String>();

    let app_handle_bg = app_handle.clone();
    tokio::spawn(async move {
        match client.run_with_info(shutdown_rx, early_info_tx).await {
            Ok(_) => {}
            Err(e) => {
                // If we haven't sent info yet, send the error
                let _ = err_tx.send(e.to_string());
            }
        }
        // Clear state on exit
        if let Ok(mut guard) = shutdown_ref.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = info_ref.lock() {
            *guard = None;
        }
        let _ = app_handle_bg.emit("publish-status-changed", false);
        tracing::info!("Publish stopped");
    });

    // Wait for either publish info (success) or error
    tokio::select! {
        info = early_info_rx => {
            let info = info.map_err(|_| "Publish task terminated unexpectedly".to_string())?;
            let response = PublishInfoResponse {
                url: info.url,
                token: info.token,
                transport: info.transport,
            };
            if let Ok(mut guard) = state.publish_info.lock() {
                *guard = Some(response.clone());
            }
            let _ = app_handle.emit("publish-status-changed", true);
            Ok(response)
        }
        err = err_rx => {
            let err_msg = err.unwrap_or_else(|_| "Unknown publish error".to_string());
            if let Ok(mut guard) = state.publish_shutdown.lock() {
                *guard = None;
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub fn stop_publish(
    app_handle: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<String, String> {
    let tx = {
        let mut guard = state
            .publish_shutdown
            .lock()
            .map_err(|_| "Lock poisoned".to_string())?;
        guard.take()
    };

    // Clear publish info
    if let Ok(mut guard) = state.publish_info.lock() {
        *guard = None;
    }

    match tx {
        Some(tx) => {
            let _ = tx.send(());
            let _ = app_handle.emit("publish-status-changed", false);
            Ok("Publish stopped".to_string())
        }
        None => Err("Not currently publishing".to_string()),
    }
}

#[derive(Serialize)]
pub struct PublishStatusResponse {
    publishing: bool,
    info: Option<PublishInfoResponse>,
}

#[tauri::command]
pub fn publish_status(state: tauri::State<AppState>) -> Result<PublishStatusResponse, String> {
    let publishing = state.publish_running();
    let info = state
        .publish_info
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    Ok(PublishStatusResponse { publishing, info })
}

// -- Fleet (crew) commands --

#[derive(Serialize)]
pub struct FleetStatusResponse {
    initialized: bool,
    remote_url: Option<String>,
    ahead: usize,
    behind: usize,
}

#[tauri::command]
pub fn fleet_status() -> FleetStatusResponse {
    if !fleet::is_initialized() {
        return FleetStatusResponse {
            initialized: false,
            remote_url: None,
            ahead: 0,
            behind: 0,
        };
    }

    let dir = match fleet::fleet_dir() {
        Ok(d) => d,
        Err(_) => {
            return FleetStatusResponse {
                initialized: false,
                remote_url: None,
                ahead: 0,
                behind: 0,
            }
        }
    };

    let git = fleet::FleetGit::new(dir);
    let remote_url = git.remote_url();
    let (ahead, behind) = if git.has_remote() {
        git.divergence().unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    FleetStatusResponse {
        initialized: true,
        remote_url,
        ahead,
        behind,
    }
}

#[derive(Serialize)]
pub struct FleetPullResult {
    added: Vec<String>,
    updated: Vec<String>,
    locally_modified: Vec<String>,
    conflicts: Vec<String>,
}

#[tauri::command]
pub async fn fleet_pull(state: tauri::State<'_, AppState>) -> Result<FleetPullResult, String> {
    if !fleet::is_initialized() {
        return Err(
            "Fleet not initialized. Run `harbor crew init` from the terminal first.".to_string(),
        );
    }

    let dir = fleet::fleet_dir().map_err(|e| e.to_string())?;
    let git = fleet::FleetGit::new(dir);

    if git.has_remote() {
        git.pull().map_err(|e| e.to_string())?;
    }

    let fleet_config = fleet::load().map_err(|e| e.to_string())?;

    if fleet_config.servers.is_empty() {
        return Ok(FleetPullResult {
            added: vec![],
            updated: vec![],
            locally_modified: vec![],
            conflicts: vec![],
        });
    }

    let mut local = harbor_core::HarborConfig::load().map_err(|e| e.to_string())?;
    let mut fleet_state = fleet::FleetState::load();
    let result = fleet::merge(&mut local, &fleet_config, &mut fleet_state, false);

    if result.has_changes() {
        local.save().map_err(|e| e.to_string())?;
        fleet_state.save().map_err(|e| e.to_string())?;

        // Reload the in-memory config from disk.
        if let Ok(fresh) = harbor_core::HarborConfig::load() {
            if let Ok(mut guard) = state.config.lock() {
                *guard = fresh;
            }
        }

        let s = (*state).clone();
        tauri::async_runtime::spawn(async move { auto_sync_and_reload(&s).await });
    }

    Ok(FleetPullResult {
        added: result.added().iter().map(|s| s.to_string()).collect(),
        updated: result.updated().iter().map(|s| s.to_string()).collect(),
        locally_modified: result
            .locally_modified()
            .iter()
            .map(|s| s.to_string())
            .collect(),
        conflicts: result
            .conflicts()
            .iter()
            .map(|(n, _)| n.to_string())
            .collect(),
    })
}

fn normalize_host_key(display_name: &str) -> String {
    match display_name {
        "Claude Code" => "claude".to_string(),
        "Claude Desktop" => "claude-desktop".to_string(),
        "Cline" => "cline".to_string(),
        "Codex" => "codex".to_string(),
        "Cursor" => "cursor".to_string(),
        "Roo Code" => "roo-code".to_string(),
        "VS Code" => "vscode".to_string(),
        "Windsurf" => "windsurf".to_string(),
        other => other.to_lowercase().replace(' ', ""),
    }
}

// -- App behaviour commands --

#[tauri::command]
pub fn get_hide_on_close(state: tauri::State<AppState>) -> bool {
    state.hide_on_close()
}

#[tauri::command]
pub fn set_hide_on_close(state: tauri::State<AppState>, enabled: bool) -> Result<(), String> {
    state.with_config_mut(|c| {
        c.harbor.hide_on_close = enabled;
    })
}

#[tauri::command]
pub fn autostart_is_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn autostart_enable(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().enable().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn autostart_disable(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().disable().map_err(|e| e.to_string())
}
