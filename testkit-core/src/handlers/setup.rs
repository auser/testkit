use std::fmt::Debug;

use crate::{
    DatabaseBackend, DatabasePool, TestContext, TestDatabaseInstance, TransactionHandler,
    TransactionStarter,
};

/// A struct for handling database setup
#[derive(Debug)]
#[must_use]
pub struct SetupHandler<DB, S>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
{
    db: TestDatabaseInstance<DB>,
    setup_fn: S,
}

impl<DB, S> SetupHandler<DB, S>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
{
    pub fn new(db: TestDatabaseInstance<DB>, setup_fn: S) -> Self {
        Self { db, setup_fn }
    }
}

impl<DB, S, Fut> SetupHandler<DB, S>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: FnOnce(&mut <DB::Pool as DatabasePool>::Connection) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
{
    /// Execute an operation with a transaction
    pub fn with_transaction<TFn, TxFut>(self, transaction_fn: TFn) -> TransactionHandler<DB, S, TFn>
    where
        TxFut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
        for<'tx> TFn: FnOnce(&'tx mut <TestContext<DB> as TransactionStarter<DB>>::Transaction) -> TxFut
            + Send
            + Sync
            + 'static,
        TestContext<DB>: TransactionStarter<DB>,
    {
        TransactionHandler::new(self.db, self.setup_fn, transaction_fn)
    }

    /// Execute the setup operation only
    pub async fn execute(
        self,
    ) -> Result<(TestContext<DB>, <DB::Pool as DatabasePool>::Connection), DB::Error> {
        // Get a connection from the pool
        let mut conn = self.db.acquire_connection().await?;

        // Execute the setup function directly (not through db.setup)
        (self.setup_fn)(&mut conn).await?;

        // Create the context
        let ctx = TestContext::new(self.db);

        // Return both the context and the connection
        Ok((ctx, conn))
    }
}
