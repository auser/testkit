use std::{fmt::Display, sync::Arc};

use parking_lot::Mutex;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::{
    backend::{DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    test_db::sync_drop_database,
};

/// A unique name for a database
#[derive(Debug, Clone)]
pub struct DatabaseName(String);

impl DatabaseName {
    /// Create a new database name with a prefix
    pub fn new(prefix: &str) -> Self {
        Self(format!("{}_{}", prefix, Uuid::new_v4()))
    }

    /// Get the database name as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for DatabaseName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A template database that can be used to create immutable copies
pub struct DatabaseTemplate<B: DatabaseBackend + Clone + Send + 'static>
where
    B::Pool: DatabasePool<Connection = B::Connection>,
{
    backend: B,
    config: PoolConfig,
    name: DatabaseName,
    replicas: Arc<Mutex<Vec<DatabaseName>>>,
    semaphore: Arc<Semaphore>,
}

impl<B: DatabaseBackend + Clone + Send + 'static> DatabaseTemplate<B> {
    /// Create a new template database
    pub async fn new(backend: B, config: PoolConfig, max_replicas: usize) -> Result<Self> {
        let name = DatabaseName::new("testkit");
        backend.create_database(&name).await?;

        Ok(Self {
            backend,
            config,
            name,
            replicas: Arc::new(Mutex::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(max_replicas)),
        })
    }

    /// Initialize the template database with a setup function
    pub async fn initialize_template<F, Fut>(&self, setup: F) -> Result<()>
    where
        F: FnOnce(B::Connection) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let pool = self.backend.create_pool(&self.name, &self.config).await?;
        let conn = pool.acquire().await?;
        setup(conn).await?;
        Ok(())
    }

    /// Get an immutable copy of the template database
    pub async fn get_immutable_database(&self) -> Result<ImmutableDatabase<'_, B>> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| PoolError::PoolCreationFailed(e.to_string()))?;

        let name = DatabaseName::new("test");
        self.backend
            .create_database_from_template(&name, &self.name)
            .await?;

        let pool = self.backend.create_pool(&name, &self.config).await?;
        self.replicas.lock().push(name.clone());

        Ok(ImmutableDatabase {
            name,
            pool,
            backend: self.backend.clone(),
            _permit,
        })
    }
}

impl<B: DatabaseBackend + Clone + Send + 'static> Drop for DatabaseTemplate<B> {
    fn drop(&mut self) {
        let replicas = self.replicas.lock().clone();
        let backend = self.backend.clone();
        let name = self.name.clone();

        println!("Dropping template database: {}", name);
        for replica in replicas {
            println!("Dropping replica database: {}", replica);
            let connection_string = backend.connection_string(&name);
            if let Err(e) = sync_drop_database(&connection_string) {
                tracing::error!("Failed to drop replica database: {}", e);
            }
        }

        let connection_string = backend.connection_string(&name);
        if let Err(e) = sync_drop_database(&connection_string) {
            tracing::error!("Failed to drop template database: {}", e);
        }
    }
}

impl<B: DatabaseBackend + Clone + Send + 'static> Clone for DatabaseTemplate<B>
where
    B::Pool: DatabasePool<Connection = B::Connection>,
{
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            config: self.config.clone(),
            name: self.name.clone(),
            replicas: self.replicas.clone(),
            semaphore: self.semaphore.clone(),
        }
    }
}

/// An immutable copy of a template database
pub struct ImmutableDatabase<'a, B: DatabaseBackend + Clone + Send + 'static> {
    name: DatabaseName,
    pool: B::Pool,
    #[allow(dead_code)]
    backend: B,
    _permit: tokio::sync::SemaphorePermit<'a>,
}

impl<'a, B: DatabaseBackend + Clone + Send + 'static> ImmutableDatabase<'a, B> {
    /// Get the pool for this database
    pub fn get_pool(&self) -> &B::Pool {
        &self.pool
    }

    /// Get the name of this database
    pub fn get_name(&self) -> &DatabaseName {
        &self.name
    }
}

impl<'a, B: DatabaseBackend + Clone + Send + 'static> Drop for ImmutableDatabase<'a, B> {
    fn drop(&mut self) {
        let name = self.name.clone();
        let connection_string = self.backend.connection_string(&name);

        if let Err(e) = sync_drop_database(&connection_string) {
            tracing::error!("Failed to drop database: {:?}", e);
        }
    }
}
