use crate::error::{HarborError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const REGISTRY_API_BASE: &str = "https://registry.modelcontextprotocol.io";

// ---------------------------------------------------------------------------
// Wire-format structs (private, match the raw API JSON shape)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawSearchResponse {
    servers: Vec<RawServerEntry>,
    metadata: RawMetadata,
}

#[derive(Deserialize)]
struct RawServerEntry {
    server: RawServer,
    #[allow(dead_code)]
    #[serde(default, rename = "_meta")]
    meta: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct RawServer {
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "websiteUrl")]
    website_url: Option<String>,
    #[serde(default)]
    repository: Option<RawRepository>,
    #[serde(default)]
    packages: Vec<RawPackage>,
}

#[derive(Deserialize)]
struct RawRepository {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct RawPackage {
    #[serde(default, rename = "registryType")]
    registry_type: Option<String>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "runtimeHint")]
    runtime_hint: Option<String>,
    #[serde(default)]
    transport: Option<RawTransport>,
    #[serde(default, rename = "environmentVariables")]
    environment_variables: Vec<RawEnvVar>,
}

#[derive(Deserialize)]
struct RawTransport {
    #[serde(default, rename = "type")]
    type_: Option<String>,
}

#[derive(Deserialize)]
struct RawEnvVar {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "isRequired")]
    is_required: bool,
    #[serde(default, rename = "isSecret")]
    is_secret: bool,
    #[serde(default)]
    default: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawMetadata {
    #[allow(dead_code)]
    #[serde(default)]
    count: u32,
    #[serde(default, rename = "nextCursor")]
    next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// Public structs (flattened, what consumers use)
// ---------------------------------------------------------------------------

/// A server from the official MCP registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryServer {
    pub name: String,
    pub title: Option<String>,
    pub description: String,
    pub version: Option<String>,
    pub website_url: Option<String>,
    pub repository_url: Option<String>,
    pub is_official: bool,
    pub package: Option<PackageInfo>,
}

/// Installation package info for an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub registry_type: String,
    pub identifier: String,
    pub version: Option<String>,
    pub runtime_hint: Option<String>,
    pub environment_variables: Vec<EnvVarSpec>,
}

/// An environment variable that an MCP server expects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarSpec {
    pub name: String,
    pub description: Option<String>,
    pub is_required: bool,
    pub is_secret: bool,
    pub default: Option<String>,
}

/// Search results from the MCP registry.
#[derive(Debug)]
pub struct SearchResponse {
    pub servers: Vec<RegistryServer>,
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the official MCP server registry.
pub struct RegistryClient {
    http: reqwest::Client,
}

impl Default for RegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Fetch a single page of servers from the registry API.
    async fn fetch_page(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<RegistryServer>, Option<String>)> {
        let mut params: Vec<(&str, String)> = vec![
            ("search", query.to_string()),
            ("limit", limit.to_string()),
            ("version", "latest".to_string()),
        ];
        if let Some(c) = cursor {
            params.push(("cursor", c.to_string()));
        }

        let response = self
            .http
            .get(format!("{REGISTRY_API_BASE}/v0.1/servers"))
            .query(&params)
            .send()
            .await
            .map_err(|e| HarborError::VaultError(format!("MCP Registry request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HarborError::VaultError(format!(
                "MCP Registry returned {status}: {body}"
            )));
        }

        let raw: RawSearchResponse = response.json().await.map_err(|e| {
            HarborError::VaultError(format!("Failed to parse MCP Registry response: {e}"))
        })?;

        let servers = raw.servers.into_iter().map(flatten_server).collect();
        Ok((servers, raw.metadata.next_cursor))
    }

    /// Search for MCP servers in the official registry.
    ///
    /// Over-fetches from the API (up to 100) so we have a pool for client-side
    /// relevance sorting, filters out noise, then trims to the requested limit.
    pub async fn search(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<SearchResponse> {
        let requested_limit = limit.unwrap_or(10) as usize;
        let fetch_limit = (requested_limit * 10).min(100);

        let (mut servers, next_cursor) = self.fetch_page(query, cursor, fetch_limit).await?;

        // Score, sort, and drop irrelevant noise.
        let q = query.to_lowercase();
        servers.sort_by(|a, b| {
            relevance_score(b, &q)
                .partial_cmp(&relevance_score(a, &q))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Drop servers that scored below the relevance floor — these matched
        // only on namespace prefix (e.g. "github" in `io.github.*`) and have
        // nothing to do with the user's actual query.
        servers.retain(|s| relevance_score(s, &q) > 0.0);
        servers.truncate(requested_limit);

        Ok(SearchResponse {
            servers,
            next_cursor,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Score a server's relevance to a lowercased query.  Higher = more relevant.
///
/// Returns a negative score for servers with no content match (slug, title, or
/// description).  The caller filters these out with `score > 0.0`.
fn relevance_score(server: &RegistryServer, query: &str) -> f64 {
    let slug = server
        .name
        .rsplit('/')
        .next()
        .unwrap_or(&server.name)
        .to_lowercase();
    let title_lower = server.title.as_deref().unwrap_or("").to_lowercase();
    let desc_lower = server.description.to_lowercase();

    // --- Content matching (slug, title, description) ---
    let mut content: f64 = 0.0;

    // Slug matching (strongest signal).
    if slug == query {
        content += 100.0;
    } else if slug.starts_with(query) || slug.ends_with(query) {
        content += 60.0;
    } else if slug
        .split(['-', '_'])
        .any(|w| w == query)
    {
        content += 50.0;
    } else if slug.contains(query) {
        content += 30.0;
    }

    // Title matching.
    if title_lower.split_whitespace().any(|w| w == query) {
        content += 20.0;
    } else if title_lower.contains(query) {
        content += 10.0;
    }

    // Description matching (weak signal, breaks ties).
    if desc_lower.contains(query) {
        content += 5.0;
    }

    // No content match → not relevant, regardless of bonuses.
    if content == 0.0 {
        return -1.0;
    }

    // --- Bonuses (only applied when there's a real content match) ---
    if server.package.is_some() {
        content += 3.0;
    }
    content -= (slug.len() as f64) * 0.1;

    content
}

fn flatten_server(entry: RawServerEntry) -> RegistryServer {
    let raw = entry.server;

    // Pick the first stdio-compatible package.
    let package = raw
        .packages
        .into_iter()
        .find(|p| {
            p.transport
                .as_ref()
                .and_then(|t| t.type_.as_deref())
                .map(|t| t == "stdio")
                .unwrap_or(false)
        })
        .and_then(|p| {
            let registry_type = p.registry_type?;
            let identifier = p.identifier?;
            Some(PackageInfo {
                registry_type,
                identifier,
                version: p.version,
                runtime_hint: p.runtime_hint,
                environment_variables: p
                    .environment_variables
                    .into_iter()
                    .map(|e| EnvVarSpec {
                        name: e.name,
                        description: e.description,
                        is_required: e.is_required,
                        is_secret: e.is_secret,
                        default: e.default.and_then(|v| match v {
                            serde_json::Value::String(s) => Some(s),
                            serde_json::Value::Null => None,
                            other => Some(other.to_string()),
                        }),
                    })
                    .collect(),
            })
        });

    // The _meta `io.modelcontextprotocol.registry/official` key with status
    // "active" is present on ALL servers — it just means the listing is active,
    // not that the server is published by the MCP org. We can't reliably
    // determine "official" from the API, so we leave it false.
    let is_official = false;

    RegistryServer {
        name: raw.name,
        title: raw.title,
        description: raw.description.unwrap_or_default(),
        version: raw.version,
        website_url: raw.website_url,
        repository_url: raw.repository.and_then(|r| r.url),
        is_official,
        package,
    }
}
