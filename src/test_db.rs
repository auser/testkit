use std::process::Command;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::Result,
    pool::PoolConfig,
    PoolError,
};
use url::Url;
use uuid::Uuid;

/// A test database that handles setup, connections, and cleanup
pub struct TestDatabase<B: DatabaseBackend + 'static> {
    /// The database backend
    pub backend: B,
    /// The connection pool
    pub pool: B::Pool,
    // /// Database name for cleanup
    // db_name: String,
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
            // db_name,
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

impl<B> Drop for TestDatabase<B>
where
    B: DatabaseBackend + Send + Sync + Clone + 'static,
{
    fn drop(&mut self) {
        println!("Dropping test database");
        let connection_string = self.pool.connection_string();

        println!("Dropping database: {}", connection_string);
        if let Err(e) = sync_drop_database(&connection_string) {
            tracing::error!("Failed to drop database: {:?}", e);
            println!("Failed to drop database: {:?}", e);
        }
        println!("Dropped database: {}", connection_string);
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
        println!("Skipping drop of system user: {}", role_name);
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
            println!("Skipping drop of current user: {}", role_name);
            return Ok(());
        }

        return Err(PoolError::DatabaseDropFailed(error));
    }
    Ok(())
}
