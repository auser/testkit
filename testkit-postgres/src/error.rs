use thiserror::Error;

/// Errors that can occur when working with PostgreSQL databases
#[derive(Debug, Error)]
pub enum Error {
    /// Database connection errors
    #[error("Database connection error: {0}")]
    ConnectionError(#[from] sqlx::Error),

    /// URL parsing errors
    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),

    /// Generic error with message
    #[error("{0}")]
    Generic(String),

    /// Runtime error
    #[error("Runtime error: {0}")]
    RuntimeError(#[from] tokio::io::Error),
}

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Self::ConnectionError(e) => Self::Generic(format!("Database connection error: {}", e)),
            Self::UrlError(e) => Self::Generic(format!("URL parsing error: {}", e)),
            Self::Generic(s) => Self::Generic(s.clone()),
            Self::RuntimeError(e) => Self::Generic(format!("Runtime error: {}", e)),
        }
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}
