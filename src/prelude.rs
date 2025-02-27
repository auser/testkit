// Re-exports of the most commonly used items for convenience
pub use crate::backend::{Connection, DatabaseBackend, DatabasePool};
pub use crate::error::{err, map_db_err, ok, to_result, PoolError, Result};
pub use crate::migrations::{RunSql, SqlSource};
pub use crate::pool::PoolConfig;
pub use crate::test_db::{DatabaseName, TestDatabase, TestDatabaseTemplate};
pub use crate::wrapper::{ResourcePool, Reusable};

// Feature-specific backend exports
#[cfg(feature = "sqlx-sqlite")]
pub use crate::backends::sqlite::SqliteBackend;
#[cfg(feature = "mysql")]
pub use crate::backends::MySqlBackend;
#[cfg(feature = "postgres")]
pub use crate::backends::PostgresBackend;
#[cfg(feature = "sqlx-postgres")]
pub use crate::backends::SqlxPostgresBackend;

// Macros
#[cfg(any(
    feature = "postgres",
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite"
))]
pub use crate::with_test_db;

// Environment utilities
pub use crate::env::*;
