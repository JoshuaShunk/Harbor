use crate::error::{HarborError, Result};
use std::collections::BTreeMap;
use tracing::{info, warn};

const SERVICE_NAME: &str = "harbor";
/// Single keychain entry holding all secrets as a JSON object.
const STORE_KEY: &str = "_harbor_vault_store";

/// Encrypted secret storage using OS keychain with env-var fallback.
///
/// All secrets are stored in a **single** keychain entry as a JSON blob
/// (`{"key": "value", ...}`). This means only one OS keychain prompt
/// is required after an app update changes the binary signature.
pub struct Vault;

impl Vault {
    // ── Core CRUD ──────────────────────────────────────────────

    /// Store a secret in the vault.
    pub fn set(key: &str, value: &str) -> Result<()> {
        let mut store = Self::read_store()?;
        store.insert(key.to_string(), value.to_string());
        Self::write_store(&store)?;
        info!(key = key, "Secret stored in vault");
        Ok(())
    }

    /// Retrieve a secret from the vault. Falls back to OS environment variable.
    pub fn get(key: &str) -> Result<String> {
        let mut store = Self::read_store()?;

        // Fast path: key is in the consolidated store
        if let Some(value) = store.get(key) {
            return Ok(value.clone());
        }

        // Lazy migration: try legacy per-key keychain entry
        if let Some(value) = Self::try_legacy_get(key) {
            store.insert(key.to_string(), value.clone());
            let _ = Self::write_store(&store); // best-effort migrate
            let _ = Self::try_legacy_delete(key); // clean up old entry
            info!(
                key = key,
                "Migrated legacy vault entry to consolidated store"
            );
            return Ok(value);
        }

        // Fallback to OS environment variable
        std::env::var(key).map_err(|_| {
            HarborError::VaultError(format!("Secret '{key}' not found in vault or environment"))
        })
    }

    /// Delete a secret from the vault.
    pub fn delete(key: &str) -> Result<()> {
        let mut store = Self::read_store()?;
        if store.remove(key).is_none() {
            return Err(HarborError::VaultError(format!(
                "Secret '{key}' not found in vault"
            )));
        }
        Self::write_store(&store)?;
        info!(key = key, "Secret deleted from vault");
        Ok(())
    }

    /// List all stored secret keys (not values).
    pub fn list_keys() -> Result<Vec<String>> {
        let store = Self::read_store()?;
        Ok(store.keys().cloned().collect())
    }

    // ── Resolution helpers ─────────────────────────────────────

    /// Resolve a `vault:KEY_NAME` reference to its actual value.
    pub fn resolve(reference: &str) -> Result<String> {
        if let Some(key) = reference.strip_prefix("vault:") {
            Self::get(key)
        } else {
            Ok(reference.to_string())
        }
    }

    /// Resolve all vault references in an env/header map.
    ///
    /// Handles three patterns:
    /// - `vault:key_name` — entire value is a vault reference
    /// - `Bearer vault:key_name` — vault reference embedded after a prefix
    /// - `plain_value` — returned as-is
    pub fn resolve_env(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        env.iter()
            .map(|(key, value)| {
                let resolved = if value.starts_with("vault:") {
                    Self::resolve(value).unwrap_or_else(|e| {
                        warn!(key = key, error = %e, "Failed to resolve vault reference");
                        String::new()
                    })
                } else if let Some(pos) = value.find("vault:") {
                    // Embedded vault reference (e.g. "Bearer vault:oauth:slack:access_token")
                    let prefix = &value[..pos];
                    let vault_ref = &value[pos..];
                    match Self::resolve(vault_ref) {
                        Ok(resolved_val) => format!("{prefix}{resolved_val}"),
                        Err(e) => {
                            warn!(key = key, error = %e, "Failed to resolve embedded vault reference");
                            String::new()
                        }
                    }
                } else {
                    value.clone()
                };
                (key.clone(), resolved)
            })
            .collect()
    }

    // ── Internal: consolidated store ───────────────────────────

    /// Read the entire secret store from the single keychain entry.
    fn read_store() -> Result<BTreeMap<String, String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, STORE_KEY)
            .map_err(|e| HarborError::VaultError(format!("Failed to create keyring entry: {e}")))?;

        match entry.get_password() {
            Ok(json) => {
                let store: BTreeMap<String, String> =
                    serde_json::from_str(&json).unwrap_or_default();
                Ok(store)
            }
            Err(keyring::Error::NoEntry) => Ok(BTreeMap::new()),
            Err(e) => {
                warn!(error = %e, "Failed to read vault store from keychain");
                Ok(BTreeMap::new())
            }
        }
    }

    /// Write the entire secret store back to the single keychain entry.
    fn write_store(store: &BTreeMap<String, String>) -> Result<()> {
        let json = serde_json::to_string(store)
            .map_err(|e| HarborError::VaultError(format!("Failed to serialize vault: {e}")))?;

        let entry = keyring::Entry::new(SERVICE_NAME, STORE_KEY)
            .map_err(|e| HarborError::VaultError(format!("Failed to create keyring entry: {e}")))?;

        // On macOS, set_password can fail with "already exists" if the entry
        // was created with different attributes.  Delete first, then retry.
        if let Err(e) = entry.set_password(&json) {
            let _ = entry.delete_credential();
            entry.set_password(&json).map_err(|e2| {
                HarborError::VaultError(format!(
                    "Failed to write vault store (even after delete): {e2} (original: {e})"
                ))
            })?;
        }

        Ok(())
    }

    // ── Internal: legacy migration helpers ─────────────────────

    /// Try to read a key from the old per-key keychain format.
    fn try_legacy_get(key: &str) -> Option<String> {
        let entry = keyring::Entry::new(SERVICE_NAME, key).ok()?;
        entry.get_password().ok()
    }

    /// Try to delete a key from the old per-key keychain format.
    fn try_legacy_delete(key: &str) -> Option<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key).ok()?;
        entry.delete_credential().ok()
    }
}
