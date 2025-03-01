#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlx-sqlite")]
pub mod sqlite;
#[cfg(any(feature = "sqlx-postgres", feature = "sqlx-mysql"))]
pub mod sqlx;

#[cfg(feature = "mysql")]
pub use mysql::MySqlBackend;
#[cfg(feature = "postgres")]
pub use postgres::PostgresBackend;
#[cfg(feature = "sqlx-sqlite")]
pub use sqlite::SqliteBackend;
#[cfg(feature = "sqlx-postgres")]
pub use sqlx::SqlxPostgresBackend;

// Aliases for backward compatibility with tests
#[cfg(feature = "sqlx-sqlite")]
pub use sqlite::SqliteBackend as SqlxSqliteBackend;
#[cfg(feature = "sqlx-mysql")]
pub use sqlx::SqlxMySqlBackend;
