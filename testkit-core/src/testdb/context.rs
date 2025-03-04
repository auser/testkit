use std::fmt::Display;

use uuid::Uuid;

/// Database context trait for transactions
pub trait DatabaseContext<Conn> {
    /// Get a reference to the database connection
    fn connection(&self) -> &Conn;

    /// Get a mutable reference to the database connection
    fn connection_mut(&mut self) -> &mut Conn;

    /// Get the database configuration
    fn config(&self) -> &DatabaseConfig;
}

/// Database context that can be used with transactions
#[derive(Debug, Clone)]
pub struct DefaultDatabaseContext<Conn> {
    /// The actual database connection
    pub connection: Conn,
    /// Configuration used to establish the connection
    pub config: DatabaseConfig,
}

impl<Conn> DefaultDatabaseContext<Conn> {
    /// Create a new database context with a connection and configuration
    pub fn new(connection: Conn, config: DatabaseConfig) -> Self {
        Self { connection, config }
    }
}

impl<Conn> DatabaseContext<Conn> for DefaultDatabaseContext<Conn> {
    fn connection(&self) -> &Conn {
        &self.connection
    }

    fn connection_mut(&mut self) -> &mut Conn {
        &mut self.connection
    }

    fn config(&self) -> &DatabaseConfig {
        &self.config
    }
}

/// Configuration for database connections
#[derive(Debug, Clone, PartialEq, Eq)]
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
        let admin_url = std::env::var("ADMIN_DATABASE_URL")?;
        let user_url = std::env::var("DATABASE_URL")?;
        Ok(Self::new(admin_url, user_url))
    }
}

/// A unique database name
#[derive(Debug, Clone)]
pub struct DatabaseName(String);

impl DatabaseName {
    /// Create a new unique database name with an optional prefix
    pub fn new(prefix: Option<&str>) -> Self {
        let uuid = Uuid::new_v4();
        let safe_uuid = uuid.to_string().replace('-', "_");
        Self(format!("{}_{}", prefix.unwrap_or("testkit"), safe_uuid))
    }

    /// Get the database name as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for DatabaseName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
