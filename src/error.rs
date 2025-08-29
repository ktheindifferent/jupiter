use std::fmt;
use std::error::Error as StdError;

#[derive(Debug)]
pub enum JupiterError {
    DatabaseError(String),  // Store as string to handle both postgres and tokio_postgres errors
    ConfigurationError(String),
    ValidationError(String),
    ConnectionError(String),
    SslError(String),
    IoError(std::io::Error),
    SerializationError(serde_json::Error),
    AuthenticationError(String),
    RateLimitError(String),
}

impl fmt::Display for JupiterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JupiterError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            JupiterError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            JupiterError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            JupiterError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            JupiterError::SslError(msg) => write!(f, "SSL error: {}", msg),
            JupiterError::IoError(e) => write!(f, "IO error: {}", e),
            JupiterError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            JupiterError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            JupiterError::RateLimitError(msg) => write!(f, "Rate limit error: {}", msg),
        }
    }
}

impl StdError for JupiterError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            JupiterError::IoError(e) => Some(e),
            JupiterError::SerializationError(e) => Some(e),
            _ => None,
        }
    }
}

// postgres::Error is actually re-exported from tokio_postgres, so we only need one impl
impl From<postgres::Error> for JupiterError {
    fn from(err: postgres::Error) -> Self {
        JupiterError::DatabaseError(err.to_string())
    }
}

impl From<std::io::Error> for JupiterError {
    fn from(err: std::io::Error) -> Self {
        JupiterError::IoError(err)
    }
}

impl From<serde_json::Error> for JupiterError {
    fn from(err: serde_json::Error) -> Self {
        JupiterError::SerializationError(err)
    }
}

impl From<std::env::VarError> for JupiterError {
    fn from(err: std::env::VarError) -> Self {
        JupiterError::ConfigurationError(format!("Environment variable error: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, JupiterError>;