use std::path::PathBuf;

use async_trait::async_trait;
use sqlx::{Pool, Sqlite, Transaction};

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    test_db::DatabaseName,
};

pub struct SqliteConnection {
    pub pool: Pool<Sqlite>,
}

#[async_trait]
impl Connection for SqliteConnection {
    type Transaction<'conn> = Transaction<'conn, Sqlite>;

    async fn is_valid(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }

    async fn reset(&mut self) -> Result<()> {
        // SQLite doesn't need resetting
        Ok(())
    }

    async fn execute(&mut self, sql: &str) -> Result<()> {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn begin(&mut self) -> Result<Self::Transaction<'_>> {
        self.pool
            .begin()
            .await
            .map_err(|e| PoolError::TransactionError(e.to_string()))
    }
}

#[derive(Clone)]
pub struct SqliteBackend {
    base_path: PathBuf,
}

impl SqliteBackend {
    pub async fn new(base_path: &str) -> Result<Self> {
        Ok(Self {
            base_path: PathBuf::from(base_path),
        })
    }

    fn get_db_path(&self, name: &DatabaseName) -> PathBuf {
        self.base_path.join(format!("{}.db", name))
    }
}

#[async_trait]
impl DatabaseBackend for SqliteBackend {
    type Connection = SqliteConnection;
    type Pool = SqliteDbPool;

    async fn connect(&self) -> Result<Self::Pool> {
        // For SQLite, we connect to a temporary in-memory database as the default
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5u32)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| PoolError::PoolCreationFailed(e.to_string()))?;

        Ok(SqliteDbPool {
            pool,
            url: "sqlite::memory:".to_string(),
        })
    }

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let db_path = self.get_db_path(name);

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                PoolError::DatabaseError(format!("Failed to create directory: {}", e))
            })?;
        }

        if db_path.exists() {
            std::fs::remove_file(&db_path).map_err(|e| {
                PoolError::DatabaseError(format!("Failed to remove database: {}", e))
            })?;
        }

        // Create empty database file
        std::fs::File::create(&db_path)
            .map_err(|e| PoolError::DatabaseError(format!("Failed to create database: {}", e)))?;
        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        let template_path = self.get_db_path(template);
        let db_path = self.get_db_path(name);

        std::fs::copy(&template_path, &db_path)
            .map_err(|e| PoolError::DatabaseError(format!("Failed to copy database: {}", e)))?;
        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        let db_path = self.get_db_path(name);
        if db_path.exists() {
            std::fs::remove_file(&db_path).map_err(|e| {
                PoolError::DatabaseError(format!("Failed to remove database: {}", e))
            })?;
        }
        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, config: &PoolConfig) -> Result<Self::Pool> {
        let db_path = self.get_db_path(name);
        let url = format!("sqlite:{}", db_path.display());
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(config.max_size as u32)
            .connect(&url)
            .await
            .map_err(|e| PoolError::PoolCreationFailed(e.to_string()))?;

        Ok(SqliteDbPool { pool, url })
    }

    async fn terminate_connections(&self, _name: &DatabaseName) -> Result<()> {
        // SQLite doesn't need to terminate connections explicitly
        // as they are typically file-based and local
        Ok(())
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        format!("sqlite:{}", self.get_db_path(name).display())
    }
}

#[derive(Clone)]
pub struct SqliteDbPool {
    pool: Pool<Sqlite>,
    url: String,
}

impl SqliteDbPool {
    /// Get the underlying SQLx pool for direct SQLx operations
    pub fn sqlx_pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

#[async_trait]
impl DatabasePool for SqliteDbPool {
    type Connection = SqliteConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        Ok(SqliteConnection {
            pool: self.pool.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connection is automatically returned to the pool when dropped
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.url.clone()
    }
}
