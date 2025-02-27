pub use crate::backend::*;
pub use crate::pool::*;

#[cfg(feature = "mysql")]
pub use crate::backends::MySqlBackend;
#[cfg(feature = "postgres")]
pub use crate::backends::PostgresBackend;
#[cfg(feature = "sqlx-postgres")]
pub use crate::backends::SqlxPostgresBackend;
pub use crate::env::*;
pub use crate::template::{DatabaseName, DatabaseTemplate, ImmutableDatabase};
pub use crate::with_test_db;
pub use crate::SqlSource;
pub use crate::TestDatabase;

// Re-exports
#[cfg(any(
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite",
    feature = "sqlx-mysql"
))]
pub use sqlx::Executor;
