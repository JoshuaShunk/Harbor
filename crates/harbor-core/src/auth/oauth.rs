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
    pub default_client_id: Option<String>,
    pub default_client_secret: Option<String>,
    pub supports_pkce: bool,
    /// If true, the provider requires HTTPS redirect URIs.
    /// We use the remote redirect page at harbormcp.ai which forwards to localhost.
    pub requires_https_redirect: bool,
    /// If true, scopes are sent as `user_scope` instead of `scope` in the auth URL,
    /// and the access token is read from `authed_user.access_token` in the response.
    /// This is used for Slack user-token OAuth flows.
    pub uses_user_scope: bool,
    /// Dynamic client registration endpoint (RFC 7591).
    /// If set, Harbor will register itself to obtain a client_id before starting the auth flow.
    pub registration_endpoint: Option<String>,
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
            default_client_id: option_env!("HARBOR_GITHUB_CLIENT_ID").map(String::from),
            default_client_secret: option_env!("HARBOR_GITHUB_CLIENT_SECRET").map(String::from),
            supports_pkce: false,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: None,
        },
        OAuthProvider {
            id: "google".into(),
            display_name: "Google".into(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            scopes: vec![
                "https://www.googleapis.com/auth/drive.readonly".into(),
                "https://www.googleapis.com/auth/gmail.modify".into(),
                "https://www.googleapis.com/auth/calendar".into(),
                "https://www.googleapis.com/auth/spreadsheets".into(),
                "https://www.googleapis.com/auth/documents".into(),
                "https://www.googleapis.com/auth/chat.messages".into(),
                "https://www.googleapis.com/auth/chat.spaces.readonly".into(),
                "https://www.googleapis.com/auth/admin.directory.user.readonly".into(),
            ],
            default_client_id: option_env!("HARBOR_GOOGLE_CLIENT_ID").map(String::from),
            default_client_secret: option_env!("HARBOR_GOOGLE_CLIENT_SECRET").map(String::from),
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: None,
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
                "users:read.email".into(),
                "search:read.public".into(),
                "search:read.private".into(),
            ],
            default_client_id: option_env!("HARBOR_SLACK_CLIENT_ID").map(String::from),
            default_client_secret: option_env!("HARBOR_SLACK_CLIENT_SECRET").map(String::from),
            supports_pkce: false,
            requires_https_redirect: true,
            uses_user_scope: true,
            registration_endpoint: None,
        },
        OAuthProvider {
            id: "atlassian".into(),
            display_name: "Atlassian".into(),
            auth_url: "https://mcp.atlassian.com/v1/authorize".into(),
            token_url: "https://cf.mcp.atlassian.com/v1/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://cf.mcp.atlassian.com/v1/register".into()),
        },
        OAuthProvider {
            id: "linear".into(),
            display_name: "Linear".into(),
            auth_url: "https://mcp.linear.app/authorize".into(),
            token_url: "https://mcp.linear.app/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://mcp.linear.app/register".into()),
        },
        OAuthProvider {
            id: "notion".into(),
            display_name: "Notion".into(),
            auth_url: "https://mcp.notion.com/authorize".into(),
            token_url: "https://mcp.notion.com/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://mcp.notion.com/register".into()),
        },
        OAuthProvider {
            id: "sentry".into(),
            display_name: "Sentry".into(),
            auth_url: "https://mcp.sentry.dev/oauth/authorize".into(),
            token_url: "https://mcp.sentry.dev/oauth/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://mcp.sentry.dev/oauth/register".into()),
        },
        OAuthProvider {
            id: "figma".into(),
            display_name: "Figma".into(),
            auth_url: "https://www.figma.com/oauth/mcp".into(),
            token_url: "https://api.figma.com/v1/oauth/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://api.figma.com/v1/oauth/mcp/register".into()),
        },
        OAuthProvider {
            id: "stripe".into(),
            display_name: "Stripe".into(),
            auth_url: "https://access.stripe.com/mcp/oauth2/authorize".into(),
            token_url: "https://access.stripe.com/mcp/oauth2/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://access.stripe.com/mcp/oauth2/register".into()),
        },
        OAuthProvider {
            id: "vercel".into(),
            display_name: "Vercel".into(),
            auth_url: "https://vercel.com/oauth/authorize".into(),
            token_url: "https://vercel.com/api/login/oauth/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://vercel.com/api/login/oauth/register".into()),
        },
        OAuthProvider {
            id: "supabase".into(),
            display_name: "Supabase".into(),
            auth_url: "https://api.supabase.com/v1/oauth/authorize".into(),
            token_url: "https://api.supabase.com/v1/oauth/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some(
                "https://api.supabase.com/platform/oauth/apps/register".into(),
            ),
        },
        OAuthProvider {
            id: "cloudflare".into(),
            display_name: "Cloudflare".into(),
            auth_url: "https://bindings.mcp.cloudflare.com/oauth/authorize".into(),
            token_url: "https://bindings.mcp.cloudflare.com/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://bindings.mcp.cloudflare.com/register".into()),
        },
        OAuthProvider {
            id: "neon".into(),
            display_name: "Neon".into(),
            auth_url: "https://mcp.neon.tech/api/authorize".into(),
            token_url: "https://mcp.neon.tech/api/token".into(),
            scopes: vec![],
            default_client_id: None,
            default_client_secret: None,
            supports_pkce: true,
            requires_https_redirect: false,
            uses_user_scope: false,
            registration_endpoint: Some("https://mcp.neon.tech/api/register".into()),
        },
    ]
}

/// Look up the provider for a given server based on its registry qualified name.
/// Matches on the server segment (after the last `/`) to avoid false positives
/// from namespace prefixes like `io.github.*`.
pub fn provider_for_server(qualified_name: &str) -> Option<&'static str> {
    let server_part = qualified_name
        .rsplit('/')
        .next()
        .unwrap_or(qualified_name)
        .to_lowercase();
    if server_part.contains("github") {
        Some("github")
    } else if server_part.contains("google")
        || server_part.contains("gdrive")
        || server_part.contains("gmail")
    {
        Some("google")
    } else if server_part.contains("slack") {
        Some("slack")
    } else if server_part.contains("atlassian")
        || server_part.contains("jira")
        || server_part.contains("confluence")
    {
        Some("atlassian")
    } else if server_part.contains("linear") {
        Some("linear")
    } else if server_part.contains("notion") {
        Some("notion")
    } else if server_part.contains("sentry") {
        Some("sentry")
    } else if server_part.contains("figma") {
        Some("figma")
    } else if server_part.contains("stripe") {
        Some("stripe")
    } else if server_part.contains("vercel") {
        Some("vercel")
    } else if server_part.contains("supabase") {
        Some("supabase")
    } else if server_part.contains("cloudflare") {
        Some("cloudflare")
    } else if server_part.contains("neon") {
        Some("neon")
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
        "atlassian" => "ATLASSIAN_ACCESS_TOKEN",
        "linear" => "LINEAR_ACCESS_TOKEN",
        "notion" => "NOTION_ACCESS_TOKEN",
        "sentry" => "SENTRY_ACCESS_TOKEN",
        "figma" => "FIGMA_ACCESS_TOKEN",
        "stripe" => "STRIPE_ACCESS_TOKEN",
        "vercel" => "VERCEL_ACCESS_TOKEN",
        "supabase" => "SUPABASE_ACCESS_TOKEN",
        "cloudflare" => "CLOUDFLARE_ACCESS_TOKEN",
        "neon" => "NEON_ACCESS_TOKEN",
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
// Dynamic client registration (RFC 7591)
// ---------------------------------------------------------------------------

/// Register a client dynamically with a provider that supports RFC 7591.
/// Returns the `client_id` (and optional `client_secret`) and stores them
/// in the vault for reuse.
///
/// Always re-registers because the local callback server uses an ephemeral
/// port, so the redirect_uri changes each time.
async fn dynamic_register(
    provider_id: &str,
    registration_endpoint: &str,
    redirect_uri: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "client_name": "Harbor MCP Hub",
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    });

    let response = client
        .post(registration_endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| HarborError::OAuthError(format!("Dynamic client registration failed: {e}")))?;

    let status = response.status();
    let body_text = response.text().await.map_err(|e| {
        HarborError::OAuthError(format!("Failed to read registration response: {e}"))
    })?;

    if !status.is_success() {
        return Err(HarborError::OAuthError(format!(
            "Dynamic client registration returned {status}: {body_text}"
        )));
    }

    let resp: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| {
        HarborError::OAuthError(format!("Failed to parse registration response: {e}"))
    })?;

    let client_id = resp["client_id"]
        .as_str()
        .ok_or_else(|| HarborError::OAuthError("No client_id in registration response".into()))?
        .to_string();

    // Store for reuse
    Vault::set(&format!("oauth:{provider_id}:client_id"), &client_id)?;

    if let Some(secret) = resp["client_secret"].as_str() {
        Vault::set(&format!("oauth:{provider_id}:client_secret"), secret)?;
    }

    info!(
        provider = provider_id,
        "Dynamic client registration complete"
    );
    Ok(client_id)
}

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
    let default_id;
    let client_id = if let Some(o) = client_id_override {
        o
    } else if let Some(ref d) = provider.default_client_id {
        d.as_str()
    } else {
        default_id = String::new();
        &default_id
    };

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

    // For user-scope providers (Slack), the user token is in authed_user.access_token
    let access_token = if provider.uses_user_scope {
        body.get("authed_user")
            .and_then(|u| u.get("access_token"))
            .and_then(|v| v.as_str())
            .or_else(|| body["access_token"].as_str())
            .ok_or_else(|| {
                HarborError::OAuthError("No access_token in user-scope response".into())
            })?
            .to_string()
    } else {
        body["access_token"]
            .as_str()
            .ok_or_else(|| HarborError::OAuthError("No access_token in response".into()))?
            .to_string()
    };

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

    let client_id = Vault::get("oauth:google:client_id")
        .ok()
        .or_else(|| provider.default_client_id.clone())
        .unwrap_or_default();
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
    let keys_json = serde_json::to_string_pretty(&oauth_keys)
        .map_err(|e| HarborError::OAuthError(format!("Failed to serialize OAuth keys: {e}")))?;
    std::fs::write(&keys_path, keys_json)
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
    let creds_json = serde_json::to_string_pretty(&creds)
        .map_err(|e| HarborError::OAuthError(format!("Failed to serialize credentials: {e}")))?;
    std::fs::write(&creds_path, creds_json)
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
    token_valid_for(provider_id, 0)
}

/// Check if a token will still be valid after `buffer_secs` seconds.
/// Used to proactively refresh before expiry.
pub fn token_valid_for(provider_id: &str, buffer_secs: i64) -> bool {
    if Vault::get(&format!("oauth:{provider_id}:access_token")).is_err() {
        return false;
    }
    // Check expiry if recorded
    if let Ok(expires_str) = Vault::get(&format!("oauth:{provider_id}:expires_at")) {
        if let Ok(expires_at) = expires_str.parse::<i64>() {
            return chrono::Utc::now().timestamp() + buffer_secs < expires_at;
        }
    }
    true // No expiry means token doesn't expire (e.g. GitHub)
}

pub fn clear_tokens(provider_id: &str) -> Result<()> {
    let _ = Vault::delete(&format!("oauth:{provider_id}:access_token"));
    let _ = Vault::delete(&format!("oauth:{provider_id}:refresh_token"));
    let _ = Vault::delete(&format!("oauth:{provider_id}:expires_at"));
    let _ = Vault::delete(&format!("oauth:{provider_id}:team_id"));
    info!(provider = provider_id, "OAuth tokens cleared");
    Ok(())
}

/// Refresh an OAuth access token using the stored refresh token.
///
/// Returns the new access token on success. If no refresh token is stored,
/// returns an error suggesting re-authentication.
pub async fn refresh_access_token(provider_id: &str) -> Result<String> {
    let provider = builtin_providers()
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| HarborError::OAuthError(format!("Unknown provider: {provider_id}")))?;

    let refresh_token =
        Vault::get(&format!("oauth:{provider_id}:refresh_token")).map_err(|_| {
            HarborError::OAuthError(format!(
                "No refresh token for {provider_id}. Re-authenticate with: harbor dock {}",
                provider_id
            ))
        })?;

    let client_id = Vault::get(&format!("oauth:{provider_id}:client_id"))
        .ok()
        .or_else(|| provider.default_client_id.clone())
        .unwrap_or_default();

    let client_secret = Vault::get(&format!("oauth:{provider_id}:client_secret"))
        .ok()
        .or_else(|| provider.default_client_secret.clone());

    let mut params: Vec<(&str, &str)> = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh_token),
        ("client_id", &client_id),
    ];

    let secret_owned;
    if let Some(ref s) = client_secret {
        secret_owned = s.clone();
        params.push(("client_secret", &secret_owned));
    }

    let http = reqwest::Client::new();
    let response = http
        .post(&provider.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| HarborError::OAuthError(format!("Token refresh request failed: {e}")))?;

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| HarborError::OAuthError(format!("Failed to parse refresh response: {e}")))?;

    // Slack error format
    if body.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        let error = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        return Err(HarborError::OAuthError(format!(
            "Token refresh failed: {error}"
        )));
    }

    if let Some(error) = body.get("error") {
        let desc = body
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        return Err(HarborError::OAuthError(format!(
            "Token refresh failed: {} {desc}",
            error.as_str().unwrap_or("unknown"),
        )));
    }

    let access_token = body["access_token"]
        .as_str()
        .ok_or_else(|| HarborError::OAuthError("No access_token in refresh response".into()))?
        .to_string();

    let expires_at = body
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .map(|secs| chrono::Utc::now().timestamp() + secs);

    // Store updated tokens
    let tokens = OAuthTokens {
        access_token: access_token.clone(),
        refresh_token: body
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or(Some(refresh_token)),
        expires_at,
        token_type: body
            .get("token_type")
            .and_then(|v| v.as_str())
            .unwrap_or("bearer")
            .to_string(),
        team_id: body
            .get("team")
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    store_tokens(provider_id, &tokens)?;
    info!(provider = provider_id, "OAuth access token refreshed");

    Ok(access_token)
}

// ---------------------------------------------------------------------------
// OAuth flow orchestration
// ---------------------------------------------------------------------------

/// Complete an OAuth flow: exchange the authorization code for tokens, store
/// client credentials and tokens in the vault. Shared between CLI and desktop.
pub async fn complete_oauth_flow(
    provider_id: &str,
    code: &str,
    callback_port: u16,
    pkce: Option<&PkceChallenge>,
) -> Result<OAuthTokens> {
    let provider = builtin_providers()
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| HarborError::OAuthError(format!("Unknown provider: {provider_id}")))?;

    let custom_client_id = Vault::get(&format!("oauth:{provider_id}:client_id")).ok();
    let custom_client_secret = Vault::get(&format!("oauth:{provider_id}:client_secret")).ok();

    let effective_client_id = custom_client_id
        .clone()
        .or_else(|| provider.default_client_id.clone())
        .ok_or_else(|| {
            HarborError::OAuthError(format!(
                "No client ID configured for {provider_id}. Set one via the vault."
            ))
        })?;
    let effective_client_secret = custom_client_secret
        .clone()
        .or_else(|| provider.default_client_secret.clone());

    let redirect = if provider.requires_https_redirect {
        HTTPS_REDIRECT_BASE.to_string()
    } else {
        format!("http://127.0.0.1:{callback_port}/callback")
    };

    let tokens = exchange_code(
        &provider,
        code,
        &redirect,
        pkce.map(|p| p.code_verifier.as_str()),
        custom_client_id.as_deref(),
        custom_client_secret.as_deref(),
    )
    .await?;

    // Only store client credentials if they were custom overrides (not compile-time defaults)
    if custom_client_id.is_some() {
        let _ = Vault::set(
            &format!("oauth:{provider_id}:client_id"),
            &effective_client_id,
        );
    }
    if custom_client_secret.is_some() {
        if let Some(ref secret) = effective_client_secret {
            let _ = Vault::set(&format!("oauth:{provider_id}:client_secret"), secret);
        }
    }

    store_tokens(provider_id, &tokens)?;

    Ok(tokens)
}

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

    // Obtain client_id: DCR providers always re-register (ephemeral port),
    // otherwise check vault override → compile-time default.
    let client_id = if let Some(ref reg_endpoint) = provider.registration_endpoint {
        dynamic_register(provider_id, reg_endpoint, &redirect_uri).await?
    } else if let Ok(id) = Vault::get(&format!("oauth:{provider_id}:client_id")) {
        id
    } else if let Some(ref default_id) = provider.default_client_id {
        default_id.clone()
    } else {
        return Err(HarborError::OAuthError(format!(
            "No client ID configured for {provider_id}. Set one via the vault."
        )));
    };

    let pkce = if provider.supports_pkce {
        Some(generate_pkce())
    } else {
        None
    };

    let mut auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code",
        provider.auth_url,
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
    );

    // Only include scope parameter when scopes are defined (some providers
    // like Stripe reject an empty scope= parameter).
    if !provider.scopes.is_empty() {
        let scope_param = if provider.uses_user_scope {
            "user_scope"
        } else {
            "scope"
        };
        let scope_str = provider.scopes.join(" ");
        auth_url.push_str(&format!(
            "&{}={}",
            scope_param,
            urlencoding::encode(&scope_str)
        ));
    }

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
