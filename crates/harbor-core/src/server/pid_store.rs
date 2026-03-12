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
