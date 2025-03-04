use crate::PostgresError;
use async_trait::async_trait;
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;
use testkit_core::{
    DatabaseBackend, DatabaseConfig, DatabaseName, DatabasePool, TestDatabaseConnection,
    TestDatabaseInstance,
};
use url::Url;

/// A connection to a PostgreSQL database using tokio-postgres
#[derive(Clone)]
pub struct PostgresConnection {
    client: Arc<deadpool_postgres::Client>,
    connection_string: String,
}

impl PostgresConnection {
    /// Get a reference to the underlying database client
    pub fn client(&self) -> &deadpool_postgres::Client {
        &self.client
    }
}

impl TestDatabaseConnection for PostgresConnection {
    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A connection pool for PostgreSQL using deadpool-postgres
#[derive(Clone)]
pub struct PostgresPool {
    pool: Arc<deadpool_postgres::Pool>,
    connection_string: String,
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Connection = PostgresConnection;
    type Error = PostgresError;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        // Get a connection from the pool
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        // Return a new PostgresConnection
        Ok(PostgresConnection {
            client: Arc::new(client),
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<(), Self::Error> {
        // The deadpool automatically handles connection release when the client is dropped
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

/// A PostgreSQL database backend using tokio-postgres
#[derive(Clone, Debug)]
pub struct PostgresBackend {
    config: DatabaseConfig,
}

#[async_trait]
impl DatabaseBackend for PostgresBackend {
    type Connection = PostgresConnection;
    type Pool = PostgresPool;
    type Error = PostgresError;

    async fn new(config: DatabaseConfig) -> Result<Self, Self::Error> {
        // Validate the config
        if config.admin_url.is_empty() || config.user_url.is_empty() {
            return Err(PostgresError::ConfigError(
                "Admin and user URLs must be provided".into(),
            ));
        }

        Ok(Self { config })
    }

    async fn create_pool(
        &self,
        name: &DatabaseName,
        _config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        // Create connection config from the URL
        let connection_string = self.connection_string(name);
        let pg_config = tokio_postgres::config::Config::from_str(&connection_string)
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        // Create deadpool manager
        let mgr_config = deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        };
        let mgr =
            deadpool_postgres::Manager::from_config(pg_config, tokio_postgres::NoTls, mgr_config);

        // Create the pool
        let pool = deadpool_postgres::Pool::builder(mgr)
            .max_size(20)
            .build()
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        Ok(PostgresPool {
            pool: Arc::new(pool),
            connection_string,
        })
    }

    async fn create_database(
        &self,
        _pool: &Self::Pool,
        name: &DatabaseName,
    ) -> Result<(), Self::Error> {
        // Create admin connection to create the database
        let _admin_config = tokio_postgres::config::Config::from_str(&self.config.admin_url)
            .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        let (client, connection) =
            tokio_postgres::connect(&self.config.admin_url, tokio_postgres::NoTls)
                .await
                .map_err(|e| PostgresError::ConnectionError(e.to_string()))?;

        // Spawn the connection handler
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        // Create the database
        let db_name = name.as_str();
        let create_query = format!("CREATE DATABASE \"{}\"", db_name);

        client
            .execute(&create_query, &[])
            .await
            .map_err(|e| PostgresError::DatabaseCreationError(e.to_string()))?;

        Ok(())
    }

    fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        // Parse the admin URL to extract connection parameters
        let url = match Url::parse(&self.config.admin_url) {
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
        let output = std::process::Command::new("psql")
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
        let output = std::process::Command::new("psql")
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
        // Parse the base URL and replace the database name
        let base_url = &self.config.user_url;

        // Simple string replacement to change the database name
        if let Some(db_pos) = base_url.rfind('/') {
            let (prefix, _) = base_url.split_at(db_pos + 1);
            return format!("{}{}", prefix, name.as_str());
        }

        // Fallback
        format!("postgres://localhost/{}", name.as_str())
    }
}

/// A PostgreSQL transaction using tokio-postgres
pub struct PostgresTransaction {
    client: Arc<deadpool_postgres::Client>,
}

#[async_trait]
impl TransactionTrait for PostgresTransaction {
    type Error = PostgresError;

    async fn commit(&mut self) -> Result<(), Self::Error> {
        self.client
            .execute("COMMIT", &[])
            .await
            .map_err(|e| PostgresError::TransactionError(e.to_string()))?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), Self::Error> {
        self.client
            .execute("ROLLBACK", &[])
            .await
            .map_err(|e| PostgresError::TransactionError(e.to_string()))?;
        Ok(())
    }
}

/// Implementation of TransactionManager for PostgreSQL
#[async_trait]
impl TransactionManager for TestDatabaseInstance<PostgresBackend> {
    type Error = PostgresError;
    type Tx = PostgresTransaction;
    type Connection = PostgresConnection;

    async fn begin_transaction(&mut self) -> Result<Self::Tx, Self::Error> {
        // Get a connection from the pool
        let pool = &self.pool;
        let client = pool.acquire().await?;

        // Begin transaction
        client
            .client
            .execute("BEGIN", &[])
            .await
            .map_err(|e| PostgresError::TransactionError(e.to_string()))?;

        Ok(PostgresTransaction {
            client: Arc::clone(&client.client),
        })
    }

    async fn commit_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error> {
        tx.commit().await
    }

    async fn rollback_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error> {
        tx.rollback().await
    }
}

// Define the transaction traits locally to avoid private module issues
#[async_trait]
pub trait TransactionTrait: Send + Sync {
    type Error: Send + Sync;
    async fn commit(&mut self) -> Result<(), Self::Error>;
    async fn rollback(&mut self) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait TransactionManager: Send + Sync {
    type Error: Send + Sync;
    type Tx: TransactionTrait<Error = Self::Error> + Send + Sync;
    type Connection: Send + Sync;

    async fn begin_transaction(&mut self) -> Result<Self::Tx, Self::Error>;
    async fn commit_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error>;
    async fn rollback_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error>;
}

/// Create a new PostgreSQL backend from environment variables
///
/// This function can be used to create a backend that can be passed into `with_database()`
///
/// # Example
/// ```no_run
/// use testkit_postgres::postgres_backend;
/// use testkit_core::with_database;
///
/// async fn test() {
///     let backend = postgres_backend().await.unwrap();
///     let context = with_database(backend)
///         .execute()
///         .await
///         .unwrap();
/// }
/// ```
pub async fn postgres_backend() -> Result<PostgresBackend, PostgresError> {
    let config = DatabaseConfig::default();
    PostgresBackend::new(config).await
}

/// Create a new PostgreSQL backend with a custom config
///
/// This function can be used to create a backend that can be passed into `with_database()`
///
/// # Example
/// ```no_run
/// use testkit_postgres::{postgres_backend_with_config, DatabaseConfig};
/// use testkit_core::with_database;
///
/// async fn test() {
///     let config = DatabaseConfig::new("postgres://admin@localhost/postgres", "postgres://user@localhost/postgres");
///     let backend = postgres_backend_with_config(config).await.unwrap();
///     let context = with_database(backend)
///         .execute()
///         .await
///         .unwrap();
/// }
/// ```
pub async fn postgres_backend_with_config(
    config: DatabaseConfig,
) -> Result<PostgresBackend, PostgresError> {
    PostgresBackend::new(config).await
}
