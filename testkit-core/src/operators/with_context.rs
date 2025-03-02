use std::marker::PhantomData;

use crate::Transaction;

/// The result of `with_ctx`
#[derive(Debug)]
#[must_use]
pub struct WithContext<Context, Fun> {
    f: Fun,
    _phantom: PhantomData<Context>,
}

pub fn with_context<Context, F, T, E>(f: F) -> WithContext<Context, F>
where
    F: Fn(&mut Context) -> std::result::Result<T, E>,
{
    WithContext {
        f,
        _phantom: PhantomData,
    }
}

impl<Context, Fun, Type, Error> Transaction for WithContext<Context, Fun>
where
    Fun: Fn(&mut Context) -> std::result::Result<Type, Error>,
{
    type Context = Context;
    type Item = Type;
    type Error = Error;

    fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        (self.f)(ctx)
    }
}
