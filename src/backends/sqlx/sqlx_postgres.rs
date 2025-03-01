use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Transaction};
use url::Url;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{DbError, Result},
    pool::PoolConfig,
    DatabaseName,
};

#[derive(Debug, Clone)]
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
            .map_err(|e| DbError::new(e.to_string()))?;
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

impl SqlxPostgresConnection {
    /// Get a reference to the underlying SQLx PgPool
    ///
    /// This allows direct use of SQLx queries with this connection
    pub fn sqlx_pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn connect(&self) -> Result<PgPool> {
        PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(self.connection_string.as_str())
            .await
            .map_err(|e| DbError::new(e.to_string()))
    }
}

impl<'c> sqlx::Executor<'c> for &'c SqlxPostgresConnection {
    type Database = sqlx::Postgres;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::postgres::PgQueryResult, sqlx::postgres::PgRow>,
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
        std::result::Result<Option<sqlx::postgres::PgRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::postgres::PgTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::postgres::PgRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::postgres::PgRow>, sqlx::Error>>
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

// Also implement for mutable references
impl<'c> sqlx::Executor<'c> for &'c mut SqlxPostgresConnection {
    type Database = sqlx::Postgres;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::postgres::PgQueryResult, sqlx::postgres::PgRow>,
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
        std::result::Result<Option<sqlx::postgres::PgRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::postgres::PgTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::postgres::PgRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::postgres::PgRow>, sqlx::Error>>
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
pub struct SqlxPostgresBackend {
    config: PgPoolOptions,
    url: Url,
}

impl SqlxPostgresBackend {
    pub fn new(connection_string: &str) -> Result<Self> {
        let config = PgPoolOptions::new();
        let url = Url::parse(connection_string)
            .map_err(|e| DbError::new(format!("Invalid connection string: {}", e)))?;

        Ok(Self { config, url })
    }

    fn get_database_url(&self, name: &DatabaseName) -> String {
        let mut url = self.url.clone();
        url.set_path(name.as_str());
        url.to_string()
    }

    pub async fn connect(&self) -> Result<PgPool> {
        self.config
            .clone()
            .connect(self.url.as_str())
            .await
            .map_err(|e| DbError::new(e.to_string()))
    }
}

#[async_trait]
impl DatabaseBackend for SqlxPostgresBackend {
    type Connection = SqlxPostgresConnection;
    type Pool = SqlxPostgresPool;

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let pool = self.connect().await?;

        // Create the database
        sqlx::query(&format!(r#"CREATE DATABASE "{}""#, name.as_str()))
            .execute(&pool)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections
        self.terminate_connections(name).await?;

        let pool = self.connect().await?;

        // Drop the database
        sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}""#, name.as_str()))
            .execute(&pool)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, config: &PoolConfig) -> Result<Self::Pool> {
        let url = self.get_database_url(name);
        let pool = PgPoolOptions::new()
            .max_connections(config.max_size as u32)
            .connect(&url)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(SqlxPostgresPool {
            pool,
            connection_string: url,
        })
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        let pool = self.connect().await?;

        sqlx::query(&format!(
            r#"
                SELECT pg_terminate_backend(pid)
                FROM pg_stat_activity
                WHERE datname = '{}'
                AND pid <> pg_backend_pid()
                "#,
            name.as_str()
        ))
        .execute(&pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        let pool = self.connect().await?;

        sqlx::query(&format!(
            r#"CREATE DATABASE "{}" TEMPLATE "{}""#,
            name.as_str(),
            template.as_str()
        ))
        .execute(&pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn connect(&self) -> Result<Self::Pool> {
        let pool = self.connect().await?;
        Ok(SqlxPostgresPool {
            pool,
            connection_string: self.url.to_string(),
        })
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        self.get_database_url(name)
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
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(Self {
            pool,
            connection_string: url.to_string(),
        })
    }

    /// Get the underlying SQLx pool for direct SQLx operations
    pub fn sqlx_pool(&self) -> &PgPool {
        &self.pool
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

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

impl<'c> sqlx::Executor<'c> for &'c SqlxPostgresPool {
    type Database = sqlx::Postgres;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::postgres::PgQueryResult, sqlx::postgres::PgRow>,
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
        std::result::Result<Option<sqlx::postgres::PgRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::postgres::PgTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::postgres::PgRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::postgres::PgRow>, sqlx::Error>>
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

impl<'c> sqlx::Executor<'c> for &'c mut SqlxPostgresPool {
    type Database = sqlx::Postgres;

    fn fetch_many<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::stream::BoxStream<
        'e,
        std::result::Result<
            sqlx::Either<sqlx::postgres::PgQueryResult, sqlx::postgres::PgRow>,
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
        std::result::Result<Option<sqlx::postgres::PgRow>, sqlx::Error>,
    > {
        (&self.pool).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlx::postgres::PgTypeInfo],
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgStatement<'q>, sqlx::Error>,
    > {
        (&self.pool).prepare_with(sql, parameters)
    }

    fn execute<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<
        'e,
        std::result::Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
    > {
        (&self.pool).execute(query)
    }

    fn fetch_one<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<sqlx::postgres::PgRow, sqlx::Error>>
    {
        (&self.pool).fetch_one(query)
    }

    fn fetch_all<'e, 'q: 'e, E: sqlx::Execute<'q, Self::Database> + 'q>(
        self,
        query: E,
    ) -> futures::future::BoxFuture<'e, std::result::Result<Vec<sqlx::postgres::PgRow>, sqlx::Error>>
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
#[cfg(feature = "sqlx-postgres")]
mod tests {
    use super::*;
    use crate::{env::get_sqlx_postgres_url, pool::PoolConfig};

    #[tokio::test]
    async fn test_sqlx_backend() {
        let backend = SqlxPostgresBackend::new(&get_sqlx_postgres_url().unwrap()).unwrap();

        // Create a test database
        let db_name = DatabaseName::new(Some("sqlx_test"));
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

        let template = crate::test_db::TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .unwrap();

        // Initialize template with a table
        template
            .initialize(|conn| async move {
                sqlx::query("CREATE TABLE test (id SERIAL PRIMARY KEY, value TEXT);")
                    .execute(&conn.pool)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                sqlx::query("INSERT INTO test (value) VALUES ($1)")
                    .bind("test_value")
                    .execute(&conn.pool)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(())
            })
            .await
            .unwrap();

        // Get two separate databases
        let db1 = template.create_test_database().await.unwrap();
        let db2 = template.create_test_database().await.unwrap();

        // Verify they are separate
        let conn1 = db1.pool.acquire().await.unwrap();
        let conn2 = db2.pool.acquire().await.unwrap();

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

        let row0: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&conn1.pool)
            .await
            .unwrap();
        assert_eq!(row0.0, 1);

        // Clean up
    }
}

#[cfg(feature = "sqlx-mysql")]
pub type SqlxMySqlBackend = SqlxPostgresBackend;

// Alias for backward compatibility with tests
#[cfg(feature = "sqlx-sqlite")]
pub type SqlxSqliteBackend = crate::backends::sqlite::SqliteBackend;
