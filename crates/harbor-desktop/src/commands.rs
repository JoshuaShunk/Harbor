use harbor_core::connector::{self, resolve_env_for_host, HostServerEntry};
use harbor_core::{HarborConfig, ServerConfig};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Mutex;

pub struct AppState {
    config: Mutex<HarborConfig>,
}

impl AppState {
    pub fn new(config: HarborConfig) -> Self {
        Self {
            config: Mutex::new(config),
        }
    }

    fn with_config_mut<T>(&self, f: impl FnOnce(&mut HarborConfig) -> T) -> Result<T, String> {
        let mut config = self.config.lock().unwrap();
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
    let config = state.config.lock().unwrap();

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
    state
        .with_config_mut(|config| {
            let server = ServerConfig {
                source: None,
                command,
                args,
                env,
                enabled: true,
                auto_start: false,
                hosts: BTreeMap::new(),
            };
            config.add_server(name, server).map_err(|e| e.to_string())
        })?
        .map_err(|e| e)?;
    Ok(())
}

#[tauri::command]
pub fn remove_server(state: tauri::State<AppState>, name: String) -> Result<(), String> {
    state
        .with_config_mut(|config| config.remove_server(&name).map_err(|e| e.to_string()))?
        .map_err(|e| e)?;
    Ok(())
}

#[tauri::command]
pub fn toggle_server(
    state: tauri::State<AppState>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .with_config_mut(|config| {
            if let Some(server) = config.servers.get_mut(&name) {
                server.enabled = enabled;
                Ok(())
            } else {
                Err(format!("Server '{name}' not found"))
            }
        })?
        .map_err(|e| e)?;
    Ok(())
}

#[tauri::command]
pub fn sync_host(state: tauri::State<AppState>, host: String) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let conn = connector::get_connector(&host).map_err(|e| e.to_string())?;

    let servers = config.servers_for_host(&host);
    if servers.is_empty() {
        return Ok(format!("No enabled servers for {}", conn.host_name()));
    }

    // Refresh Google Drive credential files if any gdrive server is present
    for (_name, sc) in &servers {
        if sc.args.iter().any(|a| a.contains("gdrive")) {
            let _ = harbor_core::auth::oauth::write_gdrive_credentials();
            break;
        }
    }

    let entries: BTreeMap<String, HostServerEntry> = servers
        .iter()
        .map(|(name, sc)| {
            let resolved_env = resolve_env_for_host(&sc.env);
            (
                (*name).clone(),
                HostServerEntry {
                    command: sc.command.clone(),
                    args: sc.args.clone(),
                    env: resolved_env,
                },
            )
        })
        .collect();

    conn.write_servers(&entries).map_err(|e| e.to_string())?;
    Ok(format!(
        "Synced {} server(s) to {}",
        entries.len(),
        conn.host_name()
    ))
}

#[tauri::command]
pub fn sync_all(state: tauri::State<AppState>) -> Result<String, String> {
    let config = state.config.lock().unwrap();

    let connected_hosts: Vec<String> = config
        .hosts
        .iter()
        .filter(|(_, h)| h.connected)
        .map(|(name, _)| name.clone())
        .collect();

    if connected_hosts.is_empty() {
        return Ok("No connected hosts. Connect a host first.".to_string());
    }

    let mut results = Vec::new();
    for host_name in &connected_hosts {
        let conn = match connector::get_connector(host_name) {
            Ok(c) => c,
            Err(e) => {
                results.push(format!("{}: error ({})", host_name, e));
                continue;
            }
        };

        let servers = config.servers_for_host(host_name);
        if servers.is_empty() {
            results.push(format!("{}: skipped (no servers)", conn.host_name()));
            continue;
        }

        let entries: BTreeMap<String, HostServerEntry> = servers
            .iter()
            .map(|(name, sc)| {
                let resolved_env = resolve_env_for_host(&sc.env);
                (
                    (*name).clone(),
                    HostServerEntry {
                        command: sc.command.clone(),
                        args: sc.args.clone(),
                        env: resolved_env,
                    },
                )
            })
            .collect();

        match conn.write_servers(&entries) {
            Ok(()) => results.push(format!(
                "{}: synced {} server(s)",
                conn.host_name(),
                entries.len()
            )),
            Err(e) => results.push(format!("{}: error ({})", conn.host_name(), e)),
        }
    }

    Ok(results.join("\n"))
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
    Ok(())
}

#[tauri::command]
pub fn disconnect_host(state: tauri::State<AppState>, host: String) -> Result<(), String> {
    state.with_config_mut(|config| {
        if let Some(entry) = config.hosts.get_mut(&host) {
            entry.connected = false;
        }
    })?;
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
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        callback_server.code_rx,
    )
    .await
    .map_err(|_| "Charter timed out. The authorization window was open too long.".to_string())?
    .map_err(|_| "Charter cancelled.".to_string())?;

    // Exchange code for tokens
    let provider = oauth::builtin_providers()
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Unknown provider: {provider_id}"))?;

    let custom_client_id =
        harbor_core::Vault::get(&format!("oauth:{provider_id}:client_id")).ok();
    let custom_client_secret =
        harbor_core::Vault::get(&format!("oauth:{provider_id}:client_secret")).ok();

    // Determine the actual client credentials used (custom or default)
    let effective_client_id = custom_client_id
        .clone()
        .unwrap_or_else(|| provider.default_client_id.clone());
    let effective_client_secret = custom_client_secret
        .clone()
        .or_else(|| provider.default_client_secret.clone());

    let redirect = if provider.requires_https_redirect {
        oauth::HTTPS_REDIRECT_BASE.to_string()
    } else {
        format!("http://127.0.0.1:{port}/callback")
    };
    let tokens = oauth::exchange_code(
        &provider,
        &code,
        &redirect,
        pkce.as_ref().map(|p| p.code_verifier.as_str()),
        custom_client_id.as_deref(),
        custom_client_secret.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    // Store the effective client credentials in vault so credential files can reference them
    let _ = harbor_core::Vault::set(
        &format!("oauth:{provider_id}:client_id"),
        &effective_client_id,
    );
    if let Some(ref secret) = effective_client_secret {
        let _ = harbor_core::Vault::set(
            &format!("oauth:{provider_id}:client_secret"),
            secret,
        );
    }

    oauth::store_tokens(&provider_id, &tokens).map_err(|e| e.to_string())?;

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
    harbor_core::auth::oauth::gdrive_credential_paths()
        .ok_or_else(|| "Google Drive credentials not found. Complete the Charter flow first.".into())
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
