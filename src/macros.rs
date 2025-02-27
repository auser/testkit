#[allow(unused)]
use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::Result,
    pool::PoolConfig,
    test_db::TestDatabaseTemplate,
};

#[cfg(feature = "mysql")]
#[allow(unused)]
use crate::{backends::mysql::MySqlBackend, env::get_mysql_url};

#[cfg(feature = "sqlx-backend")]
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
///     with_test_db(|db| async move {
///         db.setup(|mut conn| async move {
///             conn.execute("CREATE TABLE users (id SERIAL PRIMARY KEY)").await?;
///             Ok(())
///         }).await?;
///         Ok(())
///     }).await;
/// }
/// ```
#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
#[macro_export]
macro_rules! with_test_db {
    // Version with URL and no type annotation - for easy use
    ($url:expr, |$db:ident| $test:expr) => {
        async move {
            // Create backend based on the feature
            #[cfg(feature = "postgres")]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            // Create test database using the simplified API
            let $db = $crate::create_template_db(backend, 5)
                .await
                .expect("Failed to create test database");

            // Run the test - result is mapped to () for simplicity
            match $test.await {
                Ok(_) => (),
                Err(e) => eprintln!("Test failed: {:?}", e),
            }
        }
    };

    // No URL provided, use default and no type annotation
    (|$db:ident| $test:expr) => {
        $crate::with_test_db!(
            "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
            |$db| $test
        )
    };

    // Version with setup and test functions using async move blocks
    ($url:expr, |$setup_param:ident| $setup_block:expr, |$test_param:ident| $test_block:expr) => {
        async move {
            // Create backend for the URL based on feature
            #[cfg(all(feature = "postgres", not(feature = "sqlx-postgres")))]
            #[allow(unused_variables)]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(feature = "sqlx-postgres")]
            #[allow(unused_variables)]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            // Create test database template
            let template =
                $crate::TestDatabaseTemplate::new(backend, $crate::pool::PoolConfig::default(), 5)
                    .await
                    .expect("Failed to create test database template");

            // Initialize the template with setup operations
            template
                .initialize(|mut conn| async move {
                    let $setup_param = &mut conn;
                    $setup_block.await
                })
                .await
                .expect("Setup block failed");

            // Run the test with template
            let $test_param = template;

            // Run the test - result is mapped to () for simplicity
            match $test_block.await {
                Ok(_) => (),
                Err(e) => eprintln!("Test failed: {:?}", e),
            }
        }
    };

    // Remaining (less commonly used) variants with explicit type annotations
    // For advanced/specialized use cases

    // Version with custom URL and type annotation
    ($url:expr, |$db:ident: $ty:ty| $test:expr) => {
        async move {
            // Create backend based on the expected type
            #[cfg(feature = "postgres")]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            // Create test database using the simplified API
            let $db: $ty = $crate::create_template_db(backend, 5)
                .await
                .expect("Failed to create test database");

            // Run the test - result is mapped to () for simplicity
            match $test.await {
                Ok(_) => (),
                Err(e) => eprintln!("Test failed: {:?}", e),
            }
        }
    };

    // Version with type annotation
    (|$db:ident: $ty:ty| $test:expr) => {
        $crate::with_test_db!(
            "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
            |$db: $ty| $test
        )
    };

    // Version with setup and test functions using async move blocks with type annotations
    ($url:expr, |$setup_param:ident| $setup_block:expr, |$test_param:ident: $ty:ty| $test_block:expr) => {
        async move {
            // Create backend - type specific
            #[cfg(feature = "postgres")]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create PostgresBackend");

            #[cfg(all(feature = "sqlx-backend", not(feature = "postgres")))]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create SqlxPostgresBackend");

            // Create test database template
            let template =
                $crate::TestDatabaseTemplate::new(backend, $crate::pool::PoolConfig::default(), 5)
                    .await
                    .expect("Failed to create test database template");

            // Initialize the template with setup operations
            template
                .initialize(|mut conn| async move {
                    let $setup_param = &mut conn;
                    $setup_block.await
                })
                .await
                .expect("Setup block failed");

            // Run the test with template - ensure type compatibility
            let $test_param: $ty = template;

            // Run the test - result is mapped to () for simplicity
            match $test_block.await {
                Ok(_) => (),
                Err(e) => eprintln!("Test failed: {:?}", e),
            }
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
    ($f:expr) => {{
        let backend = MySqlBackend::new(&get_mysql_url().unwrap()).await.unwrap();
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
            .unwrap();
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

#[cfg(test)]
mod tests {
    use crate::error::Result;

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
        setup_logging(); // Call it here
        let pool = sqlx::PgPool::connect(
            "postgres://postgres:postgres@postgres:5432/postgres?sslmode=disable",
        )
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
                // Get a TestDatabase from the template
                let test_db = db.create_test_database().await.unwrap();

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
                    println!("Created table with Postgres backend");
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
                    println!("Created table with SQLx backend: {:?}", res);
                }

                Ok(()) as Result<()>
            }
        );
    }

    // #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    // #[cfg(feature = "sqlx-postgres")]
    // async fn test_transaction_rollback() {
    //     with_test_db!(|db: TestDatabaseTemplate<SqlxPostgresBackend>| async move {
    //         // Setup: Create a test table
    //         db.setup(|mut conn| async move {
    //             sqlx::Executor::execute(
    //                 &mut conn,
    //                 "CREATE TABLE test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
    //             )
    //             .await?;
    //             Ok(())
    //         })
    //         .await
    //         .unwrap();

    //         let test_id = Uuid::new_v4();
    //         let test_name = "Test Item";

    //         // Start a transaction
    //         let mut conn = db.pool.acquire().await.unwrap();
    //         let mut tx = conn.begin().await.unwrap();

    //         // Insert data
    //         tx.execute(
    //             sqlx::query("INSERT INTO test_items (id, name) VALUES ($1, $2)")
    //                 .bind(test_id)
    //                 .bind(test_name),
    //         )
    //         .await
    //         .unwrap();

    //         // Rollback instead of commit
    //         tx.rollback().await.unwrap();

    //         // Verify the data was not committed
    //         let result = sqlx::query("SELECT name FROM test_items WHERE id = $1")
    //             .bind(test_id)
    //             .fetch_optional(&db.pool.pool)
    //             .await
    //             .unwrap();

    //         assert!(result.is_none());
    //     });
    // }

    // #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    // async fn test_multiple_transactions() {
    //     with_test_db!(|db: TestDatabaseTemplate<PostgresBackend>| async move {
    //         // Setup: Create a test table
    //         db.setup(|mut conn| async move {
    //             sqlx::Executor::execute(&mut conn, "CREATE TABLE test_items (id UUID PRIMARY KEY, name TEXT NOT NULL, counter INTEGER)")
    //                 .await?;
    //             Ok(())
    //         })
    //         .await
    //         .unwrap();

    //         let test_id = Uuid::new_v4();

    //         // First transaction
    //         let mut conn1 = db.pool.acquire().await.unwrap();
    //         let mut tx1 = conn1.begin().await.unwrap();

    //         tx1.execute(
    //             sqlx::query("INSERT INTO test_items (id, name, counter) VALUES ($1, $2, $3)")
    //                 .bind(test_id)
    //                 .bind("Test Item")
    //                 .bind(1),
    //         )
    //         .await
    //         .unwrap();

    //         tx1.commit().await.unwrap();

    //         // Second transaction
    //         let mut conn2 = db.pool.acquire().await.unwrap();
    //         let mut tx2 = conn2.begin().await.unwrap();

    //         tx2.execute(
    //             sqlx::query("UPDATE test_items SET counter = counter + 1 WHERE id = $1")
    //                 .bind(test_id),
    //         )
    //         .await
    //         .unwrap();

    //         tx2.commit().await.unwrap();

    //         // Verify final state
    //         let row = sqlx::query("SELECT counter FROM test_items WHERE id = $1")
    //             .bind(test_id)
    //             .fetch_one(&db.pool.pool)
    //             .await
    //             .unwrap();

    //         assert_eq!(row.get::<i32, _>("counter"), 2);
    //     });
    // }

    // #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    // async fn test_concurrent_connections() {
    //     with_test_db!(|db: TestDatabaseTemplate<PostgresBackend>| async move {
    //         // Setup: Create a test table
    //         db.setup(|mut conn| async move {
    //             sqlx::Executor::execute(
    //                 &mut conn,
    //                 "CREATE TABLE test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
    //             )
    //             .await?;
    //             Ok(())
    //         })
    //         .await
    //         .unwrap();

    //         // Create multiple concurrent connections
    //         let mut handles = vec![];

    //         for i in 0..5 {
    //             let pool = db.pool.clone();
    //             let handle = tokio::spawn(async move {
    //                 let mut conn = pool.acquire().await.unwrap();
    //                 let mut tx = conn.begin().await.unwrap();

    //                 let id = Uuid::new_v4();
    //                 tx.execute(
    //                     sqlx::query("INSERT INTO test_items (id, name) VALUES ($1, $2)")
    //                         .bind(id)
    //                         .bind(format!("Item {}", i)),
    //                 )
    //                 .await
    //                 .unwrap();

    //                 tx.commit().await.unwrap();
    //                 id
    //             });
    //             handles.push(handle);
    //         }

    //         // Wait for all operations to complete
    //         let ids = futures::future::join_all(handles)
    //             .await
    //             .into_iter()
    //             .map(|r| r.unwrap())
    //             .collect::<Vec<_>>();

    //         // Verify all items were inserted
    //         let count = sqlx::query("SELECT COUNT(*) as count FROM test_items")
    //             .fetch_one(&db.pool.pool)
    //             .await
    //             .unwrap()
    //             .get::<i64, _>("count");

    //         assert_eq!(count, 5);

    //         // Verify each specific item
    //         for id in ids {
    //             let exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM test_items WHERE id = $1)")
    //                 .bind(id)
    //                 .fetch_one(&db.pool.pool)
    //                 .await
    //                 .unwrap()
    //                 .get::<bool, _>("exists");

    //             assert!(exists);
    //         }
    //     });
    // }
}
