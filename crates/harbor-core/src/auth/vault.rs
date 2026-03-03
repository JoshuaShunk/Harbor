use crate::error::{HarborError, Result};
use std::collections::BTreeMap;
use tracing::{info, warn};

const SERVICE_NAME: &str = "harbor";
const KEYS_INDEX_KEY: &str = "_harbor_vault_keys";

/// Encrypted secret storage using OS keychain with env-var fallback.
///
/// Stores secrets in the OS keychain (macOS Keychain, Windows Credential Manager,
/// Linux Secret Service). A key index is maintained as a comma-separated list
/// stored under a special key so we can enumerate stored secrets.
pub struct Vault;

impl Vault {
    /// Store a secret in the vault.
    pub fn set(key: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)
            .map_err(|e| HarborError::VaultError(format!("Failed to create keyring entry: {e}")))?;

        // On macOS, set_password can fail with "already exists" if the entry
        // was created with different attributes.  Delete first, then retry.
        if let Err(e) = entry.set_password(value) {
            let _ = entry.delete_credential();
            entry.set_password(value).map_err(|e2| {
                HarborError::VaultError(format!(
                    "Failed to store secret '{key}' (even after delete): {e2} (original: {e})"
                ))
            })?;
        }

        // Update the key index
        let mut keys = Self::list_keys().unwrap_or_default();
        if !keys.contains(&key.to_string()) {
            keys.push(key.to_string());
            Self::save_key_index(&keys)?;
        }

        info!(key = key, "Secret stored in vault");
        Ok(())
    }

    /// Retrieve a secret from the vault. Falls back to OS environment variable.
    pub fn get(key: &str) -> Result<String> {
        // Try keyring first
        let entry = keyring::Entry::new(SERVICE_NAME, key)
            .map_err(|e| HarborError::VaultError(format!("Failed to create keyring entry: {e}")))?;

        match entry.get_password() {
            Ok(value) => return Ok(value),
            Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                warn!(key = key, error = %e, "Keyring lookup failed, trying env var fallback");
            }
        }

        // Fallback to OS environment variable
        std::env::var(key).map_err(|_| {
            HarborError::VaultError(format!(
                "Secret '{key}' not found in vault or environment"
            ))
        })
    }

    /// Delete a secret from the vault.
    pub fn delete(key: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, key)
            .map_err(|e| HarborError::VaultError(format!("Failed to create keyring entry: {e}")))?;

        match entry.delete_credential() {
            Ok(()) => {}
            Err(keyring::Error::NoEntry) => {
                return Err(HarborError::VaultError(format!(
                    "Secret '{key}' not found in vault"
                )));
            }
            Err(e) => {
                return Err(HarborError::VaultError(format!(
                    "Failed to delete secret '{key}': {e}"
                )));
            }
        }

        // Update key index
        let mut keys = Self::list_keys().unwrap_or_default();
        keys.retain(|k| k != key);
        Self::save_key_index(&keys)?;

        info!(key = key, "Secret deleted from vault");
        Ok(())
    }

    /// List all stored secret keys (not values).
    pub fn list_keys() -> Result<Vec<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, KEYS_INDEX_KEY)
            .map_err(|e| HarborError::VaultError(format!("Failed to read key index: {e}")))?;

        match entry.get_password() {
            Ok(csv) => {
                let keys: Vec<String> = csv
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                Ok(keys)
            }
            Err(keyring::Error::NoEntry) => Ok(Vec::new()),
            Err(e) => Err(HarborError::VaultError(format!(
                "Failed to read key index: {e}"
            ))),
        }
    }

    /// Resolve a `vault:KEY_NAME` reference to its actual value.
    pub fn resolve(reference: &str) -> Result<String> {
        if let Some(key) = reference.strip_prefix("vault:") {
            Self::get(key)
        } else {
            Ok(reference.to_string())
        }
    }

    /// Resolve all vault references in an env map.
    pub fn resolve_env(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        env.iter()
            .map(|(key, value)| {
                let resolved = if value.starts_with("vault:") {
                    Self::resolve(value).unwrap_or_else(|e| {
                        warn!(key = key, error = %e, "Failed to resolve vault reference");
                        String::new()
                    })
                } else {
                    value.clone()
                };
                (key.clone(), resolved)
            })
            .collect()
    }

    fn save_key_index(keys: &[String]) -> Result<()> {
        let csv = keys.join(",");
        let entry = keyring::Entry::new(SERVICE_NAME, KEYS_INDEX_KEY)
            .map_err(|e| HarborError::VaultError(format!("Failed to create key index entry: {e}")))?;
        entry
            .set_password(&csv)
            .map_err(|e| HarborError::VaultError(format!("Failed to save key index: {e}")))?;
        Ok(())
    }
}
