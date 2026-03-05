use super::registry::{EnvVarSpec, PackageArgSpec, RegistryClient};
use crate::error::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Config schema fetched from the MCP Registry for a server.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigSchema {
    pub package_arguments: Vec<PackageArgSpec>,
    pub environment_variables: Vec<EnvVarSpec>,
    pub registry_name: Option<String>,
}

struct CacheEntry {
    schema: Option<ConfigSchema>,
    fetched_at: Instant,
}

static CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const CACHE_TTL: Duration = Duration::from_secs(600); // 10 minutes

/// Look up config schema for a package identifier, returning cached result if fresh.
pub async fn lookup_config_schema(lookup_key: &str) -> Result<Option<ConfigSchema>> {
    // Check cache
    {
        let cache = CACHE.lock().await;
        if let Some(entry) = cache.get(lookup_key) {
            if entry.fetched_at.elapsed() < CACHE_TTL {
                return Ok(entry.schema.clone());
            }
        }
    }

    // Fetch from registry
    let client = RegistryClient::new();
    let result = client.lookup_by_identifier(lookup_key).await?;

    let schema = result.and_then(|server| {
        server.package.map(|pkg| ConfigSchema {
            package_arguments: pkg.package_arguments,
            environment_variables: pkg.environment_variables,
            registry_name: Some(server.name),
        })
    });

    // Update cache
    {
        let mut cache = CACHE.lock().await;
        cache.insert(
            lookup_key.to_string(),
            CacheEntry {
                schema: schema.clone(),
                fetched_at: Instant::now(),
            },
        );
    }

    Ok(schema)
}
