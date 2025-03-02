use std::future::Future;
use std::marker::PhantomData;

use crate::Transaction;
use async_trait::async_trait;

/// The result of `with_ctx`
#[derive(Debug)]
#[must_use]
pub struct WithContext<Context, Fun> {
    f: Fun,
    _phantom: PhantomData<Context>,
}

/// Create a transaction from a function that takes a context
pub fn with_context<Context, F, Fut, T, E>(f: F) -> WithContext<Context, F>
where
    F: Fn(&mut Context) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = std::result::Result<T, E>> + Send + 'static,
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
    Context: Send + Sync + 'static,
{
    WithContext {
        f,
        _phantom: PhantomData,
    }
}

#[async_trait]
impl<Context, Fun, Fut, Type, Error> Transaction for WithContext<Context, Fun>
where
    Fun: Fn(&mut Context) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = std::result::Result<Type, Error>> + Send + 'static,
    Type: Send + Sync + 'static,
    Error: Send + Sync + 'static,
    Context: Send + Sync + 'static,
{
    type Context = Context;
    type Item = Type;
    type Error = Error;

    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        (self.f)(ctx).await
    }
}
