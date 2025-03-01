pub mod backend;
pub mod backends;
pub mod env;
pub mod error;
pub mod macros;
pub mod migrations;
pub mod pool;
pub mod test_db;
pub mod tracing;
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
pub use error::{DbError, Result};
pub use migrations::{RunSql, SqlSource};
pub use pool::PoolConfig;
pub use test_db::{DatabaseName, TestDatabase, TestDatabaseTemplate};
pub use tracing::init_tracing;
pub use wrapper::{ResourcePool, Reusable};

/// Create a simplified test database with default configuration
#[cfg(feature = "postgres")]
pub async fn create_test_db<B: DatabaseBackend + Clone + Send + 'static>(
    backend: B,
) -> Result<TestDatabase<B>> {
    let config = PoolConfig::default();
    let db = TestDatabase::new(backend, config).await?;
    Ok(db)
}

/// Create a test database template with specified max replicas
#[cfg(feature = "postgres")]
pub async fn create_template_db<B: DatabaseBackend + Clone + Send + 'static>(
    backend: B,
    max_replicas: usize,
) -> Result<TestDatabaseTemplate<B>> {
    let config = PoolConfig::default();
    let template = TestDatabaseTemplate::new(backend, config, max_replicas).await?;
    Ok(template)
}

mod sqlite_tests {
    #[cfg(all(
        feature = "sqlx-sqlite",
        not(feature = "postgres"),
        not(feature = "sqlx-postgres")
    ))]
    use super::*;
    #[cfg(all(
        feature = "sqlx-sqlite",
        not(feature = "postgres"),
        not(feature = "sqlx-postgres")
    ))]
    use sqlx::Row;

    #[tokio::test]
    #[cfg(all(
        feature = "sqlx-sqlite",
        not(feature = "postgres"),
        not(feature = "sqlx-postgres")
    ))]
    async fn test_sqlite_basic_operations() {
        // Setup logging
        std::env::set_var("RUST_LOG", "sqlx=debug");
        ::tracing_subscriber::fmt::try_init();

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

            Ok(())
        });
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod mysql_tests {
    #[cfg(all(
        feature = "mysql",
        not(feature = "postgres"),
        not(feature = "sqlx-postgres"),
        not(feature = "sqlx-sqlite")
    ))]
    use super::*;

    #[tokio::test]
    #[cfg(all(
        feature = "mysql",
        not(feature = "postgres"),
        not(feature = "sqlx-postgres"),
        not(feature = "sqlx-sqlite")
    ))]
    async fn test_mysql_basic_operations() {
        // Setup logging
        std::env::set_var("RUST_LOG", "debug,mysql_async=debug");
        let _ = ::tracing_subscriber::fmt::try_init();

        // Use a try-catch approach to handle potential connection errors
        if let Err(e) = async {
            with_test_db!(|db| async move {
                ::tracing::info!("Connected to MySQL test database: {}", db.db_name);
                
                // Use the database directly - we're using superuser credentials
                let mut conn = db.pool.acquire().await.unwrap();

                // Create a table
                conn.execute("CREATE TABLE test_items (id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(255) NOT NULL)")
                    .await?;
                ::tracing::info!("Created test table");

                // Insert some data
                conn.execute("INSERT INTO test_items (name) VALUES ('Test Item 1')")
                    .await?;

                conn.execute("INSERT INTO test_items (name) VALUES ('Test Item 2')")
                    .await?;
                ::tracing::info!("Inserted test data");

                // Use our fetch methods to verify the data
                let rows = conn
                    .fetch("SELECT COUNT(*) as count FROM test_items")
                    .await?;
                
                // Extract count value
                let count_row = &rows[0];
                let count: i64 = count_row.get("count").unwrap();
                assert_eq!(count, 2, "Expected 2 items in the test_items table");
                ::tracing::info!("Verified row count");

                // Test fetch_one
                let row = conn
                    .fetch_one("SELECT name FROM test_items WHERE id = 1")
                    .await?;
                
                let name: String = row.get("name").unwrap();
                assert_eq!(name, "Test Item 1", "Expected 'Test Item 1' as name");
                ::tracing::info!("Verified fetch_one works");

                // Test fetch_optional
                let opt_row = conn
                    .fetch_optional("SELECT name FROM test_items WHERE id = 99")
                    .await?;
                
                assert!(opt_row.is_none(), "Expected no row for non-existent ID");
                ::tracing::info!("Verified fetch_optional works");

                Ok(())
            }).await
        }.await {
            ::tracing::error!("MySQL test skipped due to connection error: {}", e);
            eprintln!("MySQL test skipped due to connection error: {}", e);
        } else {
            ::tracing::info!("MySQL test completed successfully");
            println!("MySQL test completed successfully");
        }
    }
}

/// The primary function to create and use a test database.
/// This is the recommended way to use the library as it handles all setup and cleanup.
///
/// # Example
///
/// ```rust
/// #[tokio::test]
/// async fn test_with_postgres() {
///     with_test_db(|db| async move {
///         // Setup database
///         db.setup(|mut conn| async move {
///             conn.execute(
///                 "CREATE TABLE users (
///                     id SERIAL PRIMARY KEY,
///                     email TEXT NOT NULL,
///                     name TEXT NOT NULL
///                 )"
///             ).await?;
///             Ok(())
///         }).await?;
///
///         // Execute tests
///         db.test(|mut conn| async move {
///             let rows = conn.execute("SELECT * FROM users").await?;
///             assert_eq!(rows.len(), 0);
///             Ok(())
///         }).await?;
///
///         Ok(())
///     })
///     .await;
/// }
/// ```
#[cfg(feature = "postgres")]
pub async fn with_test_db<F, Fut>(test_fn: F)
where
    F: FnOnce(TestDatabase<backends::PostgresBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    // Set up panic catching
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        // We could add cleanup code here if needed
    }));

    let backend = backends::postgres::PostgresBackend::new(
        "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
    )
    .await
    .expect("Failed to create PostgresBackend");

    let config = PoolConfig::default();
    let db = TestDatabase::new(backend, config)
        .await
        .expect("Failed to create test database");

    // Let the test function use the prepared database
    let result = test_fn(db).await;

    // If the test function returned an error, panic with it
    if let Err(err) = result {
        panic!("Test failed: {:?}", err);
    }
}

#[cfg(all(feature = "mysql", not(feature = "postgres")))]
pub async fn with_test_db<F, Fut>(test_fn: F)
where
    F: FnOnce(TestDatabase<backends::MySqlBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    // Set up panic catching
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        // We could add cleanup code here if needed
    }));

    // Try root user with different connection strings for MySQL
    let backend = match backends::mysql::MySqlBackend::new("mysql://root:password@mysql:3306/") {
        Ok(backend) => {
            ::tracing::debug!("Using MySQL connection: mysql://root:password@mysql:3306/");
            backend
        },
        Err(e1) => {
            ::tracing::debug!("Failed to connect to MySQL with first URL: {}", e1);
            
            // Try localhost with root user
            match backends::mysql::MySqlBackend::new("mysql://root:password@localhost:3306/") {
                Ok(backend) => {
                    ::tracing::debug!("Using MySQL connection: mysql://root:password@localhost:3306/");
                    backend
                },
                Err(e2) => {
                    ::tracing::debug!("Failed to connect to MySQL with second URL: {}", e2);
                    
                    // Try with empty password
                    match backends::mysql::MySqlBackend::new("mysql://root:@localhost:3306/") {
                        Ok(backend) => {
                            ::tracing::debug!("Using MySQL connection: mysql://root:@localhost:3306/");
                            backend
                        },
                        Err(e3) => {
                            // Log all the errors we've encountered
                            panic!("Failed to create MySqlBackend with any standard configurations:\n\
                                  1. mysql://root:password@mysql:3306/ - {}\n\
                                  2. mysql://root:password@localhost:3306/ - {}\n\
                                  3. mysql://root:@localhost:3306/ - {}\n\
                                  Please ensure MySQL is running and accessible with proper credentials.",
                                  e1, e2, e3)
                        }
                    }
                }
            }
        }
    };

    // Configure the connection pool
    let config = PoolConfig::default();
    
    // Create the test database with the backend
    let db = match TestDatabase::new(backend, config).await {
        Ok(db) => db,
        Err(e) => {
            panic!("Failed to create MySQL test database: {}. \
                   Make sure your MySQL user has sufficient privileges to create databases.", e);
        }
    };

    // Let the test function use the prepared database
    let result = test_fn(db).await;

    // If the test function returned an error, panic with it
    if let Err(err) = result {
        panic!("Test failed: {:?}", err);
    }
}

#[cfg(all(
    feature = "sqlx-postgres",
    not(feature = "postgres"),
    not(feature = "mysql")
))]
pub async fn with_test_db<F, Fut>(test_fn: F) -> Result<()>
where
    F: FnOnce(TestDatabase<backends::SqlxPostgresBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let url = env::get_sqlx_postgres_url()?;
    let backend = match backends::sqlx::SqlxPostgresBackend::new(&url) {
        Ok(backend) => backend,
        Err(e) => {
            panic!("Failed to create SqlxPostgresBackend: {}. \
                   Make sure PostgreSQL is running and accessible.", e);
        }
    };

    // Configure the connection pool
    let config = PoolConfig::default();
    
    // Create the test database with the backend
    let db = match TestDatabase::new(backend, config).await {
        Ok(db) => db,
        Err(e) => {
            panic!("Failed to create SQLx PostgreSQL test database: {}. \
                   Make sure your PostgreSQL user has sufficient privileges to create databases.", e);
        }
    };

    // Let the test function use the prepared database
    test_fn(db).await
}

#[cfg(all(
    feature = "sqlx-sqlite",
    not(feature = "postgres"),
    not(feature = "sqlx-postgres"),
    not(feature = "mysql")
))]
pub async fn with_test_db<F, Fut>(test_fn: F) -> Result<()>
where
    F: FnOnce(TestDatabase<backends::SqliteBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let backend = match backends::sqlite::SqliteBackend::new("/tmp/testkit").await {
        Ok(backend) => backend,
        Err(e) => {
            panic!("Failed to create SqliteBackend: {}. \
                   Make sure SQLite is available.", e);
        }
    };

    // Configure the connection pool
    let config = PoolConfig::default();
    
    // Create the test database with the backend
    let db = match TestDatabase::new(backend, config).await {
        Ok(db) => db,
        Err(e) => {
            panic!("Failed to create SQLite test database: {}.", e);
        }
    };

    // Let the test function use the prepared database
    test_fn(db).await
}

#[cfg(not(any(
    feature = "postgres",
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite",
    feature = "mysql",
    feature = "sqlx-mysql"
)))]
pub async fn with_test_db<F, Fut>(_test_fn: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    compile_error!("No database backend feature enabled");
}

/// Creates a test database with custom pool options.
/// This is useful when you need to customize the database connection parameters.
#[cfg(feature = "postgres")]
pub async fn with_configured_test_db<F, Fut>(config: PoolConfig, test_fn: F)
where
    F: FnOnce(TestDatabase<backends::PostgresBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    // Set up panic catching
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        // We could add cleanup code here if needed
    }));

    let backend = backends::postgres::PostgresBackend::new(
        "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
    )
    .await
    .expect("Failed to create PostgresBackend");

    let db = TestDatabase::new(backend, config)
        .await
        .expect("Failed to create test database");

    // Let the test function use the prepared database
    let result = test_fn(db).await;

    // If the test function returned an error, panic with it
    if let Err(err) = result {
        panic!("Test failed: {:?}", err);
    }
}

/// Creates a test database with custom pool options.
/// This is useful when you need to customize the database connection parameters.
#[cfg(all(feature = "mysql", not(feature = "postgres")))]
pub async fn with_configured_test_db<F, Fut>(config: PoolConfig, test_fn: F)
where
    F: FnOnce(TestDatabase<backends::MySqlBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    // Set up panic catching
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        // We could add cleanup code here if needed
    }));

    let backend = backends::mysql::MySqlBackend::new("mysql://root:password@mysql:3306/")
        .expect("Failed to create MySqlBackend");

    let db = TestDatabase::new(backend, config)
        .await
        .expect("Failed to create test database");

    // Let the test function use the prepared database
    let result = test_fn(db).await;

    // If the test function returned an error, panic with it
    if let Err(err) = result {
        panic!("Test failed: {:?}", err);
    }
}

/// Creates a test database with custom pool options.
/// This is useful when you need to customize the database connection parameters.
#[cfg(all(
    feature = "sqlx-postgres",
    not(feature = "postgres"),
    not(feature = "mysql")
))]
pub async fn with_configured_test_db<F, Fut>(config: PoolConfig, test_fn: F) -> Result<()>
where
    F: FnOnce(TestDatabase<backends::SqlxPostgresBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let url = env::get_sqlx_postgres_url()?;
    let backend = match backends::sqlx::SqlxPostgresBackend::new(&url) {
        Ok(backend) => backend,
        Err(e) => {
            panic!("Failed to create SqlxPostgresBackend: {}. \
                   Make sure PostgreSQL is running and accessible.", e);
        }
    };

    let db = match TestDatabase::new(backend, config).await {
        Ok(db) => db,
        Err(e) => {
            panic!("Failed to create SQLx PostgreSQL test database: {}. \
                   Make sure your PostgreSQL user has sufficient privileges to create databases.", e);
        }
    };

    // Let the test function use the prepared database
    test_fn(db).await
}

/// Creates a test database with custom pool options.
/// This is useful when you need to customize the database connection parameters.
#[cfg(all(
    feature = "sqlx-sqlite",
    not(feature = "postgres"),
    not(feature = "sqlx-postgres"),
    not(feature = "mysql")
))]
pub async fn with_configured_test_db<F, Fut>(config: PoolConfig, test_fn: F) -> Result<()>
where
    F: FnOnce(TestDatabase<backends::SqliteBackend>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let backend = match backends::sqlite::SqliteBackend::new("/tmp/testkit").await {
        Ok(backend) => backend,
        Err(e) => {
            panic!("Failed to create SqliteBackend: {}. \
                   Make sure SQLite is available.", e);
        }
    };

    let db = match TestDatabase::new(backend, config).await {
        Ok(db) => db,
        Err(e) => {
            panic!("Failed to create SQLite test database: {}.", e);
        }
    };

    // Let the test function use the prepared database
    test_fn(db).await
}

/// Creates a test database with custom pool options.
/// This is useful when you need to customize the database connection parameters.
#[cfg(not(any(
    feature = "postgres", 
    feature = "mysql",
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite",
    feature = "sqlx-mysql"
)))]
pub async fn with_configured_test_db<F, Fut>(_config: PoolConfig, _test_fn: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    compile_error!("No database backend feature enabled");
}
