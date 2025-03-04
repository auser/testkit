// src/handlers/setup.rs
use async_trait::async_trait;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::{
    DatabaseBackend, DatabasePool, TestContext,
    handlers::{IntoTransactionHandler, TransactionHandler},
};

/// Handler for database setup operations
pub struct SetupHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: Send + Sync + 'static,
{
    setup_fn: F,
    _phantom: PhantomData<DB>,
}

/// Create a new setup handler
pub fn setup<DB, F, Fut>(setup_fn: F) -> SetupHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
{
    SetupHandler {
        setup_fn,
        _phantom: PhantomData,
    }
}

#[async_trait]
impl<DB, F, Fut> TransactionHandler<DB> for SetupHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
{
    type Item = ();
    type Error = DB::Error;

    async fn execute(self, ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        let mut conn = ctx.db.acquire_connection().await?;
        (self.setup_fn)(&mut conn).await?;
        Ok(())
    }
}

impl<DB, F, Fut> IntoTransactionHandler<DB> for SetupHandler<DB, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
{
    type Handler = Self;
    type Item = ();
    type Error = DB::Error;

    fn into_transaction_handler(self) -> Self::Handler {
        self
    }
}
