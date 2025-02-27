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
    #[cfg(any(
        feature = "sqlx-postgres",
        feature = "sqlx-mysql",
        feature = "sqlx-sqlite"
    ))]
    SqlxError(sqlx::Error),

    #[error("SQLx error: {0}")]
    #[cfg(any(
        feature = "sqlx-postgres",
        feature = "sqlx-mysql",
        feature = "sqlx-sqlite"
    ))]
    SqlxErrorMut(&'static mut sqlx::Error),

    #[error("Database drop failed: {0}")]
    DatabaseDropFailed(String),

    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("URL parse error: {0}")]
    UrlParseError(url::ParseError),
}

#[cfg(any(
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite"
))]
impl From<sqlx::Error> for PoolError {
    fn from(error: sqlx::Error) -> Self {
        PoolError::SqlxError(error)
    }
}

/// Result type for database pool operations
pub type Result<T> = std::result::Result<T, PoolError>;

/// Type helper for defining a standard result with PoolError
pub fn ok<T>(value: T) -> Result<T> {
    Ok(value)
}

/// Type helper for creating a standard error result with PoolError
pub fn err<T>(message: impl Into<String>) -> Result<T> {
    Err(PoolError::DatabaseError(message.into()))
}
