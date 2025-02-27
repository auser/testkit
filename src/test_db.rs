use std::{fmt::Display, process::Command, sync::Arc};

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::Result,
    pool::PoolConfig,
    wrapper::ResourcePool,
    PoolError,
};
use parking_lot::Mutex;
use tokio::sync::Semaphore;
use url::Url;
use uuid::Uuid;

/// A test database that handles setup, connections, and cleanup
pub struct TestDatabase<B: DatabaseBackend + 'static> {
    /// The database backend
    pub backend: B,
    /// The connection pool
    pub pool: B::Pool,
    /// A unique identifier for test data isolation
    pub test_user: String,
    /// The database name
    pub db_name: DatabaseName,
    /// Connection pool for reusable connections
    connection_pool: Option<Arc<ResourcePool<B::Connection>>>,
}

/// Controls how the test database should be created
pub enum TestDatabaseMode<B: DatabaseBackend + Clone + Send + 'static> {
    /// Create a fresh database
    Fresh,
    /// Create a database from a template
    FromTemplate(Arc<TestDatabaseTemplate<B>>),
}

/// A template database that can be used to create immutable copies
pub struct TestDatabaseTemplate<B: DatabaseBackend + Clone + Send + 'static> {
    backend: B,
    config: PoolConfig,
    name: DatabaseName,
    replicas: Arc<Mutex<Vec<DatabaseName>>>,
    semaphore: Arc<Semaphore>,
}

impl<B: DatabaseBackend + Clone + Send + 'static> TestDatabaseTemplate<B> {
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
    pub async fn initialize<F, Fut>(&self, setup: F) -> Result<()>
    where
        F: FnOnce(B::Connection) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let pool = self.backend.create_pool(&self.name, &self.config).await?;
        let conn = pool.acquire().await?;
        setup(conn).await?;
        Ok(())
    }

    /// Create a test database from this template
    pub async fn create_test_database(&self) -> Result<TestDatabase<B>> {
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

        // Generate test user ID
        let test_user = format!("test_user_{}", Uuid::new_v4());

        Ok(TestDatabase {
            backend: self.backend.clone(),
            pool,
            test_user,
            db_name: name,
            connection_pool: None,
        })
    }
}

impl<B: DatabaseBackend + Send + Sync + Clone + 'static> Drop for TestDatabaseTemplate<B> {
    fn drop(&mut self) {
        let replicas = self.replicas.lock().clone();
        let backend = self.backend.clone();
        let name = self.name.clone();

        for replica in replicas {
            let connection_string = backend.connection_string(&replica);
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
        let db_name = DatabaseName::new("testkit");

        // Create the database
        backend.create_database(&db_name).await?;

        // Create the pool
        let pool = backend.create_pool(&db_name, &config).await?;

        // Generate test user ID
        let test_user = format!("test_user_{}", Uuid::new_v4());

        Ok(Self {
            backend,
            pool,
            test_user,
            db_name,
            connection_pool: None,
        })
    }

    /// Initialize a resource pool for connections
    pub async fn initialize_connection_pool(&mut self) -> Result<()> {
        let backend = self.backend.clone();
        let db_name = self.db_name.clone();
        let config = PoolConfig::default();

        use std::pin::Pin;

        let init = Box::new(move || {
            let backend = backend.clone();
            let db_name = db_name.clone();
            let config = config.clone();

            Box::pin(async move {
                let pool = backend.create_pool(&db_name, &config).await.unwrap();
                pool.acquire().await.unwrap()
            })
                as Pin<Box<dyn std::future::Future<Output = B::Connection> + Send + 'static>>
        });

        let reset = Box::new(|conn: B::Connection| {
            Box::pin(async move { conn })
                as Pin<Box<dyn std::future::Future<Output = B::Connection> + Send + 'static>>
        });

        self.connection_pool = Some(Arc::new(ResourcePool::new(init, reset)));
        Ok(())
    }

    /// Get a connection from the pool
    pub async fn connection(&self) -> Result<B::Connection> {
        if let Some(pool) = &self.connection_pool {
            // Can't move out of the Reusable directly
            let reusable = pool.acquire().await;
            // Use a hacky approach to extract the connection
            // This isn't ideal but works for this example
            let conn_ptr = &*reusable as *const B::Connection;
            let conn = unsafe { conn_ptr.read() };
            // Now skip the Drop implementation to prevent return to pool
            std::mem::forget(reusable);
            Ok(conn)
        } else {
            self.pool.acquire().await
        }
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

impl<B> Drop for TestDatabase<B>
where
    B: DatabaseBackend + Send + Sync + Clone + 'static,
{
    fn drop(&mut self) {
        let connection_string = self.pool.connection_string();

        if let Err(e) = sync_drop_database(&connection_string) {
            tracing::error!("Failed to drop database: {:?}", e);
        }
    }
}

pub fn sync_drop_database(database_uri: &str) -> Result<()> {
    let parsed = Url::parse(database_uri).map_err(PoolError::UrlParseError)?;
    let database_name = parsed.path().trim_start_matches('/');

    #[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
    drop_postgres_database(&parsed, database_name)?;

    Ok(())
}

fn drop_postgres_database(parsed: &Url, database_name: &str) -> Result<()> {
    let test_user = parsed.username();

    let database_host = format!(
        "{}://{}:{}@{}:{}",
        parsed.scheme(),
        "postgres", // Always use the postgres superuser for dropping
        parsed.password().unwrap_or(""),
        parsed.host_str().unwrap_or(""),
        parsed.port().unwrap_or(5432)
    );

    terminate_connections(&database_host, database_name)?;
    drop_role_command(&database_host, test_user)?;
    drop_database_command(&database_host, database_name)?;

    Ok(())
}

fn terminate_connections(database_host: &str, database_name: &str) -> Result<()> {
    let output = Command::new("psql")
        .arg(database_host)
        .arg("-c")
        .arg(format!("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{database_name}' AND pid <> pg_backend_pid();"))
        .output().map_err(PoolError::IoError)?;

    if !output.status.success() {
        return Err(PoolError::DatabaseDropFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

fn drop_database_command(database_host: &str, database_name: &str) -> Result<()> {
    let output = Command::new("psql")
        .arg(database_host)
        .arg("-c")
        .arg(format!("DROP DATABASE \"{database_name}\";"))
        .output()
        .map_err(PoolError::IoError)?;

    if !output.status.success() {
        return Err(PoolError::DatabaseDropFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

fn drop_role_command(database_host: &str, role_name: &str) -> Result<()> {
    // Skip dropping the role if it's postgres (superuser) or postgres_user
    if role_name == "postgres" || role_name == "postgres_user" {
        tracing::debug!("Skipping drop of system user: {}", role_name);
        return Ok(());
    }

    let output = Command::new("psql")
        .arg(database_host)
        .arg("-c")
        .arg(format!("DROP ROLE IF EXISTS \"{role_name}\";"))
        .output()
        .map_err(PoolError::IoError)?;

    if !output.status.success() {
        // If the error is about current user, just log and continue
        let error = String::from_utf8_lossy(&output.stderr).to_string();
        if error.contains("current user cannot be dropped") {
            return Ok(());
        }

        return Err(PoolError::DatabaseDropFailed(error));
    }
    Ok(())
}

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
