use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

/// Type alias for Result with DbError as the error type
pub type Result<T = ()> = std::result::Result<T, DbError>;

#[derive(Debug)]
pub struct DbError(String);
impl DbError {
    pub fn new<S: Into<String>>(msg: S) -> Self {
        DbError(msg.into())
    }
}
impl Display for DbError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl StdError for DbError {}
// Implement From for common error types
impl From<std::io::Error> for DbError {
    fn from(err: std::io::Error) -> Self {
        DbError(format!("IO error: {}", err))
    }
}
impl From<url::ParseError> for DbError {
    fn from(err: url::ParseError) -> Self {
        DbError(format!("URL parse error: {}", err))
    }
}
#[cfg(any(
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite"
))]
impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        DbError(format!("SQLx error: {}", err))
    }
}
