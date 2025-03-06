mod context;
mod handlers;
mod testdb;
pub mod utils;

// Re-exported types and traits
pub use context::*;
pub use handlers::*;
pub use testdb::*;
pub use utils::*;

// The boxed_async macro is already exported with #[macro_export]

use std::{fmt::Debug, pin::Pin};

/// A test context that contains a database instance
#[derive(Clone)]
pub struct TestContext<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    pub db: TestDatabaseInstance<DB>,
}

impl<DB> Debug for TestContext<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TestContext {{ db: {:?} }}", self.db.db_name)
    }
}

impl<DB> TestContext<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    pub fn new(db: TestDatabaseInstance<DB>) -> Self {
        Self { db }
    }
}

/// Testing utilities for working with database handlers in a mock environment
///
/// This module provides ergonomic APIs for working with database tests, allowing
/// you to create seamless test interactions with databases.
///
/// # Examples
///
/// Using the direct API with `setup_async` and `transaction` methods:
///
/// ```rust,no_run
/// use testkit_core::*;
///
/// #[tokio::test]
/// async fn test_database() {
///     let backend = MockBackend::new();
///    
///     // Direct API with setup_async and transaction methods (no boxed_async needed)
///     let ctx = with_boxed_database(backend)
///         .setup_async(|conn| async {
///             println!("Setting up database");
///             Ok(())
///         })
///         .transaction(|conn| async {
///             println!("Running transaction");
///             Ok(())
///         })
///         .run()
///         .await
///         .expect("Test failed");
/// }
/// ```
///
/// Using the `db_test!` macro for a clean entry point:
///
/// ```rust,no_run
/// use testkit_core::*;
///
/// #[tokio::test]
/// async fn test_with_macro() {
///     let backend = MockBackend::new();
///     
///     // Variable capture works seamlessly
///     let table_name = "users".to_string();
///    
///     // Using db_test! macro as a more readable entry point
///     let ctx = db_test!(backend)
///         .setup_async(|conn| async move {
///             println!("Creating table: {}", table_name);
///             Ok(())
///         })
///         .transaction(|conn| async {
///             println!("Running transaction");
///             Ok(())
///         })
///         .run()
///         .await
///         .expect("Test failed");
/// }
/// ```
///
/// For more examples, check the `tests/ergonomic_api.rs` file.
pub mod tests {
    pub mod mock {
        // A minimal mock backend for testing
        use async_trait::async_trait;
        use std::fmt::Debug;

        use crate::{
            DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
        };

        // Define a mock connection type
        #[derive(Debug, Clone)]
        pub struct MockConnection;

        impl TestDatabaseConnection for MockConnection {
            fn connection_string(&self) -> String {
                "mock://test".to_string()
            }
        }

        // Define a mock pool type
        #[derive(Debug, Clone)]
        pub struct MockPool;

        #[async_trait]
        impl DatabasePool for MockPool {
            type Connection = MockConnection;
            type Error = MockError;

            async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
                Ok(MockConnection)
            }

            async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
                Ok(())
            }

            fn connection_string(&self) -> String {
                "mock://test".to_string()
            }
        }

        // Define a simple error type
        #[derive(Debug, Clone)]
        pub struct MockError(pub String);

        impl std::fmt::Display for MockError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "MockError: {}", self.0)
            }
        }

        impl std::error::Error for MockError {}

        impl From<String> for MockError {
            fn from(s: String) -> Self {
                MockError(s)
            }
        }

        // Define a mock backend
        #[derive(Debug, Clone, Default)]
        pub struct MockBackend;

        #[async_trait]
        impl DatabaseBackend for MockBackend {
            type Connection = MockConnection;
            type Pool = MockPool;
            type Error = MockError;

            async fn new(_config: DatabaseConfig) -> Result<Self, Self::Error> {
                Ok(Self)
            }

            async fn connect(&self, _name: &DatabaseName) -> Result<Self::Connection, Self::Error> {
                Ok(MockConnection)
            }

            async fn connect_with_string(
                &self,
                _connection_string: &str,
            ) -> Result<Self::Connection, Self::Error> {
                Ok(MockConnection)
            }

            async fn create_pool(
                &self,
                _name: &DatabaseName,
                _config: &DatabaseConfig,
            ) -> Result<Self::Pool, Self::Error> {
                Ok(MockPool)
            }

            async fn create_database(
                &self,
                _pool: &Self::Pool,
                _name: &DatabaseName,
            ) -> Result<(), Self::Error> {
                Ok(())
            }

            fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
                // In a mock implementation, log that we would drop the database
                tracing::info!("Mock dropping database: {}", name);
                Ok(())
            }

            fn connection_string(&self, _name: &DatabaseName) -> String {
                "mock://database".to_string()
            }
        }
    }
}

/// Execute a function with a newly created connection and automatically close it
///
/// This function creates a connection to the database with the given name using the
/// provided backend, then executes the operation with that connection. The connection
/// is automatically closed when the operation completes.
///
/// This is the most efficient way to perform a one-off database operation without
/// the overhead of creating and managing a connection pool.
///
/// # Example
/// ```rust,no_run
/// use testkit_core::{with_connection, DatabaseBackend, DatabaseName, boxed_async};
/// use std::fmt::{Display, Formatter};
///
/// // Define a custom error type for our example
/// #[derive(Debug)]
/// struct ExampleError(String);
///
/// impl std::error::Error for ExampleError {}
///
/// impl Display for ExampleError {
///     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
///         write!(f, "{}", self.0)
///     }
/// }
///
/// async fn example<B: DatabaseBackend>(backend: B, name: &DatabaseName) -> Result<(), B::Error>
/// where B::Error: From<ExampleError> {
///     with_connection(backend, name, |conn| boxed_async!(async move {
///         // Perform operations with the connection
///         Ok::<(), ExampleError>(())
///     })).await
/// }
/// ```
pub async fn with_connection<B, F, R, E>(
    backend: B,
    name: &DatabaseName,
    operation: F,
) -> Result<R, B::Error>
where
    B: DatabaseBackend,
    F: FnOnce(&B::Connection) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send>> + Send,
    E: std::error::Error + Send + Sync + 'static,
    B::Error: From<E>,
{
    // Create a connection
    let conn = backend.connect(name).await?;

    // Run the operation
    let result = operation(&conn).await.map_err(|e| B::Error::from(e))?;

    // Connection will be dropped automatically when it goes out of scope
    Ok(result)
}

/// Execute a function with a newly created connection using a connection string
///
/// This function creates a connection to the database using the provided connection string
/// and backend, then executes the operation with that connection. The connection is
/// automatically closed when the operation completes.
///
/// This is the most efficient way to perform a one-off database operation without
/// the overhead of creating and managing a connection pool.
///
/// # Example
/// ```rust,no_run
/// use testkit_core::{with_connection_string, DatabaseBackend, boxed_async};
/// use std::fmt::{Display, Formatter};
///
/// // Define a custom error type for our example
/// #[derive(Debug)]
/// struct ExampleError(String);
///
/// impl std::error::Error for ExampleError {}
///
/// impl Display for ExampleError {
///     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
///         write!(f, "{}", self.0)
///     }
/// }
///
/// async fn example<B: DatabaseBackend>(backend: B, connection_string: &str) -> Result<(), B::Error>
/// where B::Error: From<ExampleError> {
///     with_connection_string(backend, connection_string, |conn| boxed_async!(async move {
///         // Perform operations with the connection
///         Ok::<(), ExampleError>(())
///     })).await
/// }
/// ```
pub async fn with_connection_string<B, F, R, E>(
    backend: B,
    connection_string: &str,
    operation: F,
) -> Result<R, B::Error>
where
    B: DatabaseBackend,
    F: FnOnce(&B::Connection) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send>> + Send,
    E: std::error::Error + Send + Sync + 'static,
    B::Error: From<E>,
{
    // Create a connection using the connection string
    let conn = backend.connect_with_string(connection_string).await?;

    // Run the operation
    let result = operation(&conn).await.map_err(|e| B::Error::from(e))?;

    // Connection will be dropped automatically when it goes out of scope
    Ok(result)
}
