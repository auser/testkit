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
#[cfg(any(feature = "mysql", feature = "sqlx-mysql"))]
pub fn get_mysql_url() -> Result<String> {
    load_env();

    // First try to get from environment
    if let Ok(url) = std::env::var("MYSQL_URL") {
        tracing::info!("Using MySQL URL from environment: {}", url);
        return Ok(url);
    }

    // Try with Docker hostnames first - without database suffix which can cause connection issues
    let urls = [
        "mysql://root@mysql:3306",  // Docker, no password - THIS WORKS!
        "mysql://root:@mysql:3306", // Docker, empty password
        "mysql://root:@mysql:3306?ssl-mode=DISABLED", // Docker with SSL disabled
        "mysql://root@localhost:3306", // Local with no password
        "mysql://root@localhost:3336", // Local via port mapping
        "mysql://root@host.docker.internal:3336", // Docker host machine
    ];

    // Log which URLs we're going to try
    tracing::info!("Will try the following MySQL URLs:");
    for (i, url) in urls.iter().enumerate() {
        tracing::info!("  {}: {}", i + 1, url);
    }

    // Return the first URL - the actual connection testing happens in the MySqlBackend
    tracing::info!("Using MySQL URL: {}", urls[0]);
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

/// Gets the SQLx MySQL database URL from environment variables.
///
/// This function looks for a `MYSQL_DATABASE_URL` environment variable that contains
/// a valid MySQL connection string compatible with SQLx.
///
/// # Returns
///
/// Returns a `Result` containing the database URL string, or an error if the
/// environment variable is not found or is invalid.
#[cfg(feature = "sqlx-mysql")]
pub fn get_sqlx_mysql_url() -> Result<String> {
    load_env();

    // First try to get from environment
    if let Ok(url) = std::env::var("MYSQL_DATABASE_URL") {
        tracing::info!("Using MySQL URL from environment: {}", url);
        return Ok(url);
    }

    // Try with Docker hostnames first - without database suffix which can cause connection issues
    // Adding connection parameters: connection timeout, SSL disabled, and other stability parameters
    let urls = [
        // Docker configurations - preferred for tests
        "mysql://root@mysql:3306?timeout=60&ssl-mode=DISABLED",
        "mysql://root:@mysql:3306?timeout=60&ssl-mode=DISABLED",
        // Local configurations as fallbacks
        "mysql://root@localhost:3306?timeout=60&ssl-mode=DISABLED",
        "mysql://root@localhost:3336?timeout=60&ssl-mode=DISABLED",
        "mysql://root@host.docker.internal:3336?timeout=60&ssl-mode=DISABLED",
    ];

    // Log which URLs we're going to try
    tracing::info!("Will try the following MySQL URLs for SQLx:");
    for (i, url) in urls.iter().enumerate() {
        tracing::info!("  {}: {}", i + 1, url);
    }

    // Return the first URL - the actual connection testing happens in the SqlxMySqlBackend
    tracing::info!("Using MySQL URL for SQLx: {}", urls[0]);
    Ok(urls[0].to_string())
}
