use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::Result,
    pool::PoolConfig,
};
use uuid::Uuid;

/// A test database that handles setup, connections, and cleanup
pub struct TestDatabase<B: DatabaseBackend + 'static> {
    /// The database backend
    pub backend: B,
    /// The connection pool
    pub pool: B::Pool,
    /// Database name for cleanup
    db_name: String,
    /// A unique identifier for test data isolation
    pub test_user: String,
}

pub struct OwnedTransaction<B: DatabaseBackend>
where
    B::Connection: 'static,
{
    _conn: B::Connection, // Keep connection alive
    pub tx: <B::Connection as crate::backend::Connection>::Transaction<'static>,
}

impl<B: DatabaseBackend + 'static> TestDatabase<B> {
    /// Create a new test database with the given backend
    pub async fn new(backend: B, config: PoolConfig) -> Result<Self> {
        // Generate unique name
        let db_name = format!("testkit_{}", Uuid::new_v4().to_string().replace("-", "_"));

        // Create the database
        let db_name_obj = crate::template::DatabaseName::new(&db_name);
        backend.create_database(&db_name_obj).await?;

        // Create the pool
        let pool = backend.create_pool(&db_name_obj, &config).await?;

        // Generate test user ID
        let test_user = format!("test_user_{}", Uuid::new_v4());

        Ok(Self {
            backend,
            pool,
            db_name,
            test_user,
        })
    }

    /// Get a connection from the pool
    pub async fn connection(&self) -> Result<B::Connection> {
        self.pool.acquire().await
    }

    /// Begin a transaction
    pub async fn begin_transaction(&self) -> Result<OwnedTransaction<B>> {
        let mut conn = self.connection().await?;
        let tx = conn.begin().await?;

        // This requires your Transaction type to be 'static compatible
        let tx = unsafe {
            std::mem::transmute::<
                <B::Connection as Connection>::Transaction<'_>,
                <B::Connection as Connection>::Transaction<'static>,
            >(tx)
        };

        Ok(OwnedTransaction { _conn: conn, tx })
    }

    /// Setup the database with a function
    pub async fn setup<F, Fut>(&self, setup_fn: F) -> Result<()>
    where
        F: FnOnce(B::Connection) -> Fut + Send,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let conn = self.connection().await?;
        setup_fn(conn).await
    }
}

impl<B: DatabaseBackend + 'static> Drop for TestDatabase<B> {
    fn drop(&mut self) {
        // Clone values for the async block
        let backend = self.backend.clone();
        let db_name = self.db_name.clone();
        let db_name_obj = crate::template::DatabaseName::new(&db_name);

        // Spawn cleanup task
        tokio::spawn(async move {
            if let Err(e) = backend.drop_database(&db_name_obj).await {
                tracing::error!("Failed to drop test database {}: {}", db_name, e);
            }
        });
    }
}
