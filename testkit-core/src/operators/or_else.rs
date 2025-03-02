use std::marker::PhantomData;

use crate::{IntoTransaction, Transaction};

//
#[derive(Debug)]
#[must_use]
pub struct OrElse<Context, Fun, Context2> {
    tx: Context,
    f: Fun,
    _phantom2: PhantomData<Context2>,
}

pub fn or_else<Context, TruthBranch, Fun, FalseBranch>(
    tx: TruthBranch,
    f: Fun,
) -> OrElse<TruthBranch::Tx, Fun, FalseBranch>
where
    TruthBranch: IntoTransaction<Context>,
    FalseBranch: IntoTransaction<Context, Item = TruthBranch::Item>,
    Fun: Fn(TruthBranch::Error) -> FalseBranch,
{
    OrElse {
        tx: tx.into_transaction(),
        f,
        _phantom2: PhantomData,
    }
}
impl<Context, Context2, Fun> Transaction for OrElse<Context, Fun, Context2>
where
    Context: Transaction,
    Context2: IntoTransaction<Context::Context, Item = Context::Item, Error = Context::Error>,
    Fun: Fn(Context::Error) -> Context2,
{
    type Context = Context::Context;
    type Item = Context::Item;
    type Error = Context::Error;

    fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        let OrElse { tx, f, _phantom2 } = self;
        tx.execute(ctx)
            .or_else(|item| f(item).into_transaction().execute(ctx))
    }
}
