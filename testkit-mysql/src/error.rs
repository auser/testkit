use thiserror::Error;

/// MySQL-specific errors for the testkit
#[derive(Error, Debug, Clone)]
pub enum MySqlError {
    /// Error with the configuration
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Error connecting to the database
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Error creating a database
    #[error("Database creation error: {0}")]
    DatabaseCreationError(String),

    /// Error dropping a database
    #[error("Database drop error: {0}")]
    DatabaseDropError(String),

    /// Error executing a query
    #[error("Query execution error: {0}")]
    QueryExecutionError(String),

    /// Error with transactions
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// Generic error
    #[error("MySQL error: {0}")]
    Generic(String),
}

impl From<String> for MySqlError {
    fn from(s: String) -> Self {
        MySqlError::Generic(s)
    }
}

impl From<&str> for MySqlError {
    fn from(s: &str) -> Self {
        MySqlError::Generic(s.to_string())
    }
}

#[cfg(feature = "with-mysql-async")]
impl From<mysql_async::Error> for MySqlError {
    fn from(error: mysql_async::Error) -> Self {
        MySqlError::Generic(error.to_string())
    }
}

#[cfg(feature = "with-sqlx")]
impl From<sqlx::Error> for MySqlError {
    fn from(error: sqlx::Error) -> Self {
        MySqlError::Generic(error.to_string())
    }
}
