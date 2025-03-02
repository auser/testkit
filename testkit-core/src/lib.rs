#[cfg(feature = "tracing")]
mod tracing;

#[cfg(feature = "tracing")]
pub use tracing::*;

// #[cfg(feature = "env")]
pub mod prelude {
    // #[cfg(feature = "env")]
    // pub use crate::env::*;
    pub use crate::operators::*;
    pub use crate::result::result;
}

mod operators;
mod result;

pub use operators::*;
pub use result::*;

// /// A boxed future that resolves to a Result
// pub type BoxFuture<'a, T, E> =
//     std::pin::Pin<Box<dyn std::future::Future<Output = std::result::Result<T, E>> + Send + 'a>>;

#[must_use]
pub trait Transaction {
    /// The context of the transaction
    type Context;
    /// Return type of the transaction
    type Item;
    /// Error type of the transaction
    type Error;

    /// Execute the transaction
    fn execute<'a>(&'a self, ctx: &'a mut Self::Context) -> Result<Self::Item, Self::Error>;

    /// Box the transaction
    fn boxed<'a>(self) -> Box<Self>
    where
        Self: Sized + 'a,
    {
        Box::new(self)
    }

    fn then<Fun, NextTx, NextItem>(self, f: Fun) -> Then<Self, Fun, NextTx>
    where
        Self: Sized,
        Fun: Fn(Self::Item) -> NextTx,
        NextTx: Transaction<Context = Self::Context, Error = Self::Error, Item = NextItem>,
    {
        then(self, f)
    }
}

/// types than can be converted into transaction
pub trait IntoTransaction<Context> {
    type Tx: Transaction<Context = Context, Item = Self::Item, Error = Self::Error>;
    type Error;
    type Item;

    fn into_transaction(self) -> Self::Tx;
}

impl<Tx, Context> IntoTransaction<Context> for Tx
where
    Tx: Transaction<Context = Context>,
{
    type Tx = Tx;
    type Error = Tx::Error;
    type Item = Tx::Item;

    fn into_transaction(self) -> Self::Tx {
        self
    }
}

impl<Context, T, E> IntoTransaction<Context> for Result<T, E>
where
    T: Clone + Send + Sync,
    E: Clone + Send + Sync,
    Context: Clone + Send + Sync,
{
    type Tx = result::TxResult<Context, T, E>;
    type Error = E;
    type Item = T;

    fn into_transaction(self) -> Self::Tx {
        result::result(self)
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct TestConn<'a> {
        value: &'a i32,
        _phantom: PhantomData<&'a i32>,
    }

    impl<'a> TestConn<'a> {
        fn value(&self) -> &'a i32 {
            self.value
        }
    }

    pub fn with_test_conn<'a, F>(f: F) -> WithConn<'a, F>
    where
        F: Fn(&mut TestConn<'a>) -> Result<i32, ()>,
    {
        WithConn {
            f,
            _phantom: PhantomData,
        }
    }

    impl<'a, F> Transaction for WithConn<'a, F>
    where
        F: Fn(&mut TestConn<'a>) -> Result<i32, ()>,
    {
        type Context = TestConn<'a>;
        type Item = i32;
        type Error = ();

        fn execute(&self, ctx: &mut TestConn<'a>) -> Result<Self::Item, Self::Error> {
            (self.f)(ctx)
        }
    }

    #[derive(Debug)]
    pub struct WithConn<'a, F> {
        f: F,
        _phantom: PhantomData<&'a i32>,
    }

    #[test]
    fn test_with_test_db_works() {
        let tx = with_test_conn(move |conn| {
            // We need to make TestConn have a mutable field or use interior mutability
            // For now, we'll just return a value without modifying
            let res = *conn.value() + 1;
            Ok(res)
        });

        // Create a test value and connection
        let value = 42;
        let mut conn = TestConn {
            value: &value,
            _phantom: PhantomData,
        };

        // Execute the transaction
        let result = tx.execute(&mut conn);

        // Verify the result
        assert_eq!(result, Ok(43));
    }
}
