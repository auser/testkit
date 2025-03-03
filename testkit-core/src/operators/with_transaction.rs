use std::future::Future;
use std::marker::PhantomData;

use crate::{Transaction, TransactionManager};
use async_trait::async_trait;

/// Creates a transaction that automatically handles the transaction lifecycle.
///
/// This function:
/// 1. Creates a transaction from the database context
/// 2. Executes your function with that transaction
/// 3. Automatically commits on success
/// 4. Automatically rolls back on error
///
/// This function is typically used after setting up a database with `with_database`.
///
/// # Examples
///
/// ```rust,no_run
/// use testkit_core::{Transaction, TransactionManager, with_transaction};
///
/// // Define a simple user type for the example
/// #[derive(Debug, Clone)]
/// struct User {
///     id: i32,
///     name: String,
/// }
///
/// // Define a transaction that retrieves a user from the database
/// fn get_user<Ctx, Tx, Conn, E>(id: i32) -> impl Transaction<Context = Ctx, Item = User, Error = E>
/// where
///     Ctx: TransactionManager<Tx, Conn, Error = E> + Send + Sync + 'static,
///     Tx: Send + Sync + 'static,
///     Conn: Send + Sync + 'static,
///     E: Send + Sync + 'static,
/// {
///     with_transaction(move |ctx, tx| async move {
///         // Database operations would use the transaction...
///         // For this example, we'll just create a mock user
///         Ok(User { id, name: "Example".to_string() })
///     })
/// }
/// ```
pub fn with_transaction<F, Fut, Ctx, Tx, Conn, T, E>(
    f: F,
) -> impl Transaction<Context = Ctx, Item = T, Error = E>
where
    F: for<'a> FnOnce(&'a Ctx, &'a mut Tx) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    Ctx: TransactionManager<Tx, Conn, Error = E> + Send + Sync + 'static,
    Tx: Send + Sync + 'static,
    Conn: Send + Sync + 'static,
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    struct WithTransactionTx<F, Ctx, Tx, Conn, T, E> {
        f: F,
        _phantom: PhantomData<(Ctx, Tx, Conn, T, E)>,
    }

    #[async_trait]
    impl<F, Fut, Ctx, Tx, Conn, T, E> Transaction for WithTransactionTx<F, Ctx, Tx, Conn, T, E>
    where
        F: for<'a> FnOnce(&'a Ctx, &'a mut Tx) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<T, E>> + Send + 'static,
        Ctx: TransactionManager<Tx, Conn, Error = E> + Send + Sync + 'static,
        Tx: Send + Sync + 'static,
        Conn: Send + Sync + 'static,
        T: Send + Sync + 'static,
        E: Send + Sync + 'static,
    {
        type Context = Ctx;
        type Item = T;
        type Error = E;

        async fn execute(&self, ctx: &mut Self::Context) -> Result<Self::Item, Self::Error> {
            // Begin transaction
            let mut tx = ctx.begin_transaction().await?;

            // Clone the function to avoid lifetime issues
            let f = self.f.clone();

            // Execute the user function with the transaction
            let result = f(ctx, &mut tx).await;

            // Use mem::replace with zeroed to safely take ownership of tx
            // This avoids the "moved" error when calling commit/rollback
            let mut inner_tx = std::mem::replace(&mut tx, unsafe { std::mem::zeroed() });

            // Commit or rollback based on the result
            match &result {
                Ok(_) => {
                    // Commit the transaction
                    Ctx::commit_transaction(&mut inner_tx).await?;
                }
                Err(_) => {
                    // Try to rollback the transaction, but ignore errors
                    // We prioritize returning the original error
                    let _ = Ctx::rollback_transaction(&mut inner_tx).await;
                }
            }

            // Return the result
            result
        }
    }

    WithTransactionTx {
        f,
        _phantom: PhantomData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::transaction::tests::{MockBackend, MockTransaction};
    use crate::{DatabaseConfig, DatabaseName, TestDatabaseInstance};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_with_transaction() {
        // Create a test database instance
        let backend = MockBackend;
        let config = DatabaseConfig::default();
        let _db_name = DatabaseName::new(None);

        let mut test_instance = TestDatabaseInstance::new(backend.clone(), config)
            .await
            .unwrap();

        // A flag to track if our operation was executed
        let operation_executed = Arc::new(Mutex::new(false));
        let operation_executed_clone = operation_executed.clone();

        // Create a transaction that sets the flag
        let tx = with_transaction(move |_ctx, _tx: &mut MockTransaction| {
            let op_executed = operation_executed_clone.clone();
            async move {
                let mut guard = op_executed.lock().unwrap();
                *guard = true;
                Ok(42) // Return some value
            }
        });

        // Execute the transaction
        let result = tx.execute(&mut test_instance).await;

        // Check that the operation was executed and the result is correct
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert!(*operation_executed.lock().unwrap());
    }
}
