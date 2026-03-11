use crate::error::{HarborError, Result};
use crate::fleet::config::FleetServerDef;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Persisted record of what Harbor last merged from the fleet into the local config.
///
/// Stored at `~/.harbor/fleet-state.json` (machine-local, not committed to git).
///
/// ## Purpose
///
/// When the user has `source = "fleet"` on a local server, Harbor can't tell from the
/// TOML alone whether the user hand-edited the entry after the last pull, or whether
/// it's still exactly what the fleet wrote. The state file bridges this gap: it stores
/// the SHA-256 of the fleet definition that was last written to local config for each
/// server. On the next pull, Harbor reconstructs the same hash from the current local
/// entry (using only the fleet-visible fields) and compares:
///
/// - **Hash matches** → entry is untouched since last pull → safe to apply upstream changes
/// - **Hash differs** → user modified the entry → surface as a `LocallyModified` conflict
/// - **No stored hash** → first pull or pre-state legacy install → fall back to field comparison
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetState {
    /// Maps server name → SHA-256 hex of the `FleetServerDef` last merged into local config.
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
}

impl FleetState {
    /// Load the state file, returning an empty state if it doesn't exist or is malformed.
    pub fn load() -> Self {
        state_path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist the state to `~/.harbor/fleet-state.json`.
    pub fn save(&self) -> Result<()> {
        let path = state_path()?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json).map_err(HarborError::Io)
    }

    /// Record that `def` was successfully merged for `server`.
    ///
    /// Call this after writing a fleet server definition to local config.
    pub fn record(&mut self, server: &str, def: &FleetServerDef) {
        self.hashes.insert(server.to_string(), hash_def(def));
    }

    /// Remove the stored hash for `server`.
    ///
    /// Called when a server is undocked so stale state doesn't accumulate.
    pub fn forget(&mut self, server: &str) {
        self.hashes.remove(server);
    }

    /// Check whether the local entry for `server` has been modified since the last pull.
    ///
    /// `reconstructed` is built from the current local `ServerConfig` by extracting
    /// only the fleet-visible fields (via `FleetServerDef::from_server_config`).
    ///
    /// Returns:
    /// - `Some(true)`  — hashes match, local is clean (safe to update)
    /// - `Some(false)` — hashes differ, local was modified by the user
    /// - `None`        — no hash stored yet (first pull, or pre-state install)
    pub fn is_locally_clean(&self, server: &str, reconstructed: &FleetServerDef) -> Option<bool> {
        let stored = self.hashes.get(server)?;
        Some(*stored == hash_def(reconstructed))
    }
}

/// Deterministically hash a `FleetServerDef` for drift detection.
///
/// TOML serialization is deterministic here because all map fields
/// (`env`, `headers`) use `BTreeMap`, which serializes in sorted key order.
pub fn hash_def(def: &FleetServerDef) -> String {
    let serialized = toml::to_string(def).unwrap_or_default();
    let digest = Sha256::digest(serialized.as_bytes());
    format!("{digest:x}")
}

fn state_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or(HarborError::FleetNotInitialized)?;
    Ok(home.join(".harbor").join("fleet-state.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_def(cmd: &str) -> FleetServerDef {
        FleetServerDef {
            description: None,
            command: Some(cmd.to_string()),
            args: vec!["-y".to_string(), "pkg".to_string()],
            env: BTreeMap::new(),
            url: None,
            headers: None,
            tool_allowlist: None,
            tool_blocklist: None,
        }
    }

    #[test]
    fn same_def_produces_same_hash() {
        let a = hash_def(&sample_def("npx"));
        let b = hash_def(&sample_def("npx"));
        assert_eq!(a, b);
    }

    #[test]
    fn different_def_produces_different_hash() {
        let a = hash_def(&sample_def("npx"));
        let b = hash_def(&sample_def("uvx"));
        assert_ne!(a, b);
    }

    #[test]
    fn record_and_clean_check() {
        let mut state = FleetState::default();
        let def = sample_def("npx");
        state.record("github", &def);

        // Same def → locally clean
        assert_eq!(state.is_locally_clean("github", &def), Some(true));

        // Modified def → locally dirty
        let modified = sample_def("node");
        assert_eq!(state.is_locally_clean("github", &modified), Some(false));
    }

    #[test]
    fn no_stored_hash_returns_none() {
        let state = FleetState::default();
        assert_eq!(state.is_locally_clean("unknown", &sample_def("npx")), None);
    }

    #[test]
    fn forget_removes_hash() {
        let mut state = FleetState::default();
        state.record("github", &sample_def("npx"));
        state.forget("github");
        assert_eq!(state.is_locally_clean("github", &sample_def("npx")), None);
    }

    #[test]
    fn env_order_does_not_affect_hash() {
        let mut def1 = sample_def("npx");
        def1.env.insert("B".to_string(), "2".to_string());
        def1.env.insert("A".to_string(), "1".to_string());

        let mut def2 = sample_def("npx");
        def2.env.insert("A".to_string(), "1".to_string());
        def2.env.insert("B".to_string(), "2".to_string());

        // BTreeMap serializes in sorted order → same hash regardless of insertion order
        assert_eq!(hash_def(&def1), hash_def(&def2));
    }
}
