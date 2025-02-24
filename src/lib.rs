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
