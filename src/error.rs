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

    /// Transaction error
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// SQLx error
    // #[cfg(any(
    //     feature = "sqlx-postgres",
    //     feature = "sqlx-mysql",
    //     feature = "sqlx-sqlite"
    // ))]
    #[error("SQLx error: {0}")]
    SqlxError(sqlx::Error),

    #[error("SQLx error: {0}")]
    SqlxErrorMut(&'static mut sqlx::Error),
}

impl From<sqlx::Error> for PoolError {
    fn from(error: sqlx::Error) -> Self {
        PoolError::SqlxError(error)
    }
}

/// Result type for database pool operations
pub type Result<T> = std::result::Result<T, PoolError>;
