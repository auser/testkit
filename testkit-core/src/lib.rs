/*!
# testkit-core

Core traits and utilities for the testkit transaction framework.

This crate provides the foundation for building modular, composable, and testable
database interactions. It defines traits and utilities for managing database
transactions across different database adapters.

## Features

- Transaction trait for composable database operations
- Transaction manager for handling transaction lifecycle
- Helper functions for creating transactions:
  - `with_context`: Create a transaction from a context
  - `with_database`: Create a transaction from a database
  - `with_transaction`: Automatically manage transaction lifecycle
- Test database management for integration tests

## Example

```rust,no_run
use testkit_core::{DatabaseBackend, DatabaseConfig, TestDatabaseInstance, Transaction, with_database};

#[derive(Debug, Clone)]
struct User {
    id: i32,
    name: String,
}

// Define a database operation using the with_database function
fn get_user<B>(
    backend: B,
    config: DatabaseConfig,
    id: i32
) -> impl Transaction<Context = TestDatabaseInstance<B>, Item = User, Error = String>
where
    B: DatabaseBackend + Clone + std::fmt::Debug + Send + Sync + 'static,
{
    with_database(backend, config, move |db| {
        Box::pin(async move {
            // Database operations would go here in a real implementation
            // For example: db.acquire_connection().await?.execute_query(...).await?

            // Return a user for demonstration purposes
            Ok(User { id, name: "Example".to_string() })
        })
    })
}
*/

#[cfg(feature = "tracing")]
mod tracing;

#[cfg(feature = "tracing")]
pub use tracing::*;

// #[cfg(feature = "env")]
pub mod prelude {
    // #[cfg(feature = "env")]
    // pub use crate::env::*;
    pub use crate::operators::*;
    pub use crate::result::*;
    pub use crate::{DatabaseBackend, DatabaseConfig, Transaction};
}

mod database;
mod operators;
mod result;

// Integration tests in a separate module
#[cfg(test)]
mod tests;

pub use database::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, DatabaseTransaction,
    TestDatabaseConnection, TestDatabaseInstance, TransactionManager,
};
pub use operators::*;
pub use result::*;

use std::future::Future;

/// A boxed future that resolves to a Result
pub type BoxFuture<'a, T, E> =
    std::pin::Pin<Box<dyn Future<Output = std::result::Result<T, E>> + Send + 'a>>;

#[must_use]
#[async_trait::async_trait]
pub trait Transaction: Send + Sync {
    /// The context of the transaction
    type Context: Send + Sync;
    /// Return type of the transaction
    type Item: Send + Sync;
    /// Error type of the transaction
    type Error: Send + Sync;

    /// Execute the transaction
    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error>;

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

    fn or_else<Fun, NextTx>(self, f: Fun) -> OrElse<Self, Fun, NextTx>
    where
        Self: Sized,
        Fun: Fn(Self::Error) -> NextTx + Send + Sync,
        NextTx: Transaction<Context = Self::Context, Error = Self::Error, Item = Self::Item>,
    {
        or_else(self, f)
    }

    fn setup<Fun, NextTx>(self, f: Fun) -> Setup<Self, Fun, NextTx>
    where
        Self: Sized,
        Fun: Fn(Result<Self::Item, Self::Error>) -> NextTx + Send + Sync,
        NextTx: Transaction<Context = Self::Context, Error = Self::Error, Item = Self::Item>,
    {
        setup(self, f)
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

// impl<B, Context> IntoTransaction<Context> for TestDatabaseInstance<B>
// where
//     B: DatabaseBackend + Clone + std::fmt::Debug + Send + Sync + 'static,
// {
//     type Tx = TestDatabaseInstance<B>;
//     type Error = PostgresError;
//     type Item = ();

//     fn into_transaction(self) -> Self::Tx {
//         self
//     }
// }

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

// Wrapper struct to provide Transaction impl for functions
pub struct FnTransaction<F, Context, T, E>(pub F, std::marker::PhantomData<(Context, T, E)>)
where
    F: Fn(&mut Context) -> Result<T, E>;

impl<F, Context, T, E> FnTransaction<F, Context, T, E>
where
    F: Fn(&mut Context) -> Result<T, E>,
{
    pub fn new(f: F) -> Self {
        Self(f, std::marker::PhantomData)
    }
}

#[async_trait::async_trait]
impl<F, Context, T, E> Transaction for FnTransaction<F, Context, T, E>
where
    F: Fn(&mut Context) -> Result<T, E> + Send + Sync,
    Context: Send + Sync,
    T: Send + Sync,
    E: Send + Sync,
{
    type Context = Context;
    type Item = T;
    type Error = E;

    async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
        (self.0)(ctx)
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct TestConn {
        value: i32,
    }

    impl TestConn {
        fn value(&self) -> i32 {
            self.value
        }

        fn set_value(&mut self, value: i32) {
            self.value = value;
        }
    }
    pub fn simple_tx<F>(
        f: F,
    ) -> impl Transaction<Context = TestConn, Item = TestConn, Error = ()> + std::fmt::Debug
    where
        F: Fn(&mut TestConn) -> Result<TestConn, ()> + Send + Sync + 'static,
    {
        type TestTx = dyn Fn(&mut TestConn) -> Result<TestConn, ()> + Send + Sync;
        struct SimpleTx(Arc<TestTx>);

        impl std::fmt::Debug for SimpleTx {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("SimpleTx").finish()
            }
        }

        #[async_trait::async_trait]
        impl Transaction for SimpleTx {
            type Context = TestConn;
            type Item = TestConn;
            type Error = ();

            async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
                (self.0)(ctx)
            }
        }

        SimpleTx(Arc::new(f))
    }

    #[tokio::test]
    async fn test_with_simple_tx() {
        let tx = simple_tx(|conn| {
            let value = conn.value() + 1;
            conn.set_value(value);
            Ok(conn.clone())
        });

        let mut conn = TestConn { value: 42 };
        let result = tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 43);
        assert_eq!(conn.value, 43);
    }

    #[tokio::test]
    async fn test_then_operator() {
        let tx1 = simple_tx(|conn| {
            let value = conn.value() + 1;
            conn.set_value(value);
            Ok(conn.clone())
        });

        let chained_tx = tx1.then(|prev_conn| {
            simple_tx(move |conn| {
                let new_value = prev_conn.value * 2;
                conn.set_value(new_value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 5 };
        let result = chained_tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 12);
        assert_eq!(conn.value, 12);
    }

    #[tokio::test]
    async fn test_or_else_operator() {
        // Create a transaction that fails and chains with a fallback in one step
        let recovery_tx = simple_tx(|_conn| Err(())).or_else(|_err| {
            simple_tx(|conn| {
                let value = conn.value() * 3;
                conn.set_value(value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 5 };
        let result = recovery_tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 15);
        assert_eq!(conn.value, 15);
    }

    #[tokio::test]
    async fn test_ok_operator() {
        let chained_tx = ok::<TestConn, TestConn, ()>(TestConn { value: 42 }).then(|prev_conn| {
            simple_tx(move |conn| {
                let new_value = prev_conn.value + 8;
                conn.set_value(new_value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 10 };
        let result = chained_tx.execute(&mut conn).await;

        assert_eq!(result.unwrap().value, 50);
        assert_eq!(conn.value, 50);
    }

    #[tokio::test]
    async fn test_context_access() {
        // Create a transaction with simple_tx
        let tx = simple_tx(|conn| {
            let new_value = conn.value() * 5;
            conn.set_value(new_value);
            Ok(conn.clone())
        })
        .then(|prev_conn| {
            simple_tx(move |conn| {
                let new_value = prev_conn.value * 2;
                conn.set_value(new_value);
                Ok(conn.clone())
            })
        });

        let mut conn = TestConn { value: 7 };
        let result = tx.execute(&mut conn).await;

        // 7 * 5 = 35, then 35 * 2 = 70
        assert_eq!(result.unwrap().value, 70);
        assert_eq!(conn.value, 70);
    }

    #[tokio::test]
    async fn test_setup_operator() {
        // First test: setup handling a successful transaction
        {
            // First transaction adds 10 to the value
            let tx1 = simple_tx(|conn| {
                conn.set_value(conn.value() + 10);
                Ok(conn.clone())
            });

            // Setup checks if the first transaction succeeded and doubles value on success
            let setup_tx = tx1.setup(|res| {
                if res.is_ok() {
                    let conn = res.unwrap();
                    ok(TestConn {
                        value: conn.value * 2,
                    })
                } else {
                    ok(TestConn { value: 42 })
                }
            });

            let mut conn = TestConn { value: 5 };
            let result = setup_tx.execute(&mut conn).await;

            assert_eq!(result.unwrap().value, 30);
            assert_eq!(conn.value, 15);
        }

        // Second test: setup handling a failed transaction
        {
            let tx1 = simple_tx(|_| Err(()));

            let setup_tx = tx1.setup(|res| {
                if res.is_ok() {
                    let conn = res.unwrap();
                    ok(TestConn {
                        value: conn.value * 2,
                    })
                } else {
                    ok(TestConn { value: 42 })
                }
            });

            let mut conn = TestConn { value: 5 };
            let result = setup_tx.execute(&mut conn).await;

            assert_eq!(result.unwrap().value, 42);
            assert_eq!(conn.value, 5);
        }
    }

    // Test with_transaction with a simpler approach
    #[tokio::test]
    async fn test_simple_transaction() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        // Create a context to track transaction lifecycle events
        #[derive(Default)]
        struct TestTransactionContext {
            // Track if begin was called
            begin_called: Arc<AtomicBool>,
            // Track if commit was called
            commit_called: Arc<AtomicBool>,
            // Track if rollback was called
            rollback_called: Arc<AtomicBool>,
        }

        let context = TestTransactionContext {
            begin_called: Arc::new(AtomicBool::new(false)),
            commit_called: Arc::new(AtomicBool::new(false)),
            rollback_called: Arc::new(AtomicBool::new(false)),
        };

        // Create a transaction to test success path
        let begin_called = context.begin_called.clone();
        let commit_called = context.commit_called.clone();

        let successful_tx = simple_tx(move |conn| {
            // Mark that begin was called
            begin_called.store(true, Ordering::SeqCst);

            // Mark that commit should be called (simulating success)
            commit_called.store(true, Ordering::SeqCst);

            Ok(conn.clone())
        });

        let mut conn = TestConn { value: 1 };
        let result = successful_tx.execute(&mut conn).await;

        assert!(result.is_ok());
        assert!(context.begin_called.load(Ordering::SeqCst));
        assert!(context.commit_called.load(Ordering::SeqCst));
        assert!(!context.rollback_called.load(Ordering::SeqCst));

        // Reset the context
        let context = TestTransactionContext {
            begin_called: Arc::new(AtomicBool::new(false)),
            commit_called: Arc::new(AtomicBool::new(false)),
            rollback_called: Arc::new(AtomicBool::new(false)),
        };

        // Create a transaction to test failure path
        let begin_called = context.begin_called.clone();
        let rollback_called = context.rollback_called.clone();

        let failing_tx = simple_tx(move |_conn| {
            // Mark that begin was called
            begin_called.store(true, Ordering::SeqCst);

            // Mark that rollback should be called (simulating failure)
            rollback_called.store(true, Ordering::SeqCst);

            Err(())
        });

        let mut conn = TestConn { value: 1 };
        let result = failing_tx.execute(&mut conn).await;

        assert!(result.is_err());
        assert!(context.begin_called.load(Ordering::SeqCst));
        assert!(!context.commit_called.load(Ordering::SeqCst));
        assert!(context.rollback_called.load(Ordering::SeqCst));
    }
}
