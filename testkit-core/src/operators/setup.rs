use std::marker::PhantomData;

use crate::{IntoTransaction, Transaction};

#[derive(Debug)]
#[must_use]
pub struct Setup<Context, Fun, Context2> {
    tx: Context,
    f: Fun,
    _phantom2: PhantomData<Context2>,
}

pub fn setup<Context, Fun, Context2>(tx: Context, f: Fun) -> Setup<Context, Fun, Context2>
where
    Context: Transaction,
    Context2: IntoTransaction<Context::Context, Error = Context::Error>,
    Fun: Fn(Context::Context) -> Context2,
{
    Setup {
        tx,
        f,
        _phantom2: PhantomData,
    }
}

impl<Context, Fun, Context2> Transaction for Setup<Context, Fun, Context2>
where
    Context: Transaction,
    Context2: IntoTransaction<Context::Context, Error = Context::Error>,
    Fun: Fn(Result<Context::Item, Context::Error>) -> Context2,
{
    type Context = Context::Context;
    type Item = Context2::Item;
    type Error = Context2::Error;

    fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        let Setup { tx, f, _phantom2 } = self;
        f(tx.execute(ctx)).into_transaction().execute(ctx)
    }
}
