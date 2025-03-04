use async_trait::async_trait;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::testdb::{DatabaseBackend, DatabaseConfig};

mod and_then;
mod setup;
mod with_database;
mod with_transaction;

// Re-export all handler components
pub use and_then::AndThenHandler;
pub use setup::{SetupHandler, setup};
pub use with_database::DatabaseHandler;
pub use with_transaction::{
    DatabaseTransactionHandler, TransactionFnHandler, with_db_transaction, with_transaction,
};

/// Entry point for creating a database handler
///
/// This is the primary way to start a chain of database operations.
///
/// # Example
/// ```rust,no_run,ignore
/// use testkit_core::*;
///
/// async fn test() -> Result<(), Box<dyn std::error::Error>> {
///     let backend = testkit_core::testdb::tests::MockBackend::new();
///     let ctx = with_database(backend)
///        .setup(|conn| async { /* setup code */ Ok(()) })
///        .with_transaction(|tx| async { /* transaction code */ Ok(()) })
///        .execute()
///        .await?;
///     Ok(())
/// }
/// ```
#[must_use]
#[allow(dead_code)]
pub fn with_database<DB>(backend: DB) -> DatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    DatabaseEntryPoint { backend }
}

/// Handler that serves as the entry point for database operations
pub struct DatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    backend: DB,
}

impl<DB> DatabaseEntryPoint<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Initialize a new database and set it up
    pub fn setup<S, Fut>(self, setup_fn: S) -> DatabaseSetupHandler<DB, S>
    where
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        S: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut
            + Send
            + Sync
            + 'static,
    {
        DatabaseSetupHandler {
            backend: self.backend,
            setup_fn,
        }
    }

    /// Initialize a database with a transaction
    pub fn with_transaction<F, Fut>(
        self,
        transaction_fn: F,
    ) -> DatabaseWithTransactionHandler<DB, F>
    where
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    {
        DatabaseWithTransactionHandler {
            backend: self.backend,
            transaction_fn,
        }
    }

    /// Execute the database initialization
    pub async fn execute(self) -> Result<crate::TestContext<DB>, DB::Error> {
        let config = DatabaseConfig::default();
        let db_instance = crate::testdb::TestDatabaseInstance::new(self.backend, config).await?;
        Ok(crate::TestContext::new(db_instance))
    }
}

/// Handler that initializes a database and then runs a setup function
pub struct DatabaseSetupHandler<DB, S>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
{
    backend: DB,
    setup_fn: S,
}

impl<DB, S> DatabaseSetupHandler<DB, S>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
{
    /// Add a transaction function to this handler
    pub fn with_transaction<F, Fut>(
        self,
        transaction_fn: F,
    ) -> DatabaseSetupWithTransactionHandler<DB, S, F>
    where
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    {
        DatabaseSetupWithTransactionHandler {
            backend: self.backend,
            setup_fn: self.setup_fn,
            transaction_fn,
        }
    }

    /// Execute this handler
    pub async fn execute<Fut>(self) -> Result<crate::TestContext<DB>, DB::Error>
    where
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        S: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut
            + Send
            + Sync
            + 'static,
    {
        // Create the database instance
        let config = DatabaseConfig::default();
        let db_instance = crate::testdb::TestDatabaseInstance::new(self.backend, config).await?;

        // Run the setup function
        db_instance.setup(self.setup_fn).await?;

        // Return the context
        Ok(crate::TestContext::new(db_instance))
    }
}

/// Handler that initializes a database and then runs a setup function followed by a transaction
pub struct DatabaseSetupWithTransactionHandler<DB, S, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
    F: Send + Sync + 'static,
{
    backend: DB,
    setup_fn: S,
    transaction_fn: F,
}

impl<DB, S, F> DatabaseSetupWithTransactionHandler<DB, S, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
    F: Send + Sync + 'static,
{
    /// Execute this handler
    pub async fn execute<SFut, TFut>(self) -> Result<crate::TestContext<DB>, DB::Error>
    where
        SFut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        S: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> SFut
            + Send
            + Sync
            + 'static,
        TFut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> TFut + Send + Sync + 'static,
    {
        // Create the database instance
        let config = DatabaseConfig::default();
        let db_instance = crate::testdb::TestDatabaseInstance::new(self.backend, config).await?;

        // Run the setup function
        db_instance.setup(self.setup_fn).await?;

        // Create a context
        let ctx = crate::TestContext::new(db_instance.clone());

        // Run the transaction
        let mut conn = ctx.db.acquire_connection().await?;
        (self.transaction_fn)(&mut conn).await?;
        ctx.db.release_connection(conn).await?;

        // Return the context
        Ok(ctx)
    }
}

/// Handler that initializes a database and then runs a transaction
pub struct DatabaseWithTransactionHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
{
    backend: DB,
    transaction_fn: F,
}

impl<DB, F> DatabaseWithTransactionHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
{
    /// Execute this handler
    pub async fn execute<Fut>(self) -> Result<crate::TestContext<DB>, DB::Error>
    where
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    {
        // Create the database instance
        let config = DatabaseConfig::default();
        let db_instance = crate::testdb::TestDatabaseInstance::new(self.backend, config).await?;

        // Create a context
        let ctx = crate::TestContext::new(db_instance.clone());

        // Run the transaction
        let mut conn = ctx.db.acquire_connection().await?;
        (self.transaction_fn)(&mut conn).await?;
        ctx.db.release_connection(conn).await?;

        // Return the context
        Ok(ctx)
    }
}

/// Trait for handlers that can be executed in a transaction context
#[async_trait]
pub trait TransactionHandler<DB>: Send + Sync
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// The result type of this handler
    type Item;
    /// The error type
    type Error: From<DB::Error> + Send + Sync;

    /// Execute the handler with the given context
    async fn execute(self, ctx: &mut crate::TestContext<DB>) -> Result<Self::Item, Self::Error>;

    /// Execute this handler with a new context
    ///
    /// This is a convenience method that creates a new context using the provided database
    /// backend and then executes the handler with it.
    async fn execute_standalone(self, backend: DB) -> Result<Self::Item, Self::Error>
    where
        Self: Sized,
    {
        // Create a context with the provided database
        let config = DatabaseConfig::default();
        let db_instance = crate::testdb::TestDatabaseInstance::new(backend, config).await?;
        let mut ctx = crate::TestContext::new(db_instance);

        // Execute with the new context
        self.execute(&mut ctx).await
    }

    /// Chain two handlers together, where the second handler may depend on the result of the first
    fn and_then<F, B>(self, f: F) -> AndThenHandler<DB, Self, B, F>
    where
        Self: Sized,
        B: TransactionHandler<DB, Error = Self::Error> + Send + Sync,
        F: FnOnce(Self::Item) -> B + Send + Sync + 'static,
    {
        AndThenHandler {
            first: self,
            next_fn: f,
            _phantom: PhantomData,
        }
    }

    /// Add a setup operation to this handler
    fn setup<S, Fut, E>(
        self,
        setup_fn: S,
    ) -> impl TransactionHandler<DB, Item = (), Error = Self::Error>
    where
        Self: Sized,
        E: From<DB::Error> + From<Self::Error> + Send + Sync,
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        S: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut
            + Send
            + Sync
            + 'static,
    {
        self.and_then(move |_| {
            let handler = setup(setup_fn);
            SetupHandlerWrapper::<DB, S, Self::Error>::new(handler)
        })
    }

    /// Add a transaction operation to this handler
    fn with_transaction<F, Fut, E>(
        self,
        transaction_fn: F,
    ) -> impl TransactionHandler<DB, Item = (), Error = Self::Error>
    where
        Self: Sized,
        E: From<DB::Error> + From<Self::Error> + Send + Sync,
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    {
        self.and_then(move |_| {
            let handler = with_transaction(transaction_fn);
            TransactionFnHandlerWrapper::<DB, F, Self::Error>::new(handler)
        })
    }

    /// Create a database transaction handler from this handler
    fn with_db_transaction<F, Fut, E>(
        self,
        db: crate::TestDatabaseInstance<DB>,
        transaction_fn: F,
    ) -> impl TransactionHandler<DB, Item = crate::TestContext<DB>, Error = Self::Error>
    where
        Self: Sized,
        E: From<DB::Error> + From<Self::Error> + Send + Sync,
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    {
        self.and_then(move |_| {
            let handler = with_db_transaction(db, transaction_fn);
            DbTransactionHandlerWrapper::<DB, F, Self::Error>::new(handler)
        })
    }

    /// Run this handler with a new database instance
    async fn run_with_database(self, backend: DB) -> Result<crate::TestContext<DB>, Self::Error>
    where
        Self: Sized,
    {
        let config = DatabaseConfig::default();
        let db_instance = crate::testdb::TestDatabaseInstance::new(backend, config).await?;
        let mut ctx = crate::TestContext::new(db_instance);

        self.execute(&mut ctx).await?;

        Ok(ctx)
    }
}

/// Types that can be converted into a transaction handler
pub trait IntoTransactionHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// The handler type
    type Handler: TransactionHandler<DB, Item = Self::Item, Error = Self::Error>;
    /// The result type
    type Item;
    /// The error type
    type Error: From<DB::Error> + Send + Sync;

    /// Convert this type into a transaction handler
    fn into_transaction_handler(self) -> Self::Handler;
}

/// Helper function to run a transaction handler with a database
pub async fn run_with_database<DB, H>(
    backend: DB,
    handler: H,
) -> Result<crate::TestContext<DB>, H::Error>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    H: TransactionHandler<DB>,
{
    let config = DatabaseConfig::default();
    let db_instance = crate::testdb::TestDatabaseInstance::new(backend, config).await?;
    let mut ctx = crate::TestContext::new(db_instance);

    handler.execute(&mut ctx).await?;

    Ok(ctx)
}

// Wrapper types to handle error conversion

/// Wrapper for SetupHandler that converts error types
pub struct SetupHandlerWrapper<DB, S, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    inner: SetupHandler<DB, S>,
    _error: PhantomData<E>,
}

impl<DB, S, E> SetupHandlerWrapper<DB, S, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    pub fn new(inner: SetupHandler<DB, S>) -> Self {
        Self {
            inner,
            _error: PhantomData,
        }
    }
}

#[async_trait]
impl<DB, S, Fut, E> TransactionHandler<DB> for SetupHandlerWrapper<DB, S, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    E: From<DB::Error> + Send + Sync,
{
    type Item = ();
    type Error = E;

    async fn execute(self, ctx: &mut crate::TestContext<DB>) -> Result<Self::Item, Self::Error> {
        self.inner.execute(ctx).await.map_err(|e| e.into())
    }
}

/// Wrapper for TransactionFnHandler that converts error types
pub struct TransactionFnHandlerWrapper<DB, F, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    inner: TransactionFnHandler<DB, F>,
    _error: PhantomData<E>,
}

impl<DB, F, E> TransactionFnHandlerWrapper<DB, F, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    pub fn new(inner: TransactionFnHandler<DB, F>) -> Self {
        Self {
            inner,
            _error: PhantomData,
        }
    }
}

#[async_trait]
impl<DB, F, Fut, E> TransactionHandler<DB> for TransactionFnHandlerWrapper<DB, F, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    E: From<DB::Error> + Send + Sync,
{
    type Item = ();
    type Error = E;

    async fn execute(self, ctx: &mut crate::TestContext<DB>) -> Result<Self::Item, Self::Error> {
        self.inner.execute(ctx).await.map_err(|e| e.into())
    }
}

/// Wrapper for DatabaseTransactionHandler that converts error types
pub struct DbTransactionHandlerWrapper<DB, F, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    inner: DatabaseTransactionHandler<DB, F>,
    _error: PhantomData<E>,
}

impl<DB, F, E> DbTransactionHandlerWrapper<DB, F, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    pub fn new(inner: DatabaseTransactionHandler<DB, F>) -> Self {
        Self {
            inner,
            _error: PhantomData,
        }
    }
}

#[async_trait]
impl<DB, F, Fut, E> TransactionHandler<DB> for DbTransactionHandlerWrapper<DB, F, E>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    E: From<DB::Error> + Send + Sync,
{
    type Item = crate::TestContext<DB>;
    type Error = E;

    async fn execute(self, ctx: &mut crate::TestContext<DB>) -> Result<Self::Item, Self::Error> {
        self.inner.execute(ctx).await.map_err(|e| e.into())
    }
}
