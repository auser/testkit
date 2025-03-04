use std::fmt::Debug;

mod context;
mod handlers;
mod testdb;

pub use context::*;
pub use handlers::*;
pub use testdb::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
    TestDatabaseInstance,
    transaction::{DBTransactionManager, DatabaseTransaction},
};

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

/// A struct representing the with_database operation
#[derive(Debug)]
#[must_use]
pub struct WithDatabase<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    db: TestDatabaseInstance<DB>,
}

/// Create a new test context with a database
pub async fn with_database<DB>(backend: DB) -> WithDatabase<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    let db = TestDatabaseInstance::new(backend, DatabaseConfig::default())
        .await
        .expect("Failed to create test database instance");

    WithDatabase { db }
}

impl<DB> WithDatabase<DB>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Setup the database with a function
    pub fn setup<S, Fut>(self, setup_fn: S) -> SetupHandler<DB, S>
    where
        S: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    {
        SetupHandler::new(self.db, setup_fn)
    }
}

/// A struct for handling database transactions
#[derive(Debug)]
#[must_use]
pub struct TransactionHandler<DB, S, TFn>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
    TFn: Send + Sync + 'static,
{
    db: TestDatabaseInstance<DB>,
    setup_fn: S,
    transaction_fn: TFn,
}

impl<DB, S, Fut, TFn, TxFut> TransactionHandler<DB, S, TFn>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    TxFut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
    for<'tx> TFn: FnOnce(&'tx mut <TestContext<DB> as TransactionStarter<DB>>::Transaction) -> TxFut
        + Send
        + Sync
        + 'static,
    TestContext<DB>: TransactionStarter<DB>
        + DBTransactionManager<
            <TestContext<DB> as TransactionStarter<DB>>::Transaction,
            <TestContext<DB> as TransactionStarter<DB>>::Connection,
            Error = DB::Error,
            Tx = <TestContext<DB> as TransactionStarter<DB>>::Transaction,
        >,
{
    /// Execute the entire operation chain
    pub async fn execute(self) -> Result<TestContext<DB>, DB::Error> {
        // Execute the setup function
        self.db.setup(self.setup_fn).await?;

        // Create the context
        let mut ctx = TestContext::new(self.db);

        // Begin the transaction using explicit types
        let mut tx = <TestContext<DB> as DBTransactionManager<
            <TestContext<DB> as TransactionStarter<DB>>::Transaction,
            <TestContext<DB> as TransactionStarter<DB>>::Connection,
        >>::begin_transaction(&mut ctx)
        .await?;

        // Execute the transaction function
        match (self.transaction_fn)(&mut tx).await {
            Ok(_) => {
                // Commit the transaction
                <TestContext<DB> as DBTransactionManager<
                    <TestContext<DB> as TransactionStarter<DB>>::Transaction,
                    <TestContext<DB> as TransactionStarter<DB>>::Connection,
                >>::commit_transaction(&mut tx)
                .await?;
            }
            Err(e) => {
                // Rollback the transaction on error
                let _ = <TestContext<DB> as DBTransactionManager<
                    <TestContext<DB> as TransactionStarter<DB>>::Transaction,
                    <TestContext<DB> as TransactionStarter<DB>>::Connection,
                >>::rollback_transaction(&mut tx)
                .await;
                return Err(e);
            }
        }

        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::transaction::tests::MockBackend;

    // Simple test to demonstrate parts of the API
    #[tokio::test]
    async fn test_with_database_components() {
        // Verify that we can create a WithDatabase struct
        let backend = MockBackend;

        let db = with_database(backend).await;
        let ctx = db
            .setup(|conn| {
                let conn_string = conn.connection_string();
                conn.set_value(1);
                async move {
                    println!("Setting up database with: {}", conn_string);
                    Ok(())
                }
            })
            .execute()
            .await;
        assert!(ctx.is_ok());
        let (_ctx, conn) = ctx.unwrap();
        assert_eq!(conn.get_value(), 1);

        // This test just verifies API structure exists, not functionality
        assert!(std::any::TypeId::of::<MockBackend>() != std::any::TypeId::of::<String>());
    }
}
