// Re-exports of the most commonly used items for convenience
pub use crate::backend::{Connection, DatabaseBackend, DatabasePool};
pub use crate::error::{PoolError, Result};
pub use crate::migrations::{RunSql, SqlSource};
pub use crate::pool::PoolConfig;
pub use crate::test_db::{DatabaseName, TestDatabase, TestDatabaseTemplate};
pub use crate::wrapper::{ResourcePool, Reusable};

// Feature-specific backend exports
#[cfg(feature = "mysql")]
pub use crate::backends::MySqlBackend;
#[cfg(feature = "postgres")]
pub use crate::backends::PostgresBackend;
#[cfg(feature = "sqlx-postgres")]
pub use crate::backends::SqlxPostgresBackend;

// Macros
pub use crate::with_test_db;

// Environment utilities
pub use crate::env::*;
