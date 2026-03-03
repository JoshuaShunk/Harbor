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
}

pub type Result<T> = std::result::Result<T, HarborError>;
