use async_trait::async_trait;

/// Trait for managing database transactions
///
/// This trait allows for common transaction operations like beginning, committing, and
/// rolling back transactions. It's used by the `with_transaction` operator to provide
/// automatic transaction management.
#[async_trait]
pub trait DBTransactionManager<Tx, Conn>: Send + Sync {
    /// The error type for transaction operations
    type Error: Send + Sync;

    /// The transaction type returned by begin_transaction
    type Tx: DatabaseTransaction<Error = Self::Error> + Send + Sync;

    /// Begin a new transaction
    async fn begin_transaction(&mut self) -> Result<Self::Tx, Self::Error>;

    /// Commit a transaction
    async fn commit_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error>;

    /// Rollback a transaction
    async fn rollback_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error>;
}

/// Trait for objects that represent database transactions
///
/// This trait allows database-specific transaction types to be used
/// with the transaction management system.
#[async_trait]
pub trait DatabaseTransaction: Send + Sync {
    /// The error type for transaction operations
    type Error: Send + Sync;

    /// Commit the transaction
    async fn commit(&mut self) -> Result<(), Self::Error>;

    /// Rollback the transaction
    async fn rollback(&mut self) -> Result<(), Self::Error>;
}

// Implementation of TransactionManager for TestDatabaseInstance
#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
        TestDatabaseInstance,
    };
    use std::fmt::Debug;

    // Mock transaction type for testing
    #[derive(Debug, Clone, Default)]
    pub struct MockTransaction {
        committed: bool,
        rolled_back: bool,
    }

    impl MockTransaction {
        pub fn is_committed(&self) -> bool {
            self.committed
        }

        pub fn is_rolled_back(&self) -> bool {
            self.rolled_back
        }
    }

    #[async_trait]
    impl DatabaseTransaction for MockTransaction {
        type Error = String;

        async fn commit(&mut self) -> Result<(), Self::Error> {
            self.committed = true;
            Ok(())
        }

        async fn rollback(&mut self) -> Result<(), Self::Error> {
            self.rolled_back = true;
            Ok(())
        }
    }

    // Mock connection type for testing
    #[derive(Debug, Clone)]
    pub struct MockConnection(i32);

    impl MockConnection {
        pub fn set_value(&mut self, value: i32) {
            self.0 = value;
        }

        pub fn get_value(&self) -> i32 {
            self.0
        }
    }

    impl TestDatabaseConnection for MockConnection {
        fn connection_string(&self) -> String {
            format!("mock://localhost/test{}", self.0)
        }
    }

    // Mock pool for testing
    #[derive(Debug, Clone)]
    pub struct MockPool;

    #[async_trait]
    impl DatabasePool for MockPool {
        type Connection = MockConnection;
        type Error = String;

        async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
            Ok(MockConnection(0))
        }

        async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
            Ok(())
        }

        fn connection_string(&self) -> String {
            "mock://localhost/test".to_string()
        }
    }

    // Mock backend for testing
    #[derive(Debug, Clone)]
    pub struct MockBackend;

    #[async_trait]
    impl DatabaseBackend for MockBackend {
        type Connection = MockConnection;
        type Pool = MockPool;
        type Error = String;

        async fn new(_config: DatabaseConfig) -> Result<Self, Self::Error> {
            Ok(MockBackend)
        }

        async fn connect(&self, _name: &DatabaseName) -> Result<Self::Connection, Self::Error> {
            Ok(MockConnection(0))
        }

        async fn connect_with_string(
            &self,
            _connection_string: &str,
        ) -> Result<Self::Connection, Self::Error> {
            Ok(MockConnection(0))
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

        fn drop_database(&self, _name: &DatabaseName) -> Result<(), Self::Error> {
            Ok(())
        }

        fn connection_string(&self, _name: &DatabaseName) -> String {
            "mock://localhost/test".to_string()
        }
    }

    // Mock implementation of TransactionManager for TestDatabaseInstance with MockBackend
    #[async_trait]
    impl DBTransactionManager<MockTransaction, MockConnection> for TestDatabaseInstance<MockBackend> {
        type Error = String;
        type Tx = MockTransaction;

        async fn begin_transaction(&mut self) -> Result<Self::Tx, Self::Error> {
            Ok(MockTransaction::default())
        }

        async fn commit_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error> {
            tx.commit().await
        }

        async fn rollback_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error> {
            tx.rollback().await
        }
    }
}
