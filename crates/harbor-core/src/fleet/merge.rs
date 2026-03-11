use crate::config::HarborConfig;
use crate::fleet::config::{FleetConfig, FleetServerDef, FLEET_SOURCE};
use crate::fleet::state::FleetState;
use std::collections::BTreeMap;

/// The outcome of merging a single server from the fleet into the local config.
#[derive(Debug, Clone)]
pub enum MergeAction {
    /// Server was not present locally and has been added.
    Added,
    /// Server was fleet-managed and its definition has been updated.
    Updated,
    /// Server was fleet-managed and is already up to date — no change made.
    Unchanged,
    /// Server exists locally with a non-fleet source; skipped to avoid overwriting.
    Conflict { reason: String },
    /// Server is fleet-managed but the user hand-edited it since the last pull.
    ///
    /// Harbor will not overwrite user changes. The user must explicitly resolve
    /// the conflict: either `harbor undock <name>` then `harbor crew pull` to
    /// accept the upstream version, or `harbor crew push <name>` to share their
    /// modified version with the team.
    LocallyModified,
}

impl MergeAction {
    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Added | Self::Updated)
    }
}

/// Aggregated result of a full fleet merge.
#[derive(Debug)]
pub struct MergeResult {
    /// One entry per server in the fleet, keyed by server name.
    pub actions: BTreeMap<String, MergeAction>,
}

impl MergeResult {
    pub fn added(&self) -> Vec<&str> {
        self.filter(|a| matches!(a, MergeAction::Added))
    }

    pub fn updated(&self) -> Vec<&str> {
        self.filter(|a| matches!(a, MergeAction::Updated))
    }

    pub fn unchanged(&self) -> Vec<&str> {
        self.filter(|a| matches!(a, MergeAction::Unchanged))
    }

    pub fn conflicts(&self) -> Vec<(&str, &str)> {
        self.actions
            .iter()
            .filter_map(|(name, action)| {
                if let MergeAction::Conflict { reason } = action {
                    Some((name.as_str(), reason.as_str()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn locally_modified(&self) -> Vec<&str> {
        self.filter(|a| matches!(a, MergeAction::LocallyModified))
    }

    pub fn has_changes(&self) -> bool {
        self.actions.values().any(|a| a.is_changed())
    }

    fn filter<F>(&self, pred: F) -> Vec<&str>
    where
        F: Fn(&MergeAction) -> bool,
    {
        self.actions
            .iter()
            .filter(|(_, a)| pred(a))
            .map(|(n, _)| n.as_str())
            .collect()
    }
}

/// Merge fleet server definitions into the local Harbor config.
///
/// ## Merge rules (per server)
///
/// | Local state | Hash state | Action |
/// |---|---|---|
/// | Not present locally | — | **Add** with `source = "fleet"` |
/// | Present, `source = "fleet"`, hash clean | fleet def changed | **Update** definition |
/// | Present, `source = "fleet"`, hash clean | fleet def unchanged | **Unchanged** |
/// | Present, `source = "fleet"`, hash dirty | any | **LocallyModified** — skip |
/// | Present, `source = "fleet"`, no hash yet | fleet def unchanged | **Unchanged** |
/// | Present, `source = "fleet"`, no hash yet | fleet def changed | **Updated** (legacy compat) |
/// | Present, different/missing source | — | **Conflict** — skip |
///
/// ## State updates
///
/// When `dry_run` is false, `state` is updated in-place for every Add or Update
/// so it reflects the new baseline for future drift detection. Callers must
/// `state.save()` after a successful pull.
pub fn merge(
    local: &mut HarborConfig,
    fleet: &FleetConfig,
    state: &mut FleetState,
    dry_run: bool,
) -> MergeResult {
    let mut actions = BTreeMap::new();

    for (name, fleet_def) in &fleet.servers {
        let action = compute_action(name, fleet_def, local, state);

        if !dry_run && action.is_changed() {
            apply_action(name, fleet_def, &action, local);
            state.record(name, fleet_def);
        }

        actions.insert(name.clone(), action);
    }

    MergeResult { actions }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn compute_action(
    name: &str,
    fleet_def: &FleetServerDef,
    local: &HarborConfig,
    state: &FleetState,
) -> MergeAction {
    match local.servers.get(name) {
        None => MergeAction::Added,

        Some(existing) if existing.source.as_deref() == Some(FLEET_SOURCE) => {
            // Reconstruct the fleet-visible fields from the current local entry.
            // If the user hand-edited command/args/env/url/headers/tool filters,
            // this will differ from what Harbor last wrote.
            let reconstructed = FleetServerDef::from_server_config(existing);

            match state.is_locally_clean(name, &reconstructed) {
                Some(false) => {
                    // Hash mismatch: user has modified this entry since last pull.
                    MergeAction::LocallyModified
                }
                Some(true) | None => {
                    // Hash matches (clean) or no hash stored (legacy/first pull):
                    // fall back to field comparison.
                    if fleet_def.is_equivalent_to(existing) {
                        MergeAction::Unchanged
                    } else {
                        MergeAction::Updated
                    }
                }
            }
        }

        Some(_) => MergeAction::Conflict {
            reason: "server exists locally with a non-fleet source".to_string(),
        },
    }
}

fn apply_action(
    name: &str,
    fleet_def: &FleetServerDef,
    action: &MergeAction,
    local: &mut HarborConfig,
) {
    let server_config = match action {
        MergeAction::Added => fleet_def.to_server_config(),
        MergeAction::Updated => {
            let existing = local.servers.get(name).expect("exists — checked above");
            fleet_def.to_server_config_preserving(existing)
        }
        _ => return,
    };

    local.servers.insert(name.to_string(), server_config);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use std::collections::BTreeMap;

    fn base_local_server(cmd: &str) -> ServerConfig {
        ServerConfig {
            source: None,
            command: Some(cmd.to_string()),
            args: vec![],
            env: BTreeMap::new(),
            url: None,
            headers: None,
            enabled: true,
            auto_start: false,
            hosts: BTreeMap::new(),
            tool_allowlist: None,
            tool_blocklist: None,
            tool_hosts: BTreeMap::new(),
        }
    }

    fn fleet_server(cmd: &str) -> ServerConfig {
        let mut s = base_local_server(cmd);
        s.source = Some(FLEET_SOURCE.to_string());
        s
    }

    fn base_fleet_def(cmd: &str) -> FleetServerDef {
        FleetServerDef {
            description: None,
            command: Some(cmd.to_string()),
            args: vec![],
            env: BTreeMap::new(),
            url: None,
            headers: None,
            tool_allowlist: None,
            tool_blocklist: None,
        }
    }

    fn empty_state() -> FleetState {
        FleetState::default()
    }

    // ── Basic merge behaviour (backward-compat, no stored hashes) ─────────────

    #[test]
    fn adds_new_server() {
        let mut local = HarborConfig::default();
        let mut fleet = FleetConfig::default();
        fleet
            .servers
            .insert("github".to_string(), base_fleet_def("npx"));

        let result = merge(&mut local, &fleet, &mut empty_state(), false);
        assert!(matches!(result.actions["github"], MergeAction::Added));
        assert!(local.servers.contains_key("github"));
        assert_eq!(local.servers["github"].source.as_deref(), Some("fleet"));
    }

    #[test]
    fn updates_fleet_managed_server_no_hash() {
        let mut local = HarborConfig::default();
        let mut existing = fleet_server("npx");
        existing.args = vec!["-y".to_string(), "old-pkg".to_string()];
        local.servers.insert("github".to_string(), existing);

        let mut fleet_def = base_fleet_def("npx");
        fleet_def.args = vec!["-y".to_string(), "new-pkg".to_string()];

        let mut fleet = FleetConfig::default();
        fleet.servers.insert("github".to_string(), fleet_def);

        let result = merge(&mut local, &fleet, &mut empty_state(), false);
        assert!(matches!(result.actions["github"], MergeAction::Updated));
        assert_eq!(
            local.servers["github"].args,
            vec!["-y".to_string(), "new-pkg".to_string()]
        );
    }

    #[test]
    fn unchanged_when_identical_no_hash() {
        let mut local = HarborConfig::default();
        local
            .servers
            .insert("github".to_string(), fleet_server("npx"));

        let mut fleet = FleetConfig::default();
        fleet
            .servers
            .insert("github".to_string(), base_fleet_def("npx"));

        let result = merge(&mut local, &fleet, &mut empty_state(), false);
        assert!(matches!(result.actions["github"], MergeAction::Unchanged));
    }

    #[test]
    fn conflict_for_non_fleet_server() {
        let mut local = HarborConfig::default();
        local
            .servers
            .insert("github".to_string(), base_local_server("my-custom-cmd"));

        let mut fleet = FleetConfig::default();
        fleet
            .servers
            .insert("github".to_string(), base_fleet_def("npx"));

        let result = merge(&mut local, &fleet, &mut empty_state(), false);
        assert!(matches!(
            result.actions["github"],
            MergeAction::Conflict { .. }
        ));
        assert_eq!(
            local.servers["github"].command.as_deref(),
            Some("my-custom-cmd")
        );
    }

    #[test]
    fn dry_run_makes_no_changes() {
        let mut local = HarborConfig::default();
        let mut fleet = FleetConfig::default();
        fleet
            .servers
            .insert("github".to_string(), base_fleet_def("npx"));

        let result = merge(&mut local, &fleet, &mut empty_state(), true);
        assert!(matches!(result.actions["github"], MergeAction::Added));
        assert!(!local.servers.contains_key("github"));
    }

    #[test]
    fn preserves_per_machine_state_on_update() {
        let mut local = HarborConfig::default();
        let mut existing = fleet_server("npx");
        existing.enabled = false;
        existing.auto_start = true;
        existing.hosts.insert("cursor".to_string(), false);
        local.servers.insert("github".to_string(), existing);

        let mut fleet_def = base_fleet_def("npx");
        fleet_def.args = vec!["--new".to_string()];
        let mut fleet = FleetConfig::default();
        fleet.servers.insert("github".to_string(), fleet_def);

        merge(&mut local, &fleet, &mut empty_state(), false);

        let updated = &local.servers["github"];
        assert_eq!(updated.enabled, false);
        assert_eq!(updated.auto_start, true);
        assert_eq!(updated.hosts.get("cursor"), Some(&false));
        assert_eq!(updated.args, vec!["--new".to_string()]);
    }

    // ── Hash-based drift detection ────────────────────────────────────────────

    #[test]
    fn locally_modified_detected_via_hash() {
        let fleet_def = base_fleet_def("npx");

        // Simulate a previous pull: state records the fleet def hash.
        let mut state = FleetState::default();
        state.record("github", &fleet_def);

        // User has since changed args locally.
        let mut local = HarborConfig::default();
        let mut modified = fleet_server("npx");
        modified.args = vec!["--user-edited".to_string()];
        local.servers.insert("github".to_string(), modified);

        // Fleet still has the original def.
        let mut fleet = FleetConfig::default();
        fleet.servers.insert("github".to_string(), fleet_def);

        let result = merge(&mut local, &fleet, &mut state, false);
        assert!(
            matches!(result.actions["github"], MergeAction::LocallyModified),
            "expected LocallyModified, got {:?}",
            result.actions["github"]
        );
        // Local server must not be overwritten.
        assert_eq!(
            local.servers["github"].args,
            vec!["--user-edited".to_string()]
        );
    }

    #[test]
    fn clean_hash_allows_upstream_update() {
        let fleet_def_v1 = base_fleet_def("npx");

        // Simulate a previous pull.
        let mut state = FleetState::default();
        state.record("github", &fleet_def_v1);

        // Local reflects exactly what was pulled (clean).
        let mut local = HarborConfig::default();
        local
            .servers
            .insert("github".to_string(), fleet_server("npx"));

        // Fleet now has a new definition.
        let mut fleet_def_v2 = base_fleet_def("npx");
        fleet_def_v2.args = vec!["-y".to_string(), "v2-pkg".to_string()];
        let mut fleet = FleetConfig::default();
        fleet
            .servers
            .insert("github".to_string(), fleet_def_v2.clone());

        let result = merge(&mut local, &fleet, &mut state, false);
        assert!(
            matches!(result.actions["github"], MergeAction::Updated),
            "expected Updated, got {:?}",
            result.actions["github"]
        );
        // Hash should now be updated to reflect v2.
        let reconstructed = FleetServerDef::from_server_config(&local.servers["github"]);
        assert_eq!(state.is_locally_clean("github", &reconstructed), Some(true));
    }

    #[test]
    fn per_machine_edits_do_not_trigger_locally_modified() {
        let fleet_def = base_fleet_def("npx");

        let mut state = FleetState::default();
        state.record("github", &fleet_def);

        // User changed only `enabled` and `auto_start` — per-machine fields
        // excluded from FleetServerDef, so hash should still match.
        let mut local = HarborConfig::default();
        let mut s = fleet_server("npx");
        s.enabled = false;
        s.auto_start = true;
        local.servers.insert("github".to_string(), s);

        let mut fleet = FleetConfig::default();
        fleet.servers.insert("github".to_string(), fleet_def);

        let result = merge(&mut local, &fleet, &mut state, false);
        // Should be Unchanged, not LocallyModified.
        assert!(
            matches!(result.actions["github"], MergeAction::Unchanged),
            "expected Unchanged (per-machine edits should be invisible to hash), got {:?}",
            result.actions["github"]
        );
    }

    #[test]
    fn state_updated_after_add() {
        let fleet_def = base_fleet_def("npx");
        let mut state = FleetState::default();
        let mut local = HarborConfig::default();

        let mut fleet = FleetConfig::default();
        fleet
            .servers
            .insert("github".to_string(), fleet_def.clone());

        merge(&mut local, &fleet, &mut state, false);

        // State should now have a hash for github.
        let reconstructed = FleetServerDef::from_server_config(&local.servers["github"]);
        assert_eq!(state.is_locally_clean("github", &reconstructed), Some(true));
    }
}
