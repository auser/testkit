use async_trait::async_trait;
use sqlx::{mysql::MySqlPoolOptions, MySql, MySqlPool, Transaction};
use url::Url;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{DbError, Result},
    pool::PoolConfig,
    DatabaseName,
};

#[derive(Debug, Clone)]
pub struct SqlxMySqlConnection {
    pub(crate) pool: MySqlPool,
    connection_string: String,
}

#[async_trait]
impl Connection for SqlxMySqlConnection {
    type Transaction<'conn> = Transaction<'conn, MySql>;

    async fn is_valid(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }

    async fn reset(&mut self) -> Result<()> {
        // MySQL doesn't have DISCARD ALL like PostgreSQL, but connections are generally
        // automatically reset in the connection pool
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
            sqlx::query(stmt)
                .execute(&self.pool)
                .await
                .map_err(|e| DbError::new(format!("Failed to execute '{}': {}", stmt, e)))?;
        }
        Ok(())
    }

    async fn begin(&mut self) -> Result<Self::Transaction<'_>> {
        self.pool
            .begin()
            .await
            .map_err(|e| DbError::new(e.to_string()))
    }
}

impl SqlxMySqlConnection {
    /// Get a reference to the underlying SQLx MySqlPool
    ///
    /// This allows direct use of SQLx queries with this connection
    pub fn sqlx_pool(&self) -> &MySqlPool {
        &self.pool
    }

    pub async fn connect(&self) -> Result<MySqlPool> {
        MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(self.connection_string.as_str())
            .await
            .map_err(|e| DbError::new(e.to_string()))
    }
}

impl<'c> sqlx::Executor<'c> for &'c SqlxMySqlConnection {
    type Database = sqlx::MySql;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::mysql::MySqlQueryResult, sqlx::mysql::MySqlRow>,
            sqlx::Error,
        >,
    > {
        (&self.pool).fetch_many(query)
    }

    fn fetch_optional<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<Option<sqlx::mysql::MySqlRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::mysql::MySqlTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::mysql::MySqlRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::mysql::MySqlRow>, sqlx::Error>>
    {
        (&self.pool).fetch_all(query)
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::Describe<Self::Database>, sqlx::Error>,
    > {
        (&self.pool).describe(sql)
    }
}

impl<'c> sqlx::Executor<'c> for SqlxMySqlConnection {
    type Database = sqlx::MySql;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::mysql::MySqlQueryResult, sqlx::mysql::MySqlRow>,
            sqlx::Error,
        >,
    > {
        (&self.pool).fetch_many(query)
    }

    fn fetch_optional<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<Option<sqlx::mysql::MySqlRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::mysql::MySqlTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::mysql::MySqlRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::mysql::MySqlRow>, sqlx::Error>>
    {
        (&self.pool).fetch_all(query)
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::Describe<Self::Database>, sqlx::Error>,
    > {
        (&self.pool).describe(sql)
    }
}

#[derive(Debug, Clone)]
pub struct SqlxMySqlBackend {
    config: MySqlPoolOptions,
    url: Url,
}

impl SqlxMySqlBackend {
    pub fn new(connection_string: &str) -> Result<Self> {
        // Configure MySQL pool options with more robust settings for testing
        let config = MySqlPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .idle_timeout(std::time::Duration::from_secs(600))
            .max_lifetime(Some(std::time::Duration::from_secs(1800)))
            .test_before_acquire(true);

        let url = Url::parse(connection_string)
            .map_err(|e| DbError::new(format!("Invalid connection string: {}", e)))?;

        tracing::debug!("Created SqlxMySqlBackend with URL base: {}", url.as_str());
        Ok(Self { config, url })
    }

    fn get_database_url(&self, name: &DatabaseName) -> String {
        let mut url = self.url.clone();
        url.set_path(name.as_str());
        let result = url.to_string();
        tracing::debug!("Generated database URL for {}: {}", name, result);
        result
    }

    pub async fn connect(&self) -> Result<MySqlPool> {
        tracing::debug!("Connecting to MySQL at {}", self.url);
        self.config
            .clone()
            .connect(self.url.as_str())
            .await
            .map_err(|e| DbError::new(format!("Failed to connect to MySQL: {}", e)))
    }
}

#[async_trait]
impl DatabaseBackend for SqlxMySqlBackend {
    type Connection = SqlxMySqlConnection;
    type Pool = SqlxMySqlPool;

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let pool = self.connect().await?;

        // Create the database with MySQL syntax
        sqlx::query(&format!("CREATE DATABASE `{}`", name.as_str()))
            .execute(&pool)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections
        self.terminate_connections(name).await?;

        let pool = self.connect().await?;

        // Drop the database with MySQL syntax
        sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", name.as_str()))
            .execute(&pool)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, config: &PoolConfig) -> Result<Self::Pool> {
        let url = self.get_database_url(name);
        let pool = MySqlPoolOptions::new()
            .max_connections(config.max_size as u32)
            .connect(&url)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(SqlxMySqlPool {
            pool,
            connection_string: url,
        })
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        let pool = self.connect().await?;

        // MySQL approach to kill connections
        let result: Vec<(i64,)> = sqlx::query_as(&format!(
            "SELECT ID FROM INFORMATION_SCHEMA.PROCESSLIST WHERE DB = '{}'",
            name.as_str()
        ))
        .fetch_all(&pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;

        for (id,) in result {
            sqlx::query(&format!("KILL {}", id))
                .execute(&pool)
                .await
                .map_err(|e| DbError::new(e.to_string()))?;
        }

        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        // MySQL doesn't have a native TEMPLATE feature like PostgreSQL
        // We'll create the new database and then copy all data from the template

        // First create the target database
        self.create_database(name).await?;

        // Get connection to source database
        let source_url = self.get_database_url(template);
        let source_pool = MySqlPoolOptions::new()
            .connect(&source_url)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Get connection to target database
        let target_url = self.get_database_url(name);
        let target_pool = MySqlPoolOptions::new()
            .connect(&target_url)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Get all tables from the template database
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT table_name FROM information_schema.tables 
             WHERE table_schema = ? AND table_type = 'BASE TABLE'",
        )
        .bind(template.as_str())
        .fetch_all(&source_pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;

        // For each table in the template database
        for (table_name,) in tables {
            // Create table structure in the target database (with CREATE TABLE LIKE)
            sqlx::query(&format!(
                "CREATE TABLE `{}`.`{}` LIKE `{}`.`{}`",
                name.as_str(),
                table_name,
                template.as_str(),
                table_name
            ))
            .execute(&target_pool)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

            // Copy data from template to new database
            sqlx::query(&format!(
                "INSERT INTO `{}`.`{}` SELECT * FROM `{}`.`{}`",
                name.as_str(),
                table_name,
                template.as_str(),
                table_name
            ))
            .execute(&target_pool)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;
        }

        Ok(())
    }

    async fn connect(&self) -> Result<Self::Pool> {
        let pool = self.connect().await?;
        Ok(SqlxMySqlPool {
            pool,
            connection_string: self.url.to_string(),
        })
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        self.get_database_url(name)
    }
}

#[derive(Debug, Clone)]
pub struct SqlxMySqlPool {
    pool: MySqlPool,
    connection_string: String,
}

impl SqlxMySqlPool {
    pub fn new(url: &str, max_size: usize) -> Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(max_size as u32)
            .connect_lazy(url)
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(Self {
            pool,
            connection_string: url.to_string(),
        })
    }

    pub fn sqlx_pool(&self) -> &MySqlPool {
        &self.pool
    }
}

#[async_trait]
impl DatabasePool for SqlxMySqlPool {
    type Connection = SqlxMySqlConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        Ok(SqlxMySqlConnection {
            pool: self.pool.clone(),
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connections are automatically returned to the pool when dropped
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

impl<'c> sqlx::Executor<'c> for &'c SqlxMySqlPool {
    type Database = sqlx::MySql;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::mysql::MySqlQueryResult, sqlx::mysql::MySqlRow>,
            sqlx::Error,
        >,
    > {
        (&self.pool).fetch_many(query)
    }

    fn fetch_optional<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<Option<sqlx::mysql::MySqlRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::mysql::MySqlTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::mysql::MySqlRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::mysql::MySqlRow>, sqlx::Error>>
    {
        (&self.pool).fetch_all(query)
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::Describe<Self::Database>, sqlx::Error>,
    > {
        (&self.pool).describe(sql)
    }
}

impl<'c> sqlx::Executor<'c> for SqlxMySqlPool {
    type Database = sqlx::MySql;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::mysql::MySqlQueryResult, sqlx::mysql::MySqlRow>,
            sqlx::Error,
        >,
    > {
        (&self.pool).fetch_many(query)
    }

    fn fetch_optional<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<Option<sqlx::mysql::MySqlRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::mysql::MySqlTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::mysql::MySqlQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::mysql::MySqlRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::mysql::MySqlRow>, sqlx::Error>>
    {
        (&self.pool).fetch_all(query)
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::Describe<Self::Database>, sqlx::Error>,
    > {
        (&self.pool).describe(sql)
    }
}

#[cfg(test)]
mod tests {
    // These tests will be added when the feature is included in the crate
}
