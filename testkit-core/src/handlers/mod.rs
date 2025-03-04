use async_trait::async_trait;
use std::fmt::Debug;

use crate::testdb::{DatabaseBackend, DatabaseConfig, transaction::DatabaseTransaction};

mod and_then;
mod setup;
mod with_database;
mod with_transaction;

// Re-export all handler components
pub use and_then::{AndThenHandler, TransactionHandlerExt};
pub use setup::{SetupHandler, setup};
pub use with_database::DatabaseHandler;
pub use with_transaction::{
    DatabaseTransactionHandler, TransactionFnHandler, with_db_transaction, with_transaction,
};

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
