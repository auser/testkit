#[allow(unused)]
use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    pool::PoolConfig,
    template::DatabaseTemplate,
};

#[cfg(feature = "mysql")]
#[allow(unused)]
use crate::{backends::mysql::MySqlBackend, env::get_mysql_url};

#[cfg(feature = "sqlx-postgres")]
#[allow(unused)]
use crate::{backends::sqlx::SqlxPostgresBackend, env::get_sqlx_postgres_url};

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
    // Version with custom URL and type annotation
    ($url:expr, |$db:ident: $ty:ty| $test:expr) => {
        async {
            // Create backend
            #[cfg(feature = "postgres")]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(feature = "sqlx-postgres")]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            // Create test database
            let $db: $ty =
                $crate::test_db::TestDatabase::new(backend, $crate::pool::PoolConfig::default())
                    .await
                    .expect("Failed to create test database");

            // Run the test
            $test.await
        }
        .await
    };

    // Version with inferred type (uses default URL)
    ($test:expr) => {
        with_test_db!(
            "postgres://postgres:postgres@host.docker.internal:5432/postgres?sslmode=disable",
            |db| $test.await
        )
    };


    // Version with type annotation (uses default URL)
    (|$db:ident: $ty:ty| $test:expr) => {
        with_test_db!(
            "postgres://postgres:postgres@host.docker.internal:5432/postgres?sslmode=disable",
            |$db: $ty| $test
        )
    };

    // Version with setup function
    (|$db:ident| $setup:expr, $test:expr) => {
        async {
            // Create backend
            let backend = $crate::backends::postgres::PostgresBackend::new(
                "postgres://postgres:postgres@host.docker.internal:5432/postgres?sslmode=disable",
            )
            .await
            .expect("Failed to create database backend");

            // Create test database
            let $db =
                $crate::test_db::TestDatabase::new(backend, $crate::pool::PoolConfig::default())
                    .await
                    .expect("Failed to create test database");

            // Run setup
            $db.setup($setup)
                .await
                .expect("Failed to setup test database");

            // Run the test
            $test.await
        }
    };

    // Version with setup and test functions using async move blocks
    ($url:expr, |mut $db:ident| async move $setup:block, |$test_db:ident| async move $test:block) => {
        async {
            // Create backend
            #[cfg(feature = "postgres")]
            let backend = $crate::backends::postgres::PostgresBackend::new($url)
                .await
                .expect("Failed to create database backend");

            #[cfg(feature = "sqlx-postgres")]
            let backend = $crate::backends::sqlx::SqlxPostgresBackend::new($url)
                .expect("Failed to create database backend");

            // Create test database
            let mut $db =
                $crate::test_db::TestDatabase::new(backend, $crate::pool::PoolConfig::default())
                    .await
                    .expect("Failed to create test database");

            // Run setup
            $db.setup(|conn| async move {
                let mut $db = conn;
                $setup
            })
            .await
            .expect("Failed to setup test database");

            // Run the test
            let $test_db = $db;
            async move $test.await
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
        let template = DatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        let db = template.get_immutable_database().await.unwrap();
        let test_db = TestDatabase::new(
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
            .await
            .unwrap();
        let template = DatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        let db = template.get_immutable_database().await.unwrap();
        let test_db = TestDatabase::new(
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
        let template = DatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        let db = template.get_immutable_database().await.unwrap();
        let test_db = TestDatabase::new(
            db.get_pool().clone(),
            format!("test_user_{}", uuid::Uuid::new_v4()),
        );

        $f(test_db).await
    }};
}

#[cfg(test)]
mod tests {
    use crate::TestDatabase;

    use super::*;
    use sqlx::{Executor, Row};
    use uuid::Uuid;

    fn setup_logging() {
        std::env::set_var("RUST_LOG", "sqlx=debug");
        let _ = tracing_subscriber::fmt::try_init(); // Use try_init() and ignore errors
    }

    #[tokio::test]
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
            |db: TestDatabase<SqlxPostgresBackend>| async move {
                // Setup: Create a test table
                db.setup(|mut conn| async move {
                    let res = sqlx::query!(
                        "CREATE TABLE some_test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
                    )
                    .execute(&mut conn)
                    .await
                    .unwrap();
                    println!("res: {:?}", res);
                    Ok(())
                })
                .await
                .unwrap();
                // // Test transaction with commit
                // let mut conn = db.pool.acquire().await.unwrap();
                // let mut tx = conn.begin().await.unwrap();

                // let test_id = Uuid::new_v4();
                // let test_name = "Test Item";

                // tx.execute(
                //     sqlx::query("INSERT INTO test_items (id, name) VALUES ($1, $2)")
                //         .bind(test_id)
                //         .bind(test_name),
                // )
                // .await
                // .unwrap();

                // tx.commit().await.unwrap();

                // // Verify the data was committed
                // let row = sqlx::query("SELECT name FROM test_items WHERE id = $1")
                //     .bind(test_id)
                //     .fetch_one(&db.pool)
                //     .await
                //     .unwrap();

                // assert_eq!(row.get::<&str, _>("name"), test_name);
            }
        );
    }

    // #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    // #[cfg(feature = "sqlx-postgres")]
    // async fn test_transaction_rollback() {
    //     with_test_db!(|db: TestDatabase<SqlxPostgresBackend>| async move {
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
    //     with_test_db!(|db: TestDatabase<PostgresBackend>| async move {
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
    //     with_test_db!(|db: TestDatabase<PostgresBackend>| async move {
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
