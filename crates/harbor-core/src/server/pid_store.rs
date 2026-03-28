use crate::error::{HarborError, Result};
use std::fs;
use std::path::PathBuf;

/// Manages PID files at `~/.harbor/run/<name>.pid` for detached server processes.
pub struct PidStore;

impl PidStore {
    fn run_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".harbor")
            .join("run")
    }

    fn pid_path(name: &str) -> PathBuf {
        Self::run_dir().join(format!("{}.pid", name))
    }

    pub fn write(name: &str, pid: u32) -> Result<()> {
        let dir = Self::run_dir();
        fs::create_dir_all(&dir).map_err(HarborError::Io)?;
        fs::write(Self::pid_path(name), pid.to_string()).map_err(HarborError::Io)?;
        Ok(())
    }

    pub fn read(name: &str) -> Option<u32> {
        fs::read_to_string(Self::pid_path(name))
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    pub fn remove(name: &str) {
        let _ = fs::remove_file(Self::pid_path(name));
    }

    /// Returns true if the process with the given PID is still running.
    pub fn is_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            // kill -0 sends no signal but errors if the process doesn't exist
            std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
        #[cfg(windows)]
        {
            std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid), "/NH"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                .unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_path_format() {
        let path = PidStore::pid_path("test-server");
        assert!(path.ends_with("test-server.pid"));
        assert!(path.to_string_lossy().contains(".harbor"));
        assert!(path.to_string_lossy().contains("run"));
    }

    #[test]
    fn test_run_dir_contains_harbor() {
        let dir = PidStore::run_dir();
        assert!(dir.to_string_lossy().contains(".harbor"));
        assert!(dir.to_string_lossy().contains("run"));
    }

    #[test]
    fn test_write_and_read_pid() {
        let name = format!("test-pid-{}", std::process::id());
        let test_pid = 12345u32;

        // Write
        PidStore::write(&name, test_pid).expect("Failed to write PID");

        // Read
        let read_pid = PidStore::read(&name);
        assert_eq!(read_pid, Some(test_pid));

        // Cleanup
        PidStore::remove(&name);
        assert_eq!(PidStore::read(&name), None);
    }

    #[test]
    fn test_read_nonexistent_returns_none() {
        let result = PidStore::read("nonexistent-server-xyz-123");
        assert_eq!(result, None);
    }

    #[test]
    fn test_remove_nonexistent_does_not_panic() {
        // Should not panic even if file doesn't exist
        PidStore::remove("nonexistent-server-abc-456");
    }

    #[test]
    fn test_write_overwrites_existing() {
        let name = format!("test-overwrite-{}", std::process::id());

        PidStore::write(&name, 111).unwrap();
        assert_eq!(PidStore::read(&name), Some(111));

        PidStore::write(&name, 222).unwrap();
        assert_eq!(PidStore::read(&name), Some(222));

        PidStore::remove(&name);
    }

    #[test]
    fn test_is_running_with_current_process() {
        // Current process should be running
        let current_pid = std::process::id();
        assert!(PidStore::is_running(current_pid));
    }

    #[test]
    fn test_is_running_with_invalid_pid() {
        // Very high PID unlikely to exist
        assert!(!PidStore::is_running(999999999));
    }
}
