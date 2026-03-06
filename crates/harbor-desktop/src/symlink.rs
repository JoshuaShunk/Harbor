use std::path::PathBuf;
use tauri::Manager;

const SYMLINK_PATH: &str = "/usr/local/bin/harbor";

/// Find the bundled CLI binary inside the .app bundle.
fn bundled_cli_path(handle: &tauri::AppHandle) -> Option<PathBuf> {
    let resource_dir = handle.path().resource_dir().ok()?;
    let cli_path = resource_dir.join("harbor");
    if cli_path.exists() {
        Some(cli_path)
    } else {
        None
    }
}

/// Install or update the `/usr/local/bin/harbor` symlink to point at the
/// bundled CLI binary inside the .app bundle.
///
/// - Skips if the symlink already points to the correct target.
/// - Silently does nothing if `/usr/local/bin` doesn't exist or isn't writable
///   (user hasn't granted permissions — no need to nag).
pub fn install_cli_symlink(handle: &tauri::AppHandle) {
    let Some(cli_path) = bundled_cli_path(handle) else {
        tracing::debug!("No bundled CLI binary found, skipping symlink");
        return;
    };

    let symlink_path = PathBuf::from(SYMLINK_PATH);

    // Check if symlink already points to the right place
    if let Ok(existing_target) = std::fs::read_link(&symlink_path) {
        if existing_target == cli_path {
            tracing::debug!("CLI symlink already up to date");
            return;
        }
        // Points somewhere else — remove and re-create
        if std::fs::remove_file(&symlink_path).is_err() {
            tracing::debug!("Could not remove existing symlink, skipping");
            return;
        }
    }

    // If a regular file (not symlink) exists at the path, don't overwrite it.
    // The user may have installed the standalone CLI separately.
    if symlink_path.exists() && !symlink_path.is_symlink() {
        tracing::debug!("Regular file exists at {SYMLINK_PATH}, skipping symlink to avoid overwriting standalone CLI");
        return;
    }

    match std::os::unix::fs::symlink(&cli_path, &symlink_path) {
        Ok(()) => tracing::info!(
            target = %cli_path.display(),
            "Installed CLI symlink at {SYMLINK_PATH}"
        ),
        Err(e) => tracing::debug!(
            error = %e,
            "Could not create CLI symlink at {SYMLINK_PATH} (this is normal without write access)"
        ),
    }
}
