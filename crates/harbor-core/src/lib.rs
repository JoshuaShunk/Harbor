pub mod auth;
pub mod catalog;
pub mod config;
pub mod connector;
pub mod error;
pub mod gateway;
pub mod marketplace;
pub mod relay;
pub mod server;
pub mod sync;
pub mod updater;

// Re-exports for convenience
pub use auth::oauth::{OAuthProvider, OAuthTokens};
pub use auth::vault::Vault;
pub use catalog::{AuthKind, ExtraArgs, NativeServer};
pub use config::{HarborConfig, HostConfig, ServerConfig};
pub use error::{HarborError, Result};
