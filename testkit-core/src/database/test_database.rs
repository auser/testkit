use crate::{DatabaseConfig, DatabaseName};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::fmt::Debug;
use std::fmt::Display;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

/// A trait for database connections that can be pooled
#[async_trait]
pub trait Connection: Send {
    /// The transaction type for this connection
    type Transaction<'conn>: Send + 'conn
    where
        Self: 'conn;

    /// The error type for this connection
    type Error: Send + Sync + Clone + From<String> + From<&'static str> + Display + Debug;

    /// Check if the connection is valid
    async fn is_valid(&self) -> bool;

    /// Reset the connection state
    async fn reset(&mut self) -> Result<(), Self::Error>;

    /// Execute a SQL query
    async fn execute(&mut self, sql: &str) -> Result<(), Self::Error>;

    /// Begin a new transaction
    async fn begin(&mut self) -> Result<Self::Transaction<'_>, Self::Error>;
}

/// A trait for database pools that can be used to acquire and release connections
#[async_trait]
pub trait DatabasePool: Send + Sync + Clone {
    /// The type of connection this pool provides
    type Pool: Send + Sync + Clone;
    /// The type of connection this pool provides
    type Connection: Send + Sync + Clone;
    /// The type of error this pool can return
    type Error: Send + Sync + Clone + From<String> + From<&'static str> + Display + Debug;

    /// Acquire a connection from the pool
    async fn acquire(&self) -> Result<Self::Connection, Self::Error>;

    /// Release a connection back to the pool
    async fn release(&self, conn: Self::Pool) -> Result<(), Self::Error>;

    /// Get the database URL for this pool
    fn connection_string(&self) -> String;
}

/// Trait defining a test database abstraction
///
/// A test database is a temporary database instance that is created for testing
/// and automatically cleaned up when it is dropped.
///
/// Implementations of this trait should:
/// 1. Create a new database with a unique name when `new()` is called
/// 2. Implement `Drop` to clean up the database when finished
/// 3. Provide a context that can be used with transactions
/// A trait for database backends that can create and manage databases
#[async_trait]
pub trait DatabaseBackend: Send + Sync + Clone {
    type Connection: Connection<Error = Self::Error>;
    type Pool: DatabasePool;
    type Error: Send + Sync + Clone + From<String> + From<&'static str> + Display + Debug;

    /// Connect to the database
    async fn connect(&self) -> Result<Self::Pool, Self::Error>;

    /// Create a new database with the given name
    async fn create_database(&self, name: &DatabaseName) -> Result<(), Self::Error>;

    /// Drop a database with the given name
    async fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error>;

    /// Create a new connection pool for the given database
    async fn create_pool(
        &self,
        name: &DatabaseName,
        config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error>;

    /// Terminate all connections to the given database
    async fn terminate_connections(&self, name: &DatabaseName) -> Result<(), Self::Error>;

    /// Create a new database from a template
    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<(), Self::Error>;

    /// Create a test user with limited privileges
    async fn create_test_user(
        &self,
        _name: &DatabaseName,
        _username: &str,
    ) -> Result<(), Self::Error> {
        // Default implementation does nothing
        // This is optional for backends that don't support user creation
        Ok(())
    }

    /// Grant necessary privileges to a test user
    async fn grant_privileges(
        &self,
        _name: &DatabaseName,
        _username: &str,
    ) -> Result<(), Self::Error> {
        // Default implementation does nothing
        // This is optional for backends that don't support privilege management
        Ok(())
    }

    /// Get a connection string for the admin/superuser
    fn get_admin_connection_string(&self, name: &DatabaseName) -> String {
        // Default implementation just returns the regular connection string
        self.connection_string(name)
    }

    /// Get the connection string for the given database
    fn connection_string(&self, name: &DatabaseName) -> String;

    /// Convert a pool connection to a backend connection
    ///
    /// This is used for compatibility between pool connections and backend connections
    /// The default implementation returns an error since most backends will need to
    /// implement their own conversion
    async fn convert_connection(
        &self,
        _conn: <Self::Pool as DatabasePool>::Connection,
    ) -> Result<Self::Connection, Self::Error> {
        Err(Self::Error::from("Connection conversion not implemented"))
    }
}

/// Type alias for a pool of database connections
type PooledConnections<P> = Option<Arc<Mutex<Vec<P>>>>;

/// A test database that handles setup, connections, and cleanup
pub struct TestDatabase<B>
where
    B: DatabaseBackend + 'static,
{
    /// The database backend
    pub backend: B,
    /// The connection pool
    pub pool: B::Pool,
    /// A unique identifier for test data isolation
    pub test_user: String,
    /// The database name
    pub db_name: DatabaseName,
    /// Connection pool for reusable connections
    connection_pool: PooledConnections<<B::Pool as DatabasePool>::Connection>,
    /// The database config
    pub config: DatabaseConfig,
}

impl<B> TestDatabase<B>
where
    B: DatabaseBackend + 'static,
{
    /// Create a new test database with the given backend
    pub async fn new(backend: B, config: DatabaseConfig) -> Result<Self, B::Error> {
        // Generate unique name
        let db_name = DatabaseName::new(None);

        // Create database with better error handling
        tracing::debug!("Creating test database: {}", db_name);
        match backend.create_database(&db_name).await {
            Ok(_) => tracing::debug!("Successfully created database {}", db_name),
            Err(e) => {
                tracing::error!("Failed to create database {}: {}", db_name, e);
                return Err(e);
            }
        }

        // Create connection pool for the database
        tracing::debug!("Creating connection pool for database: {}", db_name);
        let pool = match backend.create_pool(&db_name, &config).await {
            Ok(p) => {
                tracing::debug!("Successfully created connection pool for {}", db_name);
                p
            }
            Err(e) => {
                tracing::error!("Failed to create connection pool for {}: {}", db_name, e);
                return Err(e);
            }
        };

        // Generate test user ID
        let test_user = format!("testkit_{}", Uuid::new_v4());

        Ok(Self {
            backend,
            pool,
            test_user,
            db_name,
            connection_pool: None,
            config,
        })
    }

    /// Returns a reference to the backend
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns a reference to the database name
    pub fn name(&self) -> &DatabaseName {
        &self.db_name
    }

    /// Initialize a simple pool for connections
    /// We're replacing ResourcePool with a simpler Mutex<Vec<Connection>> implementation
    pub async fn initialize_connection_pool(&mut self) -> Result<(), B::Error> {
        // Initialize an empty connection pool
        self.connection_pool = Some(Arc::new(Mutex::new(Vec::new())));
        Ok(())
    }

    /// Get a connection from the pool or acquire a new one
    pub async fn connection(&self) -> Result<<B::Pool as DatabasePool>::Connection, B::Error> {
        if let Some(pool) = &self.connection_pool {
            // Try to reuse a connection from our pool
            let mut guard = pool.lock();
            if let Some(conn) = guard.pop() {
                return Ok(conn);
            }

            // If no connection available, acquire a new one
            drop(guard); // Release the lock before async operation
        }

        // Acquire a new connection directly from the database pool
        self.pool
            .acquire()
            .await
            .map_err(|e| B::Error::from(format!("Failed to acquire connection: {}", e)))
    }

    /// Setup the database with a function
    /// The connection handling approach needs to match the expected B::Connection type
    pub async fn setup<F, Fut>(&self, setup_fn: F) -> Result<(), B::Error>
    where
        F: FnOnce(&mut <B::Pool as DatabasePool>::Connection) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), B::Error>> + Send,
    {
        // Get a connection from the pool
        let mut conn = self.connection().await?;

        // Call the setup function with a mutable reference to the connection
        let result = setup_fn(&mut conn).await;

        // Return the connection to the pool if we have one
        if let Some(pool) = &self.connection_pool {
            pool.lock().push(conn);
        }

        result
    }

    /// Execute a test function with a database connection
    pub async fn test<F, Fut, T>(&self, test_fn: F) -> Result<T, B::Error>
    where
        F: FnOnce(&mut <B::Pool as DatabasePool>::Connection) -> Fut + Send,
        Fut: std::future::Future<Output = Result<T, B::Error>> + Send,
        T: Send + 'static,
    {
        // Get a connection from the pool
        let mut conn = self.connection().await?;

        // Call the test function with a mutable reference to the connection
        let result = test_fn(&mut conn).await;

        // Return the connection to the pool if we have one
        if let Some(pool) = &self.connection_pool {
            pool.lock().push(conn);
        }

        result
    }

    /// Drop the database when it's no longer needed
    pub async fn sync_drop_database(&self) -> Result<(), B::Error> {
        tracing::debug!("Explicitly dropping database: {}", self.db_name);
        self.backend.drop_database(&self.db_name).await
    }
}

/// A template database that can be used to create immutable copies
#[derive(Clone)]
pub struct TestDatabaseTemplate<B>
where
    B: DatabaseBackend + Clone + Send + 'static,
{
    backend: B,
    config: DatabaseConfig,
    name: DatabaseName,
    replicas: Arc<Mutex<Vec<DatabaseName>>>,
    semaphore: Arc<Semaphore>,
    #[allow(dead_code)]
    base_connection_string: String,
    _phantom: std::marker::PhantomData<B>,
}

impl<B> TestDatabaseTemplate<B>
where
    B: DatabaseBackend + Clone + Send + 'static,
{
    /// Create a new template database
    pub async fn new(
        backend: B,
        config: DatabaseConfig,
        max_replicas: usize,
    ) -> Result<Self, B::Error> {
        let name = DatabaseName::new(None);
        backend.create_database(&name).await?;

        // Store the connection string before moving backend
        let connection_string = backend.connection_string(&name);

        Ok(Self {
            backend,
            config,
            name,
            replicas: Arc::new(Mutex::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(max_replicas)),
            base_connection_string: connection_string,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Returns the name of this template database
    pub fn name(&self) -> &DatabaseName {
        &self.name
    }

    /// Returns a reference to the backend
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns a reference to the config
    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    /// Initialize the template database with a setup function
    pub async fn initialize<F, Fut>(&self, setup: F) -> Result<(), B::Error>
    where
        F: FnOnce(&mut <B::Pool as DatabasePool>::Connection) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), B::Error>> + Send + 'static,
    {
        let pool = self.backend.create_pool(&self.name, &self.config).await?;
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| B::Error::from(format!("Failed to acquire connection: {}", e)))?;

        // Execute setup with the connection
        setup(&mut conn).await?;

        Ok(())
    }

    /// Create a test database from this template
    pub async fn create_test_database(&self) -> Result<TestDatabase<B>, B::Error> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| B::Error::from(format!("Pool creation failed: {}", e)))?;

        let name = DatabaseName::new(None);
        self.backend
            .create_database_from_template(&name, &self.name)
            .await?;

        let pool = self.backend.create_pool(&name, &self.config).await?;
        self.replicas.lock().push(name.clone());

        // Generate test user ID
        let test_user = format!("testkit_user_{}", Uuid::new_v4());

        Ok(TestDatabase {
            backend: self.backend.clone(),
            pool,
            test_user,
            db_name: name,
            config: self.config.clone(),
            connection_pool: None,
        })
    }
}

impl<B> Drop for TestDatabaseTemplate<B>
where
    B: DatabaseBackend + Clone + Send + 'static,
{
    fn drop(&mut self) {
        let replicas = self.replicas.lock().clone();
        let backend = self.backend.clone();
        let name = self.name.clone();

        for replica in replicas {
            let _connection_string = backend.connection_string(&replica);
            // Use backend's drop_database method instead of missing sync_drop_database function
            let _ = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { backend.drop_database(&replica).await })
            });
        }

        let _connection_string = backend.connection_string(&name);
        // Use backend's drop_database method instead of missing sync_drop_database function
        let _ = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { backend.drop_database(&name).await })
        });
    }
}

/// OwnedTransaction with simplified generic parameters
pub struct OwnedTransaction<T> {
    _conn: T, // Keep connection alive
    pub tx: T,
}

// Helper function to begin a transaction
pub async fn begin_transaction<B>(
    db: &TestDatabase<B>,
) -> Result<OwnedTransaction<<B::Pool as DatabasePool>::Connection>, B::Error>
where
    B: DatabaseBackend + 'static,
{
    let conn = db.connection().await?;

    // Create the transaction - this is a placeholder
    // In a real implementation, you would call begin() on the connection
    // and store both the connection and transaction

    Ok(OwnedTransaction {
        _conn: conn.clone(),
        tx: conn,
    })
}

// Helper function to create a test database with the given backend, config, and setup function
#[allow(dead_code)]
pub async fn with_database_setup_raw<B, F, Fut>(
    backend: B,
    config: DatabaseConfig,
    setup_fn: F,
) -> Result<TestDatabase<B>, B::Error>
where
    B: DatabaseBackend + 'static,
    F: FnOnce(&mut <B::Pool as DatabasePool>::Connection) -> Fut + Send,
    Fut: std::future::Future<Output = Result<(), B::Error>> + Send,
{
    // Create the test database
    let mut db = TestDatabase::new(backend, config).await?;

    // Initialize connection pool
    db.initialize_connection_pool().await?;

    // Run the setup function
    db.setup(setup_fn).await?;

    Ok(db)
}

// Helper function to create a test database template
pub async fn with_database_template<B>(
    backend: B,
    config: DatabaseConfig,
    max_replicas: usize,
) -> Result<TestDatabaseTemplate<B>, B::Error>
where
    B: DatabaseBackend + Clone + Send + 'static,
{
    TestDatabaseTemplate::new(backend, config, max_replicas).await
}

// Helper function to create a test database template with a setup function
pub async fn with_database_template_setup<B, F, Fut>(
    backend: B,
    config: DatabaseConfig,
    max_replicas: usize,
    setup_fn: F,
) -> Result<TestDatabaseTemplate<B>, B::Error>
where
    B: DatabaseBackend + Clone + Send + 'static,
    F: FnOnce(&mut <B::Pool as DatabasePool>::Connection) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), B::Error>> + Send + 'static,
{
    // Create the template
    let template = TestDatabaseTemplate::new(backend, config, max_replicas).await?;

    // Initialize with the setup function
    template.initialize(setup_fn).await?;

    Ok(template)
}
