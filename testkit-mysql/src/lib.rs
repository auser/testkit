mod error;
mod mysql_async;
mod sqlx_mysql;

#[cfg(feature = "with-mysql-async")]
pub use mysql_async::{MySqlBackend, MySqlConnection, MySqlPool, mysql_backend_with_config};

#[cfg(feature = "with-sqlx")]
pub use sqlx_mysql::{
    SqlxMySqlBackend, SqlxMySqlConnection, SqlxMySqlPool, sqlx_mysql_backend_with_config,
};

pub use error::MySqlError;

// Re-export core types from testkit-core
pub use testkit_core::{DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool};
