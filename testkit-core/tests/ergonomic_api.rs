use std::fmt::Debug;

use async_trait::async_trait;
use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
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
async fn test_direct_api() {
    let backend = MockBackend::new();

    // Test the direct methods without boxed_async
    let ctx = testkit_core::with_boxed_database(backend)
        .setup_async(|_conn| async { Ok(()) })
        .transaction(|_conn| async { Ok(()) })
        .run()
        .await;

    assert!(ctx.is_ok(), "Failed to execute test: {:?}", ctx.err());
}

#[tokio::test]
#[ignore]
async fn test_with_db_test_macro() {
    let backend = MockBackend::new();
    let table_name = "users".to_string();

    // Test the db_test! macro
    let ctx = testkit_core::db_test!(backend)
        .setup_async({
            let _table = table_name.clone();
            move |_conn| async move { Ok(()) }
        })
        .transaction({
            let _table = table_name;
            move |_conn| async move { Ok(()) }
        })
        .run()
        .await;

    assert!(ctx.is_ok(), "Failed to execute test: {:?}", ctx.err());
}

#[tokio::test]
#[ignore]
async fn test_with_capturing_variables() {
    let backend = MockBackend::new();
    let table_name = "users".to_string();
    let column_names = vec!["id", "name", "email"];
    let row_count = 10;

    // Test that we can correctly capture variables without having to wrap in boxed_async
    let ctx = testkit_core::with_boxed_database(backend)
        .setup_async({
            let _table = table_name.clone();
            let _columns = column_names.clone();
            move |_conn| async move { Ok(()) }
        })
        .transaction({
            let _table = table_name;
            let _count = row_count;
            move |_conn| async move { Ok(()) }
        })
        .run()
        .await;

    assert!(
        ctx.is_ok(),
        "Failed to execute test with captured variables: {:?}",
        ctx.err()
    );
}
