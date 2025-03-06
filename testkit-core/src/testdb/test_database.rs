use async_trait::async_trait;
use parking_lot::Mutex;
use std::fmt::{Debug, Display};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Configuration for database connections
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    /// Connection string for admin operations (schema changes, etc.)
    pub admin_url: String,
    /// Connection string for regular operations
    pub user_url: String,
    /// Maximum number of connections to the database
    pub max_connections: Option<usize>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|e| {
            panic!("Failed to create DatabaseConfig: {}", e);
        })
    }
}

impl DatabaseConfig {
    /// Create a new configuration with explicit connection strings
    pub fn new(admin_url: impl Into<String>, user_url: impl Into<String>) -> Self {
        Self {
            admin_url: admin_url.into(),
            user_url: user_url.into(),
            max_connections: None,
        }
    }

    /// Get a configuration from environment variables
    /// Uses ADMIN_DATABASE_URL and DATABASE_URL
    pub fn from_env() -> std::result::Result<Self, std::env::VarError> {
        #[cfg(feature = "dotenvy")]
        let _ = dotenvy::from_filename(".env");
        let user_url = std::env::var("DATABASE_URL")?;
        let admin_url = std::env::var("ADMIN_DATABASE_URL").unwrap_or(user_url.clone());
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

pub trait TestDatabaseConnection {
    fn connection_string(&self) -> String;
}

#[async_trait]
pub trait DatabasePool: Send + Sync + Clone {
    type Connection: Send + Sync + TestDatabaseConnection;
    type Error: Send + Sync + From<String> + Display + Debug;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error>;
    async fn release(&self, conn: Self::Connection) -> Result<(), Self::Error>;
    fn connection_string(&self) -> String;
}

/// Trait defining a test database abstraction
#[async_trait]
pub trait DatabaseBackend: Send + Sync + Clone + Debug {
    type Connection: Send + Sync + Clone;
    type Pool: Send + Sync + DatabasePool<Connection = Self::Connection, Error = Self::Error>;
    type Error: Send + Sync + Clone + From<String> + Display + Debug;

    async fn new(config: DatabaseConfig) -> Result<Self, Self::Error>;

    /// Create a new connection pool for the given database
    async fn create_pool(
        &self,
        name: &DatabaseName,
        config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error>;

    /// Create a single connection to the given database
    /// This is useful for cases where a full pool is not needed
    async fn connect(&self, name: &DatabaseName) -> Result<Self::Connection, Self::Error> {
        // Default implementation connects using the connection string for the given database name
        let connection_string = self.connection_string(name);
        self.connect_with_string(&connection_string).await
    }

    /// Create a single connection using a connection string directly
    /// This is useful for connecting to databases that may not have been created by TestKit
    async fn connect_with_string(
        &self,
        connection_string: &str,
    ) -> Result<Self::Connection, Self::Error>;

    /// Create a new database with the given name
    async fn create_database(
        &self,
        pool: &Self::Pool,
        name: &DatabaseName,
    ) -> Result<(), Self::Error>;

    /// Drop a database with the given name
    fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error>;

    /// Get the connection string for the given database
    fn connection_string(&self, name: &DatabaseName) -> String;
}

/// A test database that handles setup, connections, and cleanup
/// TODO: Create a TestManager that can handle connection pooling and cleanup
#[derive(Clone)]
pub struct TestDatabaseInstance<B>
where
    B: DatabaseBackend + 'static + Clone + Debug + Send + Sync,
{
    /// The database backend
    pub backend: B,
    /// The connection pool
    pub pool: B::Pool,
    /// The database name
    pub db_name: DatabaseName,
    /// The connection pool
    pub connection_pool: Option<Arc<Mutex<Vec<B::Connection>>>>,
}

impl<B> Debug for TestDatabaseInstance<B>
where
    B: DatabaseBackend + 'static + Clone + Debug + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TestDatabaseInstance {{ backend: {:?}, db_name: {:?} }}",
            self.backend, self.db_name
        )
    }
}

impl<B> TestDatabaseInstance<B>
where
    B: DatabaseBackend + 'static + Clone + Debug + Send + Sync,
{
    /// Create a new test database with the given backend
    pub async fn new(backend: B, config: DatabaseConfig) -> Result<Self, B::Error> {
        // Generate unique name
        let db_name = DatabaseName::new(None);

        tracing::debug!("Creating connection pool for database: {}", db_name);
        let pool = backend.create_pool(&db_name, &config).await?;

        tracing::debug!("Creating database: {}", db_name);
        backend.create_database(&pool, &db_name).await?;

        let inst = Self {
            backend,
            pool,
            db_name,
            connection_pool: None,
        };

        Ok(inst)
    }

    /// Returns a reference to the backend
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns a reference to the database name
    pub fn name(&self) -> &DatabaseName {
        &self.db_name
    }

    /// Create a single connection to the database without using the pool
    /// This is useful for cases where a single connection is needed for a specific operation
    pub async fn connect(&self) -> Result<B::Connection, B::Error> {
        self.backend.connect(&self.db_name).await
    }

    /// Execute a function with a one-off connection and automatically close it after use
    /// This is the most efficient way to perform a one-off database operation
    pub async fn with_connection<F, R, E>(&self, operation: F) -> Result<R, B::Error>
    where
        F: FnOnce(&B::Connection) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send>> + Send,
        E: std::error::Error + Send + Sync + 'static,
        B::Error: From<E>,
    {
        // Create a connection
        let conn = self.connect().await?;

        // Run the operation
        let result = operation(&conn).await.map_err(|e| B::Error::from(e))?;

        // Connection will be dropped automatically when it goes out of scope
        Ok(result)
    }

    /// Get a connection from the pool or acquire a new one
    pub async fn acquire_connection(
        &self,
    ) -> Result<<B::Pool as DatabasePool>::Connection, B::Error> {
        let conn = match &self.connection_pool {
            Some(pool) => {
                let mut guard = pool.lock();
                let conn = guard
                    .pop()
                    .ok_or(B::Error::from("No connection available".to_string()))?;
                drop(guard);
                conn
            }
            None => self.pool.acquire().await?,
        };

        Ok(conn)
    }

    /// Release a connection back to the pool
    pub async fn release_connection(
        &self,
        conn: <B::Pool as DatabasePool>::Connection,
    ) -> Result<(), B::Error> {
        if let Some(pool) = &self.connection_pool {
            pool.lock().push(conn);
        }

        Ok(())
    }

    /// Setup the database with a function
    /// The connection handling approach needs to match the expected B::Connection type
    pub async fn setup<F, Fut>(&self, setup_fn: F) -> Result<(), B::Error>
    where
        F: FnOnce(&mut <B::Pool as DatabasePool>::Connection) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), B::Error>> + Send,
    {
        // Get a connection from the pool
        let mut conn = self.acquire_connection().await?;

        // Call the setup function with a mutable reference to the connection
        let result = setup_fn(&mut conn).await;

        // Return the connection to the pool if we have one
        if let Some(pool) = &self.connection_pool {
            pool.lock().push(conn);
        }

        result
    }
}

impl<B> Drop for TestDatabaseInstance<B>
where
    B: DatabaseBackend + Clone + Debug + Send + Sync + 'static,
{
    fn drop(&mut self) {
        let name = self.db_name.clone();

        if let Err(err) = self.backend.drop_database(&name) {
            tracing::error!("Failed to drop database {}: {}", name, err);
        } else {
            tracing::info!("Successfully dropped database {} during Drop", name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_name() {
        let name = DatabaseName::new(None);
        assert_ne!(name.as_str(), "");
    }
}
