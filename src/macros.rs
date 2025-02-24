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
use crate::{backends::mysql::MySqlBackend, env::get_mysql_url};

#[cfg(feature = "sqlx-postgres")]
use crate::{backends::sqlx::SqlxPostgresBackend, env::get_sqlx_postgres_url};

#[cfg(feature = "sqlx-sqlite")]
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
pub struct TestDatabase<'a, B: DatabaseBackend> {
    /// The connection pool for this test database
    pub test_pool: &'a B::Pool,
    /// A unique identifier for the test user, useful for test data isolation
    pub test_user: String,
}

impl<'a, B: DatabaseBackend> TestDatabase<'a, B> {
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
    ($f:expr) => {{
        let backend = PostgresBackend::new(&get_postgres_url().unwrap())
            .await
            .unwrap();
        let template = DatabaseTemplate::new(backend, PoolConfig::default(), 1)
            .await
            .unwrap();

        let db = template.get_immutable_database().await.unwrap();
        let test_db = TestDatabase {
            test_pool: db.get_pool(),
            test_user: format!("test_user_{}", uuid::Uuid::new_v4()),
        };

        $f(test_db).await
    }};
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
        let test_db = TestDatabase {
            test_pool: db.get_pool(),
            test_user: format!("test_user_{}", uuid::Uuid::new_v4()),
        };

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
        let test_db = TestDatabase {
            test_pool: db.get_pool(),
            test_user: format!("test_user_{}", uuid::Uuid::new_v4()),
        };

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
        let test_db = TestDatabase {
            test_pool: db.get_pool(),
            test_user: format!("test_user_{}", uuid::Uuid::new_v4()),
        };

        $f(test_db).await
    }};
}
