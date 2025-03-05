use std::fmt::Debug;
use std::sync::{Mutex, OnceLock};

use async_trait::async_trait;

use crate::{
    boxed_async,
    handlers::with_boxed_database,
    testdb::{DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection},
};

/// A mock connection for testing
#[derive(Debug, Clone)]
struct MockConnection {
    connection_string: String,
}

impl TestDatabaseConnection for MockConnection {
    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A mock connection pool for testing
#[derive(Debug, Clone)]
struct MockPool {
    connection_string: String,
}

#[async_trait]
impl DatabasePool for MockPool {
    type Connection = MockConnection;
    type Error = MockError;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        Ok(MockConnection {
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A mock error type for testing
#[derive(Debug, Clone)]
struct MockError(String);

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

// Replace the static atomic counters with synchronized versions
static TEST_STATE: OnceLock<Mutex<TestState>> = OnceLock::new();

fn get_test_state() -> &'static Mutex<TestState> {
    TEST_STATE.get_or_init(|| Mutex::new(TestState::default()))
}

#[derive(Default)]
struct TestState {
    setup_called: bool,
    transaction_called: bool,
}

// Update the helper functions
fn was_setup_called() -> bool {
    let state = get_test_state().lock().unwrap();
    state.setup_called
}

fn was_transaction_called() -> bool {
    let state = get_test_state().lock().unwrap();
    state.transaction_called
}

fn mark_setup_called() {
    let mut state = get_test_state().lock().unwrap();
    state.setup_called = true;
}

fn mark_transaction_called() {
    let mut state = get_test_state().lock().unwrap();
    state.transaction_called = true;
}

/// Reset the state counters to their default values
fn reset_counters() {
    let mut guard = get_test_state().lock().unwrap();
    guard.setup_called = false;
    guard.transaction_called = false;
}

/// Run a test with proper isolation and state cleanup
async fn with_test_fixture<F, Fut>(test_name: &str, test_fn: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    // Reset state before test
    reset_counters();

    // Run the test
    test_fn().await;

    // Reset state after test to avoid interference with next test
    reset_counters();
}

/// A mock database backend for testing
#[derive(Debug, Clone)]
struct MockBackend;

impl MockBackend {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DatabaseBackend for MockBackend {
    type Connection = MockConnection;
    type Pool = MockPool;
    type Error = MockError;

    async fn new(_config: DatabaseConfig) -> Result<Self, Self::Error> {
        Ok(Self::new())
    }

    async fn create_pool(
        &self,
        name: &DatabaseName,
        _config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        Ok(MockPool {
            connection_string: format!("mock://db/{}", name),
        })
    }

    async fn create_database(
        &self,
        _pool: &Self::Pool,
        _name: &DatabaseName,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn drop_database(&self, _name: &DatabaseName) -> Result<(), Self::Error> {
        Ok(())
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        format!("mock://db/{}", name)
    }
}

// Update the test functions to use the fixture

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_with_database_only() {
    with_test_fixture("test_with_database_only", || async {
        let backend = MockBackend::new();

        let ctx = with_boxed_database(backend)
            .execute()
            .await
            .expect("Failed to execute database handler");

        assert!(!was_setup_called(), "Setup should not have been called");
        assert!(
            !was_transaction_called(),
            "Transaction should not have been called"
        );

        // Verify we have a valid context with a database instance
        assert!(
            ctx.db.name().as_str().starts_with("testkit_"),
            "Expected DB name to start with 'testkit_', got: {}",
            ctx.db.name()
        );
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_with_database_and_setup() {
    with_test_fixture("test_with_database_and_setup", || async {
        let backend = MockBackend::new();

        let ctx = with_boxed_database(backend)
            .setup(|_conn| {
                boxed_async!(async {
                    mark_setup_called();
                    Ok(())
                })
            })
            .execute()
            .await
            .expect("Failed to execute database handler");

        assert!(was_setup_called());
        assert!(!was_transaction_called());

        // Verify we have a valid context with a database instance
        assert!(ctx.db.name().as_str().starts_with("testkit_"));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_with_database_and_transaction() {
    with_test_fixture("test_with_database_and_transaction", || async {
        let backend = MockBackend::new();

        let ctx = with_boxed_database(backend)
            .with_transaction(|_conn| {
                boxed_async!(async {
                    mark_transaction_called();
                    Ok(())
                })
            })
            .execute()
            .await
            .expect("Failed to execute database handler");

        assert!(!was_setup_called(), "Setup should not have been called");
        assert!(
            was_transaction_called(),
            "Transaction should have been called"
        );

        // Verify we have a valid context with a database instance
        assert!(ctx.db.name().as_str().starts_with("testkit_"));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_with_database_setup_and_transaction() {
    with_test_fixture("test_with_database_setup_and_transaction", || async {
        let backend = MockBackend::new();

        let ctx = with_boxed_database(backend)
            .setup(|_conn| {
                boxed_async!(async {
                    mark_setup_called();
                    Ok(())
                })
            })
            .with_transaction(|_conn| {
                boxed_async!(async {
                    mark_transaction_called();
                    Ok(())
                })
            })
            .execute()
            .await
            .expect("Failed to execute database handler");

        assert!(was_setup_called(), "Setup should have been called");
        assert!(
            was_transaction_called(),
            "Transaction should have been called"
        );

        // Verify we have a valid context with a database instance
        assert!(ctx.db.name().as_str().starts_with("testkit_"));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_error_in_setup() {
    with_test_fixture("test_error_in_setup", || async {
        let backend = MockBackend::new();

        let result = with_boxed_database(backend)
            .setup(|_conn| boxed_async!(async { Err(MockError("Setup error".to_string())) }))
            .execute()
            .await;

        assert!(result.is_err(), "Expected setup to return an error");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_error_in_transaction() {
    with_test_fixture("test_error_in_transaction", || async {
        let backend = MockBackend::new();

        let result = with_boxed_database(backend)
            .with_transaction(|_conn| {
                boxed_async!(async { Err(MockError("Transaction error".to_string())) })
            })
            .execute()
            .await;

        assert!(result.is_err(), "Expected transaction to return an error");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_error_in_setup_with_transaction() {
    with_test_fixture("test_error_in_setup_with_transaction", || async {
        let backend = MockBackend::new();

        let result = with_boxed_database(backend)
            .setup(|_conn| boxed_async!(async { Err(MockError("Setup error".to_string())) }))
            .with_transaction(|_conn| {
                boxed_async!(async {
                    mark_transaction_called();
                    Ok(())
                })
            })
            .execute()
            .await;

        assert!(result.is_err(), "Expected setup to return an error");
        assert!(
            !was_transaction_called(),
            "Transaction should not have been called after setup failed"
        );
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_database_entry_point_directly() {
    with_test_fixture("test_database_entry_point_directly", || async {
        let backend = MockBackend::new();
        let ctx = with_boxed_database(backend)
            .execute()
            .await
            .expect("Failed to execute database entry point directly");

        // Verify we have a valid context with a database instance
        assert!(
            ctx.db.name().as_str().starts_with("testkit_"),
            "Expected DB name to start with 'testkit_', got: {}",
            ctx.db.name()
        );
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_isolated_transaction() {
    with_test_fixture("test_isolated_transaction", || async {
        // Use direct transaction function marking
        mark_transaction_called();

        assert!(was_transaction_called());
    })
    .await;
}
