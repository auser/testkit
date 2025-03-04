mod context;
mod handlers;
mod testdb;
mod utils;

// Re-exported types and traits
pub use context::*;
pub use handlers::*;
pub use testdb::*;
pub use utils::*;

// Include the boxed_async macro
// The macro is already exported with #[macro_export]

use std::fmt::Debug;

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

// /// A struct representing the with_database operation
// #[derive(Debug)]
// #[must_use]
// pub struct WithDatabase<DB>
// where
//     DB: DatabaseBackend + Send + Sync + Debug + 'static,
// {
//     db: TestDatabaseInstance<DB>,
// }

// /// Create a new test context with a database
// pub async fn with_database<DB>(backend: DB) -> WithDatabase<DB>
// where
//     DB: DatabaseBackend + Send + Sync + Debug + 'static,
// {
//     let db = TestDatabaseInstance::new(backend, DatabaseConfig::default())
//         .await
//         .expect("Failed to create test database instance");

//     WithDatabase { db }
// }

// impl<DB> WithDatabase<DB>
// where
//     DB: DatabaseBackend + Send + Sync + Debug + 'static,
// {
//     /// Setup the database with a function
//     pub fn setup<S, Fut>(self, setup_fn: S) -> SetupHandler<DB, S>
//     where
//         S: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
//         Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
//     {
//         SetupHandler {
//             db: self.db,
//             setup_fn,
//         }
//     }
// }

// /// A struct for handling database setup
// #[derive(Debug)]
// #[must_use]
// pub struct SetupHandler<DB, S>
// where
//     DB: DatabaseBackend + Send + Sync + Debug + 'static,
//     S: Send + Sync + 'static,
// {
//     db: TestDatabaseInstance<DB>,
//     setup_fn: S,
// }

// impl<DB, S, Fut> SetupHandler<DB, S>
// where
//     DB: DatabaseBackend + Send + Sync + Debug + 'static,
//     S: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
//     Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
// {
//     /// Execute an operation with a transaction
//     pub fn with_transaction<TFn, TxFut>(self, transaction_fn: TFn) -> TransactionHandler<DB, S, TFn>
//     where
//         TxFut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
//         for<'tx> TFn: FnOnce(&'tx mut <TestContext<DB> as TransactionStarter<DB>>::Transaction) -> TxFut
//             + Send
//             + Sync
//             + 'static,
//         TestContext<DB>: TransactionStarter<DB>,
//     {
//         TransactionHandler {
//             db: self.db,
//             setup_fn: self.setup_fn,
//             transaction_fn,
//         }
//     }

//     /// Execute the setup operation only
//     pub async fn execute(
//         self,
//     ) -> Result<(TestContext<DB>, <DB::Pool as DatabasePool>::Connection), DB::Error> {
//         // Get a connection from the pool
//         let mut conn = self.db.acquire_connection().await?;

//         // Execute the setup function directly (not through db.setup)
//         (self.setup_fn)(&mut conn).await?;

//         // Create the context
//         let ctx = TestContext::new(self.db);

//         // Return both the context and the connection
//         Ok((ctx, conn))
//     }
// }

// /// A struct for handling database transactions
// #[derive(Debug)]
// #[must_use]
// pub struct TransactionHandler<DB, S, TFn>
// where
//     DB: DatabaseBackend + Send + Sync + Debug + 'static,
//     S: Send + Sync + 'static,
//     TFn: Send + Sync + 'static,
// {
//     db: TestDatabaseInstance<DB>,
//     setup_fn: S,
//     transaction_fn: TFn,
// }

// impl<DB, S, Fut, TFn, TxFut> TransactionHandler<DB, S, TFn>
// where
//     DB: DatabaseBackend + Send + Sync + Debug + 'static,
//     S: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
//     Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
//     TxFut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
//     for<'tx> TFn: FnOnce(&'tx mut <TestContext<DB> as TransactionStarter<DB>>::Transaction) -> TxFut
//         + Send
//         + Sync
//         + 'static,
//     TestContext<DB>: TransactionStarter<DB>
//         + DBTransactionManager<
//             <TestContext<DB> as TransactionStarter<DB>>::Transaction,
//             <TestContext<DB> as TransactionStarter<DB>>::Connection,
//             Error = DB::Error,
//             Tx = <TestContext<DB> as TransactionStarter<DB>>::Transaction,
//         >,
// {
//     /// Execute the entire operation chain
//     pub async fn execute(self) -> Result<TestContext<DB>, DB::Error> {
//         // Execute the setup function
//         self.db.setup(self.setup_fn).await?;

//         // Create the context
//         let mut ctx = TestContext::new(self.db);

//         // Begin the transaction using explicit types
//         let mut tx = <TestContext<DB> as DBTransactionManager<
//             <TestContext<DB> as TransactionStarter<DB>>::Transaction,
//             <TestContext<DB> as TransactionStarter<DB>>::Connection,
//         >>::begin_transaction(&mut ctx)
//         .await?;

//         // Execute the transaction function
//         match (self.transaction_fn)(&mut tx).await {
//             Ok(_) => {
//                 // Commit the transaction
//                 <TestContext<DB> as DBTransactionManager<
//                     <TestContext<DB> as TransactionStarter<DB>>::Transaction,
//                     <TestContext<DB> as TransactionStarter<DB>>::Connection,
//                 >>::commit_transaction(&mut tx)
//                 .await?;
//             }
//             Err(e) => {
//                 // Rollback the transaction on error
//                 let _ = <TestContext<DB> as DBTransactionManager<
//                     <TestContext<DB> as TransactionStarter<DB>>::Transaction,
//                     <TestContext<DB> as TransactionStarter<DB>>::Connection,
//                 >>::rollback_transaction(&mut tx)
//                 .await;
//                 return Err(e);
//             }
//         }

//         Ok(ctx)
//     }
// }

// Re-export key components from handlers module
pub use handlers::with_database;

// Re-export testing types for mocks
#[cfg(test)]
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

        // Define a mock error type
        #[derive(Debug, Clone)]
        pub struct MockError(pub String);

        impl From<String> for MockError {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl std::fmt::Display for MockError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "MockError")
            }
        }

        impl std::error::Error for MockError {}

        // Define a mock backend type
        #[derive(Debug, Clone)]
        pub struct MockBackend;

        #[async_trait]
        impl DatabaseBackend for MockBackend {
            type Connection = MockConnection;
            type Pool = MockPool;
            type Error = MockError;

            async fn new(_config: DatabaseConfig) -> Result<Self, Self::Error> {
                Ok(Self)
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
                "mock://test".to_string()
            }
        }
    }
}
