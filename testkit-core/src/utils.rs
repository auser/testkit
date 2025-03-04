use std::future::Future;
use std::pin::Pin;

/// Macro to automatically box an async block for use with the boxed database API
///
/// This macro makes the boxed database API more ergonomic by hiding the need to
/// manually use Box::pin() around async blocks.
#[macro_export]
macro_rules! boxed_async {
    (async $block:block) => {
        Box::pin(async $block) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send + '_>>
    };
    (async move $block:block) => {
        Box::pin(async move $block) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send + '_>>
    };
}

/// Macro to implement the TransactionHandler trait for a type
///
/// This macro makes it easier to implement the TransactionHandler trait for types
/// that work with databases. It automatically handles the Box::pin wrapping for async functions.
#[macro_export]
macro_rules! impl_transaction_handler {
    ($type:ty, $db:ty, $item:ty, $error:ty) => {
        #[async_trait::async_trait]
        impl $crate::handlers::TransactionHandler<$db> for $type {
            type Item = $item;
            type Error = $error;

            async fn execute(
                self,
                ctx: &mut $crate::TestContext<$db>,
            ) -> Result<Self::Item, Self::Error> {
                self.execute_impl(ctx).await
            }
        }
    };
}

/// Boxes an async closure to handle lifetime issues
///
/// This function takes a closure that returns a Future and wraps it in a
/// Pin<Box<dyn Future>> to solve lifetime problems. This is particularly useful
/// for async closures that capture variables from their environment.
pub fn boxed_future<T, F, Fut, E>(
    f: F,
) -> impl FnOnce(T) -> Pin<Box<dyn Future<Output = Result<(), E>> + Send>>
where
    F: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    move |t| {
        let future = f(t);
        Box::pin(future) as Pin<Box<dyn Future<Output = Result<(), E>> + Send>>
    }
}

/// A more ergonomic macro for database operations that hides all the boxing complexity
///
/// This macro provides a clean syntax for database operations without requiring
/// the user to manually use Box::new(), Box::pin(), or boxed_async!
///
/// # Example:
///
/// ```rust,no_run,ignore
/// use testkit_core::db_test;
///
/// #[tokio::test]
/// async fn test_database() {
///     let backend = MockBackend::new();
///     
///     // Using the macro for a clean API
///     let ctx = db_test!(backend)
///         .setup_async(|conn| async {
///             // Setup operations...
///             Ok(())
///         })
///         .transaction(|conn| async {
///             // Transaction operations...
///             Ok(())
///         })
///         .run()
///         .await
///         .expect("Test failed");
/// }
/// ```
#[macro_export]
macro_rules! db_test {
    ($backend:expr) => {
        $crate::with_boxed_database($backend)
    };
    ($backend:expr, $config:expr) => {
        $crate::with_boxed_database_config($backend, $config)
    };
}

/// A macro to simplify setting up a database
///
/// This macro provides a cleaner API for the setup phase without requiring
/// manual boxing.
#[macro_export]
macro_rules! setup {
    ($backend:expr, |$conn:ident| $body:expr) => {
        $crate::with_boxed_database($backend)
            .setup(|$conn| $crate::boxed_async!($body))
    };
}

/// A macro to simplify running a transaction
///
/// This macro provides a cleaner API for the transaction phase without requiring
/// manual boxing.
#[macro_export]
macro_rules! transaction {
    ($backend:expr, |$conn:ident| $body:expr) => {
        $crate::with_boxed_database($backend)
            .with_transaction(|$conn| $crate::boxed_async!($body))
    };
}

/// A macro to simplify both setup and transaction phases
///
/// This macro provides a cleaner API for both setup and transaction phases without
/// requiring manual boxing.
#[macro_export]
macro_rules! setup_and_transaction {
    ($backend:expr, 
     setup: |$setup_conn:ident| $setup_body:expr,
     transaction: |$tx_conn:ident| $tx_body:expr) => {
        $crate::with_boxed_database($backend)
            .setup(|$setup_conn| $crate::boxed_async!($setup_body))
            .with_transaction(|$tx_conn| $crate::boxed_async!($tx_body))
    };
}
