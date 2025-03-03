//! PostgreSQL backend for testkit-core
//!
//! This library provides implementations of the DatabaseBackend trait for PostgreSQL
//! databases using the sqlx library. It allows for creating and managing test
//! databases for integration testing.

use async_trait::async_trait;
use sqlx::{
    ConnectOptions, Pool, Postgres,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{fmt::Debug, str::FromStr};
use testkit_core::{
    DatabaseBackend, DatabaseConfig as CoreDatabaseConfig, DatabaseName as CoreDatabaseName,
    DatabasePool, TestDatabaseConnection,
};
use url::Url;

mod error;
pub use error::Error;

// Re-export with aliases to avoid name conflicts
pub type DatabaseConfig = CoreDatabaseConfig;
pub type DatabaseName = CoreDatabaseName;

/// A PostgreSQL connection implementation
pub struct PostgresConnection {
    // The underlying connection pool that can be used directly
    pub pool: Pool<Postgres>,
    connection_string: String,
}

impl TestDatabaseConnection for PostgresConnection {
    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A connection pool for PostgreSQL
#[derive(Clone)]
pub struct PostgresPool {
    pool: Pool<Postgres>,
    connection_string: String,
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Connection = PostgresConnection;
    type Error = Error;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        Ok(PostgresConnection {
            pool: self.pool.clone(),
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
        // Nothing to do here as the connection will be dropped
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A PostgreSQL backend for database tests
#[derive(Debug, Clone)]
pub struct PostgresBackend;

#[async_trait]
impl DatabaseBackend for PostgresBackend {
    type Connection = PostgresConnection;
    type Pool = PostgresPool;
    type Error = Error;

    async fn create_pool(
        &self,
        name: &CoreDatabaseName,
        _config: &CoreDatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        let connection_string = self.connection_string(name);

        let options = PgConnectOptions::from_str(&connection_string)?
            .clone()
            .log_statements(tracing::log::LevelFilter::Debug)
            .log_slow_statements(
                tracing::log::LevelFilter::Warn,
                std::time::Duration::from_secs(1),
            );

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        Ok(PostgresPool {
            pool,
            connection_string,
        })
    }

    async fn create_database(
        &self,
        pool: &Self::Pool,
        name: &CoreDatabaseName,
    ) -> Result<(), Self::Error> {
        let db_name = name.as_str();
        // Make it safe for SQL by escaping quotes
        let safe_db_name = db_name.replace('\'', "''");

        let query = format!(
            "CREATE DATABASE \"{}\" ENCODING 'UTF8' LC_COLLATE 'en_US.utf8' LC_CTYPE 'en_US.utf8' TEMPLATE template0",
            safe_db_name
        );

        // Using the pool directly as an executor
        sqlx::query(&query).execute(&pool.pool).await?;

        Ok(())
    }

    fn drop_database(&self, name: &CoreDatabaseName) -> Result<(), Self::Error> {
        let db_name = name.as_str();
        // Make it safe for SQL
        let safe_db_name = db_name.replace('\'', "''");

        let query = format!("DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)", safe_db_name);

        // We need to execute this synchronously in Drop
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        rt.block_on(async {
            let config =
                CoreDatabaseConfig::from_env().map_err(|e| Error::Generic(e.to_string()))?;
            let admin_url = config.admin_url;

            let options = PgConnectOptions::from_str(&admin_url)?
                .clone()
                .log_statements(tracing::log::LevelFilter::Debug);

            let conn = sqlx::postgres::PgPool::connect_with(options).await?;
            sqlx::query(&query).execute(&conn).await?;

            Ok::<_, Error>(())
        })?;

        Ok(())
    }

    fn connection_string(&self, name: &CoreDatabaseName) -> String {
        // We need to get the admin connection string and replace the database name
        let config = CoreDatabaseConfig::from_env().unwrap_or_else(|_| {
            panic!("Failed to load database configuration from environment");
        });

        // Parse URL and set the database name
        let admin_url = &config.admin_url;

        // If admin_url is already in the format of "postgres://user:pass@host:port/dbname",
        // we can parse it as a URL and replace the database name
        if admin_url.starts_with("postgres://") || admin_url.starts_with("postgresql://") {
            if let Ok(mut url) = Url::parse(admin_url) {
                // Replace the database name in the path
                url.set_path(&format!("/{}", name.as_str()));
                return url.to_string();
            }
        }

        // If URL parsing fails or it's not a standard postgres URL,
        // try to construct a valid connection string

        // Extract host, port, user, password from DATABASE_URL environment variable
        // Format: postgres://postgres:postgres@postgres:5432/postgres
        let parts: Vec<&str> = admin_url.split('@').collect();
        if parts.len() == 2 {
            let auth_parts: Vec<&str> = parts[0].split("://").collect();
            if auth_parts.len() == 2 {
                let user_pass: Vec<&str> = auth_parts[1].split(':').collect();
                let host_port_db: Vec<&str> = parts[1].split('/').collect();

                if user_pass.len() == 2 && host_port_db.len() >= 1 {
                    let host_port: Vec<&str> = host_port_db[0].split(':').collect();

                    if host_port.len() == 2 {
                        // We have all the parts to construct a valid connection string
                        return format!(
                            "postgres://{}:{}@{}:{}/{}",
                            user_pass[0],
                            user_pass[1],
                            host_port[0],
                            host_port[1],
                            name.as_str()
                        );
                    }
                }
            }
        }

        // Last resort: try to extract just the database name from the URL and replace it
        if let Some(last_slash) = admin_url.rfind('/') {
            format!("{}/{}", &admin_url[..last_slash], name.as_str())
        } else {
            // If no slash, append the database name
            format!("{}/{}", admin_url, name.as_str())
        }
    }
}

/// Convenience function to create a PostgreSQL backend
pub fn postgres() -> PostgresBackend {
    PostgresBackend
}

// Re-export key testkit-core types for convenience
pub use testkit_core::{
    IntoTransaction, Transaction, TransactionManager, with_database, with_transaction,
};

// PostgreSQL transaction type - we use '_ for the lifetime to work with sqlx Executor trait
pub type PgTransaction = sqlx::Transaction<'static, Postgres>;

// Implement TransactionManager for TestDatabaseInstance with PostgresBackend
#[async_trait]
impl TransactionManager<PgTransaction, PostgresConnection>
    for testkit_core::TestDatabaseInstance<PostgresBackend>
{
    type Error = Error;

    async fn begin_transaction(&mut self) -> Result<PgTransaction, Self::Error> {
        let conn = self.acquire_connection().await?;
        // Use a Box::leak to ensure the lifetime is 'static
        let pool = Box::leak(Box::new(conn.pool.clone()));
        let tx = pool.begin().await?;
        Ok(tx)
    }

    async fn commit_transaction(tx: &mut PgTransaction) -> Result<(), Self::Error> {
        // Use a temporary replacement to avoid ownership issues
        let temp = std::mem::replace(tx, unsafe { std::mem::zeroed() });
        temp.commit().await?;
        Ok(())
    }

    async fn rollback_transaction(tx: &mut PgTransaction) -> Result<(), Self::Error> {
        // Use a temporary replacement to avoid ownership issues
        let temp = std::mem::replace(tx, unsafe { std::mem::zeroed() });
        temp.rollback().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testkit_core::TestDatabaseInstance;
    use tokio::runtime::Runtime;

    async fn setup_test_database() -> Result<TestDatabaseInstance<PostgresBackend>, Error> {
        let _ = dotenvy::dotenv();
        let backend = PostgresBackend;
        let config = CoreDatabaseConfig::from_env().unwrap();
        let instance = TestDatabaseInstance::new(backend, config).await?;
        Ok(instance)
    }

    #[test]
    fn test_create_database() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let result = setup_test_database().await;
            assert!(
                result.is_ok(),
                "Failed to create test database: {:?}",
                result.err()
            );

            // Database is dropped automatically when instance goes out of scope
            let instance = result.unwrap();
            let name = instance.name().clone();

            // Test that we can get a connection
            let conn = instance.acquire_connection().await;
            assert!(
                conn.is_ok(),
                "Failed to acquire connection: {:?}",
                conn.err()
            );

            // Explicitly drop the database
            drop(instance);

            // Check that database was dropped
            let backend = PostgresBackend;
            let config = CoreDatabaseConfig::from_env().unwrap();
            let admin_pool = backend
                .create_pool(&CoreDatabaseName::new(Some("admin")), &config)
                .await
                .unwrap();

            // Try to connect to the dropped database - should fail
            let db_url = backend.connection_string(&name);
            let conn_result = sqlx::postgres::PgPool::connect(&db_url).await;
            assert!(conn_result.is_err(), "Database was not dropped");
        });
    }
}
