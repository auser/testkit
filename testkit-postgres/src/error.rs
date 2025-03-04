use std::fmt::Debug;
use thiserror::Error;

/// Error type for PostgreSQL operations
#[derive(Error, Debug, Clone)]
pub enum PostgresError {
    /// Error connecting to the database
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Error executing a query
    #[error("Query execution error: {0}")]
    QueryError(String),

    /// Error creating a database
    #[error("Database creation error: {0}")]
    DatabaseCreationError(String),

    /// Error dropping a database
    #[error("Database drop error: {0}")]
    DatabaseDropError(String),

    /// Error during transaction operations
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// Error from configuration
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Any other error
    #[error("Other error: {0}")]
    Other(String),
}

impl From<String> for PostgresError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for PostgresError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

// Feature-specific error conversions
#[cfg(feature = "postgres")]
impl From<tokio_postgres::Error> for PostgresError {
    fn from(err: tokio_postgres::Error) -> Self {
        Self::QueryError(err.to_string())
    }
}

#[cfg(feature = "sqlx")]
impl From<sqlx::Error> for PostgresError {
    fn from(err: sqlx::Error) -> Self {
        Self::QueryError(err.to_string())
    }
}
