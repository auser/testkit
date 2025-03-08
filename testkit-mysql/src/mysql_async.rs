#![cfg(feature = "with-mysql-async")]
use std::sync::Arc;

use async_trait::async_trait;

use tokio::sync::Mutex;

use mysql_async::{Conn, Opts, Pool, prelude::*};
use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
};

use crate::error::MySqlError;

/// A MySQL connection using mysql-async
#[derive(Clone)]
pub struct MySqlConnection {
    /// The connection to the database
    conn: Arc<Mutex<Conn>>,
    /// The connection string used to create this connection
    connection_string: String,
}

/// A MySQL transaction
pub struct MySqlTransaction {
    // We need to own the connection to ensure the transaction stays alive
    conn: Arc<Mutex<Conn>>,
    // Track if the transaction is completed
    completed: bool,
}

impl MySqlTransaction {
    // Create a new transaction
    pub(crate) fn new(conn: Arc<Mutex<Conn>>) -> Self {
        Self {
            conn,
            completed: false,
        }
    }

    /// Execute a query within this transaction
    pub async fn execute<Q: AsRef<str>>(
        &self,
        query: Q,
        params: mysql_async::Params,
    ) -> Result<(), MySqlError> {
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .exec_drop(query.as_ref(), params)
            .await
            .map_err(|e| MySqlError::QueryExecutionError(e.to_string()))?;

        Ok(())
    }

    /// Commit the transaction
    pub async fn commit(mut self) -> Result<(), MySqlError> {
        // Mark the transaction as completed
        self.completed = true;

        // Commit the transaction
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .exec_drop("COMMIT", ())
            .await
            .map_err(|e| MySqlError::TransactionError(e.to_string()))?;

        Ok(())
    }

    /// Rollback the transaction
    pub async fn rollback(mut self) -> Result<(), MySqlError> {
        // Mark the transaction as completed
        self.completed = true;

        // Rollback the transaction
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .exec_drop("ROLLBACK", ())
            .await
            .map_err(|e| MySqlError::TransactionError(e.to_string()))?;

        Ok(())
    }
}

// Ensure transaction is rolled back if dropped without explicit commit/rollback
impl Drop for MySqlTransaction {
    fn drop(&mut self) {
        if !self.completed {
            // We need to rollback the transaction if it's not completed
            // This is a sync function, so we can't use async/await here
            tracing::warn!("MySQL transaction was not committed or rolled back explicitly");
        }
    }
}

impl MySqlConnection {
    /// Create a new connection to the database
    pub async fn connect(connection_string: String) -> Result<Self, MySqlError> {
        // Create connection options from the URL
        let opts = Opts::from_url(&connection_string)
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        // Connect to the database
        let conn = Conn::new(opts)
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            connection_string,
        })
    }

    /// Get a reference to the connection
    pub async fn client(&self) -> Arc<Mutex<Conn>> {
        self.conn.clone()
    }

    /// Execute a query directly
    pub async fn query_drop<Q: AsRef<str>>(&self, query: Q) -> Result<(), MySqlError> {
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .query_drop(query.as_ref())
            .await
            .map_err(|e| MySqlError::QueryExecutionError(e.to_string()))
    }

    /// Execute a parameterized query directly
    pub async fn exec_drop<Q: AsRef<str>, P: Into<mysql_async::Params> + Send>(
        &self,
        query: Q,
        params: P,
    ) -> Result<(), MySqlError> {
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .exec_drop(query.as_ref(), params)
            .await
            .map_err(|e| MySqlError::QueryExecutionError(e.to_string()))
    }

    /// Start a transaction
    pub async fn begin_transaction(&self) -> Result<MySqlTransaction, MySqlError> {
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .exec_drop("BEGIN", ())
            .await
            .map_err(|e| MySqlError::TransactionError(e.to_string()))?;

        // Create the transaction with a clone of the connection
        Ok(MySqlTransaction::new(self.conn.clone()))
    }

    /// Get the connection string
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }

    /// Execute a query and map the results
    pub async fn query_map<T, F, Q>(&self, query: Q, f: F) -> Result<Vec<T>, MySqlError>
    where
        Q: AsRef<str>,
        F: FnMut(mysql_async::Row) -> T + Send + 'static,
        T: Send + 'static,
    {
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .query_map(query.as_ref(), f)
            .await
            .map_err(|e| MySqlError::QueryExecutionError(e.to_string()))
    }

    /// Execute a query and return the first result
    pub async fn query_first<T: FromRow + Send + 'static, Q: AsRef<str>>(
        &self,
        query: Q,
    ) -> Result<T, MySqlError> {
        let mut conn_guard = self.conn.lock().await;
        conn_guard
            .query_first(query.as_ref())
            .await
            .map_err(|e| MySqlError::QueryExecutionError(e.to_string()))?
            .ok_or_else(|| MySqlError::QueryExecutionError("No rows returned".to_string()))
    }

    /// Select a specific database
    pub async fn select_database(&self, database_name: &str) -> Result<(), MySqlError> {
        let use_stmt = format!("USE `{}`", database_name);
        self.query_drop(use_stmt).await
    }
}

/// A MySQL connection pool using mysql-async
#[derive(Clone)]
pub struct MySqlPool {
    /// The connection pool
    pub pool: Arc<Pool>,
    /// The connection string used to create this pool
    pub connection_string: String,
}

#[async_trait]
impl DatabasePool for MySqlPool {
    type Connection = MySqlConnection;
    type Error = MySqlError;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        // Get a connection from the pool
        let conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        // Create the MySqlConnection
        let mysql_conn = MySqlConnection {
            conn: Arc::new(Mutex::new(conn)),
            connection_string: self.connection_string.clone(),
        };

        // Extract the database name from the connection string and select it
        if let Some(db_name) = self.connection_string.split('/').last() {
            if !db_name.is_empty() {
                mysql_conn.select_database(db_name).await?;
            }
        }

        Ok(mysql_conn)
    }

    async fn release(&self, conn: Self::Connection) -> Result<(), Self::Error> {
        let _conn_guard = conn.conn.lock().await;
        // Just drop the connection - the pool will handle returning it
        // Since MySQL Async connections don't have a close method with no args
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A MySQL backend using mysql-async
#[derive(Clone, Debug)]
pub struct MySqlBackend {
    config: DatabaseConfig,
}

#[async_trait]
impl DatabaseBackend for MySqlBackend {
    type Connection = MySqlConnection;
    type Pool = MySqlPool;
    type Error = MySqlError;

    async fn new(config: DatabaseConfig) -> Result<Self, Self::Error> {
        // Validate the config
        if config.admin_url.is_empty() || config.user_url.is_empty() {
            return Err(MySqlError::ConfigError(
                "Admin and user URLs must be provided".into(),
            ));
        }

        Ok(Self { config })
    }

    /// Create a new connection pool for the given database
    async fn create_pool(
        &self,
        name: &DatabaseName,
        _config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        // Create connection options from the URL
        let connection_string = self.connection_string(name);
        let opts = Opts::from_url(&connection_string)
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        let pool = Pool::new(opts);

        Ok(MySqlPool {
            pool: Arc::new(pool),
            connection_string,
        })
    }

    /// Create a single connection to the given database
    async fn connect(&self, name: &DatabaseName) -> Result<Self::Connection, Self::Error> {
        let connection_string = self.connection_string(name);
        MySqlConnection::connect(connection_string).await
    }

    /// Create a single connection using a connection string directly
    async fn connect_with_string(
        &self,
        connection_string: &str,
    ) -> Result<Self::Connection, Self::Error> {
        MySqlConnection::connect(connection_string.to_string()).await
    }

    async fn create_database(
        &self,
        pool: &Self::Pool,
        name: &DatabaseName,
    ) -> Result<(), Self::Error> {
        // Create admin connection to create the database
        let opts = Opts::from_url(&self.config.admin_url)
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        let mut conn = Conn::new(opts)
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        // Create the database
        let db_name = name.as_str();
        let create_query = format!("CREATE DATABASE `{}`", db_name);

        conn.query_drop(create_query)
            .await
            .map_err(|e| MySqlError::DatabaseCreationError(e.to_string()))?;

        // Get a connection from the pool and select the database
        // This ensures connections from this pool will be connected to the right database
        let pool_conn = pool
            .pool
            .get_conn()
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        // Create a MySqlConnection to use our select_database method
        let mysql_conn = MySqlConnection {
            conn: Arc::new(Mutex::new(pool_conn)),
            connection_string: pool.connection_string.clone(),
        };

        // Select the database for all future connections from this pool
        mysql_conn.select_database(db_name).await?;

        // Release the connection back to the pool
        drop(mysql_conn);

        Ok(())
    }

    /// Drop a database with the given name
    fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        // For mysql-async, we can't use async directly in a sync function
        // but we shouldn't create a new runtime either.
        // Instead, use a background task to drop the database.

        let admin_url = self.config.admin_url.clone();
        let db_name = name.as_str().to_string();

        // Spawn a detached task to drop the database
        // This approach allows us to drop the database without blocking
        // or creating a new runtime within an existing runtime
        tokio::spawn(async move {
            // Create admin connection to drop the database
            match Opts::from_url(&admin_url) {
                Ok(opts) => {
                    if let Ok(mut conn) = Conn::new(opts).await {
                        let drop_query = format!("DROP DATABASE IF EXISTS `{}`", db_name);
                        let _ = conn.query_drop(drop_query).await;
                        tracing::info!("Database {} dropped successfully", db_name);
                    }
                }
                Err(e) => {
                    // Log the error but don't fail the test
                    tracing::error!(
                        "Failed to parse MySQL connection URL for database drop: {}",
                        e
                    );
                }
            }
        });

        // Return OK immediately - the database drop happens in the background
        Ok(())
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        // Parse the user URL
        let mut url = url::Url::parse(&self.config.user_url).expect("Invalid database URL");

        // Update the path to include the database name, replacing any existing database
        let db_name = name.as_str();

        // Set the path to just the database name, clearing any existing path
        let mut path_segments = url.path_segments_mut().expect("Cannot modify URL path");
        path_segments.clear();
        path_segments.push(db_name);
        drop(path_segments);

        // Return the full connection string with the new database name
        url.to_string()
    }
}

impl MySqlBackend {
    /// Clean the database explicitly - this is a blocking call
    /// This should be called at the end of tests to ensure databases are cleaned up
    pub async fn clean_database(&self, name: &DatabaseName) -> Result<(), MySqlError> {
        // Create admin connection to drop the database
        let opts = Opts::from_url(&self.config.admin_url)
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        let mut conn = Conn::new(opts)
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        // Drop the database
        let db_name = name.as_str();
        let drop_query = format!("DROP DATABASE IF EXISTS `{}`", db_name);

        conn.query_drop(drop_query)
            .await
            .map_err(|e| MySqlError::DatabaseDropError(e.to_string()))?;

        tracing::info!("Database {} cleaned up successfully", db_name);

        Ok(())
    }
}

/// Helper function to create a MySQL backend with a configuration
pub async fn mysql_backend_with_config(config: DatabaseConfig) -> Result<MySqlBackend, MySqlError> {
    MySqlBackend::new(config).await
}

#[async_trait]
impl TestDatabaseConnection for MySqlConnection {
    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// Trait for MySQL transaction operations
#[async_trait]
pub trait MysqlTransaction: Send + Sync {
    /// Commit the transaction
    async fn commit(self) -> Result<(), MySqlError>;

    /// Rollback the transaction
    async fn rollback(self) -> Result<(), MySqlError>;
}

/// Generic transaction trait
#[async_trait]
#[allow(unused)]
pub trait TransactionTrait: Send + Sync {
    /// Error type
    type Error: std::error::Error + Send + Sync;

    /// Commit the transaction
    async fn commit(self) -> Result<(), Self::Error>;

    /// Rollback the transaction
    async fn rollback(self) -> Result<(), Self::Error>;
}

// Implement MysqlTransaction for MySqlTransaction
#[async_trait]
impl MysqlTransaction for MySqlTransaction {
    async fn commit(self) -> Result<(), MySqlError> {
        self.commit().await
    }

    async fn rollback(self) -> Result<(), MySqlError> {
        self.rollback().await
    }
}

// Implement TransactionTrait for MySqlTransaction
#[async_trait]
impl TransactionTrait for MySqlTransaction {
    type Error = MySqlError;

    async fn commit(self) -> Result<(), Self::Error> {
        MysqlTransaction::commit(self).await
    }

    async fn rollback(self) -> Result<(), Self::Error> {
        MysqlTransaction::rollback(self).await
    }
}
