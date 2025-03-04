use crate::{
    DBTransactionManager, DatabaseBackend, DatabasePool, TestContext, TestDatabaseInstance,
    TransactionStarter,
};

use async_trait::async_trait;
use std::fmt::Debug;

/// Trait for handlers that can be executed in the context of a database
#[async_trait]
pub trait DatabaseHandler<DB>: Send + Sync
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
{
    /// Execute the handler with the given context
    async fn execute(&self, ctx: &mut TestContext<DB>) -> Result<(), DB::Error>;
}

// Implementation of DatabaseHandler for FnOnce closures
#[async_trait]
impl<DB, F, Fut> DatabaseHandler<DB> for F
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    F: FnOnce(&mut TestContext<DB>) -> Fut + Send + Sync + Clone,
    Fut: std::future::Future<Output = Result<(), DB::Error>> + Send + 'static,
{
    async fn execute(&self, ctx: &mut TestContext<DB>) -> Result<(), DB::Error> {
        self.clone()(ctx).await
    }
}

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

impl<DB, S, TFn> TransactionHandler<DB, S, TFn>
where
    DB: DatabaseBackend + Send + Sync + Debug + 'static,
    S: Send + Sync + 'static,
    TFn: Send + Sync + 'static,
{
    pub fn new(db: TestDatabaseInstance<DB>, setup_fn: S, transaction_fn: TFn) -> Self {
        Self {
            db,
            setup_fn,
            transaction_fn,
        }
    }
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
