use thiserror::Error;

/// Error type for database pool operations
#[derive(Debug, Error)]
pub enum PoolError {
    /// Failed to create a pool
    #[error("Failed to create pool: {0}")]
    PoolCreationFailed(String),

    /// Failed to acquire a connection from the pool
    #[error("Failed to acquire connection: {0}")]
    ConnectionAcquisitionFailed(String),

    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Invalid URL
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Migration error
    #[error("Migration error: {0}")]
    MigrationError(String),
}

/// Result type for database pool operations
pub type Result<T> = std::result::Result<T, PoolError>;
