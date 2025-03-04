use async_trait::async_trait;
use std::fmt::Debug;

use crate::testdb::{DatabaseBackend, transaction::DatabaseTransaction};

mod setup;
use crate::TestContext;
pub(crate) use setup::SetupHandler;

/// Trait for handlers that can be executed in the context of a database
#[async_trait]
pub trait DatabaseHandler<DB>: Send + Sync
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Execute the handler with the given context
    async fn execute(&self, ctx: &mut TestContext<DB>) -> Result<(), DB::Error>;
}

/// Trait for handlers that can be executed in the context of a transaction
#[async_trait]
pub trait TransactionHandler<DB, T>: Send + Sync
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    T: DatabaseTransaction<Error = DB::Error> + Send + Sync + 'static,
{
    /// Execute the handler with the given transaction
    #[allow(unused)]
    async fn execute(&self, tx: &mut T) -> Result<(), DB::Error>;
}

// Implementation of DatabaseHandler for FnOnce closures
#[async_trait]
impl<DB, F, Fut> DatabaseHandler<DB> for F
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: FnOnce(&mut TestContext<DB>) -> Fut + Send + Sync + Clone,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
{
    async fn execute(&self, ctx: &mut TestContext<DB>) -> Result<(), DB::Error> {
        self.clone()(ctx).await
    }
}

// Implementation of TransactionHandler for FnOnce closures
#[async_trait]
impl<DB, T, F, Fut> TransactionHandler<DB, T> for F
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    T: DatabaseTransaction<Error = DB::Error> + Send + Sync + 'static,
    F: FnOnce(&mut T) -> Fut + Send + Sync + Clone,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
{
    async fn execute(&self, tx: &mut T) -> Result<(), DB::Error> {
        self.clone()(tx).await
    }
}
