// src/handlers/and_then.rs
use async_trait::async_trait;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::{
    DatabaseBackend, TestContext,
    handlers::{IntoTransactionHandler, TransactionHandler},
};

/// Handler for chaining multiple operations together
pub struct AndThenHandler<DB, A, B, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    A: TransactionHandler<DB> + Send + Sync,
    B: TransactionHandler<DB, Error = A::Error> + Send + Sync,
    F: FnOnce(A::Item) -> B + Send + Sync + 'static,
{
    pub first: A,
    pub next_fn: F,
    pub _phantom: PhantomData<(DB, B)>,
}

#[async_trait]
impl<DB, A, B, F> TransactionHandler<DB> for AndThenHandler<DB, A, B, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    A: TransactionHandler<DB> + Send + Sync,
    B: TransactionHandler<DB, Error = A::Error> + Send + Sync,
    F: FnOnce(A::Item) -> B + Send + Sync + 'static,
{
    type Item = B::Item;
    type Error = A::Error;

    async fn execute(self, ctx: &mut TestContext<DB>) -> Result<Self::Item, Self::Error> {
        // Execute the first handler
        let a_result = self.first.execute(ctx).await?;

        // Create the second handler from the result of the first
        let b = (self.next_fn)(a_result);

        // Execute the second handler
        b.execute(ctx).await
    }
}

impl<DB, A, B, F> IntoTransactionHandler<DB> for AndThenHandler<DB, A, B, F>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    A: TransactionHandler<DB> + Send + Sync,
    B: TransactionHandler<DB, Error = A::Error> + Send + Sync,
    F: FnOnce(A::Item) -> B + Send + Sync + 'static,
{
    type Handler = Self;
    type Item = B::Item;
    type Error = A::Error;

    fn into_transaction_handler(self) -> Self::Handler {
        self
    }
}
