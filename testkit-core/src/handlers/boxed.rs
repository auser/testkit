use std::fmt::Debug;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use crate::DatabasePool;
use crate::TestContext;
use crate::handlers::TransactionHandler;
use crate::testdb::DatabaseBackend;
use crate::testdb::DatabaseConfig;
use crate::testdb::DatabaseName;
use crate::testdb::TestDatabaseConnection;
use async_trait::async_trait;

/// Entry point for database operations with automatic boxing of closures
///
/// This provides the same functionality as `DatabaseEntryPoint` but automatically
/// boxes future closures to solve lifetime issues. Use the `boxed_async!` macro
/// to easily create boxed async blocks.
pub struct BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
}

/// Handler that stores a setup function
pub struct BoxedSetupHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
    setup_fn: Box<
        dyn for<'a> FnOnce(
                &'a mut <DB::Pool as crate::DatabasePool>::Connection,
            )
                -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send + 'a>>
            + Send
            + Sync,
    >,
}

/// Handler that stores both setup and transaction functions
pub struct BoxedTransactionHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
    setup_fn: Box<
        dyn for<'a> FnOnce(
                &'a mut <DB::Pool as crate::DatabasePool>::Connection,
            )
                -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send + 'a>>
            + Send
            + Sync,
    >,
    transaction_fn: Box<
        dyn for<'a> FnOnce(
                &'a mut <DB as DatabaseBackend>::Connection,
            )
                -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send + 'a>>
            + Send
            + Sync,
    >,
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
    /// This method takes a closure that will be executed during setup.
    /// Use the `boxed_async!` macro to create an async block that captures
    /// variables without lifetime issues.
    pub fn setup<F>(self, setup_fn: F) -> BoxedSetupHandler<DB>
    where
        F: for<'a> FnOnce(
                &'a mut <DB::Pool as crate::DatabasePool>::Connection,
            )
                -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send + 'a>>
            + Send
            + Sync
            + 'static,
    {
        BoxedSetupHandler {
            backend: self.backend,
            setup_fn: Box::new(setup_fn),
        }
    }

    /// Initialize a database with a transaction
    pub fn with_transaction<F>(self, transaction_fn: F) -> BoxedTransactionOnlyHandler<DB>
    where
        F: for<'a> FnOnce(
                &'a mut <DB as DatabaseBackend>::Connection,
            )
                -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send + 'a>>
            + Send
            + Sync
            + 'static,
    {
        BoxedTransactionOnlyHandler {
            backend: self.backend,
            transaction_fn: Box::new(transaction_fn),
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

#[async_trait]
impl<DB> TransactionHandler<DB> for BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    type Item = TestContext<DB>;
    type Error = DB::Error;

    async fn execute(self, _ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        // Create the database instance
        let db_instance =
            crate::testdb::TestDatabaseInstance::new(self.backend, DatabaseConfig::default())
                .await?;

        // Create and return the context
        Ok(crate::TestContext::new(db_instance))
    }
}

/// Handler that stores just a transaction function without setup
pub struct BoxedTransactionOnlyHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
    transaction_fn: Box<
        dyn for<'a> FnOnce(
                &'a mut <DB as DatabaseBackend>::Connection,
            )
                -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send + 'a>>
            + Send
            + Sync,
    >,
}

impl<DB> BoxedTransactionOnlyHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Execute this handler
    pub async fn execute(self) -> Result<crate::TestContext<DB>, DB::Error> {
        // Create the database instance
        let db_instance =
            crate::testdb::TestDatabaseInstance::new(self.backend, DatabaseConfig::default())
                .await?;

        // Create the context
        let ctx = crate::TestContext::new(db_instance.clone());

        // TRANSACTION: Get a connection for the transaction
        let mut conn = ctx.db.pool.acquire().await?;

        // Call the transaction function with a reference to the connection
        (self.transaction_fn)(&mut conn).await?;

        // Release the connection
        ctx.db.pool.release(conn).await?;

        // Return the context
        Ok(ctx)
    }
}

#[async_trait]
impl<DB> TransactionHandler<DB> for BoxedTransactionOnlyHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    type Item = TestContext<DB>;
    type Error = DB::Error;

    async fn execute(self, _ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        self.execute().await
    }
}

impl<DB> BoxedSetupHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Add a transaction function
    ///
    /// This method takes a closure that will be executed during transaction.
    /// Use the `boxed_async!` macro to create an async block that captures
    /// variables without lifetime issues.
    pub fn with_transaction<F>(self, transaction_fn: F) -> BoxedTransactionHandler<DB>
    where
        F: for<'a> FnOnce(
                &'a mut <DB as DatabaseBackend>::Connection,
            )
                -> Pin<Box<dyn Future<Output = Result<(), DB::Error>> + Send + 'a>>
            + Send
            + Sync
            + 'static,
    {
        BoxedTransactionHandler {
            backend: self.backend,
            setup_fn: self.setup_fn,
            transaction_fn: Box::new(transaction_fn),
        }
    }

    /// Execute this handler
    pub async fn execute(self) -> Result<crate::TestContext<DB>, DB::Error> {
        // Create the database instance
        let db_instance =
            crate::testdb::TestDatabaseInstance::new(self.backend, DatabaseConfig::default())
                .await?;

        // Create the context
        let mut ctx = crate::TestContext::new(db_instance);

        // Get a connection from the pool
        let mut conn = ctx.db.pool.acquire().await?;

        // Call the setup function with a reference to the connection
        (self.setup_fn)(&mut conn).await?;

        // Release the connection back to the pool
        ctx.db.pool.release(conn).await?;

        // Return the context
        Ok(ctx)
    }
}

#[async_trait]
impl<DB> TransactionHandler<DB> for BoxedSetupHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    type Item = TestContext<DB>;
    type Error = DB::Error;

    async fn execute(self, _ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        self.execute().await
    }
}

impl<DB> BoxedTransactionHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Execute this handler
    pub async fn execute(self) -> Result<crate::TestContext<DB>, DB::Error> {
        // Create the database instance
        let db_instance =
            crate::testdb::TestDatabaseInstance::new(self.backend, DatabaseConfig::default())
                .await?;

        // Create the context
        let mut ctx = crate::TestContext::new(db_instance);

        // SETUP: Get a connection from the pool
        let mut conn = ctx.db.pool.acquire().await?;

        // Call the setup function with a reference to the connection
        (self.setup_fn)(&mut conn).await?;

        // Release the connection back to the pool
        ctx.db.pool.release(conn).await?;

        // TRANSACTION: Get a new connection for the transaction
        let mut conn = ctx.db.pool.acquire().await?;

        // Call the transaction function with a reference to the connection
        (self.transaction_fn)(&mut conn).await?;

        // Release the connection
        ctx.db.pool.release(conn).await?;

        // Return the context
        Ok(ctx)
    }
}

#[async_trait]
impl<DB> TransactionHandler<DB> for BoxedTransactionHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    type Item = TestContext<DB>;
    type Error = DB::Error;

    async fn execute(self, _ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        self.execute().await
    }
}

/// Create a new database entry point with the given backend
///
/// This function creates a new entry point for working with databases.
/// Use the `boxed_async!` macro with `setup` and `with_transaction` to avoid lifetime issues.
pub fn with_boxed_database<DB>(backend: DB) -> BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    BoxedDatabaseEntryPoint::new(backend)
}

/// Create a new database entry point with the given backend and config
///
/// This function creates a new entry point for working with databases.
/// Use the `boxed_async!` macro with `setup` and `with_transaction` to avoid lifetime issues.
pub fn with_boxed_database_config<DB>(
    backend: DB,
    _config: DatabaseConfig,
) -> BoxedDatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    BoxedDatabaseEntryPoint::new(backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::TestDatabaseInstance;
    use async_trait::async_trait;

    #[derive(Debug, Clone)]
    struct MockError(String);

    impl std::fmt::Display for MockError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Mock error: {}", self.0)
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

    impl TestDatabaseConnection for MockConnection {
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
            MockBackend
        }
    }

    #[async_trait]
    impl crate::DatabaseBackend for MockBackend {
        type Connection = MockConnection;
        type Pool = MockPool;
        type Error = MockError;

        async fn new(_config: crate::DatabaseConfig) -> Result<Self, Self::Error> {
            Ok(MockBackend)
        }

        async fn create_pool(
            &self,
            _name: &DatabaseName,
            _config: &crate::DatabaseConfig,
        ) -> Result<Self::Pool, Self::Error> {
            Ok(MockPool)
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
            "mock://test".to_string()
        }
    }

    #[tokio::test]
    async fn test_boxed_database() {
        let backend = MockBackend::new();

        // Example with boxed_async macro
        let ctx = with_boxed_database(backend)
            .setup(|conn| {
                crate::boxed_async!(async move {
                    // This would be a query in real code
                    println!("Setting up database");
                    Ok(())
                })
            })
            .with_transaction(|conn| {
                crate::boxed_async!(async move {
                    // This would be a transaction in real code
                    println!("Running transaction");
                    Ok(())
                })
            })
            .execute()
            .await;

        assert!(ctx.is_ok());
    }
}
