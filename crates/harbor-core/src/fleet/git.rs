use crate::error::{HarborError, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A handle to the git repository that backs the fleet configuration.
///
/// All git operations run `git` as a subprocess inside the fleet directory
/// (`~/.harbor/fleet/`). This avoids a `libgit2` dependency and leverages
/// whatever credentials (SSH keys, credential helpers) the user already has
/// configured.
pub struct FleetGit {
    pub dir: PathBuf,
}

impl FleetGit {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    // -------------------------------------------------------------------------
    // Constructors
    // -------------------------------------------------------------------------

    /// Initialize a new git repository at `dir`.
    pub fn init(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).map_err(HarborError::Io)?;

        // Try `--initial-branch=main` (git ≥ 2.28); fall back to plain init.
        let ok = Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !ok {
            let out = run_git_in(dir, &["init"])?;
            require_success(out, "git init")?;
        }

        Ok(Self::new(dir.to_path_buf()))
    }

    /// Clone `remote_url` into `dir`.
    pub fn clone_from(remote_url: &str, dir: &Path) -> Result<Self> {
        let parent = dir.parent().ok_or_else(|| {
            HarborError::FleetGitError("fleet directory has no parent path".to_string())
        })?;
        std::fs::create_dir_all(parent).map_err(HarborError::Io)?;

        let dir_str = dir.to_string_lossy();
        let out = run_git_in(parent, &["clone", remote_url, &dir_str])?;
        require_success(out, "git clone")?;

        Ok(Self::new(dir.to_path_buf()))
    }

    // -------------------------------------------------------------------------
    // Remote management
    // -------------------------------------------------------------------------

    /// Set the `origin` remote (replaces any existing one).
    pub fn set_remote(&self, url: &str) -> Result<()> {
        // Remove silently — it's fine if there was no origin.
        let _ = self.run(&["remote", "remove", "origin"]);
        let out = self.run(&["remote", "add", "origin", url])?;
        require_success(out, "git remote add")?;
        Ok(())
    }

    /// Returns the `origin` remote URL, or `None` if unset.
    pub fn remote_url(&self) -> Option<String> {
        self.run_ok(&["remote", "get-url", "origin"])
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// Returns `true` if an `origin` remote is configured.
    pub fn has_remote(&self) -> bool {
        self.remote_url().is_some()
    }

    // -------------------------------------------------------------------------
    // Sync operations
    // -------------------------------------------------------------------------

    /// Pull latest changes from `origin` (fast-forward only).
    pub fn pull(&self) -> Result<()> {
        let out = self.run(&["pull", "--ff-only"])?;
        require_success(out, "git pull")?;
        Ok(())
    }

    /// Stage `harbor-fleet.toml`, commit with `message`, and push to `origin`.
    ///
    /// Returns `true` if a commit was made, `false` if there was nothing new.
    pub fn commit_and_push(&self, message: &str) -> Result<bool> {
        let out = self.run(&["add", "harbor-fleet.toml"])?;
        require_success(out, "git add")?;

        // Check whether anything is staged.
        let staged = self
            .run_ok(&["diff", "--cached", "--name-only"])
            .unwrap_or_default();

        if staged.is_empty() {
            return Ok(false);
        }

        let out = self.run(&["commit", "-m", message])?;
        require_success(out, "git commit")?;

        if self.has_remote() {
            let out = self.run(&["push"])?;
            require_success(out, "git push")?;
        }

        Ok(true)
    }

    /// Commit `harbor-fleet.toml` without pushing (used during `init`).
    ///
    /// Returns `true` if a commit was made.
    pub fn commit_local(&self, message: &str) -> Result<bool> {
        let out = self.run(&["add", "harbor-fleet.toml"])?;
        require_success(out, "git add")?;

        let staged = self
            .run_ok(&["diff", "--cached", "--name-only"])
            .unwrap_or_default();

        if staged.is_empty() {
            return Ok(false);
        }

        let out = self.run(&["commit", "-m", message])?;
        require_success(out, "git commit")?;

        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Status / introspection
    // -------------------------------------------------------------------------

    /// Returns `true` if `dir` contains a `.git` directory.
    pub fn is_repo(dir: &Path) -> bool {
        dir.join(".git").exists()
    }

    /// Returns `(ahead, behind)` relative to the upstream tracking branch.
    ///
    /// Does a silent `git fetch` first so the count reflects the actual remote.
    /// Returns `None` if there is no upstream or the count cannot be determined.
    pub fn divergence(&self) -> Option<(usize, usize)> {
        // Fetch silently — ignore errors (e.g., offline).
        let _ = Command::new("git")
            .args(["fetch", "--quiet"])
            .current_dir(&self.dir)
            .output();

        let raw = self
            .run_ok(&["rev-list", "--left-right", "--count", "HEAD...@{u}"])
            .ok()?;

        let mut parts = raw.split_whitespace();
        let ahead: usize = parts.next()?.parse().ok()?;
        let behind: usize = parts.next()?.parse().ok()?;
        Some((ahead, behind))
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    fn run(&self, args: &[&str]) -> Result<Output> {
        run_git_in(&self.dir, args)
    }

    /// Run git and return trimmed stdout, or an error with stderr.
    fn run_ok(&self, args: &[&str]) -> Result<String> {
        let out = self.run(args)?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(HarborError::FleetGitError(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Skip the test if `git` is not on PATH (e.g., minimal CI images).
    fn git_available() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Configure a throw-away git user identity so commits work even in CI
    /// where `user.name` / `user.email` are not set globally.
    fn configure_identity(dir: &Path) {
        for (key, val) in [("user.name", "Test"), ("user.email", "test@test.com")] {
            let _ = std::process::Command::new("git")
                .args(["config", key, val])
                .current_dir(dir)
                .output();
        }
    }

    #[test]
    fn is_repo_returns_false_for_plain_dir() {
        if !git_available() {
            return;
        }
        let dir = TempDir::new().unwrap();
        assert!(!FleetGit::is_repo(dir.path()));
    }

    #[test]
    fn init_creates_git_repo() {
        if !git_available() {
            return;
        }
        let dir = TempDir::new().unwrap();
        FleetGit::init(dir.path()).unwrap();
        assert!(FleetGit::is_repo(dir.path()));
    }

    #[test]
    fn init_is_idempotent() {
        if !git_available() {
            return;
        }
        let dir = TempDir::new().unwrap();
        FleetGit::init(dir.path()).unwrap();
        // Re-initialising must not return an error.
        FleetGit::init(dir.path()).unwrap();
        assert!(FleetGit::is_repo(dir.path()));
    }

    #[test]
    fn has_remote_returns_false_on_fresh_repo() {
        if !git_available() {
            return;
        }
        let dir = TempDir::new().unwrap();
        FleetGit::init(dir.path()).unwrap();
        let git = FleetGit::new(dir.path().to_path_buf());
        assert!(!git.has_remote());
        assert!(git.remote_url().is_none());
    }

    #[test]
    fn set_remote_persists_url() {
        if !git_available() {
            return;
        }
        let dir = TempDir::new().unwrap();
        FleetGit::init(dir.path()).unwrap();
        let git = FleetGit::new(dir.path().to_path_buf());

        git.set_remote("git@github.com:example/fleet.git").unwrap();

        assert!(git.has_remote());
        assert_eq!(
            git.remote_url().as_deref(),
            Some("git@github.com:example/fleet.git")
        );
    }

    #[test]
    fn commit_local_commits_fleet_toml() {
        if !git_available() {
            return;
        }
        let dir = TempDir::new().unwrap();
        FleetGit::init(dir.path()).unwrap();
        configure_identity(dir.path());

        // Write the file commit_local expects.
        std::fs::write(dir.path().join("harbor-fleet.toml"), "[fleet]\n").unwrap();

        let git = FleetGit::new(dir.path().to_path_buf());
        let committed = git.commit_local("Initial fleet").unwrap();
        assert!(committed, "expected a commit to be created");
    }

    #[test]
    fn commit_local_returns_false_when_nothing_staged() {
        if !git_available() {
            return;
        }
        let dir = TempDir::new().unwrap();
        FleetGit::init(dir.path()).unwrap();
        configure_identity(dir.path());

        // Commit once so the file is tracked.
        std::fs::write(dir.path().join("harbor-fleet.toml"), "[fleet]\n").unwrap();
        let git = FleetGit::new(dir.path().to_path_buf());
        git.commit_local("First").unwrap();

        // Second call with no changes should return false.
        let committed = git.commit_local("Second").unwrap();
        assert!(!committed, "expected no commit when nothing changed");
    }
}

fn run_git_in(dir: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HarborError::GitNotFound
            } else {
                HarborError::Io(e)
            }
        })
}

/// Converts a non-zero exit code into a `FleetGitError`.
fn require_success(out: Output, op: &str) -> Result<()> {
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let msg = if stderr.is_empty() {
        format!("{op} failed (exit {})", out.status)
    } else {
        stderr
    };
    Err(HarborError::FleetGitError(msg))
}
