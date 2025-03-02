use std::marker::PhantomData;

use crate::{IntoTransaction, Transaction};
use async_trait::async_trait;

#[derive(Debug)]
#[must_use]
pub struct Setup<Tx, F, Next> {
    tx: Tx,
    f: F,
    _phantom2: PhantomData<Next>,
}

/// Create a transaction that executes the primary transaction and then passes its result
/// to a function that returns the next transaction to execute.
pub fn setup<Tx, F, Next>(tx: Tx, f: F) -> Setup<Tx, F, Next>
where
    Tx: Transaction,
    F: Fn(Result<Tx::Item, Tx::Error>) -> Next + Send + Sync,
    Next: IntoTransaction<Tx::Context, Item = Tx::Item, Error = Tx::Error> + Send + Sync,
{
    Setup {
        tx,
        f,
        _phantom2: PhantomData,
    }
}

#[async_trait]
impl<Tx, F, Next> Transaction for Setup<Tx, F, Next>
where
    Tx: Transaction,
    F: Fn(Result<Tx::Item, Tx::Error>) -> Next + Send + Sync,
    Next: IntoTransaction<Tx::Context, Item = Tx::Item, Error = Tx::Error> + Send + Sync,
{
    type Context = Tx::Context;
    type Item = Next::Item;
    type Error = Next::Error;

    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        let result = self.tx.execute(ctx).await;
        (self.f)(result).into_transaction().execute(ctx).await
    }
}
