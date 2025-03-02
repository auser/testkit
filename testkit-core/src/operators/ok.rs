use std::marker::PhantomData;

use async_trait::async_trait;

use crate::Transaction;

#[derive(Debug)]
#[must_use]
pub struct TxnOk<Context, T, E> {
    ok: T,
    _phantom: PhantomData<(Context, E)>,
}

pub fn ok<Context, T, E>(ok: T) -> TxnOk<Context, T, E>
where
    T: Clone + Send + Sync,
    E: Clone + Send + Sync,
    Context: Clone + Send + Sync,
{
    TxnOk {
        ok,
        _phantom: PhantomData,
    }
}

#[async_trait]
impl<Context, T, E> Transaction for TxnOk<Context, T, E>
where
    T: Clone + Send + Sync,
    E: Clone + Send + Sync,
    Context: Clone + Send + Sync,
{
    type Context = Context;
    type Item = T;
    type Error = E;

    async fn execute(&self, _ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        Ok(self.ok.clone())
    }
}
