use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use crate::testdb::{DatabaseBackend, DatabaseConfig, DatabasePool};

/// Entry point for database operations with automatic boxing of closures
///
/// This provides the same functionality as `DatabaseEntryPoint` but automatically
/// boxes future closures to solve lifetime issues.
pub struct BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
}

/// Traits for setup and transaction operations
pub trait BoxedSetupFn<DB: DatabaseBackend>: Send + Sync {
    fn call(
        self: Box<Self>,
        conn: &mut <DB::Pool as crate::DatabasePool>::Connection,
    ) -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send>>;
}

pub trait BoxedTransactionFn<DB: DatabaseBackend>: Send + Sync {
    fn call(
        self: Box<Self>,
        conn: &mut <DB as DatabaseBackend>::Connection,
    ) -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send>>;
}

/// Wrapper structs for each original closure type
struct SetupFnWrapper<DB, F, Fut>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
{
    f: Option<F>,
    _phantom: std::marker::PhantomData<DB>,
}

impl<DB, F, Fut> BoxedSetupFn<DB> for SetupFnWrapper<DB, F, Fut>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
{
    fn call(
        mut self: Box<Self>,
        conn: &mut <DB::Pool as crate::DatabasePool>::Connection,
    ) -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send>> {
        let f = self
            .f
            .take()
            .expect("Setup function can only be called once");
        Box::pin(f(conn))
    }
}

struct TransactionFnWrapper<DB, F, Fut>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    f: Option<F>,
    _phantom: std::marker::PhantomData<DB>,
}

impl<DB, F, Fut> BoxedTransactionFn<DB> for TransactionFnWrapper<DB, F, Fut>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    fn call(
        mut self: Box<Self>,
        conn: &mut <DB as DatabaseBackend>::Connection,
    ) -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send>> {
        let f = self
            .f
            .take()
            .expect("Transaction function can only be called once");
        Box::pin(f(conn))
    }
}

/// A setup handler with boxed closures
pub struct BoxedSetupHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
    setup_fn: Box<dyn BoxedSetupFn<DB>>,
}

/// A transaction handler with boxed closures
pub struct BoxedTransactionHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
    transaction_fn: Box<dyn BoxedTransactionFn<DB>>,
}

impl<DB> BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Create a new entry point with the given backend
    pub fn new(backend: DB) -> Self {
        Self { backend }
    }

    /// Set up the database with the given function
    ///
    /// This method automatically boxes the closure to avoid lifetime issues.
    pub fn setup<S, Fut>(self, setup_fn: S) -> BoxedSetupHandler<DB>
    where
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        S: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut
            + Send
            + Sync
            + 'static,
    {
        let wrapper = SetupFnWrapper {
            f: Some(setup_fn),
            _phantom: std::marker::PhantomData,
        };

        BoxedSetupHandler {
            backend: self.backend,
            setup_fn: Box::new(wrapper),
        }
    }

    /// Execute this handler
    pub async fn execute(self) -> Result<crate::TestContext<DB>, DB::Error> {
        // Create the database instance
        let db_instance =
            crate::testdb::TestDatabaseInstance::new(self.backend, DatabaseConfig::default())
                .await?;

        // Create and return the context
        Ok(crate::TestContext::new(db_instance))
    }
}

impl<DB> BoxedSetupHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Add a transaction function
    ///
    /// This method automatically boxes the closure to avoid lifetime issues.
    pub fn with_transaction<F, Fut>(self, transaction_fn: F) -> BoxedTransactionHandler<DB>
    where
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    {
        let wrapper = TransactionFnWrapper {
            f: Some(transaction_fn),
            _phantom: std::marker::PhantomData,
        };

        BoxedTransactionHandler {
            backend: self.backend,
            transaction_fn: Box::new(wrapper),
        }
    }

    /// Execute this handler
    pub async fn execute(self) -> Result<crate::TestContext<DB>, DB::Error> {
        // Create the database instance using the existing API
        let db_instance = crate::testdb::TestDatabaseInstance::new(
            self.backend,
            crate::DatabaseConfig::default(),
        )
        .await?;

        // Get a connection from the pool
        let mut conn = db_instance.pool.acquire().await?;

        // Execute the setup function
        self.setup_fn.call(&mut conn).await?;

        // Return the context
        Ok(crate::TestContext::new(db_instance))
    }
}

impl<DB> BoxedTransactionHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Execute this handler
    pub async fn execute(self) -> Result<crate::TestContext<DB>, DB::Error> {
        // Create the database instance using the existing API
        let db_instance = crate::testdb::TestDatabaseInstance::new(
            self.backend,
            crate::DatabaseConfig::default(),
        )
        .await?;

        // Get a connection from the pool
        let mut conn = db_instance.pool.acquire().await?;

        // Execute the transaction function
        self.transaction_fn.call(&mut conn).await?;

        // Return the context
        Ok(crate::TestContext::new(db_instance))
    }
}

/// Create a database entry point with automatic boxing of closures
///
/// This function provides the same functionality as `with_database` but
/// automatically boxes future closures to solve lifetime issues.
///
/// # Example
/// ```rust,no_run,ignore
/// use testkit_core::with_boxed_database;
///
/// async fn my_test() -> Result<(), Box<dyn std::error::Error>> {
///     let backend = MockBackend::new();
///     
///     // No need to manually box futures - they are boxed automatically
///     let ctx = with_boxed_database(backend)
///         .setup(|conn| async move {
///             // This closure can capture variables without lifetime issues
///             // Setup code here
///             Ok(())
///         })
///         .execute()
///         .await?;
///         
///     Ok(())
/// }
/// ```
pub fn with_boxed_database<DB>(backend: DB) -> BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    BoxedDatabaseEntryPoint::new(backend)
}

/// Create a database entry point with custom config and automatic boxing
///
/// This function provides the same functionality as `with_database_config` but
/// automatically boxes future closures to solve lifetime issues.
pub fn with_boxed_database_config<DB>(
    backend: DB,
    config: DatabaseConfig,
) -> BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    // Currently we don't use the config, but we might in the future
    let _ = config;
    with_boxed_database(backend)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Define a simple MockBackend for testing
    use async_trait::async_trait;

    // Use our own mock types for testing
    #[derive(Debug, Clone)]
    struct MockError(String);

    impl std::fmt::Display for MockError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "MockError: {}", self.0)
        }
    }

    impl std::error::Error for MockError {}

    impl From<String> for MockError {
        fn from(s: String) -> Self {
            MockError(s)
        }
    }

    #[derive(Debug, Clone)]
    struct MockConnection;

    impl crate::TestDatabaseConnection for MockConnection {
        fn connection_string(&self) -> String {
            "mock://test".to_string()
        }
    }

    #[derive(Debug, Clone)]
    struct MockPool;

    #[async_trait]
    impl crate::DatabasePool for MockPool {
        type Connection = MockConnection;
        type Error = MockError;

        async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
            Ok(MockConnection)
        }

        async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
            Ok(())
        }

        fn connection_string(&self) -> String {
            "mock://test".to_string()
        }
    }

    #[derive(Debug, Clone)]
    struct MockBackend;

    impl MockBackend {
        fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl crate::DatabaseBackend for MockBackend {
        type Connection = MockConnection;
        type Pool = MockPool;
        type Error = MockError;

        async fn new(_config: crate::DatabaseConfig) -> Result<Self, Self::Error> {
            Ok(Self)
        }

        async fn create_pool(
            &self,
            _name: &crate::DatabaseName,
            _config: &crate::DatabaseConfig,
        ) -> Result<Self::Pool, Self::Error> {
            Ok(MockPool)
        }

        async fn create_database(
            &self,
            _pool: &Self::Pool,
            _name: &crate::DatabaseName,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        fn drop_database(&self, _name: &crate::DatabaseName) -> Result<(), Self::Error> {
            Ok(())
        }

        fn connection_string(&self, _name: &crate::DatabaseName) -> String {
            "mock://test".to_string()
        }
    }

    #[tokio::test]
    async fn test_boxed_database() {
        let backend = MockBackend::new();

        // Test with capturing a local variable (this would cause lifetime issues without boxing)
        let local_var = String::from("test data");

        let result = with_boxed_database(backend)
            .setup(|_conn| async move {
                // Capture the local variable
                println!("Using local variable: {}", local_var);
                Ok(())
            })
            .execute()
            .await;

        assert!(
            result.is_ok(),
            "Failed to execute boxed database: {:?}",
            result.err()
        );
    }
}
