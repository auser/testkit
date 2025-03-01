#[allow(unused)]
use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{DbError, Result},
    pool::PoolConfig,
    test_db::TestDatabaseTemplate,
};

#[cfg(feature = "mysql")]
#[allow(unused)]
use crate::{backends::mysql::MySqlBackend, env::get_mysql_url};

#[cfg(feature = "sqlx-postgres")]
#[allow(unused)]
use crate::{backends::sqlx::SqlxPostgresBackend, env::get_sqlx_postgres_url};

#[cfg(feature = "postgres")]
#[allow(unused)]
use crate::{backends::postgres::PostgresBackend, env::get_postgres_url};

#[cfg(feature = "sqlx-sqlite")]
#[allow(unused)]
use crate::{backends::sqlite::SqliteBackend, env::get_sqlite_url};

/// Creates a new PostgreSQL test database and executes the provided test function.
///
/// This macro handles the creation of a temporary database, executes the test function,
/// and ensures proper cleanup after the test completes.
///
/// # Arguments
///
/// * `$f` - A function that takes a [`TestDatabase`] and returns a future
///
/// # Example
///
/// ```rust
/// #[tokio::test]
/// async fn test_users() {
///     with_test_db!(|db| async move {
///         db.setup(|mut conn| async move {
///             conn.execute("CREATE TABLE users (id SERIAL PRIMARY KEY)").await?;
///             Ok(())
///         }).await?;
///         Ok(())
///     }).await?;
/// }
/// ```
#[cfg(any(
    feature = "postgres",
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite",
    feature = "mysql",
    feature = "sqlx-mysql"
))]
#[macro_export]
macro_rules! with_test_db {
    // Version with URL and no type annotation - for easy use
    // This variant auto-awaits the future and returns a Result that can be used with ?
    ($url:expr, |$db:ident| $test:expr) => {{
        async {
            // Import DatabaseBackend trait for backend methods
            use $crate::backend::DatabaseBackend;

            // Create backend for the URL based on feature
            #[cfg(all(feature = "postgres", not(feature = "sqlx-postgres"), not(feature = "mysql")))]
            #[allow(unused_variables)]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(feature = "sqlx-postgres")]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            #[cfg(feature = "mysql")]
            #[allow(unused_variables)]
            let backend = $crate::backends::mysql::MySqlBackend::new($url)
                .expect("Failed to create database backend");

            #[cfg(feature = "sqlx-mysql")]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlx::SqlxMySqlBackend::new($url)
                .expect("Failed to create database backend");

            #[cfg(all(
                feature = "sqlx-sqlite",
                not(feature = "postgres"),
                not(feature = "sqlx-postgres"),
                not(feature = "mysql")
            ))]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlite::SqliteBackend::new($url)
                .await
                .expect("Failed to create database backend");

            // Create test database
            let template =
                $crate::TestDatabaseTemplate::new(backend, $crate::pool::PoolConfig::default(), 5)
                    .await
                    .expect("Failed to create test database template");

            // Get a database from the template
            let $db = template
                .create_test_database()
                .await
                .expect("Failed to create test database from template");

            // Save the backend and name for explicit cleanup if needed
            let backend_copy = $db.backend().clone();
            let db_name = $db.name().clone();

            // Execute the test function and catch any panics to ensure cleanup
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
                // Wrapper to enforce proper type inference - transforms the test future's result into a well-defined type
                async fn run_with_type_inference<T, F>(
                    fut: F,
                ) -> $crate::error::Result<T>
                where
                    F: std::future::Future<Output = $crate::error::Result<T>>,
                {
                    fut.await
                }

                // This forces type inference to work correctly across all backends
                run_with_type_inference($test).await
            }));

            // Handle the result - if it's a panic, we need to explicitly drop the database
            match result {
                Ok(future) => {
                    // Run the future and handle errors
                    let test_result = future.await;
                    if let Err(e) = test_result {
                        ::tracing::error!("Test failed: {:?}", e);
                        // Explicitly drop the database before panicking
                        if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                            ::tracing::warn!("Warning: failed to drop database: {}", drop_err);
                        }
                        panic!("Test failed: {:?}", e);
                    }

                    // Return success
                    Ok::<(), $crate::error::DbError>(())
                }
                Err(e) => {
                    // Explicitly drop the database before re-panicking
                    ::tracing::error!("Test panicked, ensuring database cleanup");
                    if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                        ::tracing::error!(
                            "Warning: failed to drop database during panic recovery: {}",
                            drop_err
                        );
                    }

                    // Re-panic with the original error
                    std::panic::resume_unwind(e);
                }
            }
        }
    }};

    // No URL provided - use default URLs based on features
    // Postgres version
    (|$db:ident| $test:expr) => {
        // We use compile-time feature detection to determine which version to use
        // but execute the expression only once
        {
            #[cfg(feature = "postgres")]
            {
                $crate::with_test_db!(
                    "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
                    |$db| $test
                )
            }

            #[cfg(all(feature = "mysql", not(feature = "postgres")))]
            {
                $crate::with_test_db!(
                    "mysql://root:password@mysql:3306/",
                    |$db| $test
                )
            }

            #[cfg(all(feature = "sqlx-postgres", not(feature = "postgres"), not(feature = "mysql")))]
            {
                $crate::with_test_db!(
                    "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
                    |$db| $test
                )
            }

            #[cfg(all(
                feature = "sqlx-sqlite",
                not(feature = "postgres"),
                not(feature = "sqlx-postgres"),
                not(feature = "mysql")
            ))]
            {
                $crate::with_test_db!("sqlite_testdb", |$db| $test)
            }

            // Default empty block for when no features match
            #[cfg(not(any(
                feature = "postgres",
                feature = "sqlx-postgres",
                feature = "sqlx-sqlite",
                feature = "mysql",
                feature = "sqlx-mysql"
            )))]
            {
                compile_error!("No database backend feature enabled")
            }
        }
    };

    // Version with setup and test functions using async move blocks
    ($url:expr, |$setup_param:ident| $setup_block:expr, |$test_param:ident| $test_block:expr) => {
        async {
            // Import DatabaseBackend trait for backend methods
            use $crate::backend::DatabaseBackend;

            // Create backend for the URL based on feature
            #[cfg(all(feature = "postgres", not(feature = "sqlx-postgres"), not(feature = "mysql")))]
            #[allow(unused_variables)]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(feature = "sqlx-postgres")]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            #[cfg(feature = "mysql")]
            #[allow(unused_variables)]
            let backend = $crate::backends::mysql::MySqlBackend::new($url)
                .expect("Failed to create database backend");

            #[cfg(all(
                feature = "sqlx-sqlite",
                not(feature = "postgres"),
                not(feature = "sqlx-postgres"),
                not(feature = "mysql")
            ))]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlite::SqliteBackend::new($url)
                .await
                .expect("Failed to create database backend");

            // Create test database template
            let template =
                $crate::TestDatabaseTemplate::new(backend, $crate::pool::PoolConfig::default(), 5)
                    .await
                    .expect("Failed to create test database template");

            // Save the backend and template name for cleanup
            let template_backend = template.backend().clone();
            let template_name = template.name().clone();

            // Initialize the template with setup operations in a panic-safe way
            let setup_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
                template
                    .initialize(|mut conn| async move {
                        let $setup_param = &mut conn;
                        $setup_block.await
                    })
                    .await
            }));

            match setup_result {
                Ok(future) => {
                    if let Err(e) = future.await {
                        error!("Setup failed: {:?}", e);
                        // Explicitly drop the template database
                        if let Err(drop_err) = template_backend.drop_database(&template_name).await
                        {
                            warn!("Warning: failed to drop template database: {}", drop_err);
                        }
                        panic!("Setup failed: {:?}", e);
                    }
                }
                Err(e) => {
                    // Explicitly drop the template database
                    error!("Setup panicked, ensuring database cleanup");
                    if let Err(drop_err) = template_backend.drop_database(&template_name).await {
                        error!(
                            "Warning: failed to drop template database during panic recovery: {}",
                            drop_err
                        );
                    }
                    std::panic::resume_unwind(e);
                }
            }

            // Run the test with template in a panic-safe way
            let $test_param = template;
            // Store backend and name for explicit cleanup
            let backend_copy = $test_param.backend().clone();
            let db_name = $test_param.name().clone();

            let test_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
                let future = $test_block;
                future.await
            }));

            match test_result {
                Ok(future) => {
                    if let Err(e) = future.await {
                        error!("Test failed: {:?}", e);
                        // Explicitly drop the database
                        if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                            warn!("Warning: failed to drop database: {}", drop_err);
                        }
                        panic!("Test failed: {:?}", e);
                    }
                }
                Err(e) => {
                    // Explicitly drop the database
                    error!("Test panicked, ensuring database cleanup");
                    if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                        error!(
                            "Warning: failed to drop database during panic recovery: {}",
                            drop_err
                        );
                    }
                    std::panic::resume_unwind(e);
                }
            }

            // Return unit type so this doesn't need to be annotated
            Ok::<(), $crate::error::DbError>(())
        }
    };

    // Remaining (less commonly used) variants with explicit type annotations
    // For advanced/specialized use cases

    // Version with URL and type annotation
    ($url:expr, |$db:ident: $ty:ty| $test:expr) => {{
        async {
            // Import DatabaseBackend trait for backend methods
            use $crate::backend::DatabaseBackend;

            // Create backend for the URL based on feature
            #[cfg(all(feature = "postgres", not(feature = "sqlx-postgres"), not(feature = "mysql")))]
            #[allow(unused_variables)]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(feature = "sqlx-postgres")]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            #[cfg(feature = "mysql")]
            #[allow(unused_variables)]
            let backend = $crate::backends::mysql::MySqlBackend::new($url)
                .expect("Failed to create database backend");

            #[cfg(all(
                feature = "sqlx-sqlite",
                not(feature = "postgres"),
                not(feature = "sqlx-postgres"),
                not(feature = "mysql")
            ))]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlite::SqliteBackend::new($url)
                .await
                .expect("Failed to create database backend");

            // Create test database template
            let template =
                $crate::TestDatabaseTemplate::new(backend, $crate::pool::PoolConfig::default(), 5)
                    .await
                    .expect("Failed to create test database template");

            // Get a database from the template with explicit type
            let $db: $ty = template
                .create_test_database()
                .await
                .expect("Failed to create test database from template");

            // Save backend and name for explicit cleanup if needed
            let backend_copy = $db.backend().clone();
            let db_name = $db.name().clone();

            // Execute the test function and catch any panics to ensure cleanup
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
                let future = $test;
                future.await
            }));

            // Handle the result - if it's a panic, we need to explicitly drop the database
            match result {
                Ok(future) => {
                    // Run the future and handle errors
                    let test_result = future.await;
                    if let Err(e) = test_result {
                        error!("Test failed: {:?}", e);
                        // Explicitly drop the database before panicking
                        if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                            warn!("Warning: failed to drop database: {}", drop_err);
                        }
                        panic!("Test failed: {:?}", e);
                    }

                    // Return success
                    Ok::<(), $crate::error::DbError>(())
                }
                Err(e) => {
                    // Explicitly drop the database before re-panicking
                    error!("Test panicked, ensuring database cleanup");
                    if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                        error!(
                            "Warning: failed to drop database during panic recovery: {}",
                            drop_err
                        );
                    }

                    // Re-panic with the original error
                    std::panic::resume_unwind(e);
                }
            }
        }
    }};

    // Version with type annotation
    (|$db:ident: $ty:ty| $test:expr) => {
        // We use compile-time feature detection to determine which version to use
        {
            #[cfg(feature = "postgres")]
            {
                $crate::with_test_db!(
                    "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
                    |$db: $ty| $test
                )
            }

            #[cfg(all(feature = "mysql", not(feature = "postgres")))]
            {
                $crate::with_test_db!(
                    "mysql://root:password@mysql:3306/",
                    |$db: $ty| $test
                )
            }

            #[cfg(all(feature = "sqlx-postgres", not(feature = "postgres"), not(feature = "mysql")))]
            {
                $crate::with_test_db!(
                    "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
                    |$db: $ty| $test
                )
            }

            #[cfg(all(
                feature = "sqlx-sqlite",
                not(feature = "postgres"),
                not(feature = "sqlx-postgres"),
                not(feature = "mysql")
            ))]
            {
                $crate::with_test_db!("sqlite_testdb", |$db: $ty| $test)
            }

            #[cfg(not(any(
                feature = "postgres",
                feature = "sqlx-postgres",
                feature = "sqlx-sqlite",
                feature = "mysql"
            )))]
            {
                compile_error!("No database backend feature enabled")
            }
        }
    };

    // Version with setup and test functions using async move blocks with type annotations
    ($url:expr, |$setup_param:ident| $setup_block:expr, |$test_param:ident: $ty:ty| $test_block:expr) => {
        async {
            // Import DatabaseBackend trait for backend methods
            use $crate::backend::DatabaseBackend;

            // Create backend for the URL based on feature
            #[cfg(all(feature = "postgres", not(feature = "sqlx-postgres"), not(feature = "mysql")))]
            #[allow(unused_variables)]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create PostgresBackend");

            #[cfg(all(feature = "sqlx-postgres", not(feature = "postgres")))]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create SqlxPostgresBackend");

            #[cfg(feature = "mysql")]
            let backend = $crate::backends::mysql::MySqlBackend::new($url)
                .expect("Failed to create MySqlBackend");

            #[cfg(all(
                feature = "sqlx-sqlite",
                not(feature = "postgres"),
                not(feature = "sqlx-postgres"),
                not(feature = "mysql")
            ))]
            let backend = $crate::backends::sqlite::SqliteBackend::new($url)
                .await
                .expect("Failed to create SqliteBackend");

            // Create test database template
            let template =
                $crate::TestDatabaseTemplate::new(backend, $crate::pool::PoolConfig::default(), 5)
                    .await
                    .expect("Failed to create test database template");

            // Save the backend and template name for cleanup
            let template_backend = template.backend().clone();
            let template_name = template.name().clone();

            // Initialize the template with setup operations in a panic-safe way
            let setup_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
                template
                    .initialize(|mut conn| async move {
                        let $setup_param = &mut conn;
                        $setup_block.await
                    })
                    .await
            }));

            match setup_result {
                Ok(future) => {
                    if let Err(e) = future.await {
                        error!("Setup failed: {:?}", e);
                        // Explicitly drop the template database
                        if let Err(drop_err) = template_backend.drop_database(&template_name).await
                        {
                            warn!("Warning: failed to drop template database: {}", drop_err);
                        }
                        panic!("Setup failed: {:?}", e);
                    }
                }
                Err(e) => {
                    // Explicitly drop the template database
                    error!("Setup panicked, ensuring database cleanup");
                    if let Err(drop_err) = template_backend.drop_database(&template_name).await {
                        error!(
                            "Warning: failed to drop template database during panic recovery: {}",
                            drop_err
                        );
                    }
                    std::panic::resume_unwind(e);
                }
            }

            // Run the test with template in a panic-safe way
            let $test_param: $ty = template;
            // Store backend and name for explicit cleanup
            let backend_copy = $test_param.backend().clone();
            let db_name = $test_param.name().clone();

            let test_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
                let future = $test_block;
                future.await
            }));

            match test_result {
                Ok(future) => {
                    if let Err(e) = future.await {
                        error!("Test failed: {:?}", e);
                        // Explicitly drop the database
                        if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                            warn!("Warning: failed to drop database: {}", drop_err);
                        }
                        panic!("Test failed: {:?}", e);
                    }
                }
                Err(e) => {
                    // Explicitly drop the database
                    error!("Test panicked, ensuring database cleanup");
                    if let Err(drop_err) = backend_copy.drop_database(&db_name).await {
                        error!(
                            "Warning: failed to drop database during panic recovery: {}",
                            drop_err
                        );
                    }
                    std::panic::resume_unwind(e);
                }
            }

            // Return unit type so this doesn't need to be annotated
            Ok::<(), $crate::error::DbError>(())
        }
    };
}

/// Creates a new MySQL test database and executes the provided test function.
///
/// Similar to [`with_test_db`], but uses MySQL as the backend.
///
/// # Arguments
///
/// * `$f` - A function that takes a [`TestDatabase`] and returns a future
#[cfg(feature = "mysql")]
#[macro_export]
macro_rules! with_mysql_test_db {
    // Version without explicit URL
    (|$db:ident| $test:expr) => {
        $crate::with_test_db!("mysql://root:password@mysql:3306/", |$db| $test)
    };

    // Version with explicit URL
    ($url:expr, |$db:ident| $test:expr) => {
        $crate::with_test_db!($url, |$db| $test)
    };

    // Version with type annotation
    (|$db:ident: $ty:ty| $test:expr) => {
        $crate::with_test_db!("mysql://root:password@mysql:3306/", |$db: $ty| $test)
    };

    // Version with type annotation and URL
    ($url:expr, |$db:ident: $ty:ty| $test:expr) => {
        $crate::with_test_db!($url, |$db: $ty| $test)
    };
}

/// Creates a new SQLx PostgreSQL test database and executes the provided test function.
///
/// Similar to [`with_test_db`], but uses SQLx's PostgreSQL implementation as the backend.
///
/// # Arguments
///
/// * `$f` - A function that takes a [`TestDatabase`] and returns a future
#[cfg(feature = "sqlx-postgres")]
#[macro_export]
macro_rules! with_sqlx_test_db {
    ($f:expr) => {{
        let backend = SqlxPostgresBackend::new(&get_sqlx_postgres_url().unwrap())
            .expect("Failed to create database backend");
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        let db = template.get_immutable_database().await.unwrap();
        let test_db = TestDatabaseTemplate::new(
            db.get_pool().clone(),
            format!("test_user_{}", uuid::Uuid::new_v4()),
        );

        $f(test_db).await
    }};
}

/// Creates a new SQLite test database and executes the provided test function.
///
/// Similar to [`with_test_db`], but uses SQLite as the backend.
///
/// # Arguments
///
/// * `$f` - A function that takes a [`TestDatabase`] and returns a future
#[cfg(feature = "sqlx-sqlite")]
#[macro_export]
macro_rules! with_sqlite_test_db {
    ($f:expr) => {{
        let backend = SqliteBackend::new(&get_sqlite_url().unwrap())
            .await
            .expect("Failed to create database backend");
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        let test_db = template.create_test_database().await.unwrap();
        let _ = $f(test_db).await;
    }};

    // Version with URL provided
    ($url:expr, $f:expr) => {{
        let backend = SqliteBackend::new($url)
            .await
            .expect("Failed to create database backend");
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        let test_db = template.create_test_database().await.unwrap();
        let _ = $f(test_db).await;
    }};
}

#[cfg(test)]
mod tests {

    #[cfg(any(
        feature = "sqlx-mysql",
        feature = "sqlx-postgres",
        feature = "sqlx-sqlite"
    ))]
    fn setup_logging() {
        std::env::set_var("RUST_LOG", "sqlx=debug");
        let _ = tracing_subscriber::fmt::try_init(); // Use try_init() and ignore errors
    }

    #[tokio::test]
    #[cfg(any(
        feature = "sqlx-mysql",
        feature = "sqlx-postgres",
        feature = "sqlx-sqlite"
    ))]
    async fn test_direct_connection() {
        setup_logging();
        #[cfg(feature = "sqlx-postgres")]
        let pool = sqlx::PgPool::connect(
            "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
        )
        .await
        .expect("Failed to connect");

        #[cfg(feature = "sqlx-mysql")]
        let pool = sqlx::MySqlPool::connect("mysql://root@mysql:3306/mysql")
            .await
            .expect("Failed to connect");

        #[cfg(feature = "sqlx-sqlite")]
        let pool = sqlx::SqlitePool::connect("sqlite_testdb")
            .await
            .expect("Failed to connect");

        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&pool)
            .await
            .expect("Query failed");

        assert_eq!(result.0, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    #[cfg(feature = "sqlx-postgres")]
    async fn test_basic_database_operations() {
        with_test_db!(
            "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
            |db| async move {
                // db is already a TestDatabase instance, not a template
                let test_db = db;

                #[cfg(feature = "postgres")]
                {
                    // For PostgresBackend, directly use pool.acquire()
                    use crate::backend::{Connection, DatabasePool};
                    let mut conn = test_db.pool.acquire().await.unwrap();
                    conn.execute(
                        "CREATE TABLE some_test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
                    )
                    .await
                    .unwrap();
                    tracing::info!("Created table with Postgres backend");
                }

                #[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
                {
                    // For SqlxPostgresBackend, use DatabasePool trait to acquire connection
                    use crate::backend::DatabasePool;
                    let mut conn = test_db.pool.acquire().await.unwrap();

                    let res = sqlx::query!(
                        "CREATE TABLE some_test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
                    )
                    .execute(&mut conn)
                    .await;
                    tracing::info!("Created table with SQLx backend: {:?}", res);
                }

                Ok(()) as crate::error::Result<()>
            }
        )
        .await
        .unwrap();
    }
}
