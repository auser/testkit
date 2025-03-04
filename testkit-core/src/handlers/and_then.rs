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
    first: A,
    next_fn: F,
    _phantom: PhantomData<(DB, B)>,
}

/// Extension trait that adds combinators to transaction handlers
pub trait TransactionHandlerExt<DB>: TransactionHandler<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
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
}

// Implement the extension trait for all handlers
impl<T, DB> TransactionHandlerExt<DB> for T
where
    T: TransactionHandler<DB>,
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
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
