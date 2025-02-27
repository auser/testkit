pub mod backend;
pub mod backends;
pub mod env;
pub mod error;
pub mod macros;
pub mod migrations;
pub mod pool;
pub mod test_db;
pub mod tests;
pub mod util;
pub mod wrapper;

pub mod prelude;

pub use backend::{Connection, DatabaseBackend, DatabasePool};
#[cfg(feature = "mysql")]
pub use backends::MySqlBackend;
#[cfg(feature = "postgres")]
pub use backends::PostgresBackend;
#[cfg(feature = "sqlx-postgres")]
pub use backends::SqlxPostgresBackend;
pub use error::{PoolError, Result};
pub use migrations::{RunSql, SqlSource};
pub use pool::PoolConfig;
pub use test_db::{DatabaseName, TestDatabase, TestDatabaseTemplate};
pub use wrapper::{ResourcePool, Reusable};

/// Create a simplified test database with default configuration
#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
pub async fn create_test_db<B: DatabaseBackend + Clone + Send + 'static>(
    backend: B,
) -> Result<TestDatabase<B>> {
    TestDatabase::new(backend, PoolConfig::default()).await
}

/// Create a template database with default configuration
#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
pub async fn create_template_db<B: DatabaseBackend + Clone + Send + 'static>(
    backend: B,
    max_replicas: usize,
) -> Result<TestDatabaseTemplate<B>> {
    TestDatabaseTemplate::new(backend, PoolConfig::default(), max_replicas).await
}
