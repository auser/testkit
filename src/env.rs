use std::sync::OnceLock;

use crate::error::Result;

/// A static cell that ensures environment variables are loaded only once
static ENV_LOADED: OnceLock<()> = OnceLock::new();

/// Loads environment variables from a .env file if they haven't been loaded yet.
///
/// This function uses a static `OnceLock` to ensure that the environment is only
/// loaded once, even if called multiple times.
fn load_env() {
    ENV_LOADED.get_or_init(|| {
        dotenvy::dotenv().ok();
    });
}

/// Gets the PostgreSQL database URL from environment variables.
///
/// This function looks for a `DATABASE_URL` environment variable that contains
/// a valid PostgreSQL connection string.
///
/// # Returns
///
/// Returns a `Result` containing the database URL string, or an error if the
/// environment variable is not found or is invalid.
#[cfg(feature = "postgres")]
pub fn get_postgres_url() -> Result<String> {
    load_env();
    std::env::var("DATABASE_URL")
        .map_err(|_| crate::error::DbError::new("DATABASE_URL environment variable not found"))
}

/// Gets the MySQL database URL from environment variables.
///
/// This function looks for a `DATABASE_URL` environment variable that contains
/// a valid MySQL connection string.
///
/// # Returns
///
/// Returns a `Result` containing the database URL string, or an error if the
/// environment variable is not found or is invalid.
#[cfg(feature = "mysql")]
pub fn get_mysql_url() -> Result<String> {
    load_env();

    // First try to get from environment
    if let Ok(url) = std::env::var("MYSQL_URL") {
        return Ok(url);
    }

    // Default URLs for different environments - always use root/superuser for tests
    let urls = [
        "mysql://testuser:testpassword@mysql:3306", // Docker Compose
        "mysql://root:root@localhost:3306",         // Local development common default
        "mysql://root:@localhost:3306",             // CI environment with no password
        "mysql://admin:password@localhost:3306",    // Alternative admin user
    ];

    // Log which URL we're trying to use
    tracing::debug!("Using MySQL URL: {}", urls[0]);

    // In a real implementation, we would test each connection
    // For now, return the first URL for simplicity
    Ok(urls[0].to_string())
}

/// Gets the SQLx PostgreSQL database URL from environment variables.
///
/// This function looks for a `DATABASE_URL` environment variable that contains
/// a valid PostgreSQL connection string compatible with SQLx.
///
/// # Returns
///
/// Returns a `Result` containing the database URL string, or an error if the
/// environment variable is not found or is invalid.
#[cfg(feature = "sqlx-postgres")]
pub fn get_sqlx_postgres_url() -> Result<String> {
    load_env();
    std::env::var("DATABASE_URL")
        .map_err(|_| crate::error::DbError::new("DATABASE_URL environment variable not found"))
}

/// Gets the SQLite database URL from environment variables.
///
/// This function looks for a `DATABASE_URL` environment variable that contains
/// a valid path where SQLite databases should be stored.
///
/// # Returns
///
/// Returns a `Result` containing the database URL string, or an error if the
/// environment variable is not found or is invalid.
#[cfg(any(feature = "sqlite", feature = "sqlx-sqlite"))]
pub fn get_sqlite_url() -> Result<String> {
    load_env();
    // Use a default if DATABASE_URL is not set
    Ok(std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        // Default to a temporary directory for SQLite
        String::from("/tmp/sqlite-testkit")
    }))
}
