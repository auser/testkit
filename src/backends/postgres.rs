use std::str::FromStr;

use async_trait::async_trait;
use tokio_postgres::{Client, Config, NoTls};
use url::Url;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    template::DatabaseName,
};

pub struct PostgresConnection {
    pub(crate) client: Client,
}

#[async_trait]
impl Connection for PostgresConnection {
    async fn is_valid(&self) -> bool {
        self.client.simple_query("SELECT 1").await.is_ok()
    }

    async fn reset(&mut self) -> Result<()> {
        self.client
            .simple_query("DISCARD ALL")
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn execute(&mut self, sql: &str) -> Result<()> {
        self.client
            .batch_execute(sql)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PostgresBackend {
    config: Config,
    #[allow(unused)]
    url: Url,
}

impl PostgresBackend {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let config = Config::from_str(connection_string)
            .map_err(|e| PoolError::ConfigError(format!("Invalid connection string: {}", e)))?;
        let url = Url::parse(connection_string)
            .map_err(|e| PoolError::ConfigError(format!("Invalid connection string: {}", e)))?;

        // Create a connection to postgres database
        let mut postgres_url = url.clone();
        postgres_url.set_path("/postgres");

        let postgres_config = Config::from_str(postgres_url.as_str())
            .map_err(|e| PoolError::ConfigError(format!("Invalid connection string: {}", e)))?;

        // Try to connect and create the database
        if let Ok((client, connection)) = postgres_config.connect(NoTls).await {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    tracing::error!("Connection error: {}", e);
                }
            });

            let db_name = url.path().trim_start_matches('/');
            let _ = client
                .execute(&format!(r#"CREATE DATABASE "{}""#, db_name), &[])
                .await;
        }

        Ok(Self { config, url })
    }

    #[allow(dead_code)]
    fn get_database_url(&self, name: &DatabaseName) -> String {
        let mut url = self.url.clone();
        url.set_path(name.as_str());
        url.to_string()
    }
}

#[async_trait]
impl DatabaseBackend for PostgresBackend {
    type Connection = PostgresConnection;
    type Pool = PostgresPool;

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
        let mut pool_config = self.config.clone();
        pool_config.dbname(name.as_str());
        Ok(PostgresPool::new(pool_config, config.max_size))
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
pub struct PostgresPool {
    config: Config,
    #[allow(unused)]
    max_size: usize,
}

impl PostgresPool {
    pub fn new(config: Config, max_size: usize) -> Self {
        Self { config, max_size }
    }
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Connection = PostgresConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        let (client, connection) = self
            .config
            .connect(NoTls)
            .await
            .map_err(|e| PoolError::ConnectionAcquisitionFailed(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("Connection error: {}", e);
            }
        });

        Ok(PostgresConnection { client })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connection is automatically closed when dropped
        Ok(())
    }
}
