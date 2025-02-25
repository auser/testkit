use std::marker::PhantomData;
use std::sync::Arc;

pub struct TestDatabase<P: DatabasePool> {
    pool: Arc<P>,
    _phantom: PhantomData<P>,
}

impl<P: DatabasePool> TestDatabase<P> {
    pub async fn new(pool: P) -> Self {
        Self {
            pool: Arc::new(pool),
            _phantom: PhantomData,
        }
    }

    pub async fn connection(
        &self,
    ) -> Result<P::Connection, <P::Connection as DatabaseConnection>::Error> {
        self.pool.connection().await
    }

    pub async fn begin(&self) -> Result<P::Tx, <P::Connection as DatabaseConnection>::Error> {
        self.pool.begin().await
    }

    pub async fn setup<F, Fut>(
        &self,
        setup_fn: F,
    ) -> Result<(), <P::Connection as DatabaseConnection>::Error>
    where
        F: FnOnce(P::Connection) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), <P::Connection as DatabaseConnection>::Error>>
            + Send,
    {
        self.pool.setup(setup_fn).await
    }
}

impl<P: DatabasePool> Drop for TestDatabase<P> {
    fn drop(&mut self) {
        // This will be a blocking operation to ensure the database is dropped
        // even if the test panics
        let pool = Arc::clone(&self.pool);

        // Use a runtime handle to run the async drop function
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                // Drop database logic here
                if let Ok(mut conn) = pool.connection().await {
                    let _ = conn.execute("DROP DATABASE IF EXISTS test_db").await;
                }
            });
        });
    }
}
