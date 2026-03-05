use crate::config::HarborConfig;
use crate::HarborError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const GITHUB_API_URL: &str = "https://api.github.com/repos/JoshuaShunk/Harbor/releases/latest";
const CACHE_FILE: &str = "update-cache.json";
const CACHE_TTL_SECONDS: i64 = 86400; // 24 hours

// --- GitHub API types ---

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

// --- Public types ---

#[derive(Debug, Clone)]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub checksum_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCache {
    pub checked_at: i64,
    pub latest_version: String,
    pub update_available: bool,
}

// --- Platform detection (compile-time) ---

pub fn current_target() -> &'static str {
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "linux"),
    )))]
    {
        "unsupported"
    }
}

// --- Version comparison ---

fn version_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u64, u64, u64) {
        let parts: Vec<u64> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };
    parse(latest) > parse(current)
}

// --- Core functions ---

/// Check GitHub releases for a newer version.
pub async fn check_for_update() -> crate::Result<UpdateCheck> {
    let client = reqwest::Client::new();
    let release: GitHubRelease = client
        .get(GITHUB_API_URL)
        .header(
            "User-Agent",
            format!("harbor-cli/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|e| HarborError::ConnectorError {
            host: "github".into(),
            reason: format!("Failed to check for updates: {e}"),
        })?
        .json()
        .await
        .map_err(|e| HarborError::ConnectorError {
            host: "github".into(),
            reason: format!("Failed to parse release response: {e}"),
        })?;

    let latest = release.tag_name.trim_start_matches('v').to_string();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let update_available = version_newer(&latest, &current);

    let target = current_target();
    let tarball_name = format!("harbor-cli-{target}.tar.gz");
    let checksum_name = format!("{tarball_name}.sha256");

    let download_url = release
        .assets
        .iter()
        .find(|a| a.name == tarball_name)
        .map(|a| a.browser_download_url.clone());

    let checksum_url = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .map(|a| a.browser_download_url.clone());

    Ok(UpdateCheck {
        current_version: current,
        latest_version: latest,
        update_available,
        download_url,
        checksum_url,
    })
}

/// Download the update tarball and verify its SHA256 checksum.
pub async fn download_and_verify(update: &UpdateCheck) -> crate::Result<PathBuf> {
    let download_url = update
        .download_url
        .as_ref()
        .ok_or_else(|| HarborError::ConnectorError {
            host: "github".into(),
            reason: format!("No CLI binary available for target: {}", current_target()),
        })?;

    let client = reqwest::Client::new();

    let harbor_dir = HarborConfig::default_dir()?;
    let tmp_dir = harbor_dir.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;

    let tarball_path = tmp_dir.join("harbor-update.tar.gz");

    let response = client
        .get(download_url)
        .header(
            "User-Agent",
            format!("harbor-cli/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|e| HarborError::ConnectorError {
            host: "github".into(),
            reason: format!("Download failed: {e}"),
        })?;

    let bytes = response.bytes().await.map_err(|e| {
        HarborError::Io(std::io::Error::other(format!(
            "Failed to read download: {e}"
        )))
    })?;

    std::fs::write(&tarball_path, &bytes)?;

    // Verify SHA256 if checksum URL is available
    if let Some(ref checksum_url) = update.checksum_url {
        let checksum_resp = client
            .get(checksum_url)
            .header(
                "User-Agent",
                format!("harbor-cli/{}", env!("CARGO_PKG_VERSION")),
            )
            .send()
            .await
            .map_err(|e| HarborError::ConnectorError {
                host: "github".into(),
                reason: format!("Checksum download failed: {e}"),
            })?;

        let checksum_text =
            checksum_resp
                .text()
                .await
                .map_err(|e| HarborError::ConnectorError {
                    host: "github".into(),
                    reason: format!("Failed to read checksum: {e}"),
                })?;

        let expected_hash =
            checksum_text
                .split_whitespace()
                .next()
                .ok_or_else(|| HarborError::ConnectorError {
                    host: "github".into(),
                    reason: "Invalid checksum file format".into(),
                })?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual_hash = format!("{:x}", hasher.finalize());

        if actual_hash != expected_hash {
            let _ = std::fs::remove_file(&tarball_path);
            return Err(HarborError::ConnectorError {
                host: "github".into(),
                reason: format!(
                    "Checksum mismatch! Expected {expected_hash}, got {actual_hash}. Download may be corrupted."
                ),
            });
        }
    }

    Ok(tarball_path)
}

/// Extract the binary from the tarball and replace the target binary.
pub fn extract_and_replace(tarball_path: &Path, target_binary: &Path) -> crate::Result<()> {
    let tmp_dir = tarball_path
        .parent()
        .expect("tarball should have a parent directory");
    let extracted_binary = tmp_dir.join("harbor");

    let status = std::process::Command::new("tar")
        .args([
            "xzf",
            &tarball_path.display().to_string(),
            "-C",
            &tmp_dir.display().to_string(),
        ])
        .status()?;

    if !status.success() {
        return Err(HarborError::ConnectorError {
            host: "github".into(),
            reason: "Failed to extract update tarball".into(),
        });
    }

    if !extracted_binary.exists() {
        return Err(HarborError::ConnectorError {
            host: "github".into(),
            reason: "Extracted tarball does not contain 'harbor' binary".into(),
        });
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&extracted_binary, std::fs::Permissions::from_mode(0o755))?;
    }

    // Resolve symlinks to get the real path
    let target_binary = if target_binary.is_symlink() {
        std::fs::canonicalize(target_binary)?
    } else {
        target_binary.to_path_buf()
    };

    // Move current binary out of the way first (macOS kills processes that
    // overwrite their own running binary, but allows renaming it).
    let backup_path = target_binary.with_extension("bak");
    if target_binary.exists() {
        std::fs::rename(&target_binary, &backup_path).map_err(|e| HarborError::ConnectorError {
            host: "update".into(),
            reason: format!("Failed to move current binary aside: {e}"),
        })?;
    }

    // Place new binary at the original path
    match std::fs::rename(&extracted_binary, &target_binary) {
        Ok(()) => {}
        Err(_) => {
            // Cross-device: fall back to copy (safe now since original is moved)
            std::fs::copy(&extracted_binary, &target_binary).map_err(|e| {
                // Restore backup on failure
                let _ = std::fs::rename(&backup_path, &target_binary);
                HarborError::Io(e)
            })?;
            let _ = std::fs::remove_file(&extracted_binary);
        }
    }

    // Clean up
    let _ = std::fs::remove_file(&backup_path);
    let _ = std::fs::remove_file(tarball_path);

    Ok(())
}

// --- Binary location ---

/// Find the harbor binary in PATH.
pub fn which_harbor() -> Option<PathBuf> {
    std::process::Command::new("which")
        .arg("harbor")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| PathBuf::from(s.trim()))
            } else {
                None
            }
        })
}

// --- Cache ---

fn cache_path() -> crate::Result<PathBuf> {
    Ok(HarborConfig::default_dir()?.join(CACHE_FILE))
}

/// Read the cached version check. Returns None if missing or expired.
pub fn read_cache() -> Option<UpdateCache> {
    let path = cache_path().ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    let cache: UpdateCache = serde_json::from_str(&content).ok()?;

    let now = chrono::Utc::now().timestamp();
    if now - cache.checked_at > CACHE_TTL_SECONDS {
        return None;
    }

    Some(cache)
}

/// Write a version check result to the cache.
pub fn write_cache(update: &UpdateCheck) -> crate::Result<()> {
    let cache = UpdateCache {
        checked_at: chrono::Utc::now().timestamp(),
        latest_version: update.latest_version.clone(),
        update_available: update.update_available,
    };
    let path = cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string(&cache)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Clear the cache (called after a successful update).
pub fn clear_cache() -> crate::Result<()> {
    let path = cache_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_newer() {
        assert!(version_newer("1.0.0", "0.9.0"));
        assert!(version_newer("0.4.0", "0.3.2"));
        assert!(version_newer("0.3.3", "0.3.2"));
        assert!(!version_newer("0.3.2", "0.3.2"));
        assert!(!version_newer("0.3.1", "0.3.2"));
        assert!(version_newer("1.0.0", "0.99.99"));
    }

    #[test]
    fn test_current_target_not_unsupported() {
        // On CI/dev machines this should resolve to a real target
        let target = current_target();
        assert_ne!(target, "unsupported");
    }
}
