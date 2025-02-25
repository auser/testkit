use std::str::FromStr;

use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Transaction};
use tokio_postgres::{Config, NoTls};
use url::Url;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    template::DatabaseName,
};

pub struct SqlxPostgresConnection {
    pub(crate) pool: PgPool,
    connection_string: String,
}

#[async_trait]
impl Connection for SqlxPostgresConnection {
    type Transaction<'conn> = Transaction<'conn, Postgres>;

    async fn is_valid(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }

    async fn reset(&mut self) -> Result<()> {
        sqlx::query("DISCARD ALL")
            .execute(&self.pool)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
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

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

impl SqlxPostgresConnection {
    /// Get a reference to the underlying SQLx PgPool
    ///
    /// This allows direct use of SQLx queries with this connection
    pub fn sqlx_pool(&self) -> &PgPool {
        &self.pool
    }
}

#[derive(Debug, Clone)]
pub struct SqlxPostgresBackend {
    config: Config,
    url: Url,
}

impl SqlxPostgresBackend {
    pub fn new(connection_string: &str) -> Result<Self> {
        let config = Config::from_str(connection_string)
            .map_err(|e| PoolError::ConfigError(format!("Invalid connection string: {}", e)))?;
        let url = Url::parse(connection_string)
            .map_err(|e| PoolError::ConfigError(format!("Invalid connection string: {}", e)))?;

        Ok(Self { config, url })
    }

    fn get_database_url(&self, name: &DatabaseName) -> String {
        let mut url = self.url.clone();
        url.set_path(name.as_str());
        url.to_string()
    }
}

#[async_trait]
impl DatabaseBackend for SqlxPostgresBackend {
    type Connection = SqlxPostgresConnection;
    type Pool = SqlxPostgresPool;

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let (client, connection) = self
            .config
            .connect(NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("Connection error: {}", e);
            }
        });

        client
            .execute(&format!(r#"CREATE DATABASE "{}""#, name), &[])
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections
        self.terminate_connections(name).await?;

        let (client, connection) = self
            .config
            .connect(NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("Connection error: {}", e);
            }
        });

        client
            .execute(&format!(r#"DROP DATABASE IF EXISTS "{}""#, name), &[])
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, config: &PoolConfig) -> Result<Self::Pool> {
        let url = self.get_database_url(name);
        let pool = PgPoolOptions::new()
            .max_connections(config.max_size as u32)
            .connect(&url)
            .await
            .map_err(|e| PoolError::PoolCreationFailed(e.to_string()))?;

        Ok(SqlxPostgresPool {
            pool,
            connection_string: url,
        })
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        let (client, connection) = self
            .config
            .connect(NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("Connection error: {}", e);
            }
        });

        client
            .execute(
                &format!(
                    r#"
                    SELECT pg_terminate_backend(pid)
                    FROM pg_stat_activity
                    WHERE datname = '{}'
                    AND pid <> pg_backend_pid()
                    "#,
                    name
                ),
                &[],
            )
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        let (client, connection) = self
            .config
            .connect(NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("Connection error: {}", e);
            }
        });

        client
            .execute(
                &format!(r#"CREATE DATABASE "{}" TEMPLATE "{}""#, name, template),
                &[],
            )
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SqlxPostgresPool {
    pool: PgPool,
    connection_string: String,
}

impl SqlxPostgresPool {
    pub fn new(url: &str, max_size: usize) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_size as u32)
            .connect_lazy(url)
            .map_err(|e| PoolError::PoolCreationFailed(e.to_string()))?;
        Ok(Self {
            pool,
            connection_string: url.to_string(),
        })
    }
}

#[async_trait]
impl DatabasePool for SqlxPostgresPool {
    type Connection = SqlxPostgresConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        Ok(SqlxPostgresConnection {
            pool: self.pool.clone(),
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connection is automatically returned to the pool when dropped
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "sqlx-postgres")]
mod tests {
    use super::*;
    use crate::{env::get_sqlx_postgres_url, pool::PoolConfig, template::DatabaseTemplate};

    #[tokio::test]
    async fn test_sqlx_backend() {
        let backend = SqlxPostgresBackend::new(&get_sqlx_postgres_url().unwrap()).unwrap();

        // Create a test database
        let db_name = DatabaseName::new("sqlx_test");
        backend.create_database(&db_name).await.unwrap();

        // Create a pool
        let pool = backend
            .create_pool(&db_name, &PoolConfig::default())
            .await
            .unwrap();

        // Test connection
        let conn = pool.acquire().await.unwrap();
        pool.release(conn).await.unwrap();

        // Clean up
        backend.drop_database(&db_name).await.unwrap();
    }

    #[tokio::test]
    async fn test_sqlx_template() {
        let backend = SqlxPostgresBackend::new(&get_sqlx_postgres_url().unwrap()).unwrap();

        let template = DatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .unwrap();

        // Initialize template with a table
        template
            .initialize_template(|conn| async move {
                sqlx::query(
                    "CREATE TABLE test (id SERIAL PRIMARY KEY, value TEXT);
                     INSERT INTO test (value) VALUES ($1);",
                )
                .bind("test_value")
                .execute(&conn.pool)
                .await
                .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
                Ok(())
            })
            .await
            .unwrap();

        // Get two separate databases
        let db1 = template.get_immutable_database().await.unwrap();
        let db2 = template.get_immutable_database().await.unwrap();

        // Verify they are separate
        let conn1 = db1.get_pool().acquire().await.unwrap();
        let conn2 = db2.get_pool().acquire().await.unwrap();

        // Insert into db1
        sqlx::query("INSERT INTO test (value) VALUES ($1)")
            .bind("test1")
            .execute(&conn1.pool)
            .await
            .unwrap();

        // Insert into db2
        sqlx::query("INSERT INTO test (value) VALUES ($1)")
            .bind("test2")
            .execute(&conn2.pool)
            .await
            .unwrap();

        // Verify data is separate
        let row1: (String,) = sqlx::query_as("SELECT value FROM test WHERE value = 'test1'")
            .fetch_one(&conn1.pool)
            .await
            .unwrap();
        assert_eq!(row1.0, "test1");

        let row2: (String,) = sqlx::query_as("SELECT value FROM test WHERE value = 'test2'")
            .fetch_one(&conn2.pool)
            .await
            .unwrap();
        assert_eq!(row2.0, "test2");
    }
}
