use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("config: {0}")]
    Config(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("integrity check failed for {what}: expected {expected}, got {actual}")]
    Integrity { what: String, expected: String, actual: String },
    #[error("{0}")]
    Other(String),
}
