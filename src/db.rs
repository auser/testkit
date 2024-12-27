use deadpool_postgres::Pool;
use once_cell::sync::Lazy;
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use std::{
    env,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::Duration,
};
use wait_timeout::ChildExt;

use crate::error::{TestkitError, TestkitResult};

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "postgres")]
pub use postgres::connect_to_db;
#[cfg(feature = "postgres")]
pub(crate) use postgres::*;

#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "mysql")]
pub use mysql::*;

static ATOMIC_COUNTER: Lazy<Mutex<AtomicUsize>> = Lazy::new(|| Mutex::new(AtomicUsize::new(1)));
const TIMEOUT: u64 = 5;

#[async_trait::async_trait]
pub trait TestDatabaseTrait: Send + Sync {}

#[derive(Debug, Clone)]
pub struct TestPoolOptions {
    pub user: Option<String>,
    pub password: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub database_name: Option<String>,
    pub tracing: bool,
}

impl TestPoolOptions {
    pub fn new() -> Self {
        Self {
            user: Some("postgres".to_string()),
            password: Some("postgres".to_string()),
            host: Some("postgres".to_string()),
            port: Some(5432),
            database_name: None,
            tracing: false,
        }
    }

    pub fn to_uri(&self) -> String {
        let user = self.user.clone().unwrap_or("postgres".to_string());
        generate_test_db_uri(
            self,
            &self
                .database_name
                .clone()
                .unwrap_or(generate_database_name()),
            &user,
        )
    }
}

pub struct TestPoolOptionsBuilder {
    opts: TestPoolOptions,
}

impl TestPoolOptionsBuilder {
    pub fn new() -> Self {
        Self {
            opts: TestPoolOptions::new(),
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.opts.user = Some(user.into());
        self
    }

    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.opts.password = Some(password.into());
        self
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.opts.host = Some(host.into());
        self
    }

    pub fn with_port(mut self, port: i32) -> Self {
        self.opts.port = Some(port);
        self
    }

    pub fn with_database_name(mut self, database_name: impl Into<String>) -> Self {
        self.opts.database_name = Some(database_name.into());
        self
    }

    pub fn with_tracing(mut self, tracing: bool) -> Self {
        self.opts.tracing = tracing;
        self
    }

    pub fn build(self) -> TestPoolOptions {
        self.opts
    }
}

#[derive(Debug, Clone)]
pub struct TestDatabase {
    pub uri: String,
    #[cfg(feature = "postgres")]
    pub postgres_pool: deadpool_postgres::Pool,
    #[cfg(feature = "mysql")]
    pub mysql_pool: mysql_async::Pool,
    #[cfg(feature = "postgres")]
    pub test_pool: deadpool_postgres::Pool,
    #[cfg(feature = "mysql")]
    pub test_pool: mysql_async::Pool,
    pub test_user: String, // Add this field to store the test user name
}

impl TestDatabase {
    #[cfg(feature = "postgres")]
    pub async fn new(opts: Option<TestPoolOptions>) -> TestkitResult<Self> {
        let opts = opts.unwrap_or_default();
        let (uri, postgres_pool, test_pool, test_user) = create_test_pool(Some(opts)).await?;
        Ok(Self {
            uri,
            postgres_pool,
            test_pool,
            test_user,
        })
    }

    #[cfg(feature = "mysql")]
    pub async fn new(opts: Option<TestPoolOptions>) -> TestkitResult<Self> {
        let opts = opts.unwrap_or_default();
        let (uri, postgres_pool, test_pool, test_user) = create_test_pool(Some(opts)).await?;
        Ok(Self {
            uri,
            postgres_pool,
            test_pool,
            test_user,
        })
    }

    pub async fn setup<F, Fut, T>(&self, setup_fn: F) -> TestkitResult<T>
    where
        F: FnOnce(deadpool_postgres::Client) -> Fut + Send + Sync,
        Fut: Future<Output = TestkitResult<T>> + Send,
        T: TestDatabaseTrait + Send + 'static,
    {
        let client = self.postgres_pool.get().await?;

        // Generate the postgres superuser URI
        let superuser_uri = get_database_host_from_uri(&self.uri)?;

        // Grant permissions to the test user
        grant_permissions(&superuser_uri, &self.test_user).await?;

        setup_fn(client).await
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if let Err(e) = sync_drop_database(&self.uri) {
            tracing::error!("Failed to drop database: {:?}", e);
        }
    }
}

pub async fn create_test_pool(
    opts: Option<TestPoolOptions>,
) -> TestkitResult<(String, Pool, Pool, String)> {
    let opts = opts.unwrap_or_default();
    if opts.tracing {
        let _ = env_logger::try_init();
    }
    let database_name = generate_database_name();

    // Generate URIs for postgres and test user
    let postgres_uri = generate_test_db_uri(
        &opts,
        &database_name,
        &opts.user.clone().unwrap_or("postgres".to_string()),
    );
    tracing::info!("Attempting to create test pool with URI: {}", postgres_uri);

    // Create postgres pool
    let postgres_pool = create_connection_pool(&postgres_uri)?;

    // Always try to drop the database first, in case it wasn't cleaned up properly
    if let Err(e) = sync_drop_database(&postgres_uri) {
        tracing::warn!("Failed to drop existing database: {:?}", e);
    }

    // Create database
    match create_database(&postgres_uri).await {
        Ok(_) => tracing::info!("Database created successfully"),
        Err(e) => {
            tracing::error!("Failed to create database: {:?}", e);
            return Err(e);
        }
    }

    // Create test user
    let test_user = match create_test_user(&postgres_uri).await {
        Ok(user) => {
            tracing::info!("Test user '{}' created or already exists", user);
            user
        }
        Err(e) => {
            tracing::error!("Failed to create test user: {:?}", e);
            return Err(e);
        }
    };

    // Run migrations
    match run_migrations(&postgres_uri).await {
        Ok(_) => tracing::info!("Migrations run successfully"),
        Err(e) => {
            tracing::error!("Failed to run migrations: {:?}", e);
            return Err(e);
        }
    }

    // Grant permissions to test user
    match grant_permissions(&postgres_uri, &test_user).await {
        Ok(_) => tracing::info!("Permissions granted successfully"),
        Err(e) => {
            tracing::error!("Failed to grant permissions: {:?}", e);
            return Err(e);
        }
    }

    // Create URI and pool for test user
    let test_db_uri = generate_test_db_uri(&opts, &database_name, &test_user);
    let test_pool = match create_connection_pool(&test_db_uri) {
        Ok(pool) => {
            tracing::info!("Test user connection pool created successfully");
            pool
        }
        Err(e) => {
            tracing::error!("Failed to create test user connection pool: {:?}", e);
            return Err(e);
        }
    };

    Ok((test_db_uri, postgres_pool, test_pool, test_user))
}

fn generate_database_name() -> String {
    let count = get_next_count();
    format!("test_testkit_{}", count)
}

// fn generate_database_uri(opts: &TestPoolOptions, database_name: &str) -> String {
//     let port = opts.port.unwrap_or(5432);
//     let host = opts.host.as_deref().unwrap_or("localhost");
//     let user = opts.user.as_deref().unwrap_or("postgres");
//     let password = opts.password.as_deref().unwrap_or("testpassword");

//     format!("postgresql://{user}:{password}@{host}:{port}/{database_name}")
// }

async fn create_database(database_uri: &str) -> TestkitResult<()> {
    let mut child = dbmate_command(Command::new("dbmate"), database_uri)
        .arg("create")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let _stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let status_code = match child.wait_timeout(Duration::from_secs(TIMEOUT))? {
        Some(status) => status.code(),
        None => {
            child.kill()?;
            child.wait()?.code()
        }
    };

    if status_code != Some(0) {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = line?;
            if line.contains("already exists") {
                return Err(TestkitError::DatabaseAlreadyExists(line));
            }
            eprintln!("{}", line);
        }
        return Err(TestkitError::DatabaseCreationFailed);
    }

    Ok(())
}

pub async fn run_migrations(database_uri: &str) -> TestkitResult<()> {
    let output = dbmate_command(Command::new("dbmate"), database_uri)
        .arg("up")
        .output()?;

    if !output.status.success() {
        return Err(TestkitError::MigrationFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(())
}

pub async fn drop_database(database_uri: &str) -> TestkitResult<()> {
    sync_drop_database(database_uri)
}

pub(crate) fn dbmate_command(mut cmd: Command, database_uri: &str) -> Command {
    let migration_dir = get_migrations_dir().expect("unable to get migrations directory");
    cmd.arg("--url")
        .arg(database_uri)
        .arg("--no-dump-schema")
        .arg("--migrations-dir")
        .arg(migration_dir.as_path().to_str().unwrap());
    cmd
}

pub(crate) fn get_next_count() -> usize {
    let counter = ATOMIC_COUNTER.lock().unwrap();
    counter.fetch_add(1, Ordering::SeqCst)
}

pub(crate) fn get_migrations_dir() -> TestkitResult<PathBuf> {
    let migration_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join(".")
        .join("migrations");
    Ok(migration_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_pool() -> TestkitResult<()> {
        // Test successful pool creation
        let pool = create_test_pool(None).await;
        assert!(pool.is_ok());

        // Verify pool is valid by executing a simple query
        let (_conn_str, _postgres_pool, test_pool, _test_user) = pool.unwrap();
        let conn = test_pool.get().await?;
        let row = conn.query_one("SELECT 1", &[]).await?;
        let value: i32 = row.get(0);
        assert_eq!(value, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_test_pool_unique_databases() -> TestkitResult<()> {
        // Create two pools and verify they use different databases
        let pool1 = create_test_pool(None).await;
        let pool2 = create_test_pool(None).await;

        assert!(pool1.is_ok());
        assert!(pool2.is_ok());

        let (_conn_str1, _postgres_pool1, test_pool1, _test_user1) = pool1.unwrap();
        let (_conn_str2, _postgres_pool2, test_pool2, _test_user2) = pool2.unwrap();

        let db1 = test_pool1.get().await?;
        let db2 = test_pool2.get().await?;

        // Get database names
        let query = "SELECT current_database()";
        let row1 = db1.query_one(query, &[]).await?;
        let name1: String = row1.get(0);
        let row2 = db2.query_one(query, &[]).await?;
        let name2: String = row2.get(0);

        assert_ne!(name1, name2);

        Ok(())
    }
}
