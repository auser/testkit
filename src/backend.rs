use async_trait::async_trait;

use crate::{error::Result, pool::PoolConfig, template::DatabaseName};

/// A trait for database connections that can be pooled
#[async_trait]
pub trait Connection: Send {
    /// Check if the connection is valid
    async fn is_valid(&self) -> bool;

    /// Reset the connection state
    async fn reset(&mut self) -> Result<()>;

    /// Execute a SQL query
    async fn execute(&mut self, sql: &str) -> Result<()>;
}

/// A trait for database backends that can create and manage databases
#[async_trait]
pub trait DatabaseBackend: Send + Sync + Clone {
    /// The type of connection this backend provides
    type Connection: Connection;
    /// The type of pool this backend provides
    type Pool: DatabasePool<Connection = Self::Connection>;

    /// Create a new database with the given name
    async fn create_database(&self, name: &DatabaseName) -> Result<()>;

    /// Drop a database with the given name
    async fn drop_database(&self, name: &DatabaseName) -> Result<()>;

    /// Create a new connection pool for the given database
    async fn create_pool(&self, name: &DatabaseName, config: &PoolConfig) -> Result<Self::Pool>;

    /// Terminate all connections to the given database
    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()>;

    /// Create a new database from a template
    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()>;
}

/// A trait for database pools that can be used to acquire and release connections
#[async_trait]
pub trait DatabasePool: Send + Sync + Clone {
    /// The type of connection this pool provides
    type Connection: Connection;

    /// Acquire a connection from the pool
    async fn acquire(&self) -> Result<Self::Connection>;

    /// Release a connection back to the pool
    async fn release(&self, conn: Self::Connection) -> Result<()>;
}
