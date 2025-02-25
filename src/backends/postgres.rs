use async_trait::async_trait;
use futures::{future::BoxFuture, Stream};
use sqlx::{
    postgres::{PgPoolOptions, PgQueryResult, PgRow, PgStatement, PgTypeInfo},
    Describe, Either, Execute, Executor, PgPool, Postgres, Transaction,
};
use std::pin::Pin;
use url::Url;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    template::DatabaseName,
};

#[derive(Debug)]
pub struct PostgresConnection {
    pub(crate) pool: PgPool,
    connection_string: String,
}

impl<'c> Executor<'c> for &'c mut PostgresConnection {
    type Database = Postgres;

    fn fetch_many<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> Pin<
        Box<
            dyn Stream<Item = std::result::Result<Either<PgQueryResult, PgRow>, sqlx::Error>>
                + Send
                + 'e,
        >,
    >
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
    {
        Box::pin(self.pool.fetch_many(query))
    }

    fn fetch_optional<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxFuture<'e, std::result::Result<Option<PgRow>, sqlx::Error>>
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
    {
        self.pool.fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [PgTypeInfo],
    ) -> BoxFuture<'e, std::result::Result<PgStatement<'q>, sqlx::Error>>
    where
        'c: 'e,
    {
        self.pool.prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxFuture<'e, std::result::Result<PgQueryResult, sqlx::Error>>
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
    {
        self.pool.execute(query)
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> BoxFuture<'e, std::result::Result<Describe<Self::Database>, sqlx::Error>>
    where
        'c: 'e,
    {
        self.pool.describe(sql)
    }
}

#[async_trait]
impl Connection for PostgresConnection {
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
        // Split the SQL into individual statements
        let statements: Vec<&str> = sql
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        // Execute each statement separately
        for stmt in statements {
            sqlx::query(stmt).execute(&self.pool).await.map_err(|e| {
                PoolError::DatabaseError(format!("Failed to execute '{}': {}", stmt, e))
            })?;
        }
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

impl PostgresConnection {
    /// Get a reference to the underlying SQLx PgPool
    ///
    /// This allows direct use of SQLx queries with this connection
    pub fn sqlx_pool(&self) -> &PgPool {
        &self.pool
    }
}

#[derive(Debug, Clone)]
pub struct PostgresBackend {
    url: Url,
}

impl PostgresBackend {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let url = Url::parse(connection_string)
            .map_err(|e| PoolError::ConfigError(format!("Invalid connection string: {}", e)))?;

        // Create a connection to postgres database
        let mut postgres_url = url.clone();
        postgres_url.set_path("/postgres");

        // Try to connect and create the database
        if let Ok(pool) = PgPool::connect(postgres_url.as_str()).await {
            let db_name = url.path().trim_start_matches('/');
            let _ = sqlx::query(&format!(r#"CREATE DATABASE "{}""#, db_name))
                .execute(&pool)
                .await;
        }

        Ok(Self { url })
    }

    fn get_database_url(&self, name: &DatabaseName) -> String {
        let mut url = self.url.clone();
        url.set_path(name.as_str());
        url.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct PostgresPool {
    pub(crate) pool: PgPool,
    connection_string: String,
}

impl PostgresPool {
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
impl DatabaseBackend for PostgresBackend {
    type Connection = PostgresConnection;
    type Pool = PostgresPool;

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let pool = PgPool::connect(self.url.as_str())
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        sqlx::query(&format!(r#"CREATE DATABASE "{}""#, name))
            .execute(&pool)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections
        self.terminate_connections(name).await?;

        let pool = PgPool::connect(self.url.as_str())
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}""#, name))
            .execute(&pool)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, config: &PoolConfig) -> Result<Self::Pool> {
        let url = self.get_database_url(name);
        PostgresPool::new(&url, config.max_size)
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        let pool = PgPool::connect(self.url.as_str())
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        sqlx::query(&format!(
            r#"
            SELECT pg_terminate_backend(pid)
            FROM pg_stat_activity
            WHERE datname = '{}'
            AND pid <> pg_backend_pid()
            "#,
            name
        ))
        .execute(&pool)
        .await
        .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        let pool = PgPool::connect(self.url.as_str())
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        sqlx::query(&format!(
            r#"CREATE DATABASE "{}" TEMPLATE "{}""#,
            name, template
        ))
        .execute(&pool)
        .await
        .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Connection = PostgresConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        Ok(PostgresConnection {
            pool: self.pool.clone(),
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connection is automatically returned to the pool when dropped
        Ok(())
    }
}
