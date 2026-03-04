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
    /// Command to run (e.g. "npx", "uvx")
    pub command: &'static str,
    /// Default arguments
    pub args: &'static [&'static str],
    /// Authentication requirement
    pub auth: AuthKind,
}

// ---------------------------------------------------------------------------
// The catalog
// ---------------------------------------------------------------------------

/// Returns the full catalog of native MCP servers.
pub fn catalog() -> Vec<NativeServer> {
    vec![
        // --- OAuth servers ---
        NativeServer {
            id: "github",
            display_name: "GitHub",
            description: "GitHub API — repos, issues, PRs, code search",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-github"],
            auth: AuthKind::OAuth("github".into()),
        },
        NativeServer {
            id: "google-drive",
            display_name: "Google Drive",
            description: "Read-only Google Drive — search and read files",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-gdrive"],
            auth: AuthKind::OAuth("google".into()),
        },
        NativeServer {
            id: "slack",
            display_name: "Slack",
            description: "Slack workspace — channels, messages, users",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-slack"],
            auth: AuthKind::OAuth("slack".into()),
        },
        // --- No-auth servers ---
        NativeServer {
            id: "filesystem",
            display_name: "Filesystem",
            description: "Local filesystem — read and write files (pass paths after --)",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-filesystem"],
            auth: AuthKind::None,
        },
        NativeServer {
            id: "fetch",
            display_name: "Fetch",
            description: "HTTP fetch — retrieve and convert web pages to markdown",
            command: "uvx",
            args: &["mcp-server-fetch"],
            auth: AuthKind::None,
        },
        NativeServer {
            id: "memory",
            display_name: "Memory",
            description: "Persistent memory via a knowledge graph",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-memory"],
            auth: AuthKind::None,
        },
        NativeServer {
            id: "puppeteer",
            display_name: "Puppeteer",
            description: "Browser automation — navigate, screenshot, interact with pages",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-puppeteer"],
            auth: AuthKind::None,
        },
        NativeServer {
            id: "sqlite",
            display_name: "SQLite",
            description:
                "SQLite database — query and manage local databases (pass db path after --)",
            command: "uvx",
            args: &["mcp-server-sqlite"],
            auth: AuthKind::None,
        },
        // --- Manual-token servers ---
        NativeServer {
            id: "brave-search",
            display_name: "Brave Search",
            description: "Web and local search via the Brave Search API",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-brave-search"],
            auth: AuthKind::ManualToken {
                env_var: "BRAVE_API_KEY",
                description: "Brave Search API key (https://brave.com/search/api/)",
            },
        },
        NativeServer {
            id: "postgres",
            display_name: "PostgreSQL",
            description:
                "PostgreSQL database — read-only query access (pass connection string after --)",
            command: "npx",
            args: &["-y", "@modelcontextprotocol/server-postgres"],
            auth: AuthKind::None,
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
        "google-drive",
        "slack",
        "filesystem",
        "fetch",
        "memory",
        "puppeteer",
        "sqlite",
        "brave-search",
        "postgres",
    ]
}

/// Build the environment variable map for a native server.
pub fn build_env(server: &NativeServer) -> Result<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    match &server.auth {
        AuthKind::None => {}
        AuthKind::OAuth(provider_id) => match provider_id.as_str() {
            "github" => {
                env.insert(
                    "GITHUB_PERSONAL_ACCESS_TOKEN".into(),
                    "vault:oauth:github:access_token".into(),
                );
            }
            "google" => {
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
        },
        AuthKind::ManualToken { env_var, .. } => {
            env.insert(
                env_var.to_string(),
                format!("vault:{}", env_var.to_lowercase()),
            );
        }
    }
    Ok(env)
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
