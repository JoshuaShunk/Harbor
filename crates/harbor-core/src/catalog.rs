use crate::auth::oauth;
use crate::auth::vault::Vault;
use crate::error::{HarborError, Result};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Auth requirement for a native server
// ---------------------------------------------------------------------------

/// How a native server authenticates.
#[derive(Debug, Clone)]
pub enum AuthKind {
    /// No auth needed — server works out of the box.
    None,
    /// Standard OAuth flow — maps to a built-in `OAuthProvider`.
    OAuth(String),
    /// Server needs a user-supplied token/key stored in the vault.
    ManualToken {
        env_var: &'static str,
        description: &'static str,
    },
}

// ---------------------------------------------------------------------------
// Extra arguments hint for the UI
// ---------------------------------------------------------------------------

/// Describes additional arguments a native server accepts (appended after defaults).
#[derive(Debug, Clone)]
pub enum ExtraArgs {
    /// No extra args needed.
    None,
    /// One or more directory paths (e.g. filesystem allowed dirs).
    Directories { label: &'static str },
    /// A single file path (e.g. sqlite database).
    FilePath {
        label: &'static str,
        extensions: &'static [&'static str],
    },
    /// Free-form text (e.g. postgres connection string).
    TextInput {
        label: &'static str,
        placeholder: &'static str,
    },
}

// ---------------------------------------------------------------------------
// Native server definition
// ---------------------------------------------------------------------------

/// A curated, first-party MCP server that ships with Harbor.
#[derive(Debug, Clone)]
pub struct NativeServer {
    /// Short id used on CLI: `harbor dock github`
    pub id: &'static str,
    /// Human-friendly display name
    pub display_name: &'static str,
    /// Brief description
    pub description: &'static str,
    /// Command to run (e.g. "npx", "uvx") — `None` for remote servers
    pub command: Option<&'static str>,
    /// Default arguments
    pub args: &'static [&'static str],
    /// Remote HTTP endpoint — `None` for stdio servers
    pub url: Option<&'static str>,
    /// Default HTTP headers for remote servers (e.g. content-type)
    pub default_headers: &'static [(&'static str, &'static str)],
    /// Authentication requirement
    pub auth: AuthKind,
    /// Extra arguments the UI should prompt for
    pub extra_args: ExtraArgs,
}

impl NativeServer {
    /// Whether this is a remote HTTP server (as opposed to a local stdio server).
    pub fn is_remote(&self) -> bool {
        self.url.is_some()
    }
}

// ---------------------------------------------------------------------------
// The catalog
// ---------------------------------------------------------------------------

/// Returns the full catalog of native MCP servers.
pub fn catalog() -> Vec<NativeServer> {
    vec![
        // --- Remote servers (first-party vendor) ---
        NativeServer {
            id: "github",
            display_name: "GitHub",
            description: "GitHub API — repos, issues, PRs, code search",
            command: None,
            args: &[],
            url: Some("https://api.githubcopilot.com/mcp/"),
            default_headers: &[],
            auth: AuthKind::ManualToken {
                env_var: "GITHUB_PERSONAL_ACCESS_TOKEN",
                description: "GitHub Personal Access Token (https://github.com/settings/tokens)",
            },
            extra_args: ExtraArgs::None,
        },
        // --- OAuth servers ---
        NativeServer {
            id: "atlassian",
            display_name: "Atlassian",
            description: "Jira, Confluence & Compass — issues, pages, search",
            command: None,
            args: &[],
            url: Some("https://mcp.atlassian.com/v1/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("atlassian".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "slack",
            display_name: "Slack",
            description: "Slack workspace — channels, messages, users",
            command: None,
            args: &[],
            url: Some("https://mcp.slack.com/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("slack".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "linear",
            display_name: "Linear",
            description: "Linear — issues, projects, cycles, comments",
            command: None,
            args: &[],
            url: Some("https://mcp.linear.app/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("linear".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "notion",
            display_name: "Notion",
            description: "Notion — pages, databases, docs, tasks",
            command: None,
            args: &[],
            url: Some("https://mcp.notion.com/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("notion".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "sentry",
            display_name: "Sentry",
            description: "Sentry — errors, issues, performance monitoring",
            command: None,
            args: &[],
            url: Some("https://mcp.sentry.dev/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("sentry".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "figma",
            display_name: "Figma",
            description: "Figma — design inspection, Dev Mode, components",
            command: None,
            args: &[],
            url: Some("https://mcp.figma.com/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("figma".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "stripe",
            display_name: "Stripe",
            description: "Stripe — payments, customers, subscriptions, webhooks",
            command: None,
            args: &[],
            url: Some("https://mcp.stripe.com"),
            default_headers: &[],
            auth: AuthKind::OAuth("stripe".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "vercel",
            display_name: "Vercel",
            description: "Vercel — deployments, projects, domains, logs",
            command: None,
            args: &[],
            url: Some("https://mcp.vercel.com"),
            default_headers: &[],
            auth: AuthKind::OAuth("vercel".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "supabase",
            display_name: "Supabase",
            description: "Supabase — database, auth, storage, edge functions",
            command: None,
            args: &[],
            url: Some("https://mcp.supabase.com/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("supabase".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "cloudflare",
            display_name: "Cloudflare",
            description: "Cloudflare — Workers, D1, R2, KV bindings",
            command: None,
            args: &[],
            url: Some("https://bindings.mcp.cloudflare.com/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("cloudflare".into()),
            extra_args: ExtraArgs::None,
        },
        NativeServer {
            id: "neon",
            display_name: "Neon",
            description: "Neon — serverless Postgres databases, branches, SQL",
            command: None,
            args: &[],
            url: Some("https://mcp.neon.tech/mcp"),
            default_headers: &[],
            auth: AuthKind::OAuth("neon".into()),
            extra_args: ExtraArgs::None,
        },
        // --- No-auth servers ---
        NativeServer {
            id: "filesystem",
            display_name: "Filesystem",
            description: "Local filesystem — read and write files",
            command: Some("npx"),
            args: &["-y", "@modelcontextprotocol/server-filesystem"],
            url: None,
            default_headers: &[],
            auth: AuthKind::None,
            extra_args: ExtraArgs::Directories {
                label: "Allowed directories",
            },
        },
        NativeServer {
            id: "playwright",
            display_name: "Playwright",
            description: "Browser automation — navigate, screenshot, interact with pages",
            command: Some("npx"),
            args: &["-y", "@playwright/mcp@latest"],
            url: None,
            default_headers: &[],
            auth: AuthKind::None,
            extra_args: ExtraArgs::None,
        },
        // --- Google Workspace (OAuth via Harbor, token passed to gws) ---
        NativeServer {
            id: "google-workspace",
            display_name: "Google Workspace",
            description: "Google Workspace — Drive, Gmail, Calendar, Sheets, Docs, Chat, and more",
            command: Some("npx"),
            args: &["-y", "@googleworkspace/cli", "mcp", "-s"],
            url: None,
            default_headers: &[],
            auth: AuthKind::OAuth("google".into()),
            extra_args: ExtraArgs::TextInput {
                label: "Services",
                placeholder: "drive,gmail,calendar,sheets (or 'all')",
            },
        },
        // --- Manual-token servers ---
        NativeServer {
            id: "brave-search",
            display_name: "Brave Search",
            description: "Web and local search via the Brave Search API",
            command: Some("npx"),
            args: &["-y", "@brave/brave-search-mcp-server"],
            url: None,
            default_headers: &[],
            auth: AuthKind::ManualToken {
                env_var: "BRAVE_API_KEY",
                description: "Brave Search API key (https://brave.com/search/api/)",
            },
            extra_args: ExtraArgs::None,
        },
    ]
}

/// Look up a native server by its short id.
pub fn lookup(id: &str) -> Option<NativeServer> {
    catalog().into_iter().find(|s| s.id == id)
}

/// List all native server ids (for help text and error messages).
pub fn all_ids() -> Vec<&'static str> {
    // Return from a static-like list to avoid re-allocating NativeServer each time
    vec![
        "github",
        "atlassian",
        "slack",
        "linear",
        "notion",
        "sentry",
        "figma",
        "stripe",
        "vercel",
        "supabase",
        "cloudflare",
        "neon",
        "filesystem",
        "playwright",
        "google-workspace",
        "brave-search",
    ]
}

/// Build the environment variable map for a native server.
pub fn build_env(server: &NativeServer) -> Result<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    match &server.auth {
        AuthKind::None => {}
        AuthKind::OAuth(provider_id) => {
            // Remote OAuth servers use Authorization header, not env vars
            if !server.is_remote() {
                match provider_id.as_str() {
                    "google" => {
                        // Google Workspace CLI reads token from this env var
                        // (highest priority, bypasses gws's own auth)
                        if server.id == "google-workspace" {
                            env.insert(
                                "GOOGLE_WORKSPACE_CLI_TOKEN".into(),
                                "vault:oauth:google:access_token".into(),
                            );
                        } else {
                            // Legacy: gdrive MCP server uses credential files
                            let (oauth_path, creds_path) =
                                oauth::gdrive_credential_paths().ok_or_else(|| {
                                    HarborError::OAuthError(
                                        "Google Drive credentials not written yet. Complete OAuth first."
                                            .into(),
                                    )
                                })?;
                            env.insert("GDRIVE_OAUTH_PATH".into(), oauth_path);
                            env.insert("GDRIVE_CREDENTIALS_PATH".into(), creds_path);
                        }
                    }
                    "slack" => {
                        env.insert(
                            "SLACK_BOT_TOKEN".into(),
                            "vault:oauth:slack:access_token".into(),
                        );
                        env.insert("SLACK_TEAM_ID".into(), "vault:oauth:slack:team_id".into());
                    }
                    other => {
                        env.insert(
                            format!("{}_TOKEN", other.to_uppercase()),
                            format!("vault:oauth:{other}:access_token"),
                        );
                    }
                }
            }
        }
        AuthKind::ManualToken { env_var, .. } => {
            // For remote servers, auth goes in headers, not env vars
            if !server.is_remote() {
                env.insert(
                    env_var.to_string(),
                    format!("vault:{}", env_var.to_lowercase()),
                );
            }
        }
    }
    Ok(env)
}

/// Build the HTTP headers map for a remote native server.
pub fn build_headers(server: &NativeServer) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();

    // Add default headers from the catalog definition
    for (k, v) in server.default_headers {
        headers.insert(k.to_string(), v.to_string());
    }

    // Add auth headers for remote servers
    if server.is_remote() {
        match &server.auth {
            AuthKind::ManualToken { env_var, .. } => {
                headers.insert(
                    "Authorization".into(),
                    format!("Bearer vault:{}", env_var.to_lowercase()),
                );
            }
            AuthKind::OAuth(provider_id) => {
                headers.insert(
                    "Authorization".into(),
                    format!("Bearer vault:oauth:{provider_id}:access_token"),
                );
            }
            AuthKind::None => {}
        }
    }

    headers
}

/// Check whether a native server's auth requirement is already satisfied.
pub fn has_auth(server: &NativeServer) -> bool {
    match &server.auth {
        AuthKind::None => true,
        AuthKind::OAuth(provider_id) => oauth::has_valid_token(provider_id),
        AuthKind::ManualToken { env_var, .. } => {
            Vault::get(&env_var.to_lowercase()).is_ok() || std::env::var(env_var).is_ok()
        }
    }
}
