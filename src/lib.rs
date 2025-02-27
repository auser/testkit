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

/// Example of using the test db macro without explicit type annotations
#[doc(hidden)]
#[cfg(test)]
#[cfg(feature = "postgres")]
pub async fn example_without_type_annotations() {
    with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |_conn| async move {
            // Setup code goes here
            Ok(()) as crate::error::Result<()>
        },
        |db| async move {
            // Type of db is inferred as TestDatabaseTemplate<PostgresBackend>
            let test_db = db.create_test_database().await.unwrap();
            let mut conn = test_db.pool.acquire().await.unwrap();
            conn.execute("SELECT 1").await.unwrap();
            Ok(()) as crate::error::Result<()>
        }
    );
}

/// Example of using the test db macro with explicit type annotations
#[doc(hidden)]
#[cfg(test)]
#[cfg(feature = "postgres")]
pub async fn example_with_type_annotations() {
    let _ = with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |_conn| async move {
            // Setup code goes here
            Ok(()) as crate::error::Result<()>
        },
        |db: TestDatabaseTemplate<PostgresBackend>| async move {
            // Explicitly typed as TestDatabaseTemplate<PostgresBackend>
            let test_db = db.create_test_database().await.unwrap();
            let mut conn = test_db.pool.acquire().await.unwrap();
            conn.execute("SELECT 1").await.unwrap();
            Ok(()) as crate::error::Result<()>
        }
    );
}

/// Example of using the test db macro with custom URL
#[doc(hidden)]
#[cfg(test)]
#[cfg(feature = "postgres")]
pub async fn example_with_custom_url() {
    let _ = with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |db| async move {
            // Type inferred, custom URL specified
            let test_db = db.create_test_database().await.unwrap();
            let mut conn = test_db.pool.acquire().await.unwrap();
            conn.execute("SELECT 1").await.unwrap();
            Ok(()) as crate::error::Result<()>
        }
    );
}

/// Example of using the test db macro with PostgreSQL backend and default URL
#[doc(hidden)]
#[cfg(test)]
#[cfg(all(feature = "postgres", not(feature = "sqlx-backend")))]
pub async fn example_with_pg_default_url() {
    let _ = with_test_db!(|db: TestDatabaseTemplate<PostgresBackend>| async move {
        // Uses default URL
        let test_db = db.create_test_database().await.unwrap();
        #[allow(unused_variables)]
        let conn = test_db.pool.acquire().await.unwrap();
        // test code here
        Ok(()) as crate::error::Result<()>
    });
}

/// Example of using the test db macro with PostgreSQL backend and explicit URL
#[doc(hidden)]
#[cfg(test)]
#[cfg(all(feature = "postgres", not(feature = "sqlx-backend")))]
pub async fn example_with_pg_and_url() {
    let _ = with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |_conn| async move {
            // Setup code goes here
            Ok(()) as crate::error::Result<()>
        },
        |db: TestDatabaseTemplate<PostgresBackend>| async move {
            // Explicitly typed as TestDatabaseTemplate<PostgresBackend>
            let test_db = db.create_test_database().await.unwrap();
            #[allow(unused_variables)]
            let conn = test_db.pool.acquire().await.unwrap();
            // test code here
            Ok(()) as crate::error::Result<()>
        }
    );
}

/// Example of using the test db macro with SQLx PostgreSQL backend
#[doc(hidden)]
#[cfg(test)]
#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
pub async fn example_with_sqlx_postgres() {
    with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres",
        |_conn| async move {
            // Setup code goes here
            Ok(()) as crate::error::Result<()>
        },
        |db: TestDatabaseTemplate<SqlxPostgresBackend>| async move {
            // Explicitly typed as TestDatabaseTemplate<SqlxPostgresBackend>
            let test_db = db.create_test_database().await.unwrap();
            let mut conn = test_db.pool.acquire().await.unwrap();
            sqlx::query("SELECT 1").execute(&mut conn).await.unwrap();
            Ok(()) as crate::error::Result<()>
        }
    );
}

/// Example of using the test db macro with SQLx PostgreSQL backend and default URL
#[doc(hidden)]
#[cfg(test)]
#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
pub async fn example_with_sqlx_postgres_default_url() {
    let _ = with_test_db!(|db: TestDatabaseTemplate<SqlxPostgresBackend>| async move {
        // Uses default URL
        let test_db = db.create_test_database().await.unwrap();
        let mut conn = test_db.pool.acquire().await.unwrap();
        sqlx::query("SELECT 1").execute(&mut conn).await.unwrap();
        Ok(()) as crate::error::Result<()>
    });
}
