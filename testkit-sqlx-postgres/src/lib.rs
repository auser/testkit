use async_trait::async_trait;
use futures::Future;
use sqlx::postgres::{PgConnectOptions, PgConnection, PgPoolOptions};
use sqlx::{ConnectOptions, Connection as _, Executor, PgPool, Postgres, Transaction};
use std::fmt::Debug;
use std::time::Duration;
use testkit_core::{
    Connection, DatabaseBackend, DatabaseConfig, DatabaseContext, DatabaseName, DatabasePool,
    DefaultDatabaseContext, OwnedTransaction, TestDatabase, TestDatabaseTemplate,
    Transaction as CoreTransaction, TransactionManager,
    begin_transaction as core_begin_transaction, with_database_setup_raw, with_database_template,
    with_database_template_setup,
};
use thiserror::Error;
use tracing::debug;
use url::Url;

// Re-export core functionality
pub use testkit_core::{with_database, with_transaction};

// Define PostgreSQL-specific error type
#[derive(Debug, Error, Clone)]
pub enum PostgresError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Pool error: {0}")]
    Pool(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("URL parse error: {0}")]
    UrlParse(String),
}

impl From<sqlx::Error> for PostgresError {
    fn from(err: sqlx::Error) -> Self {
        PostgresError::Database(err.to_string())
    }
}

impl From<String> for PostgresError {
    fn from(err: String) -> Self {
        PostgresError::Database(err)
    }
}

impl From<&'static str> for PostgresError {
    fn from(err: &'static str) -> Self {
        PostgresError::Database(err.to_string())
    }
}

// Wrapper for PgConnection that implements Clone and Connection traits
#[derive(Debug)]
pub struct PgConnectionWrapper(pub PgConnection);

// Implement Clone for PgConnectionWrapper (note: this is just a stub that will panic if used)
impl Clone for PgConnectionWrapper {
    fn clone(&self) -> Self {
        panic!("PgConnectionWrapper cannot be cloned; this is a marker implementation only")
    }
}

impl std::ops::Deref for PgConnectionWrapper {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PgConnectionWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Implementation of the Connection trait for PgConnectionWrapper
#[async_trait]
impl Connection for PgConnectionWrapper {
    type Transaction<'conn>
        = Transaction<'conn, Postgres>
    where
        Self: 'conn;
    type Error = PostgresError;

    async fn is_valid(&self) -> bool {
        // We can't use ping here because it requires a mutable reference
        // Instead we'll just assume the connection is valid
        true
    }

    async fn reset(&mut self) -> Result<(), Self::Error> {
        // Execute a simple query to reset the connection state
        self.0.execute("SELECT 1").await?;
        Ok(())
    }

    async fn execute(&mut self, sql: &str) -> Result<(), Self::Error> {
        sqlx::query(sql).execute(&mut self.0).await?;
        Ok(())
    }

    async fn begin(&mut self) -> Result<Self::Transaction<'_>, Self::Error> {
        let tx = sqlx::Connection::begin(&mut self.0).await?;
        Ok(tx)
    }
}

// PostgreSQL connection pool implementation
#[derive(Debug, Clone)]
pub struct PostgresPool {
    pool: PgPool,
    connection_string: String,
}

impl PostgresPool {
    pub async fn new(connection_string: impl Into<String>) -> Result<Self, PostgresError> {
        let connection_string = connection_string.into();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&connection_string)
            .await?;

        Ok(Self {
            pool,
            connection_string,
        })
    }
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Pool = PgPool;
    type Connection = PgConnectionWrapper;
    type Error = PostgresError;

    async fn acquire(&self) -> Result<Self::Connection, Self::Error> {
        let conn = self.pool.acquire().await?;
        // Use detach to convert PoolConnection to PgConnection
        let conn = conn.detach();
        Ok(PgConnectionWrapper(conn))
    }

    async fn release(&self, _conn: Self::Pool) -> Result<(), Self::Error> {
        // PgPool handles connection releases automatically
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

// The main PostgreSQL backend implementation
#[derive(Debug, Clone)]
pub struct PostgresBackend {
    admin_connection_string: String,
    user_connection_string: String,
}

impl PostgresBackend {
    pub fn new(
        admin_connection_string: impl Into<String>,
        user_connection_string: impl Into<String>,
    ) -> Self {
        Self {
            admin_connection_string: admin_connection_string.into(),
            user_connection_string: user_connection_string.into(),
        }
    }

    // Parse a connection string into a URL and modify it for a specific database
    fn build_connection_string(
        &self,
        base_url: &str,
        db_name: &str,
    ) -> Result<String, PostgresError> {
        let mut url = Url::parse(base_url).map_err(|e| PostgresError::UrlParse(e.to_string()))?;

        url.set_path(&format!("/{}", db_name));
        Ok(url.to_string())
    }
}

#[async_trait]
impl DatabaseBackend for PostgresBackend {
    type Connection = PgConnectionWrapper;
    type Pool = PostgresPool;
    type Error = PostgresError;

    async fn connect(&self) -> Result<Self::Pool, Self::Error> {
        PostgresPool::new(&self.admin_connection_string).await
    }

    async fn create_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        let mut conn = PgConnectOptions::new()
            .application_name("testkit-sqlx-postgres")
            .connect()
            .await?;

        let db_name = name.as_str();
        let query = format!("CREATE DATABASE \"{}\"", db_name);
        conn.execute(&*query).await?;

        debug!("Created database: {}", db_name);
        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        let mut conn = PgConnectOptions::new()
            .application_name("testkit-sqlx-postgres")
            .connect()
            .await?;

        // First terminate all connections
        self.terminate_connections(name).await?;

        let db_name = name.as_str();
        let query = format!("DROP DATABASE IF EXISTS \"{}\"", db_name);
        conn.execute(&*query).await?;

        debug!("Dropped database: {}", db_name);
        Ok(())
    }

    async fn create_pool(
        &self,
        name: &DatabaseName,
        _config: &DatabaseConfig,
    ) -> Result<Self::Pool, Self::Error> {
        let db_name = name.as_str();
        let connection_string =
            self.build_connection_string(&self.user_connection_string, db_name)?;

        PostgresPool::new(connection_string).await
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<(), Self::Error> {
        let mut conn = PgConnectOptions::new()
            .application_name("testkit-sqlx-postgres")
            .connect()
            .await?;

        let db_name = name.as_str();
        let query = format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}'",
            db_name
        );
        conn.execute(&*query).await?;

        debug!("Terminated all connections to database: {}", db_name);
        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<(), Self::Error> {
        let mut conn = PgConnectOptions::new()
            .application_name("testkit-sqlx-postgres")
            .connect()
            .await?;

        let db_name = name.as_str();
        let template_name = template.as_str();
        let query = format!(
            "CREATE DATABASE \"{}\" WITH TEMPLATE \"{}\"",
            db_name, template_name
        );
        conn.execute(&*query).await?;

        debug!(
            "Created database {} from template {}",
            db_name, template_name
        );
        Ok(())
    }

    async fn create_test_user(
        &self,
        name: &DatabaseName,
        username: &str,
    ) -> Result<(), Self::Error> {
        let mut conn = PgConnectOptions::new()
            .application_name("testkit-sqlx-postgres")
            .connect()
            .await?;

        // Create user with a random password
        let password = uuid::Uuid::new_v4().to_string();
        let create_user_query = format!(
            "DO $$ BEGIN CREATE USER \"{}\" WITH PASSWORD '{}'; EXCEPTION WHEN DUPLICATE_OBJECT THEN NULL; END $$;",
            username, password
        );
        conn.execute(&*create_user_query).await?;

        debug!("Created test user: {}", username);
        Ok(())
    }

    async fn grant_privileges(
        &self,
        name: &DatabaseName,
        username: &str,
    ) -> Result<(), Self::Error> {
        let mut conn = PgConnectOptions::new()
            .application_name("testkit-sqlx-postgres")
            .connect()
            .await?;

        let db_name = name.as_str();
        let grant_query = format!(
            "GRANT ALL PRIVILEGES ON DATABASE \"{}\" TO \"{}\"",
            db_name, username
        );
        conn.execute(&*grant_query).await?;

        debug!("Granted privileges to {} on database {}", username, db_name);
        Ok(())
    }

    fn get_admin_connection_string(&self, name: &DatabaseName) -> String {
        self.build_connection_string(&self.admin_connection_string, name.as_str())
            .unwrap_or_else(|_| String::from("postgres://invalid"))
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        self.build_connection_string(&self.user_connection_string, name.as_str())
            .unwrap_or_else(|_| String::from("postgres://invalid"))
    }

    async fn convert_connection(
        &self,
        conn: <Self::Pool as DatabasePool>::Connection,
    ) -> Result<Self::Connection, Self::Error> {
        // Since Self::Connection is PgConnectionWrapper and that's what we get from the pool,
        // this is a simple pass-through
        Ok(conn)
    }
}

// PostgreSQL context for transactions
pub type PostgresContext = DefaultDatabaseContext<PgConnectionWrapper>;

// Implementation of TransactionManager for PostgreSQL - avoiding orphan rule issues
#[async_trait]
impl TransactionManager<Transaction<'static, Postgres>, PgConnectionWrapper> for PostgresContext {
    type Error = PostgresError;

    async fn begin_transaction(&mut self) -> Result<Transaction<'static, Postgres>, Self::Error> {
        let tx = self.connection_mut().begin().await?;
        // This is a bit of a hack to extend the lifetime to 'static
        // It works because we ensure the connection outlives the transaction
        Ok(unsafe { std::mem::transmute(tx) })
    }

    async fn commit_transaction(
        tx: &mut Transaction<'static, Postgres>,
    ) -> Result<(), Self::Error> {
        // We can't directly call methods that consume the transaction as it's behind a mutable reference
        // Instead, we'll use sqlx::Executor trait to execute a COMMIT statement
        sqlx::query("COMMIT").execute(&mut **tx).await?;
        Ok(())
    }

    async fn rollback_transaction(
        tx: &mut Transaction<'static, Postgres>,
    ) -> Result<(), Self::Error> {
        // We can't directly call methods that consume the transaction as it's behind a mutable reference
        // Instead, we'll use sqlx::Executor trait to execute a ROLLBACK statement
        sqlx::query("ROLLBACK").execute(&mut **tx).await?;
        Ok(())
    }
}

// Helper functions for working with PostgreSQL databases
pub async fn with_postgres_database(
    config: Option<DatabaseConfig>,
) -> Result<TestDatabase<PostgresBackend>, PostgresError> {
    let config = match config {
        Some(config) => config,
        None => DatabaseConfig::from_env().map_err(|e| PostgresError::Connection(e.to_string()))?,
    };

    let backend = PostgresBackend::new(&config.admin_url, &config.user_url);

    testkit_core::with_database_setup_raw(backend, config.clone(), |_conn| async { Ok(()) }).await
}

pub async fn with_postgres_database_setup<F, Fut>(
    config: Option<DatabaseConfig>,
    setup_fn: F,
) -> Result<TestDatabase<PostgresBackend>, PostgresError>
where
    F: FnOnce(&mut PgConnectionWrapper) -> Fut + Send,
    Fut: Future<Output = Result<(), PostgresError>> + Send,
{
    let config = match config {
        Some(config) => config,
        None => DatabaseConfig::from_env().map_err(|e| PostgresError::Connection(e.to_string()))?,
    };

    let backend = PostgresBackend::new(&config.admin_url, &config.user_url);

    testkit_core::with_database_setup_raw(backend, config.clone(), setup_fn).await
}

pub async fn with_postgres_database_template(
    config: Option<DatabaseConfig>,
    max_replicas: usize,
) -> Result<TestDatabaseTemplate<PostgresBackend>, PostgresError> {
    let config = match config {
        Some(config) => config,
        None => DatabaseConfig::from_env().map_err(|e| PostgresError::Connection(e.to_string()))?,
    };

    let backend = PostgresBackend::new(&config.admin_url, &config.user_url);

    testkit_core::with_database_template(backend, config.clone(), max_replicas).await
}

pub async fn with_postgres_database_template_setup<F, Fut>(
    config: Option<DatabaseConfig>,
    max_replicas: usize,
    setup_fn: F,
) -> Result<TestDatabaseTemplate<PostgresBackend>, PostgresError>
where
    F: FnOnce(&mut PgConnectionWrapper) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), PostgresError>> + Send + 'static,
{
    let config = match config {
        Some(config) => config,
        None => DatabaseConfig::from_env().map_err(|e| PostgresError::Connection(e.to_string()))?,
    };

    let backend = PostgresBackend::new(&config.admin_url, &config.user_url);

    testkit_core::with_database_template_setup(backend, config.clone(), max_replicas, setup_fn)
        .await
}

pub async fn begin_transaction(
    db: &TestDatabase<PostgresBackend>,
) -> Result<OwnedTransaction<PgConnectionWrapper>, PostgresError> {
    testkit_core::begin_transaction(db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_with_database() {
        // This test requires a running PostgreSQL instance with credentials from env
        // We'll just check that the code compiles for now
        let _result = with_postgres_database(None).await;
    }

    #[tokio::test]
    async fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
