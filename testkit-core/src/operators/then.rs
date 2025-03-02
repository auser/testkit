use std::marker::PhantomData;

use async_trait::async_trait;

use crate::{IntoTransaction, Transaction};

#[derive(Debug)]
#[must_use]
pub struct Then<PrimaryTx, NextFn, NextTx> {
    tx: PrimaryTx,
    f: NextFn,
    _phantom2: PhantomData<NextTx>,
}

pub fn then<PrimaryTx, NextFn, NextTx>(tx: PrimaryTx, f: NextFn) -> Then<PrimaryTx, NextFn, NextTx>
where
    PrimaryTx: Transaction,
    NextTx: Transaction,
    NextFn: Fn(PrimaryTx::Item) -> NextTx,
{
    Then {
        tx,
        f,
        _phantom2: PhantomData,
    }
}

#[async_trait]
impl<PrimaryTx, NextFn, NextTx> Transaction for Then<PrimaryTx, NextFn, NextTx>
where
    // Primary transaction
    PrimaryTx: Transaction,
    // Function to create next transaction
    NextFn: Fn(PrimaryTx::Item) -> NextTx + Send + Sync,
    // Next transaction must be compatible with primary
    NextTx: IntoTransaction<PrimaryTx::Context, Item = PrimaryTx::Item, Error = PrimaryTx::Error>
        + Send
        + Sync,
{
    type Context = PrimaryTx::Context;
    type Item = PrimaryTx::Item;
    type Error = PrimaryTx::Error;

    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        // First await the primary transaction
        let result = self.tx.execute(ctx).await?;

        // Then pass the result to the function and execute the second transaction
        (self.f)(result).into_transaction().execute(ctx).await
    }
}
