use std::path::PathBuf;

use async_trait::async_trait;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    template::DatabaseName,
};

pub struct SqliteConnection {
    pub pool: Pool<Sqlite>,
}

#[async_trait]
impl Connection for SqliteConnection {
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
    type Pool = SqlitePool;

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let db_path = self.get_db_path(name);
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

    async fn create_pool(&self, name: &DatabaseName, _config: &PoolConfig) -> Result<Self::Pool> {
        let db_path = self.get_db_path(name);
        let pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
            .await
            .map_err(|e| PoolError::PoolCreationFailed(e.to_string()))?;
        Ok(pool)
    }
}
