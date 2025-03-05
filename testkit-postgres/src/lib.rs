// Common traits used across feature implementations
mod common_traits;
pub use common_traits::*;

#[cfg(feature = "postgres")]
pub mod tokio_postgres;

#[cfg(feature = "sqlx")]
pub mod sqlx_postgres;

// Error types for the library
mod error;
pub use error::*;

/// Re-export the traits from testkit-core
pub use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestContext,
    TestDatabaseConnection, TestDatabaseInstance,
};

// Export feature-specific implementations
#[cfg(feature = "postgres")]
pub use tokio_postgres::*;

#[cfg(feature = "sqlx")]
pub use sqlx_postgres::*;

// Convenience re-exports
pub use testkit_core::with_database;
#[cfg(feature = "postgres")]
pub use testkit_core::{with_boxed_database, with_boxed_database_config};

// Re-export the boxed_async macro for easily creating boxed async blocks
pub use testkit_core::boxed_async;

/// Example of how to use the boxed database API with PostgreSQL
///
/// This example shows how to use the boxed database API to work with closures that
/// capture local variables. Use the `boxed_async!` macro to avoid manually boxing the async blocks.
///
/// ```no_run
/// use testkit_postgres::{postgres_backend, with_boxed_database, boxed_async};
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     // Create a backend
///     let backend = postgres_backend().await?;
///     
///     // Some local variables that would cause lifetime issues with regular closures
///     let table_name = String::from("users");
///     let table_name_for_tx = table_name.clone(); // Clone for use in transaction
///     
///     // Use the boxed database API with the boxed_async! macro
///     let ctx = with_boxed_database(backend)
///         .setup(move |conn| boxed_async!(async move {
///             // Create a table using the captured variable
///             let query = format!(
///                 "CREATE TABLE {} (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
///                 table_name
///             );
///             conn.client().execute(&query, &[]).await?;
///             Ok(())
///         }))
///         .with_transaction(move |conn| boxed_async!(async move {
///             // Insert data using the cloned variable
///             let query = format!("INSERT INTO {} (name) VALUES ($1)", table_name_for_tx);
///             conn.client().execute(&query, &[&"John Doe"]).await?;
///             Ok(())
///         }))
///         .execute()
///         .await?;
///     
///     Ok(())
/// }
/// ```
#[allow(dead_code)]
async fn boxed_example() -> Result<(), Box<dyn std::error::Error>> {
    // This is just a dummy implementation to make the doctest compile
    // The actual example is in the doc comment above
    Ok(())
}
