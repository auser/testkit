use std::marker::PhantomData;

use crate::{IntoTransaction, Transaction};
use async_trait::async_trait;

pub fn and_then<Ctx, A, F, B>(a: A, f: F) -> AndThen<A::Tx, F, B>
where
    A: IntoTransaction<Ctx>,
    B: IntoTransaction<Ctx, Error = A::Error>,
    F: Fn(A::Item) -> B,
{
    AndThen {
        tx: a.into_transaction(),
        f,
        _phantom: PhantomData,
    }
}

/// The result of `and_then`
#[derive(Debug)]
#[must_use]
pub struct AndThen<Tx1, F, Tx2> {
    tx: Tx1,
    f: F,
    _phantom: PhantomData<Tx2>,
}

#[async_trait]
impl<Tx, Tx2, F> Transaction for AndThen<Tx, F, Tx2>
where
    Tx2: IntoTransaction<Tx::Context, Item = Tx::Item, Error = Tx::Error> + Send + Sync,
    Tx: Transaction,
    F: Fn(Tx::Item) -> Tx2 + Send + Sync,
    Tx::Item: Send + Sync,
{
    type Context = Tx::Context;
    type Item = Tx2::Item;
    type Error = Tx2::Error;

    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        let &AndThen { tx, f, .. } = &self;
        match tx.execute(ctx).await {
            Ok(item) => f(item).into_transaction().execute(ctx).await,
            Err(e) => Err(e),
        }
    }
}
