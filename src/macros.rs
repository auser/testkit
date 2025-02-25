use std::future::Future;

#[allow(unused)]
use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    backends::PostgresBackend,
    env::get_postgres_url,
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

/// A test database instance that provides access to a connection pool and test-specific data.
///
/// This struct is generic over the database backend type and provides methods for setting up
/// and interacting with a test database instance.
///
/// # Type Parameters
///
/// * `'a` - The lifetime of the database pool reference
/// * `B` - The database backend type that implements [`DatabaseBackend`]
pub struct TestDatabase<B: DatabaseBackend> {
    /// The connection pool for this test database
    pub test_pool: B::Pool,
    /// A unique identifier for the test user, useful for test data isolation
    pub test_user: String,
}

impl<B: DatabaseBackend> TestDatabase<B> {
    pub fn new(pool: B::Pool, test_user: String) -> Self {
        Self {
            test_pool: pool,
            test_user,
        }
    }

    /// Sets up the test database by executing the provided setup function.
    ///
    /// This method acquires a connection from the pool, executes the setup function with that
    /// connection, and returns the result.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The setup function type
    /// * `Fut` - The future type returned by the setup function
    /// * `T` - The result type of the setup function
    ///
    /// # Arguments
    ///
    /// * `f` - A function that takes a database connection and returns a future
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the value returned by the setup function, or an error
    /// if the setup failed.
    pub async fn setup<F, Fut, T>(&self, f: F) -> Result<T, Box<dyn std::error::Error>>
    where
        F: FnOnce(B::Connection) -> Fut,
        Fut: Future<Output = Result<T, Box<dyn std::error::Error>>>,
    {
        let conn = self.test_pool.acquire().await?;
        let result = f(conn).await?;
        Ok(result)
    }
}

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
#[cfg(feature = "postgres")]
#[macro_export]
macro_rules! with_test_db {
    ($db_type:ty, $setup:expr, |$db:ident| $test:expr) => {
        async {
            let db_pool = <$db_type>::setup_test_db()
                .await
                .expect("Failed to setup test database");

            let $db = $crate::TestDatabase::new(db_pool).await;

            if let Err(e) = $db.setup($setup).await {
                panic!("Failed to setup test database: {:?}", e);
            }

            let result = $test;
            result.await
        }
    };

    (|$db:ident| $test:expr) => {
        $crate::with_test_db!(::db_testkit::PostgresPool, |_| async { Ok(()) }, |$db| {
            $test
        })
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
    use super::*;
    use sqlx::{Executor, Row};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_basic_database_operations() {
        with_test_db!(|db: TestDatabase<PostgresBackend>| async move {
            // Setup: Create a test table
            db.setup(|mut conn| async move {
                sqlx::Executor::execute(
                    &mut conn,
                    "CREATE TABLE test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
                )
                .await?;
                Ok(())
            })
            .await
            .unwrap();

            // Test transaction with commit
            let mut conn = db.test_pool.acquire().await.unwrap();
            let mut tx = conn.begin().await.unwrap();

            let test_id = Uuid::new_v4();
            let test_name = "Test Item";

            tx.execute(
                sqlx::query("INSERT INTO test_items (id, name) VALUES ($1, $2)")
                    .bind(&test_id)
                    .bind(test_name),
            )
            .await
            .unwrap();

            tx.commit().await.unwrap();

            // Verify the data was committed
            let row = sqlx::query("SELECT name FROM test_items WHERE id = $1")
                .bind(&test_id)
                .fetch_one(&db.test_pool.pool)
                .await
                .unwrap();

            assert_eq!(row.get::<&str, _>("name"), test_name);
        });
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        with_test_db!(|db: TestDatabase<PostgresBackend>| async move {
            // Setup: Create a test table
            db.setup(|mut conn| async move {
                sqlx::Executor::execute(
                    &mut conn,
                    "CREATE TABLE test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
                )
                .await?;
                Ok(())
            })
            .await
            .unwrap();

            let test_id = Uuid::new_v4();
            let test_name = "Test Item";

            // Start a transaction
            let mut conn = db.test_pool.acquire().await.unwrap();
            let mut tx = conn.begin().await.unwrap();

            // Insert data
            tx.execute(
                sqlx::query("INSERT INTO test_items (id, name) VALUES ($1, $2)")
                    .bind(&test_id)
                    .bind(test_name),
            )
            .await
            .unwrap();

            // Rollback instead of commit
            tx.rollback().await.unwrap();

            // Verify the data was not committed
            let result = sqlx::query("SELECT name FROM test_items WHERE id = $1")
                .bind(&test_id)
                .fetch_optional(&db.test_pool.pool)
                .await
                .unwrap();

            assert!(result.is_none());
        });
    }

    #[tokio::test]
    async fn test_multiple_transactions() {
        with_test_db!(|db: TestDatabase<PostgresBackend>| async move {
            // Setup: Create a test table
            db.setup(|mut conn| async move {
                sqlx::Executor::execute(&mut conn, "CREATE TABLE test_items (id UUID PRIMARY KEY, name TEXT NOT NULL, counter INTEGER)")
                    .await?;
                Ok(())
            })
            .await
            .unwrap();

            let test_id = Uuid::new_v4();

            // First transaction
            let mut conn1 = db.test_pool.acquire().await.unwrap();
            let mut tx1 = conn1.begin().await.unwrap();

            tx1.execute(
                sqlx::query("INSERT INTO test_items (id, name, counter) VALUES ($1, $2, $3)")
                    .bind(&test_id)
                    .bind("Test Item")
                    .bind(1),
            )
            .await
            .unwrap();

            tx1.commit().await.unwrap();

            // Second transaction
            let mut conn2 = db.test_pool.acquire().await.unwrap();
            let mut tx2 = conn2.begin().await.unwrap();

            tx2.execute(
                sqlx::query("UPDATE test_items SET counter = counter + 1 WHERE id = $1")
                    .bind(&test_id),
            )
            .await
            .unwrap();

            tx2.commit().await.unwrap();

            // Verify final state
            let row = sqlx::query("SELECT counter FROM test_items WHERE id = $1")
                .bind(&test_id)
                .fetch_one(&db.test_pool.pool)
                .await
                .unwrap();

            assert_eq!(row.get::<i32, _>("counter"), 2);
        });
    }

    #[tokio::test]
    async fn test_concurrent_connections() {
        with_test_db!(|db: TestDatabase<PostgresBackend>| async move {
            // Setup: Create a test table
            db.setup(|mut conn| async move {
                sqlx::Executor::execute(
                    &mut conn,
                    "CREATE TABLE test_items (id UUID PRIMARY KEY, name TEXT NOT NULL)",
                )
                .await?;
                Ok(())
            })
            .await
            .unwrap();

            // Create multiple concurrent connections
            let mut handles = vec![];

            for i in 0..5 {
                let pool = db.test_pool.clone();
                let handle = tokio::spawn(async move {
                    let mut conn = pool.acquire().await.unwrap();
                    let mut tx = conn.begin().await.unwrap();

                    let id = Uuid::new_v4();
                    tx.execute(
                        sqlx::query("INSERT INTO test_items (id, name) VALUES ($1, $2)")
                            .bind(&id)
                            .bind(format!("Item {}", i)),
                    )
                    .await
                    .unwrap();

                    tx.commit().await.unwrap();
                    id
                });
                handles.push(handle);
            }

            // Wait for all operations to complete
            let ids = futures::future::join_all(handles)
                .await
                .into_iter()
                .map(|r| r.unwrap())
                .collect::<Vec<_>>();

            // Verify all items were inserted
            let count = sqlx::query("SELECT COUNT(*) as count FROM test_items")
                .fetch_one(&db.test_pool.pool)
                .await
                .unwrap()
                .get::<i64, _>("count");

            assert_eq!(count, 5);

            // Verify each specific item
            for id in ids {
                let exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM test_items WHERE id = $1)")
                    .bind(&id)
                    .fetch_one(&db.test_pool.pool)
                    .await
                    .unwrap()
                    .get::<bool, _>("exists");

                assert!(exists);
            }
        });
    }
}
