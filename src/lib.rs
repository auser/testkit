pub mod backend;
pub mod backends;
pub mod env;
pub mod error;
pub mod macros;
pub mod migrations;
pub mod pool;
pub mod template;
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
pub use template::{DatabaseName, DatabaseTemplate, ImmutableDatabase};

use async_trait::async_trait;
use std::error::Error;
use std::fmt::Debug;

#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    type Error: Error + Debug;

    /// Execute a raw SQL query
    async fn execute(&mut self, query: &str) -> Result<(), Self::Error>;

    /// Query with parameters and get results
    async fn query<T>(
        &mut self,
        query: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<T>, Self::Error>
    where
        T: FromRow;
}

#[async_trait]
pub trait Transaction: DatabaseConnection {
    /// Commit the transaction
    async fn commit(self) -> Result<(), <Self as DatabaseConnection>::Error>;

    /// Rollback the transaction
    async fn rollback(self) -> Result<(), <Self as DatabaseConnection>::Error>;
}

#[async_trait]
pub trait DatabasePool: Send + Sync {
    type Connection: DatabaseConnection;
    type Tx: Transaction;

    /// Get a connection from the pool
    async fn connection(
        &self,
    ) -> Result<Self::Connection, <Self::Connection as DatabaseConnection>::Error>;

    /// Start a new transaction
    async fn begin(&self) -> Result<Self::Tx, <Self::Connection as DatabaseConnection>::Error>;

    /// Setup the database with a closure
    async fn setup<F, Fut>(
        &self,
        setup_fn: F,
    ) -> Result<(), <Self::Connection as DatabaseConnection>::Error>
    where
        F: FnOnce(Self::Connection) -> Fut + Send,
        Fut: std::future::Future<
                Output = Result<(), <Self::Connection as DatabaseConnection>::Error>,
            > + Send;
}
