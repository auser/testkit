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
#[cfg(any(
    feature = "postgres",
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite"
))]
pub async fn create_test_db<B: DatabaseBackend + Clone + Send + 'static>(
    backend: B,
) -> Result<TestDatabase<B>> {
    TestDatabase::new(backend, PoolConfig::default()).await
}

/// Create a template database with default configuration
#[cfg(any(
    feature = "postgres",
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite"
))]
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
            let conn = test_db.pool.acquire().await.unwrap();
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
            let conn = test_db.pool.acquire().await.unwrap();
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
            let conn = test_db.pool.acquire().await.unwrap();
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
        let mut conn = test_db.pool.acquire().await.unwrap();

        // Create a sample table
        conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .await
            .unwrap();

        // Test code here
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
    #[cfg(feature = "sqlx-postgres")]
    use crate::backends::sqlx::SqlxPostgresBackend;

    #[cfg(feature = "sqlx-postgres")]
    with_test_db!(
        "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
        |_conn| async move {
            // Setup code goes here
            Ok(()) as crate::error::Result<()>
        },
        |db: TestDatabaseTemplate<SqlxPostgresBackend>| async move {
            // Explicitly typed as TestDatabaseTemplate<SqlxPostgresBackend>
            let test_db = db.create_test_database().await.unwrap();
            let conn = test_db.pool.acquire().await.unwrap();
            sqlx::query("SELECT 1").execute(&conn.pool).await.unwrap();
            Ok(()) as crate::error::Result<()>
        }
    );
}

/// Example of using the test db macro with SQLx PostgreSQL backend and default URL
#[doc(hidden)]
#[cfg(test)]
#[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
pub async fn example_with_sqlx_postgres_default_url() {
    with_test_db!(|db| async move {
        // Uses default URL
        let test_db = db.create_test_database().await.unwrap();
        let conn = test_db.pool.acquire().await.unwrap();

        // Use the SQLx pool directly instead of our custom connection
        sqlx::query("SELECT 1").execute(&conn.pool).await.unwrap();

        Ok(()) as crate::error::Result<()>
    });
}

/// Example of using the test db macro with SQLite backend
#[doc(hidden)]
#[cfg(test)]
#[cfg(feature = "sqlx-sqlite")]
pub async fn example_with_sqlite() {
    with_test_db!(|db| async move {
        // Uses default URL
        let test_db = db.create_test_database().await.unwrap();
        let mut conn = test_db.pool.acquire().await.unwrap();

        // Create a sample table
        conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .await
            .unwrap();

        // Insert some test data
        conn.execute("INSERT INTO users (name) VALUES ('Test User')")
            .await
            .unwrap();

        // Test code here
        Ok(()) as crate::error::Result<()>
    });
}

/// Example of using the test db macro with SQLite backend and custom path
#[doc(hidden)]
#[cfg(test)]
#[cfg(feature = "sqlx-sqlite")]
pub async fn example_with_sqlite_custom_path() {
    let _ = with_test_db!("/tmp/test_sqlite", |db| async move {
        // Uses custom path
        let test_db = db.create_test_database().await.unwrap();
        let mut conn = test_db.pool.acquire().await.unwrap();

        // Create a sample table
        conn.execute(
            "CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT NOT NULL, price REAL)",
        )
        .await
        .unwrap();

        // Test code here
        Ok(()) as crate::error::Result<()>
    });
}

#[cfg(test)]
mod sqlite_tests {
    #[cfg(feature = "sqlx-sqlite")]
    use super::*;
    #[cfg(feature = "sqlx-sqlite")]
    use sqlx::Row;

    #[tokio::test]
    #[cfg(feature = "sqlx-sqlite")]
    async fn test_sqlite_basic_operations() {
        // Setup logging
        std::env::set_var("RUST_LOG", "sqlx=debug");
        let _ = tracing_subscriber::fmt::try_init();

        with_test_db!(|db| async move {
            // Create a test database
            let test_db = db.create_test_database().await.unwrap();

            // Get a connection
            let mut conn = test_db.pool.acquire().await.unwrap();

            // Create a table
            conn.execute("CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
                .await
                .unwrap();

            // Insert some data
            conn.execute("INSERT INTO test_items (name) VALUES ('Test Item 1')")
                .await
                .unwrap();

            conn.execute("INSERT INTO test_items (name) VALUES ('Test Item 2')")
                .await
                .unwrap();

            // Query the data using raw SQL
            let result = sqlx::query("SELECT COUNT(*) as count FROM test_items")
                .fetch_one(&conn.pool)
                .await
                .unwrap();

            let count: i64 = result.get(0);
            assert_eq!(count, 2, "Expected 2 items in the test_items table");

            Ok(()) as crate::error::Result<()>
        });
    }
}
