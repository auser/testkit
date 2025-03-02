use std::marker::PhantomData;

use crate::Transaction;

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

impl<Context, T, E> Transaction for TxResult<Context, T, E>
where
    T: Clone + Send + Sync,
    E: Clone + Send + Sync,
    Context: Clone + Send + Sync,
{
    type Context = Context;
    type Item = T;
    type Error = E;

    fn execute<'a>(&'a self, _ctx: &'a mut Self::Context) -> Result<Self::Item, Self::Error> {
        self.r.clone()
    }
}
