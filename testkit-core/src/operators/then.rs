use std::marker::PhantomData;

use crate::{IntoTransaction, Transaction};

#[derive(Debug)]
#[must_use]
pub struct Then<Context1, Fun, Context2> {
    tx: Context1,
    f: Fun,
    _phantom2: PhantomData<Context2>,
}

pub fn then<Context1, Fun, Context2>(tx: Context1, f: Fun) -> Then<Context1, Fun, Context2>
where
    Context1: Transaction,
    Context2: Transaction,
    Fun: Fn(Context1::Item) -> Context2,
{
    Then {
        tx,
        f,
        _phantom2: PhantomData,
    }
}

impl<Context, Fun, Context2> Transaction for Then<Context, Fun, Context2>
where
    Context: Transaction,
    Context2: IntoTransaction<Context::Context, Error = Context::Error>,
    Fun: Fn(Result<Context::Item, Context::Error>) -> Context2,
{
    type Context = Context::Context;
    type Item = Context2::Item;
    type Error = Context2::Error;

    fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        let Then { tx, f, _phantom2 } = self;
        f(tx.execute(ctx)).into_transaction().execute(ctx)
    }
}
