use async_trait::async_trait;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::testdb::{DatabaseBackend, DatabaseConfig};

mod and_then;
pub mod boxed;
mod setup;
mod with_database;
mod with_transaction;

#[cfg(test)]
mod tests;

// Re-export all handler components
pub use and_then::AndThenHandler;
pub use boxed::{BoxedDatabaseEntryPoint, with_boxed_database, with_boxed_database_config};
pub use setup::{SetupHandler, setup};
pub use with_database::DatabaseHandler;
pub use with_transaction::{
    DatabaseTransactionHandler, TransactionFnHandler, with_db_transaction, with_transaction,
};

/// Database handlers for the testkit crate
///
/// This module provides handlers for database operations. There are two APIs available:
///
/// 1. **Standard API** - The original API that requires manual boxing of closures if you need to
///    capture variables with complex lifetimes.
///
/// 2. **Boxed API** - An enhanced API that automatically boxes closures to avoid lifetime issues
///    when capturing variables.
///
/// # Standard API Example:
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
///
/// # Boxed API Example:
/// ```rust,no_run,ignore
/// use testkit_core::*;
///
/// async fn test() -> Result<(), Box<dyn std::error::Error>> {
///     let backend = testkit_core::testdb::tests::MockBackend::new();
///     
///     // This local variable would cause lifetime issues with the standard API
///     let table_name = "users".to_string();
///     
///     let ctx = with_boxed_database(backend)
///        .setup(|conn| async move {
///            // Can capture local variables without lifetime issues
///            let query = format!("CREATE TABLE {}", table_name);
///            // setup code
///            Ok(())
///        })
///        .with_transaction(|tx| async move {
///            // Transaction code
///            Ok(())
///        })
///        .execute()
///        .await?;
///     Ok(())
/// }
/// ```
// Re-export boxed API as the primary API
pub use boxed::{
    with_boxed_database as with_database, with_boxed_database_config as with_database_config,
};

/// A trait for handlers that can work with transactions
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

/// A trait for converting types into transaction handlers
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

/// Helper function to run a handler with a new database
pub async fn run_with_database<DB, H>(
    backend: DB,
    handler: H,
) -> Result<crate::TestContext<DB>, H::Error>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    H: TransactionHandler<DB>,
{
    handler.run_with_database(backend).await
}

/// Helper wrapper for SetupHandler
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
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    S: FnOnce(&mut <DB::Pool as crate::DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    type Item = ();
    type Error = E;

    async fn execute(self, ctx: &mut crate::TestContext<DB>) -> Result<Self::Item, Self::Error> {
        Ok(self.inner.execute(ctx).await?)
    }
}

/// Helper wrapper for TransactionFnHandler
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
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    type Item = ();
    type Error = E;

    async fn execute(self, ctx: &mut crate::TestContext<DB>) -> Result<Self::Item, Self::Error> {
        Ok(self.inner.execute(ctx).await?)
    }
}

/// Helper wrapper for DatabaseTransactionHandler
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
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
    E: From<DB::Error> + Send + Sync,
{
    type Item = crate::TestContext<DB>;
    type Error = E;

    async fn execute(self, ctx: &mut crate::TestContext<DB>) -> Result<Self::Item, Self::Error> {
        Ok(self.inner.execute(ctx).await?)
    }
}
