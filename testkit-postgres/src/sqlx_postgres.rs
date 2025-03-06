use crate::error::PostgresError;
use crate::{TransactionManager, TransactionTrait};
use async_trait::async_trait;
use sqlx::postgres::{PgPool, PgPoolOptions, PgTransaction};
use sqlx::query;
use std::fmt::Debug;
use std::process::Command;
use std::sync::Arc;
use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
    TestDatabaseInstance,
};
use url;

/// A connection to a PostgreSQL database using sqlx
#[derive(Debug)]
pub struct SqlxConnection {
    // Store the pool for direct access
    pool: Arc<PgPool>,
    connection_string: String,
}

impl Clone for SqlxConnection {
    fn clone(&self) -> Self {
        // Clone just creates a new connection to the same pool
        SqlxConnection {
            pool: self.pool.clone(),
            connection_string: self.connection_string.clone(),
        }
    }
}

impl SqlxConnection {
    /// Create a new direct connection without using a pool
    pub async fn connect(connection_string: impl Into<String>) -> Result<Self, PostgresError> {
        let connection_string = connection_string.into();

        // Create a pool
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5) // Small pool for reuse
            .connect(&connection_string)
            .await
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        Ok(Self {
            pool: Arc::new(pool),
            connection_string,
        })
    }

    /// Get direct access to the underlying PostgreSQL connection
    /// This provides access to the underlying PgPool which implements the Executor trait
    pub fn pool_connection(&self) -> &PgPool {
        &self.pool
    }

    /// Get a client connection for executing SQL directly
    pub fn client(&self) -> &SqlxConnection {
        self
    }

    /// Execute a query and return the number of affected rows
    pub async fn execute(
        &self,
        query: &str,
        _params: &[&(dyn std::fmt::Debug + Sync)],
    ) -> Result<u64, PostgresError> {
        let result = sqlx::query(query)
            .execute(&*self.pool)
            .await
            .map_err(|e| PostgresError::QueryError(e.to_string()))?;
        Ok(result.rows_affected())
    }

    /// Execute a query and return the results
    pub async fn query(
        &self,
        query: &str,
        _params: &[&(dyn std::fmt::Debug + Sync)],
    ) -> Result<Vec<sqlx::postgres::PgRow>, PostgresError> {
        let result = sqlx::query(query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| PostgresError::QueryError(e.to_string()))?;
        Ok(result)
    }

    /// Execute a function with a direct connection and automatically close it after use
    /// This is the most efficient way to perform a one-off database operation
    pub async fn with_connection<F, R, E>(
        connection_string: impl Into<String>,
        operation: F,
    ) -> Result<R, PostgresError>
    where
        F: FnOnce(&mut SqlxConnection) -> futures::future::BoxFuture<'_, Result<R, E>>,
        E: std::error::Error + Send + Sync + 'static,
    {
        // Create a connection
        let mut conn = Self::connect(connection_string).await?;

        // Run the operation
        let result = operation(&mut conn)
            .await
            .map_err(|e| PostgresError::QueryError(e.to_string()))?;

        // Connection will be dropped automatically when it goes out of scope
        Ok(result)
    }
}

impl TestDatabaseConnection for SqlxConnection {
    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A connection pool for PostgreSQL using sqlx
#[derive(Clone)]
pub struct SqlxPool {
    pool: PgPool,
    connection_string: String,
}

#[async_trait]
impl DatabasePool for SqlxPool {
    type Connection = SqlxConnection;
    type Error = PostgresError;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        // Get a connection from the pool - we just need to check it works
        let _conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        // Create a SqlxConnection using the pool, without saving the connection directly
        // This approach avoids type mismatches and still allows for query execution
        Ok(SqlxConnection {
            pool: Arc::new(self.pool.clone()),
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
        // SQLx automatically returns connections to the pool when dropped
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A PostgreSQL database backend using sqlx
#[derive(Clone, Debug)]
pub struct SqlxPostgresBackend {
    config: DatabaseConfig,
}

#[async_trait]
impl DatabaseBackend for SqlxPostgresBackend {
    type Connection = SqlxConnection;
    type Pool = SqlxPool;
    type Error = PostgresError;

    async fn new(config: DatabaseConfig) -> Result<Self, Self::Error> {
        Ok(Self { config })
    }

    /// Create a pool around the given connection string
    async fn create_pool(
        &self,
        name: &DatabaseName,
        config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        let connection_string = self.connection_string(name);

        // Create a connection pool with the specified parameters
        let max_connections = config.max_connections.unwrap_or(5);

        let pool_options = PgPoolOptions::new().max_connections(max_connections as u32);

        let pool = pool_options
            .connect(&connection_string)
            .await
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        Ok(SqlxPool {
            pool,
            connection_string,
        })
    }

    /// Create a single connection to the given database
    /// This is useful for cases where a full pool is not needed
    async fn connect(&self, name: &DatabaseName) -> Result<Self::Connection, Self::Error> {
        let connection_string = self.connection_string(name);

        // Use the direct connection method we defined on SqlxConnection
        // This is more efficient as it avoids pool overhead for one-off connections
        SqlxConnection::connect(connection_string).await
    }

    /// Create a single connection using a connection string directly
    async fn connect_with_string(
        &self,
        connection_string: &str,
    ) -> Result<Self::Connection, Self::Error> {
        // Use the direct connection method we defined on SqlxConnection
        // This is more efficient as it avoids pool overhead for one-off connections
        SqlxConnection::connect(connection_string).await
    }

    async fn create_database(
        &self,
        _pool: &Self::Pool,
        name: &DatabaseName,
    ) -> Result<(), Self::Error> {
        // Parse the admin URL to extract connection parameters
        let _url = url::Url::parse(&self.config.admin_url)
            .map_err(|e| PostgresError::ConfigError(e.to_string()))?;

        // Connect to the default/admin database
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.config.admin_url)
            .await
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        // Create the database
        let db_name = name.as_str();
        let create_query = format!("CREATE DATABASE \"{}\"", db_name);

        // Execute the create database query
        query(&create_query)
            .execute(&admin_pool)
            .await
            .map_err(|e| PostgresError::DatabaseCreationError(e.to_string()))?;

        Ok(())
    }

    fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        // Parse the admin URL to extract connection parameters
        let url = match url::Url::parse(&self.config.admin_url) {
            Ok(url) => url,
            Err(e) => {
                tracing::error!("Failed to parse admin URL: {}", e);
                return Err(PostgresError::ConfigError(e.to_string()));
            }
        };

        let database_name = name.as_str();
        let test_user = url.username();

        // Format the connection string for the admin database
        let database_host = format!(
            "{}://{}:{}@{}:{}",
            url.scheme(),
            test_user,
            url.password().unwrap_or(""),
            url.host_str().unwrap_or("localhost"),
            url.port().unwrap_or(5432)
        );

        // First, terminate all connections to the database
        let output = Command::new("psql")
            .arg(&database_host)
            .arg("-c")
            .arg(format!("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}' AND pid <> pg_backend_pid();", database_name))
            .output();

        if let Err(e) = output {
            tracing::warn!(
                "Failed to terminate connections to database {}: {}",
                database_name,
                e
            );
            // Continue with drop attempt even if termination fails
        }

        // Now drop the database
        let output = Command::new("psql")
            .arg(&database_host)
            .arg("-c")
            .arg(format!("DROP DATABASE IF EXISTS \"{}\";", database_name))
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    tracing::info!("Successfully dropped database {}", name);
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::error!("Failed to drop database {}: {}", name, stderr);
                    Err(PostgresError::DatabaseDropError(stderr.to_string()))
                }
            }
            Err(e) => {
                tracing::error!("Failed to execute psql command to drop {}: {}", name, e);
                Err(PostgresError::DatabaseDropError(e.to_string()))
            }
        }
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        // Parse the URL
        let url = url::Url::parse(&self.config.admin_url).expect("Failed to parse admin URL");

        // Extract components
        let scheme = url.scheme();
        let username = url.username();
        let password = url.password().unwrap_or("");
        let host = url.host_str().unwrap_or("localhost");
        let port = url.port().unwrap_or(5432);

        // Format connection string with the given database name
        format!(
            "{}://{}:{}@{}:{}/{}",
            scheme,
            username,
            password,
            host,
            port,
            name.as_str()
        )
    }
}

/// A PostgreSQL transaction using sqlx
pub struct SqlxTransaction {
    transaction: Option<sqlx::Transaction<'static, sqlx::Postgres>>,
}

#[async_trait]
impl TransactionTrait for SqlxTransaction {
    type Error = PostgresError;

    async fn commit(&mut self) -> Result<(), Self::Error> {
        if let Some(tx) = self.transaction.take() {
            tx.commit()
                .await
                .map_err(|e| PostgresError::TransactionError(e.to_string()))
        } else {
            Err(PostgresError::TransactionError(
                "No transaction to commit".to_string(),
            ))
        }
    }

    async fn rollback(&mut self) -> Result<(), Self::Error> {
        if let Some(tx) = self.transaction.take() {
            tx.rollback()
                .await
                .map_err(|e| PostgresError::TransactionError(e.to_string()))
        } else {
            Err(PostgresError::TransactionError(
                "No transaction to rollback".to_string(),
            ))
        }
    }
}

/// Implementation of TransactionManager for PostgreSQL with sqlx
#[async_trait]
impl TransactionManager for TestDatabaseInstance<SqlxPostgresBackend> {
    type Error = PostgresError;
    type Tx = SqlxTransaction;
    type Connection = SqlxConnection;

    async fn begin_transaction(&mut self) -> Result<Self::Tx, Self::Error> {
        // We need to create a new connection and start a transaction directly
        let pool = &self.pool.pool;
        let tx: PgTransaction = pool
            .begin()
            .await
            .map_err(|e| PostgresError::TransactionError(e.to_string()))?;

        Ok(SqlxTransaction {
            transaction: Some(tx),
        })
    }

    async fn commit_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error> {
        tx.commit().await
    }

    async fn rollback_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error> {
        tx.rollback().await
    }
}

/// Create a new PostgreSQL backend using SQLx from the default configuration
pub async fn sqlx_postgres_backend() -> Result<SqlxPostgresBackend, PostgresError> {
    let config = DatabaseConfig::default();
    SqlxPostgresBackend::new(config).await
}

/// Create a new PostgreSQL backend using SQLx with the specified configuration
pub async fn sqlx_postgres_backend_with_config(
    config: DatabaseConfig,
) -> Result<SqlxPostgresBackend, PostgresError> {
    SqlxPostgresBackend::new(config).await
}
