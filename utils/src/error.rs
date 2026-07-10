use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum AstrBotError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(Arc<std::io::Error>),

    #[error("Serialization error: {0}")]
    Serde(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Custom(String),
}

impl From<std::io::Error> for AstrBotError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(Arc::new(e))
    }
}

pub type Result<T> = std::result::Result<T, AstrBotError>;
