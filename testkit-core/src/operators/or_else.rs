use std::marker::PhantomData;

use async_trait::async_trait;

use crate::{IntoTransaction, Transaction};

#[derive(Debug)]
#[must_use]
pub struct OrElse<PrimaryTx, Recovery, FallbackTx> {
    tx: PrimaryTx,
    f: Recovery,
    _phantom2: PhantomData<FallbackTx>,
}

/// Create a composite transaction that tries the primary transaction first,
/// and if it fails, executes a fallback created from the error.
pub fn or_else<Ctx, PrimarySource, Recovery, FallbackSource>(
    tx: PrimarySource,
    f: Recovery,
) -> OrElse<PrimarySource::Tx, Recovery, FallbackSource>
where
    PrimarySource: IntoTransaction<Ctx>,
    FallbackSource: IntoTransaction<Ctx, Item = PrimarySource::Item, Error = PrimarySource::Error>,
    Recovery: Fn(PrimarySource::Error) -> FallbackSource + Send + Sync,
{
    OrElse {
        tx: tx.into_transaction(),
        f,
        _phantom2: PhantomData,
    }
}

#[async_trait]
impl<PrimaryTx, Recovery, FallbackTx> Transaction for OrElse<PrimaryTx, Recovery, FallbackTx>
where
    PrimaryTx: Transaction,
    Recovery: Fn(PrimaryTx::Error) -> FallbackTx + Send + Sync,
    FallbackTx: IntoTransaction<PrimaryTx::Context, Item = PrimaryTx::Item, Error = PrimaryTx::Error>
        + Send
        + Sync,
{
    type Context = PrimaryTx::Context;
    type Item = PrimaryTx::Item;
    type Error = PrimaryTx::Error;

    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        // Try the primary transaction first
        match self.tx.execute(ctx).await {
            Ok(value) => Ok(value),
            Err(err) => {
                // On error, execute the fallback transaction
                (self.f)(err).into_transaction().execute(ctx).await
            }
        }
    }
}
