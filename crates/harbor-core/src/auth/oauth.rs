use crate::auth::vault::Vault;
use crate::error::{HarborError, Result};
use axum::{extract::Query, response::Html, routing::get, Router};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tracing::info;

// ---------------------------------------------------------------------------
// Provider definitions
// ---------------------------------------------------------------------------

/// HTTPS redirect base URL for providers that reject `http://` redirect URIs.
/// The local port is encoded in the `state` parameter as `PORT:ORIGINAL_STATE`.
/// The page at this URL extracts the port and redirects to `http://127.0.0.1:{port}/callback`.
pub const HTTPS_REDIRECT_BASE: &str = "https://harbormcp.ai/oauth/callback";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub id: String,
    pub display_name: String,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub default_client_id: String,
    pub default_client_secret: Option<String>,
    pub supports_pkce: bool,
    /// If true, the provider requires HTTPS redirect URIs.
    /// We use the remote redirect page at harbormcp.ai which forwards to localhost.
    pub requires_https_redirect: bool,
}

/// Returns the built-in OAuth providers shipped with Harbor.
///
/// Credentials are injected at **compile time** via environment variables so the
/// source code stays secret-free while release binaries carry the real values.
/// Set these in your CI/CD build environment:
///   HARBOR_GITHUB_CLIENT_ID, HARBOR_GITHUB_CLIENT_SECRET
///   HARBOR_GOOGLE_CLIENT_ID, HARBOR_GOOGLE_CLIENT_SECRET
///   HARBOR_SLACK_CLIENT_ID, HARBOR_SLACK_CLIENT_SECRET
///
/// Users can also supply their own credentials at runtime via Helm → Papers → Own Papers
/// (stored in the OS keychain, checked before these defaults).
///
/// Slack requires HTTPS redirect URIs. We work around this by using a redirect
/// page at harbormcp.ai that forwards the auth code to the local callback server.
pub fn builtin_providers() -> Vec<OAuthProvider> {
    vec![
        OAuthProvider {
            id: "github".into(),
            display_name: "GitHub".into(),
            auth_url: "https://github.com/login/oauth/authorize".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            scopes: vec!["repo".into(), "read:org".into(), "read:packages".into()],
            default_client_id: option_env!("HARBOR_GITHUB_CLIENT_ID")
                .unwrap_or("REPLACE_WITH_GITHUB_CLIENT_ID")
                .into(),
            default_client_secret: option_env!("HARBOR_GITHUB_CLIENT_SECRET").map(String::from),
            supports_pkce: false,
            requires_https_redirect: false,
        },
        OAuthProvider {
            id: "google".into(),
            display_name: "Google".into(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            scopes: vec!["https://www.googleapis.com/auth/drive.readonly".into()],
            default_client_id: option_env!("HARBOR_GOOGLE_CLIENT_ID")
                .unwrap_or("REPLACE_WITH_GOOGLE_CLIENT_ID")
                .into(),
            default_client_secret: option_env!("HARBOR_GOOGLE_CLIENT_SECRET").map(String::from),
            supports_pkce: true,
            requires_https_redirect: false,
        },
        OAuthProvider {
            id: "slack".into(),
            display_name: "Slack".into(),
            auth_url: "https://slack.com/oauth/v2/authorize".into(),
            token_url: "https://slack.com/api/oauth.v2.access".into(),
            scopes: vec![
                "channels:history".into(),
                "channels:read".into(),
                "chat:write".into(),
                "reactions:write".into(),
                "users:read".into(),
                "users.profile:read".into(),
            ],
            default_client_id: option_env!("HARBOR_SLACK_CLIENT_ID")
                .unwrap_or("REPLACE_WITH_SLACK_CLIENT_ID")
                .into(),
            default_client_secret: option_env!("HARBOR_SLACK_CLIENT_SECRET").map(String::from),
            supports_pkce: false,
            requires_https_redirect: true,
        },
    ]
}

/// Look up the provider for a given server based on its Smithery qualified name.
pub fn provider_for_server(qualified_name: &str) -> Option<&'static str> {
    let lower = qualified_name.to_lowercase();
    if lower.contains("github") {
        Some("github")
    } else if lower.contains("google") || lower.contains("gdrive") || lower.contains("gmail") {
        Some("google")
    } else if lower.contains("slack") {
        Some("slack")
    } else {
        None
    }
}

/// Map a provider ID to the env var name that its MCP servers typically expect.
pub fn env_var_for_provider(provider_id: &str) -> &'static str {
    match provider_id {
        "github" => "GITHUB_PERSONAL_ACCESS_TOKEN",
        "google" => "GOOGLE_ACCESS_TOKEN",
        "slack" => "SLACK_BOT_TOKEN",
        _ => "ACCESS_TOKEN",
    }
}

// ---------------------------------------------------------------------------
// PKCE
// ---------------------------------------------------------------------------

pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
}

pub fn generate_pkce() -> PkceChallenge {
    let mut rng = rand::thread_rng();
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let verifier: String = (0..128)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    PkceChallenge {
        code_verifier: verifier,
        code_challenge: challenge,
    }
}

// ---------------------------------------------------------------------------
// Callback server
// ---------------------------------------------------------------------------

pub struct OAuthCallbackServer {
    pub port: u16,
    pub code_rx: oneshot::Receiver<String>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl OAuthCallbackServer {
    /// Start a temporary local HTTP server to receive the OAuth callback.
    pub async fn start() -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| HarborError::OAuthError(format!("Failed to bind callback server: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| HarborError::OAuthError(format!("Failed to get port: {e}")))?
            .port();

        let (code_tx, code_rx) = oneshot::channel::<String>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let code_tx = Arc::new(Mutex::new(Some(code_tx)));

        let app = Router::new().route(
            "/callback",
            get(move |Query(params): Query<HashMap<String, String>>| {
                let code_tx = code_tx.clone();
                async move {
                    if let Some(code) = params.get("code") {
                        if let Some(tx) = code_tx.lock().await.take() {
                            let _ = tx.send(code.clone());
                        }
                        Html(CALLBACK_SUCCESS_HTML.to_string())
                    } else {
                        let error = params
                            .get("error")
                            .cloned()
                            .unwrap_or_else(|| "unknown".into());
                        Html(format!(
                            "<html><body style=\"font-family:system-ui;text-align:center;padding:4rem\">\
                            <h1>Charter Failed</h1>\
                            <p>Error: {error}. Return to Harbor and try again.</p>\
                            </body></html>"
                        ))
                    }
                }
            }),
        );

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .ok();
        });

        info!(port = port, "OAuth callback server started");

        Ok(Self {
            port,
            code_rx,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }

    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

const CALLBACK_SUCCESS_HTML: &str = r#"<html>
<body style="font-family:system-ui;text-align:center;padding:4rem;background:#0f1117;color:#e2e8f0">
<h1 style="font-size:2rem">Charted!</h1>
<p style="color:#94a3b8">Your papers have been received. You can close this window and return to Harbor.</p>
</body>
</html>"#;

// ---------------------------------------------------------------------------
// Token exchange
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: String,
    /// Slack-specific: team ID returned during OAuth
    pub team_id: Option<String>,
}

pub async fn exchange_code(
    provider: &OAuthProvider,
    code: &str,
    redirect_uri: &str,
    pkce_verifier: Option<&str>,
    client_id_override: Option<&str>,
    client_secret_override: Option<&str>,
) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    let client_id = client_id_override.unwrap_or(&provider.default_client_id);

    let mut params: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
    ];

    let verifier_owned;
    if let Some(v) = pkce_verifier {
        verifier_owned = v.to_string();
        params.push(("code_verifier", &verifier_owned));
    }

    let secret_owned;
    let client_secret = client_secret_override
        .map(String::from)
        .or_else(|| provider.default_client_secret.clone());
    if let Some(ref s) = client_secret {
        secret_owned = s.clone();
        params.push(("client_secret", &secret_owned));
    }

    let mut request = client.post(&provider.token_url).form(&params);

    // GitHub requires Accept: application/json header
    if provider.id == "github" {
        request = request.header("Accept", "application/json");
    }

    let response = request
        .send()
        .await
        .map_err(|e| HarborError::OAuthError(format!("Token exchange failed: {e}")))?;

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| HarborError::OAuthError(format!("Failed to parse token response: {e}")))?;

    // Slack uses {"ok": false, "error": "..."} format
    if body.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        let error = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        return Err(HarborError::OAuthError(format!("OAuth error: {error}")));
    }

    if let Some(error) = body.get("error") {
        let desc = body
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        return Err(HarborError::OAuthError(format!(
            "OAuth error: {} {}",
            error.as_str().unwrap_or("unknown"),
            desc,
        )));
    }

    let access_token = body["access_token"]
        .as_str()
        .ok_or_else(|| HarborError::OAuthError("No access_token in response".into()))?
        .to_string();

    let expires_at = body
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .map(|secs| chrono::Utc::now().timestamp() + secs);

    // Slack returns team ID in the response — needed by the Slack MCP server
    let team_id = body
        .get("team")
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(OAuthTokens {
        access_token,
        refresh_token: body
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(String::from),
        expires_at,
        token_type: body
            .get("token_type")
            .and_then(|v| v.as_str())
            .unwrap_or("bearer")
            .to_string(),
        team_id,
    })
}

// ---------------------------------------------------------------------------
// Token storage via Vault
// ---------------------------------------------------------------------------

pub fn store_tokens(provider_id: &str, tokens: &OAuthTokens) -> Result<()> {
    Vault::set(
        &format!("oauth:{provider_id}:access_token"),
        &tokens.access_token,
    )?;
    if let Some(ref refresh) = tokens.refresh_token {
        Vault::set(&format!("oauth:{provider_id}:refresh_token"), refresh)?;
    }
    if let Some(expires_at) = tokens.expires_at {
        Vault::set(
            &format!("oauth:{provider_id}:expires_at"),
            &expires_at.to_string(),
        )?;
    }
    if let Some(ref team_id) = tokens.team_id {
        Vault::set(&format!("oauth:{provider_id}:team_id"), team_id)?;
    }
    info!(provider = provider_id, "OAuth tokens stored in vault");

    // For Google, write credential files that the gdrive MCP server expects
    if provider_id == "google" {
        if let Err(e) = write_gdrive_credentials() {
            tracing::warn!(error = %e, "Failed to write gdrive credential files");
        }
    }

    Ok(())
}

/// Write Google Drive credential files that `@modelcontextprotocol/server-gdrive` expects.
/// Creates two files in `~/.harbor/credentials/`:
///   - `gdrive-oauth-keys.json` — client ID/secret (for token refresh)
///   - `gdrive-credentials.json` — access/refresh tokens
pub fn write_gdrive_credentials() -> Result<()> {
    let creds_dir = dirs::home_dir()
        .ok_or_else(|| HarborError::OAuthError("Cannot determine home directory".into()))?
        .join(".harbor")
        .join("credentials");
    std::fs::create_dir_all(&creds_dir)
        .map_err(|e| HarborError::OAuthError(format!("Failed to create credentials dir: {e}")))?;

    // --- OAuth keys file (client credentials) ---
    let provider = builtin_providers()
        .into_iter()
        .find(|p| p.id == "google")
        .ok_or_else(|| HarborError::OAuthError("Google provider not found".into()))?;

    let client_id =
        Vault::get("oauth:google:client_id").unwrap_or_else(|_| provider.default_client_id.clone());
    let client_secret = Vault::get("oauth:google:client_secret")
        .ok()
        .or_else(|| provider.default_client_secret.clone())
        .unwrap_or_default();

    let oauth_keys = serde_json::json!({
        "installed": {
            "client_id": client_id,
            "client_secret": client_secret,
            "redirect_uris": ["http://127.0.0.1"]
        }
    });
    let keys_path = creds_dir.join("gdrive-oauth-keys.json");
    std::fs::write(
        &keys_path,
        serde_json::to_string_pretty(&oauth_keys).unwrap(),
    )
    .map_err(|e| HarborError::OAuthError(format!("Failed to write OAuth keys: {e}")))?;

    // --- Credentials file (tokens) ---
    let access_token = Vault::get("oauth:google:access_token")?;
    let refresh_token = Vault::get("oauth:google:refresh_token").ok();
    let expires_at = Vault::get("oauth:google:expires_at")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|secs| secs * 1000); // gdrive server expects milliseconds

    let mut creds = serde_json::json!({
        "access_token": access_token,
        "scope": "https://www.googleapis.com/auth/drive.readonly",
        "token_type": "Bearer",
    });
    if let Some(refresh) = refresh_token {
        creds["refresh_token"] = serde_json::Value::String(refresh);
    }
    if let Some(expiry) = expires_at {
        creds["expiry_date"] = serde_json::Value::Number(serde_json::Number::from(expiry));
    }
    let creds_path = creds_dir.join("gdrive-credentials.json");
    std::fs::write(&creds_path, serde_json::to_string_pretty(&creds).unwrap())
        .map_err(|e| HarborError::OAuthError(format!("Failed to write credentials: {e}")))?;

    info!("Google Drive credential files written to {:?}", creds_dir);
    Ok(())
}

/// Returns the absolute paths for the gdrive credential files.
pub fn gdrive_credential_paths() -> Option<(String, String)> {
    let creds_dir = dirs::home_dir()?.join(".harbor").join("credentials");
    let keys_path = creds_dir.join("gdrive-oauth-keys.json");
    let creds_path = creds_dir.join("gdrive-credentials.json");
    if keys_path.exists() && creds_path.exists() {
        Some((
            keys_path.to_string_lossy().into_owned(),
            creds_path.to_string_lossy().into_owned(),
        ))
    } else {
        None
    }
}

pub fn get_access_token(provider_id: &str) -> Result<String> {
    Vault::get(&format!("oauth:{provider_id}:access_token"))
}

pub fn has_valid_token(provider_id: &str) -> bool {
    if Vault::get(&format!("oauth:{provider_id}:access_token")).is_err() {
        return false;
    }
    // Check expiry if recorded
    if let Ok(expires_str) = Vault::get(&format!("oauth:{provider_id}:expires_at")) {
        if let Ok(expires_at) = expires_str.parse::<i64>() {
            return chrono::Utc::now().timestamp() < expires_at;
        }
    }
    true // No expiry means token doesn't expire (e.g. GitHub)
}

pub fn clear_tokens(provider_id: &str) -> Result<()> {
    let _ = Vault::delete(&format!("oauth:{provider_id}:access_token"));
    let _ = Vault::delete(&format!("oauth:{provider_id}:refresh_token"));
    let _ = Vault::delete(&format!("oauth:{provider_id}:expires_at"));
    info!(provider = provider_id, "OAuth tokens cleared");
    Ok(())
}

// ---------------------------------------------------------------------------
// OAuth flow orchestration
// ---------------------------------------------------------------------------

pub async fn start_oauth_flow(
    provider_id: &str,
) -> Result<(String, OAuthCallbackServer, Option<PkceChallenge>)> {
    let provider = builtin_providers()
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| HarborError::OAuthError(format!("Unknown provider: {provider_id}")))?;

    let callback_server = OAuthCallbackServer::start().await?;

    // For providers requiring HTTPS redirects, use the remote redirect page
    // and encode the local port in the state parameter so it can forward back.
    let redirect_uri = if provider.requires_https_redirect {
        HTTPS_REDIRECT_BASE.to_string()
    } else {
        callback_server.redirect_uri()
    };

    // Check for user-overridden client_id
    let client_id = Vault::get(&format!("oauth:{provider_id}:client_id"))
        .unwrap_or_else(|_| provider.default_client_id.clone());

    let scope_str = provider.scopes.join(" ");

    let pkce = if provider.supports_pkce {
        Some(generate_pkce())
    } else {
        None
    };

    let mut auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}",
        provider.auth_url,
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&scope_str),
    );

    if let Some(ref pkce) = pkce {
        auth_url.push_str(&format!(
            "&code_challenge={}&code_challenge_method=S256",
            urlencoding::encode(&pkce.code_challenge)
        ));
    }

    // Google requires access_type=offline to return a refresh_token
    if provider_id == "google" {
        auth_url.push_str("&access_type=offline&prompt=consent");
    }

    // CSRF state parameter — for HTTPS redirect providers, encode the local
    // port so the redirect page can forward back to our local callback server.
    let csrf_token = uuid::Uuid::new_v4().to_string();
    let state = if provider.requires_https_redirect {
        format!("{}:{}", callback_server.port, csrf_token)
    } else {
        csrf_token
    };
    auth_url.push_str(&format!("&state={}", urlencoding::encode(&state)));

    info!(
        provider = provider_id,
        port = callback_server.port,
        "OAuth flow started"
    );

    Ok((auth_url, callback_server, pkce))
}
