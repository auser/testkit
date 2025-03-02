use std::future::Future;
use std::marker::PhantomData;

use crate::{DatabaseBackend, DatabaseConfig, DatabasePool, TestDatabase, with_database_setup_raw};

#[derive(Debug)]
#[must_use]
pub struct WithDatabase<B, TestDatabase> {
    /// The test database
    #[allow(dead_code)]
    tdb: TestDatabase,
    /// The backend
    _backend: PhantomData<B>,
}

/// Create a new test database with the specified backend and configuration
///
/// This function creates a new isolated test database using the provided backend and configuration.
/// The database will be automatically cleaned up when the returned TestDatabase is dropped.
///
/// # Example
///
/// ```rust,no_run,ignore
/// use testkit_core::prelude::*;
///
/// #[derive(Debug, Clone)]
/// struct MyDatabaseBackend {
///     value: i32,
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let backend = MyDatabaseBackend::new();
///     let config = DatabaseConfig::from_env();
///     
///     // Create a test database
///     let test_db = with_database(backend, config).await.unwrap();
///     
///     // Use test_db for your tests
///     // ...
///     
///     // Database is automatically cleaned up when test_db is dropped
/// }
/// ```
pub async fn with_database<B>(
    backend: B,
    config: DatabaseConfig,
) -> Result<WithDatabase<B, TestDatabase<B>>, B::Error>
where
    B: DatabaseBackend + 'static,
{
    let tdb = TestDatabase::new(backend, config).await?;
    Ok(WithDatabase {
        tdb,
        _backend: PhantomData,
    })
}

/// Create a new test database with a setup function
///
/// This is a convenience function that creates a test database and initializes it
/// with a setup function that can create tables, populate initial data, etc.
///
/// # Example
///
/// ```rust,no_run,ignore
/// use testkit_core::with_database_setup;
///
/// #[tokio::main]
/// async fn main() {
///     let backend = MyDatabaseBackend::new();
///     let config = DatabaseConfig::from_env();
///     
///     // Create a test database with a setup function
///     let test_db = with_database_setup(backend, config, |conn| async move {
///         // Create tables, insert data, etc.
///         conn.execute("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT)").await?;
///         Ok(())
///     }).await.unwrap();
///     
///     // Database is now set up and ready for tests
/// }
/// ```
pub async fn with_database_setup<B, F, Fut>(
    backend: B,
    config: DatabaseConfig,
    setup_fn: F,
) -> Result<TestDatabase<B>, B::Error>
where
    B: DatabaseBackend + 'static,
    F: FnOnce(&mut <B::Pool as DatabasePool>::Connection) -> Fut + Send,
    Fut: Future<Output = Result<(), B::Error>> + Send,
{
    // // Create the test database
    // let mut db = TestDatabase::new(backend, config).await?;

    // // Initialize connection pool
    // db.initialize_connection_pool().await?;

    // // Run the setup function
    // db.setup(setup_fn).await?;

    // Ok(db)
    let setup_db = with_database_setup_raw(backend, config, setup_fn).await?;
    Ok(setup_db)
}
