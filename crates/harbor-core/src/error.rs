use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum HarborError {
    #[error("Config not found at {path}")]
    ConfigNotFound { path: PathBuf },

    #[error("Failed to parse config: {0}")]
    ConfigParse(String),

    #[error("Failed to write config: {0}")]
    ConfigWrite(String),

    #[error("Server '{name}' not found")]
    ServerNotFound { name: String },

    #[error("Server '{name}' already exists")]
    ServerAlreadyExists { name: String },

    #[error("Server '{name}' is already running")]
    ServerAlreadyRunning { name: String },

    #[error("Server '{name}' is not running")]
    ServerNotRunning { name: String },

    #[error("Failed to start server '{name}': {reason}")]
    ServerStartFailed { name: String, reason: String },

    #[error("Connector error for host '{host}': {reason}")]
    ConnectorError { host: String, reason: String },

    #[error("Host config not found at {path}")]
    HostConfigNotFound { path: PathBuf },

    #[error("Vault error: {0}")]
    VaultError(String),

    #[error("OAuth error: {0}")]
    OAuthError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    // --- Relay / Publish errors ---
    #[error("Relay error: {0}")]
    RelayError(String),

    #[error("Tunnel connection failed: {reason}")]
    TunnelConnectionFailed { reason: String },

    #[error("Tunnel not found: {subdomain}")]
    TunnelNotFound { subdomain: String },

    #[error("Tool not allowed for remote access: {tool}")]
    RemoteToolDenied { tool: String },

    #[error("Noise handshake failed: {0}")]
    NoiseHandshakeFailed(String),

    #[error("Publish not active")]
    PublishNotActive,

    // --- Fleet / crew sync errors ---
    #[error("Fleet not initialized. Run `harbor crew init` to set up team sync.")]
    FleetNotInitialized,

    #[error("Git error: {0}")]
    FleetGitError(String),

    #[error("Git not found in PATH. Please install git to use fleet sync.")]
    GitNotFound,
}

pub type Result<T> = std::result::Result<T, HarborError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_config_not_found() {
        let err = HarborError::ConfigNotFound {
            path: PathBuf::from("/home/user/.harbor/config.toml"),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Config not found"));
        assert!(msg.contains("config.toml"));
    }

    #[test]
    fn test_error_display_server_not_found() {
        let err = HarborError::ServerNotFound {
            name: "my-server".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("my-server"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_error_display_server_already_exists() {
        let err = HarborError::ServerAlreadyExists {
            name: "existing".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("existing"));
        assert!(msg.contains("already exists"));
    }

    #[test]
    fn test_error_display_server_start_failed() {
        let err = HarborError::ServerStartFailed {
            name: "broken".to_string(),
            reason: "process exited".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("broken"));
        assert!(msg.contains("process exited"));
    }

    #[test]
    fn test_error_display_connector_error() {
        let err = HarborError::ConnectorError {
            host: "vscode".to_string(),
            reason: "permission denied".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("vscode"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn test_error_display_vault_error() {
        let err = HarborError::VaultError("keychain locked".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Vault"));
        assert!(msg.contains("keychain locked"));
    }

    #[test]
    fn test_error_display_oauth_error() {
        let err = HarborError::OAuthError("token expired".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("OAuth"));
        assert!(msg.contains("token expired"));
    }

    #[test]
    fn test_error_display_fleet_not_initialized() {
        let err = HarborError::FleetNotInitialized;
        let msg = format!("{}", err);
        assert!(msg.contains("Fleet not initialized"));
        assert!(msg.contains("harbor crew init"));
    }

    #[test]
    fn test_error_display_tunnel_not_found() {
        let err = HarborError::TunnelNotFound {
            subdomain: "abc123".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("abc123"));
    }

    #[test]
    fn test_error_display_remote_tool_denied() {
        let err = HarborError::RemoteToolDenied {
            tool: "dangerous_tool".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("dangerous_tool"));
        assert!(msg.contains("not allowed"));
    }

    #[test]
    fn test_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let harbor_err: HarborError = io_err.into();
        let msg = format!("{}", harbor_err);
        assert!(msg.contains("IO error"));
    }

    #[test]
    fn test_result_type_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_type_err() {
        let result: Result<i32> = Err(HarborError::ConfigParse("bad toml".to_string()));
        assert!(result.is_err());
    }
}
