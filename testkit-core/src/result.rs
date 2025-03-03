use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use async_trait::async_trait;

use crate::{BoxFuture, Transaction};

#[derive(Debug)]
#[must_use]
pub struct TxResult<Context, T, E> {
    r: Result<T, E>,
    _phantom: PhantomData<Context>,
}

pub fn result<Context, T, E>(r: Result<T, E>) -> TxResult<Context, T, E> {
    TxResult {
        r,
        _phantom: PhantomData,
    }
}

#[async_trait]
impl<Context, T, E> Transaction for TxResult<Context, T, E>
where
    T: Clone + Send + Sync,
    E: Clone + Send + Sync,
    Context: Clone + Send + Sync,
{
    type Context = Context;
    type Item = T;
    type Error = E;

    async fn execute(&self, _ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        self.r.clone()
    }
}
