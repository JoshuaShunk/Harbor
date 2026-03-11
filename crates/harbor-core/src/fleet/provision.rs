use crate::auth::vault::Vault;
use crate::fleet::config::FleetConfig;
use std::collections::BTreeMap;

/// A vault key that is referenced by fleet servers but not yet provisioned
/// in the local OS keychain.
#[derive(Debug, Clone)]
pub struct MissingKey {
    /// The vault key name (the part after `vault:`).
    pub key: String,
    /// Comma-separated list of server names that reference this key.
    pub used_by: String,
}

/// Result of scanning a fleet config for unprovisioned vault references.
#[derive(Debug)]
pub struct ProvisionReport {
    pub missing: Vec<MissingKey>,
}

impl ProvisionReport {
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Scan `fleet` for all `vault:key` references and return those that are
/// absent from the local keychain.
///
/// Both `env` values and `headers` values are checked. Handles two formats:
/// - `"vault:my_key"` — entire value is a vault reference
/// - `"Bearer vault:my_key"` — vault reference embedded after a prefix
pub fn find_missing_keys(fleet: &FleetConfig) -> ProvisionReport {
    // Collect: vault_key → Vec<server_name>
    let mut refs: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (server_name, def) in &fleet.servers {
        let values = def
            .env
            .values()
            .chain(def.headers.iter().flat_map(|h| h.values()));

        for value in values {
            if let Some(key) = extract_vault_key(value) {
                refs.entry(key).or_default().push(server_name.clone());
            }
        }
    }

    let missing = refs
        .into_iter()
        .filter(|(key, _)| Vault::get(key).is_err())
        .map(|(key, servers)| MissingKey {
            key,
            used_by: servers.join(", "),
        })
        .collect();

    ProvisionReport { missing }
}

/// Extract the vault key name from a string value, if present.
///
/// Returns `Some("my_key")` for both `"vault:my_key"` and `"Bearer vault:my_key"`.
fn extract_vault_key(value: &str) -> Option<String> {
    const PREFIX: &str = "vault:";

    let idx = value.find(PREFIX)?;
    // Everything after "vault:" until the next whitespace (or end).
    let after = &value[idx + PREFIX.len()..];
    let key = after.split_whitespace().next()?;

    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::config::FleetServerDef;

    // ── extract_vault_key unit tests ──────────────────────────────────────────

    #[test]
    fn extracts_simple_vault_ref() {
        assert_eq!(
            extract_vault_key("vault:my_key"),
            Some("my_key".to_string())
        );
    }

    #[test]
    fn extracts_embedded_vault_ref() {
        assert_eq!(
            extract_vault_key("Bearer vault:my_token"),
            Some("my_token".to_string())
        );
    }

    #[test]
    fn ignores_plain_values() {
        assert_eq!(extract_vault_key("plain_api_key_value"), None);
    }

    #[test]
    fn ignores_empty_after_prefix() {
        assert_eq!(extract_vault_key("vault:"), None);
    }

    #[test]
    fn extracts_namespaced_key() {
        // Keys like "oauth:slack:access_token" after the "vault:" prefix must
        // be preserved in full (colons are valid in key names).
        assert_eq!(
            extract_vault_key("Bearer vault:oauth:slack:access_token"),
            Some("oauth:slack:access_token".to_string())
        );
    }

    // ── find_missing_keys integration tests ───────────────────────────────────

    fn def_with_env(key: &str, value: &str) -> FleetServerDef {
        let mut env = BTreeMap::new();
        env.insert(key.to_string(), value.to_string());
        FleetServerDef {
            description: None,
            command: Some("npx".to_string()),
            args: vec![],
            env,
            url: None,
            headers: None,
            tool_allowlist: None,
            tool_blocklist: None,
        }
    }

    fn def_with_header(header: &str, value: &str) -> FleetServerDef {
        let mut headers = BTreeMap::new();
        headers.insert(header.to_string(), value.to_string());
        FleetServerDef {
            description: None,
            command: None,
            args: vec![],
            env: BTreeMap::new(),
            url: Some("https://example.com/mcp".to_string()),
            headers: Some(headers),
            tool_allowlist: None,
            tool_blocklist: None,
        }
    }

    #[test]
    fn reports_missing_vault_key_in_env() {
        let mut config = FleetConfig::default();
        config.servers.insert(
            "github".to_string(),
            def_with_env("TOKEN", "vault:__test_unprovisioned_key_env__"),
        );

        let report = find_missing_keys(&config);

        assert!(!report.is_complete());
        assert!(report
            .missing
            .iter()
            .any(|mk| mk.key == "__test_unprovisioned_key_env__"));
        assert!(report
            .missing
            .iter()
            .any(|mk| mk.used_by.contains("github")));
    }

    #[test]
    fn reports_missing_vault_key_in_header() {
        let mut config = FleetConfig::default();
        config.servers.insert(
            "slack".to_string(),
            def_with_header(
                "Authorization",
                "Bearer vault:__test_unprovisioned_key_hdr__",
            ),
        );

        let report = find_missing_keys(&config);

        assert!(report
            .missing
            .iter()
            .any(|mk| mk.key == "__test_unprovisioned_key_hdr__"));
        assert!(report.missing.iter().any(|mk| mk.used_by.contains("slack")));
    }

    #[test]
    fn groups_shared_vault_key_by_all_servers() {
        let mut config = FleetConfig::default();
        config.servers.insert(
            "server-a".to_string(),
            def_with_env("T", "vault:__test_shared_key__"),
        );
        config.servers.insert(
            "server-b".to_string(),
            def_with_env("T", "vault:__test_shared_key__"),
        );

        let report = find_missing_keys(&config);

        let entry = report
            .missing
            .iter()
            .find(|mk| mk.key == "__test_shared_key__")
            .expect("shared key must appear exactly once");
        // Both servers listed in used_by.
        assert!(entry.used_by.contains("server-a"));
        assert!(entry.used_by.contains("server-b"));
        // Only one entry for the shared key (not duplicated).
        let count = report
            .missing
            .iter()
            .filter(|mk| mk.key == "__test_shared_key__")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn ignores_plain_env_values() {
        let mut config = FleetConfig::default();
        config
            .servers
            .insert("server".to_string(), def_with_env("TOKEN", "plain_value"));

        let report = find_missing_keys(&config);

        assert!(report.is_complete());
    }

    #[test]
    fn empty_fleet_is_complete() {
        let report = find_missing_keys(&FleetConfig::default());
        assert!(report.is_complete());
    }
}
