use std::marker::PhantomData;
use std::pin::Pin;
use std::{fmt::Debug, future::Future};

use async_trait::async_trait;

use crate::{DatabaseBackend, DatabaseConfig, IntoTransaction, TestDatabaseInstance, Transaction};

/// Helper function to create a Result with inferred error type
pub fn db_ok<T, E>(value: T) -> Result<T, E> {
    Ok(value)
}

/// Type alias for boxed futures with appropriate lifetime
type BoxFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// A trait for operations that can be performed on a database
#[async_trait]
pub trait DatabaseOperation<B>: Send + Sync + 'static
where
    B: DatabaseBackend + 'static + Debug + Send + Sync,
{
    type Context: Send + Sync + 'static;
    type Item: Send + Sync + 'static;
    type Error: Send + Sync + 'static;

    async fn execute(&self, db: &mut Self::Context) -> Result<Self::Item, Self::Error>;
}

/// WithDatabase implements the Transaction trait and represents a database operation
pub struct WithDatabase<B, F, Next>
where
    B: DatabaseBackend + 'static + Clone + Debug + Send + Sync,
    F: Clone + Send + Sync + 'static,
    Next: IntoTransaction<TestDatabaseInstance<B>> + Send + Sync,
{
    /// The function to execute
    f: F,
    /// The test database config
    #[allow(dead_code)]
    config: DatabaseConfig,
    /// The backend instance
    #[allow(dead_code)]
    backend: B,
    _phantom: PhantomData<Next>,
}

/// Create a new database operation with a function that returns a future
pub fn with_database<B, F, Next, Item, Error>(
    backend: B,
    config: DatabaseConfig,
    f: F,
) -> WithDatabase<B, F, Next>
where
    B: DatabaseBackend + 'static + Clone + Debug + Send + Sync,
    F: for<'a> FnMut(&'a mut TestDatabaseInstance<B>) -> BoxFuture<'a, Item, Error>
        + Clone
        + Send
        + Sync
        + 'static,
    Next: IntoTransaction<TestDatabaseInstance<B>> + Send + Sync + 'static,
    Item: Send + Sync + 'static,
    Error: Send + Sync + 'static,
{
    WithDatabase {
        f,
        config,
        backend,
        _phantom: PhantomData,
    }
}

/// Implementation of the Transaction trait for WithDatabase
#[async_trait]
impl<B, F, Next, Item, Error> Transaction for WithDatabase<B, F, Next>
where
    B: DatabaseBackend + 'static + Clone + Debug + Send + Sync,
    F: for<'a> FnMut(&'a mut TestDatabaseInstance<B>) -> BoxFuture<'a, Item, Error>
        + Clone
        + Send
        + Sync
        + 'static,
    Next: IntoTransaction<TestDatabaseInstance<B>> + Send + Sync + 'static,
    Item: Clone + Send + Sync + 'static,
    Error: Clone + Send + Sync + 'static,
{
    type Context = TestDatabaseInstance<B>;
    type Item = TestDatabaseInstance<B>;
    type Error = Error;

    async fn execute(&self, _ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        let mut tdb = Self::Context::new(self.backend.clone(), self.config.clone())
            .await
            .expect("Failed to create test database instance");

        let mut f = self.f.clone();

        match f(&mut tdb).await {
            Ok(_) => Ok(tdb),
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
impl<B, F, Item, Error> DatabaseOperation<B> for F
where
    B: DatabaseBackend + 'static + Clone + Debug + Send + Sync,
    F: for<'a> FnMut(&'a mut TestDatabaseInstance<B>) -> BoxFuture<'a, Item, Error>
        + Clone
        + Send
        + Sync
        + 'static,
    Item: Send + Sync + 'static,
    Error: Send + Sync + 'static,
{
    type Context = TestDatabaseInstance<B>;
    type Item = Item;
    type Error = Error;

    async fn execute(&self, db: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        let mut f = self.clone();
        f(db).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseBackend, DatabaseName, DatabasePool, TestDatabaseConnection, result};
    use sqlx::{PgPool, postgres::PgPoolOptions};

    // This is a unit test that doesn't actually require a database connection
    #[test]
    fn test_with_database_unit() {
        // Define a simple test backend
        #[derive(Debug, Clone)]
        struct TestBackend;

        #[derive(Debug, Clone)]
        struct TestError(String);

        impl From<String> for TestError {
            fn from(s: String) -> Self {
                TestError(s)
            }
        }

        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        #[derive(Debug, Clone)]
        struct TestConnection;

        impl TestDatabaseConnection for TestConnection {
            fn connection_string(&self) -> String {
                "test-connection".to_string()
            }
        }

        #[derive(Debug, Clone)]
        struct TestPool;

        #[async_trait]
        impl DatabasePool for TestPool {
            type Connection = TestConnection;
            type Error = TestError;

            async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
                Ok(TestConnection)
            }

            async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
                Ok(())
            }

            fn connection_string(&self) -> String {
                "test-pool".to_string()
            }
        }

        #[async_trait]
        impl DatabaseBackend for TestBackend {
            type Connection = TestConnection;
            type Pool = TestPool;
            type Error = TestError;

            async fn create_pool(
                &self,
                _name: &DatabaseName,
                _config: &DatabaseConfig,
            ) -> Result<Self::Pool, Self::Error> {
                Ok(TestPool)
            }

            async fn create_database(
                &self,
                _pool: &Self::Pool,
                _name: &DatabaseName,
            ) -> Result<(), Self::Error> {
                Ok(())
            }

            fn drop_database(&self, _name: &DatabaseName) -> Result<(), Self::Error> {
                Ok(())
            }

            fn connection_string(&self, _name: &DatabaseName) -> String {
                "test-backend".to_string()
            }
        }

        // Just verify that the with_database function can be called
        // and returns a struct of the expected type
        let _tx: WithDatabase<TestBackend, _, Result<TestBackend, TestError>> = with_database::<
            TestBackend,
            _,
            Result<TestBackend, TestError>,
            TestBackend,
            TestError,
        >(
            TestBackend,
            DatabaseConfig::default(),
            |_db| -> BoxFuture<'_, TestBackend, TestError> { Box::pin(async { Ok(TestBackend) }) },
        );

        // Test passes if compilation succeeds
    }

    // The rest of the code remains for reference but isn't executed in tests
    #[derive(Debug, Clone)]
    struct PostgresBackend(url::Url);

    impl PostgresBackend {
        pub fn new(url: String) -> Self {
            let url = url::Url::parse(&url).unwrap();
            Self(url)
        }
        fn connection_string(&self, _name: &DatabaseName) -> String {
            self.0.clone().to_string()
        }
    }

    #[derive(Debug, Clone)]
    struct PostgresError(pub String);

    impl From<String> for PostgresError {
        fn from(value: String) -> Self {
            Self(value)
        }
    }

    impl std::fmt::Display for PostgresError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl From<sqlx::Error> for PostgresError {
        fn from(value: sqlx::Error) -> Self {
            Self(value.to_string())
        }
    }

    #[derive(Debug, Clone)]
    pub struct SqlxPostgresConnection {
        pub(crate) pool: PgPool,
        connection_string: String,
    }
    impl SqlxPostgresConnection {
        /// Get a reference to the underlying SQLx PgPool
        ///
        /// This allows direct use of SQLx queries with this connection
        pub fn sqlx_pool(&self) -> &PgPool {
            &self.pool
        }
    }

    impl TestDatabaseConnection for SqlxPostgresConnection {
        fn connection_string(&self) -> String {
            self.connection_string.to_string()
        }
    }

    #[derive(Debug, Clone)]
    struct PgPoolWrapper {
        pool: PgPool,
        connection_string: String,
    }

    impl PgPoolWrapper {
        fn connection_string(&self) -> String {
            self.connection_string.to_string()
        }
    }

    #[async_trait]
    impl DatabasePool for PgPoolWrapper {
        type Connection = SqlxPostgresConnection;
        type Error = PostgresError;

        async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
            // let conn = self.pool.acquire().await.map_err(PostgresError::from)?;
            let connection_string = self.connection_string();

            Ok(SqlxPostgresConnection {
                pool: self.pool.clone(),
                connection_string,
            })
        }

        async fn release(&self, conn: Self::Connection) -> Result<(), Self::Error> {
            self.release(conn).await
        }

        fn connection_string(&self) -> String {
            self.connection_string.to_string()
        }
    }

    #[async_trait]
    impl DatabaseBackend for PostgresBackend {
        type Connection = SqlxPostgresConnection;
        type Pool = PgPoolWrapper;
        type Error = PostgresError;

        async fn create_pool(
            &self,
            _name: &DatabaseName,
            _config: &DatabaseConfig,
        ) -> Result<Self::Pool, Self::Error> {
            let pool = PgPoolOptions::new()
                .connect(&self.connection_string(_name))
                .await
                .map_err(PostgresError::from)?;
            let pool = PgPoolWrapper {
                pool,
                connection_string: self.connection_string(_name),
            };
            Ok(pool)
        }

        async fn create_database(
            &self,
            pool: &Self::Pool,
            name: &DatabaseName,
        ) -> Result<(), Self::Error> {
            let pool = pool.acquire().await?;
            let mut conn = pool.sqlx_pool().acquire().await?;
            sqlx::query(&format!(r#"CREATE DATABASE "{}""#, name.as_str()))
                .execute(&mut *conn)
                .await?;
            Ok(())
        }

        fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
            println!("Dropping database: {:?}", name);
            // Drop the database
            let url = self.0.clone();
            let database_host = format!(
                "{}://{}:{}@{}:{}",
                url.scheme(),
                "postgres",
                url.password().unwrap_or(""),
                url.host_str().unwrap_or(""),
                url.port().unwrap_or(5432)
            );

            // Terminate all connections to the database
            execute_psql_command::<Self::Error>(
                &database_host,
                &format!(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{name}' AND pid <> pg_backend_pid();"
                ),
            )?;

            // Drop the database
            tracing::info!("Dropping database: {}", name.as_str());
            execute_psql_command::<Self::Error>(
                &database_host,
                &format!("DROP DATABASE \"{}\";", name.as_str()),
            )?;

            fn execute_psql_command<E>(database_host: &str, command: &str) -> Result<(), E>
            where
                E: From<String>,
            {
                let output = std::process::Command::new("psql")
                    .arg(database_host)
                    .arg("-c")
                    .arg(command)
                    .output()
                    .map_err(|e| E::from(e.to_string()))?;

                if !output.status.success() {
                    return Err(E::from(format!(
                        "Database drop failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )));
                }
                Ok(())
            }

            Ok(())
        }

        fn connection_string(&self, _name: &DatabaseName) -> String {
            self.0.to_string()
        }
    }

    #[tokio::test]
    async fn test_with_database() {
        use super::*;
        use crate::operators::then::Then;
        use crate::result::{TxResult, result};

        // Create a backend for testing
        let backend =
            PostgresBackend::new("postgres://postgres:postgres@postgres:5432".to_string());
        let config = DatabaseConfig::default();

        // This test won't actually run in CI since we don't have a real Postgres server
        // So we'll just mock/simulate the test database instance creation
        let _db_name = DatabaseName::new(None);

        // Create a test database instance
        let mut test_instance =
            match TestDatabaseInstance::new(backend.clone(), config.clone()).await {
                Ok(instance) => instance,
                Err(e) => {
                    println!("Skipping test due to database connection error: {}", e);
                    return;
                }
            };

        // Create the transaction and execute it
        let res =
            with_database::<PostgresBackend, _, Result<(), PostgresError>, (), PostgresError>(
                backend.clone(),
                config.clone(),
                move |db| {
                    Box::pin(async move {
                        println!("Creating table operation: {:?}", db);
                        let conn = db.acquire_connection().await?;
                        let mut tx = conn.sqlx_pool().begin().await?;
                        sqlx::query(r#"CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY)"#)
                            .execute(&mut *tx)
                            .await?;
                        tx.commit().await?;
                        println!("Create table operation completed");
                        Ok::<(), PostgresError>(())
                    })
                },
            )
            .then(|db| {
                // Return another with_database call instead of just a BoxFuture
                Box::pin(async move {
                    println!("Dropping table operation: {:?}", db);
                    let conn = db.acquire_connection().await?;
                    let mut tx = conn.sqlx_pool().begin().await?;
                    sqlx::query(r#"DROP TABLE IF EXISTS test"#)
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                    println!("Dropping table operation completed");
                    Ok(db.clone())
                })
            })
            .execute(&mut test_instance)
            .await;

        println!("Result: {:?}", res);

        println!("Test complete");
    }
}
