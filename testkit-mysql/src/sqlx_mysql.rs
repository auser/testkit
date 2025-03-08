#![cfg(feature = "with-sqlx")]
use std::fmt::{Debug, Display};
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::mysql::{MySqlPoolOptions, MySqlQueryResult, MySqlRow};
use sqlx::pool::PoolConnection;
use sqlx::{MySql, MySqlPool as SqlxPool, Pool, query, query_as};
use url::Url;

use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
};

use crate::error::MySqlError;

/// A MySQL connection using SQLx
#[derive(Clone)]
pub struct SqlxMySqlConnection {
    /// The connection to the database
    conn: Arc<PoolConnection<MySql>>,
    /// The connection string used to create this connection
    connection_string: String,
}

impl SqlxMySqlConnection {
    /// Create a new connection to the database
    pub async fn connect(connection_string: String) -> Result<Self, MySqlError> {
        let pool = SqlxPool::connect(&connection_string)
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        let conn = pool
            .acquire()
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(conn),
            connection_string,
        })
    }

    /// Get a reference to the connection
    pub fn client(&self) -> &PoolConnection<MySql> {
        &self.conn
    }

    /// Execute a query on the connection
    pub async fn execute<Q: AsRef<str>>(
        &self,
        query: Q,
        params: &[sqlx::mysql::MySqlArguments],
    ) -> Result<MySqlQueryResult, MySqlError> {
        let query_str = query.as_ref();
        sqlx::query(query_str)
            .execute(&**self.conn)
            .await
            .map_err(|e| MySqlError::QueryExecutionError(e.to_string()))
    }

    /// Start a transaction
    pub async fn begin_transaction(&self) -> Result<sqlx::Transaction<'_, MySql>, MySqlError> {
        sqlx::Acquire::begin(&**self.conn)
            .await
            .map_err(|e| MySqlError::TransactionError(e.to_string()))
    }

    /// Get the connection string
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

/// A MySQL connection pool using SQLx
#[derive(Clone)]
pub struct SqlxMySqlPool {
    /// The connection pool
    pub pool: Arc<SqlxPool>,
    /// The connection string used to create this pool
    pub connection_string: String,
}

#[async_trait]
impl DatabasePool for SqlxMySqlPool {
    type Connection = SqlxMySqlConnection;
    type Error = MySqlError;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        let conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        Ok(SqlxMySqlConnection {
            conn: Arc::new(conn),
            connection_string: self.connection_string.clone(),
        })
    }
}

/// A MySQL backend using SQLx
#[derive(Clone, Debug)]
pub struct SqlxMySqlBackend {
    config: DatabaseConfig,
}

#[async_trait]
impl DatabaseBackend for SqlxMySqlBackend {
    type Connection = SqlxMySqlConnection;
    type Pool = SqlxMySqlPool;
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
        config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        // Create connection pool
        let connection_string = self.connection_string(name);
        let pool = MySqlPoolOptions::new()
            .max_connections(config.max_connections.unwrap_or(20) as u32)
            .connect(&connection_string)
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        Ok(SqlxMySqlPool {
            pool: Arc::new(pool),
            connection_string,
        })
    }

    /// Create a single connection to the given database
    async fn connect(&self, name: &DatabaseName) -> Result<Self::Connection, Self::Error> {
        let connection_string = self.connection_string(name);
        SqlxMySqlConnection::connect(connection_string).await
    }

    /// Create a single connection using a connection string directly
    async fn connect_with_string(
        &self,
        connection_string: &str,
    ) -> Result<Self::Connection, Self::Error> {
        SqlxMySqlConnection::connect(connection_string.to_string()).await
    }

    async fn create_database(
        &self,
        _pool: &Self::Pool,
        name: &DatabaseName,
    ) -> Result<(), Self::Error> {
        // Parse the admin URL to extract connection parameters
        let url = url::Url::parse(&self.config.admin_url)
            .map_err(|e| MySqlError::ConfigError(e.to_string()))?;

        // Connect to the default/admin database
        let admin_pool = MySqlPoolOptions::new()
            .max_connections(1)
            .connect(&self.config.admin_url)
            .await
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        // Create the database
        let db_name = name.as_str();
        let create_query = format!("CREATE DATABASE `{}`", db_name);

        // Execute the create database query
        query(&create_query)
            .execute(&admin_pool)
            .await
            .map_err(|e| MySqlError::DatabaseCreationError(e.to_string()))?;

        Ok(())
    }

    fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        // Create admin connection to drop the database
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

        rt.block_on(async {
            // Connect to the default/admin database
            let admin_pool = MySqlPoolOptions::new()
                .max_connections(1)
                .connect(&self.config.admin_url)
                .await
                .map_err(|e| MySqlError::ConnectionError(e.to_string()))?;

            // Drop the database
            let db_name = name.as_str();
            let drop_query = format!("DROP DATABASE IF EXISTS `{}`", db_name);

            query(&drop_query)
                .execute(&admin_pool)
                .await
                .map_err(|e| MySqlError::DatabaseDropError(e.to_string()))?;

            Ok(())
        })
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        // Parse the user URL
        let mut url = url::Url::parse(&self.config.user_url).expect("Invalid database URL");

        // Update the path to include the database name
        let db_name = name.as_str();
        let path_segments = url.path_segments_mut().expect("Cannot modify URL path");
        path_segments.clear().push(db_name);

        url.to_string()
    }
}

/// Helper function to create a MySQL backend with a configuration
pub async fn sqlx_mysql_backend_with_config(
    config: DatabaseConfig,
) -> Result<SqlxMySqlBackend, MySqlError> {
    SqlxMySqlBackend::new(config).await
}
