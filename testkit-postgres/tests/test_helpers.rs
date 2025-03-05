#![allow(unused_imports, dead_code)]

use std::future::Future;
use std::pin::Pin;
use testkit_core::{DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection};
use testkit_postgres::PostgresError;

/// Test helper to create a test configuration with host set to postgres
pub fn test_config() -> DatabaseConfig {
    // Use postgres instead of postgres hostname
    let admin_url = "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";
    let user_url = "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable";
    DatabaseConfig::new(admin_url, user_url)
}

/// Test helper to gracefully skip tests when PostgreSQL isn't available
pub fn is_connection_error(err: &PostgresError) -> bool {
    err.to_string().contains("connection refused")
        || err.to_string().contains("timeout")
        || err.to_string().contains("does not exist")
}

/// Helper function to wrap a Future for boxed API tests
pub fn boxed_future<T, F, Fut>(
    f: F,
) -> impl FnOnce(T) -> Pin<Box<dyn Future<Output = Result<(), PostgresError>> + Send>>
where
    F: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), PostgresError>> + Send + 'static,
    T: Send + 'static,
{
    move |conn| Box::pin(f(conn))
}

#[cfg(feature = "postgres")]
pub mod postgres_helpers {
    use super::*;
    use testkit_postgres::{PostgresBackend, postgres_backend_with_config};

    pub async fn create_test_backend() -> Result<PostgresBackend, PostgresError> {
        let config = test_config();
        postgres_backend_with_config(config).await
    }
}

#[cfg(feature = "sqlx")]
pub mod sqlx_helpers {
    use super::*;
    use testkit_postgres::{SqlxPostgresBackend, sqlx_postgres_backend_with_config};

    pub async fn create_test_backend() -> Result<SqlxPostgresBackend, PostgresError> {
        let config = test_config();
        sqlx_postgres_backend_with_config(config).await
    }
}
