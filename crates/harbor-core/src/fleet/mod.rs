pub mod config;
pub mod git;
pub mod merge;
pub mod provision;
pub mod state;

pub use config::{FleetConfig, FleetMeta, FleetServerDef, FLEET_SOURCE};
pub use git::FleetGit;
pub use merge::{merge, MergeAction, MergeResult};
pub use provision::{find_missing_keys, MissingKey, ProvisionReport};
pub use state::FleetState;

use crate::error::{HarborError, Result};
use std::path::PathBuf;

// ─── Directory / file paths ───────────────────────────────────────────────────

/// Returns `~/.harbor/fleet/` — the directory that contains the fleet git repo.
pub fn fleet_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or(HarborError::FleetNotInitialized)?;
    Ok(home.join(".harbor").join("fleet"))
}

/// Returns `~/.harbor/fleet/harbor-fleet.toml`.
pub fn fleet_file() -> Result<PathBuf> {
    Ok(fleet_dir()?.join("harbor-fleet.toml"))
}

// ─── Initialization guard ─────────────────────────────────────────────────────

/// Returns `true` when the fleet directory exists and is a git repository.
pub fn is_initialized() -> bool {
    fleet_dir().map(|d| FleetGit::is_repo(&d)).unwrap_or(false)
}

// ─── Config I/O ───────────────────────────────────────────────────────────────

/// Load the fleet config from `~/.harbor/fleet/harbor-fleet.toml`.
///
/// Returns an empty `FleetConfig` if the file does not exist yet.
pub fn load() -> Result<FleetConfig> {
    let path = fleet_file()?;
    if !path.exists() {
        return Ok(FleetConfig::default());
    }
    let raw = std::fs::read_to_string(&path).map_err(HarborError::Io)?;
    Ok(toml::from_str(&raw)?)
}

/// Persist `fleet` to `~/.harbor/fleet/harbor-fleet.toml`.
pub fn save(fleet: &FleetConfig) -> Result<()> {
    let path = fleet_file()?;
    let content = toml::to_string_pretty(fleet)?;
    std::fs::write(path, content).map_err(HarborError::Io)?;
    Ok(())
}
