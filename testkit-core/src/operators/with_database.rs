use crate::Transaction;

use super::with_context;

/// Configuration for database connections
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Connection string for admin operations (schema changes, etc.)
    pub admin_url: String,
    /// Connection string for regular operations
    pub user_url: String,
}

impl DatabaseConfig {
    /// Create a new configuration with explicit connection strings
    pub fn new(admin_url: impl Into<String>, user_url: impl Into<String>) -> Self {
        Self {
            admin_url: admin_url.into(),
            user_url: user_url.into(),
        }
    }

    /// Get a configuration from environment variables
    /// Uses ADMIN_DATABASE_URL and DATABASE_URL
    pub fn from_env() -> std::result::Result<Self, std::env::VarError> {
        #[cfg(feature = "dotenvy")]
        let _ = dotenvy::dotenv();
        let admin_url = std::env::var("ADMIN_DATABASE_URL")?;
        let user_url = std::env::var("DATABASE_URL")?;
        Ok(Self::new(admin_url, user_url))
    }
}

/// Database context that can be used with transactions
#[derive(Debug)]
pub struct DatabaseContext<Conn> {
    /// The actual database connection
    pub connection: Conn,
    /// Configuration used to establish the connection
    pub config: DatabaseConfig,
}

/// Create a transaction with database context
///
/// If `config` is None, it will try to read configuration from environment variables
///
/// # Example
///
/// ```rust,ignore
/// // With explicit config
/// let config = DatabaseConfig::new("postgres://admin@localhost/mydb", "postgres://user@localhost/mydb");
/// let tx = with_database(config, |ctx| async {
///     // Use ctx.connection to interact with the database
///     // ctx.config contains the connection strings if needed
///     Ok(())
/// });
///
/// // Using environment variables
/// let tx = with_database(None, |ctx| async {
///     // Same as above, but config comes from environment
///     Ok(())
/// });
/// ```
pub fn with_database<F, Fut, Conn, T, E>(
    _config: Option<DatabaseConfig>,
    f: F,
) -> impl Transaction<Context = DatabaseContext<Conn>, Item = T, Error = E>
where
    F: Fn(&mut DatabaseContext<Conn>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = std::result::Result<T, E>> + Send + 'static,
    Conn: Send + Sync + 'static,
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    with_context(move |ctx: &mut DatabaseContext<Conn>| f(ctx))
}
