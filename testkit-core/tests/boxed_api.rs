use std::fmt::Debug;

use async_trait::async_trait;
use testkit_core::boxed_async;
use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
    with_boxed_database,
};

// Mock database setup for testing
#[derive(Debug, Clone)]
struct MockError(String);

impl std::fmt::Display for MockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mock error: {}", self.0)
    }
}

impl std::error::Error for MockError {}

impl From<String> for MockError {
    fn from(s: String) -> Self {
        MockError(s)
    }
}

#[derive(Debug, Clone)]
struct MockConnection;

impl TestDatabaseConnection for MockConnection {
    fn connection_string(&self) -> String {
        "mock://test".to_string()
    }
}

#[derive(Debug, Clone)]
struct MockPool;

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

#[derive(Debug, Clone)]
struct MockBackend;

impl MockBackend {
    fn new() -> Self {
        MockBackend
    }
}

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
        "mock://test".to_string()
    }
}

#[tokio::test]
#[ignore]
async fn test_boxed_api_with_macro() {
    let backend = MockBackend::new();

    // Table name is a variable we'll capture in closure
    let _table_name = "test_table";

    // Test that we can correctly capture variables with the boxed_async macro
    let ctx = with_boxed_database(backend)
        .setup(move |_conn| {
            boxed_async!(async move {
                // Use the connection to set up the database
                Ok(())
            })
        })
        .with_transaction(move |_conn| {
            boxed_async!(async move {
                // Use the connection to run a transaction
                Ok(())
            })
        })
        .execute()
        .await;

    assert!(
        ctx.is_ok(),
        "Failed to execute boxed database: {:?}",
        ctx.err()
    );
}

#[tokio::test]
#[ignore]
async fn test_capturing_local_variables() {
    // Test multiple variables and complex types
    let backend = MockBackend::new();
    let table_name = "users";
    let column_names = vec!["id", "name", "email"];
    let row_count = 10;

    let ctx = with_boxed_database(backend)
        .setup(move |_conn| {
            boxed_async!(async move {
                // In real code this would create a table with the specified columns
                println!(
                    "Creating table '{}' with columns: {:?}",
                    table_name, column_names
                );
                Ok(())
            })
        })
        .with_transaction(move |_conn| {
            boxed_async!(async move {
                // In real code this would insert rows
                println!("Inserting {} rows into {}", row_count, table_name);
                Ok(())
            })
        })
        .execute()
        .await;

    assert!(
        ctx.is_ok(),
        "Failed to execute boxed database with captured variables: {:?}",
        ctx.err()
    );
}
