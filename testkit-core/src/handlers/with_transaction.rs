// src/handlers/with_transaction.rs
use async_trait::async_trait;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::{
    DatabaseBackend, TestContext,
    handlers::{IntoTransactionHandler, TransactionHandler},
};

/// Handler for executing functions within a transaction
pub struct TransactionFnHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
{
    transaction_fn: F,
    _phantom: PhantomData<DB>,
}

/// Create a new transaction function handler
pub fn with_transaction<DB, F, Fut>(transaction_fn: F) -> TransactionFnHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    TransactionFnHandler {
        transaction_fn,
        _phantom: PhantomData,
    }
}

#[async_trait]
impl<DB, F, Fut> TransactionHandler<DB> for TransactionFnHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    type Item = ();
    type Error = DB::Error;

    async fn execute(self, ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        // Acquire a connection from the database
        let mut conn = ctx.db.acquire_connection().await?;

        // Execute the transaction function
        let result = (self.transaction_fn)(&mut conn).await;

        // Release the connection
        ctx.db.release_connection(conn).await?;

        result
    }
}

impl<DB, F, Fut> IntoTransactionHandler<DB> for TransactionFnHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    type Handler = Self;
    type Item = ();
    type Error = DB::Error;

    fn into_transaction_handler(self) -> Self::Handler {
        self
    }
}

/// Handler that combines a database instance with transaction management
pub struct DatabaseTransactionHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
{
    /// The database instance
    db: crate::TestDatabaseInstance<DB>,
    /// The transaction function
    transaction_fn: F,
}

/// Create a new database transaction handler
pub fn with_db_transaction<DB, F, Fut>(
    db: crate::TestDatabaseInstance<DB>,
    transaction_fn: F,
) -> DatabaseTransactionHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    DatabaseTransactionHandler { db, transaction_fn }
}

#[async_trait]
impl<DB, F, Fut> TransactionHandler<DB> for DatabaseTransactionHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    type Item = TestContext<DB>;
    type Error = DB::Error;

    async fn execute(self, _ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        // Create a new context with the database
        let ctx = TestContext::new(self.db);

        // Acquire a connection
        let mut conn = ctx.db.acquire_connection().await?;

        // Execute the transaction function
        (self.transaction_fn)(&mut conn).await?;

        // Release the connection
        ctx.db.release_connection(conn).await?;

        Ok(ctx)
    }
}

impl<DB, F, Fut> IntoTransactionHandler<DB> for DatabaseTransactionHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    F: FnOnce(&mut <DB as DatabaseBackend>::Connection) -> Fut + Send + Sync + 'static,
{
    type Handler = Self;
    type Item = TestContext<DB>;
    type Error = DB::Error;

    fn into_transaction_handler(self) -> Self::Handler {
        self
    }
}
