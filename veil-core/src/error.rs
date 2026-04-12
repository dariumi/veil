use thiserror::Error;

pub type Result<T> = std::result::Result<T, VeilError>;

#[derive(Debug, Error)]
pub enum VeilError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Protocol violation: {0}")]
    Protocol(String),

    #[error("Relay error: {0}")]
    Relay(String),

    #[error("DNS error: {0}")]
    Dns(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Timeout")]
    Timeout,

    #[error("Not authenticated")]
    NotAuthenticated,

    #[error("Access denied")]
    AccessDenied,

    #[error("Internal error: {0}")]
    Internal(String),
}
