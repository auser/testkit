use crate::{DatabaseConnection, DatabasePool, FromRow, RowLike, ToSql, Transaction};
use async_trait::async_trait;
use sqlx::{PgPool, postgres::{PgConnection, PgPoolOptions, PgRow}};
use sqlx::postgres::PgQueryResult;
use sqlx::Row as SqlxRow;
use uuid::Uuid;
use std::sync::Arc;

pub struct PostgresPool {
    pool: PgPool,
    db_name: String,
}

pub struct PostgresConnection {
    conn: PgConnection,
}

pub struct PostgresTransaction {
    tx: sqlx::Transaction<'static, sqlx::Postgres>,
}

// Implement RowLike for sqlx::postgres::PgRow
impl RowLike for PgRow {
    fn get<T: 'static>(&self, name: &str) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        SqlxRow::get(self, name).map_err(|e| e.into())
    }

    fn get_by_index<T: 'static>(&self, idx: usize) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        SqlxRow::get(self, idx).map_err(|e| e.into())
    }
}

// Implement ToSql for common types
impl ToSql for String {
    fn ty(&self) -> &str { "TEXT" }
    
    fn to_sql(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(format!("'{}'", self.replace("'", "''")))
    }
}

impl ToSql for &str {
    fn ty(&self) -> &str { "TEXT" }
    
    fn to_sql(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(format!("'{}'", self.replace("'", "''")))
    }
}

impl ToSql for i32 {
    fn ty(&self) -> &str { "INTEGER" }
    
    fn to_sql(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.to_string())
    }
}

impl ToSql for i64 {
    fn ty(&self) -> &str { "BIGINT" }
    
    fn to_sql(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.to_string())
    }
}

impl ToSql for uuid::Uuid {
    fn ty(&self) -> &str { "UUID" }
    
    fn to_sql(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(format!("'{}'", self))
    }
}

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    type Error = sqlx::Error;
    
    async fn execute(&mut self, query: &str) -> Result<(), Self::Error> {
        sqlx::query(query)
            .execute(&mut self.conn)
            .await
            .map(|_| ())
    }
    
    async fn query<T>(
        &mut self,
        query: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<T>, Self::Error> 
    where 
        T: FromRow 
    {
        // Build query with parameters
        let mut sql = query.to_string();
        if !params.is_empty() {
            // Replace $1, $2, etc. with actual values
            for (i, param) in params.iter().enumerate() {
                let placeholder = format!("${}", i + 1);
                let value = param.to_sql().map_err(|e| {
                    sqlx::Error::Protocol(format!("Failed to convert parameter: {}", e))
                })?;
                sql = sql.replace(&placeholder, &value);
            }
        }
        
        // Execute the query
        let rows = sqlx::query(&sql)
            .fetch_all(&mut self.conn)
            .await?;
        
        // Convert rows to T
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let result = T::from_row(&row).map_err(|e| {
                sqlx::Error::Protocol(format!("Failed to convert row: {}", e))
            })?;
            results.push(result);
        }
        
        Ok(results)
    }
}

#[async_trait]
impl Transaction for PostgresTransaction {
    async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }
    
    async fn rollback(self) -> Result<(), sqlx::Error> {
        self.tx.rollback().await
    }
}

#[async_trait]
impl DatabaseConnection for PostgresTransaction {
    type Error = sqlx::Error;
    
    async fn execute(&mut self, query: &str) -> Result<(), Self::Error> {
        sqlx::query(query)
            .execute(&mut self.tx)
            .await
            .map(|_| ())
    }
    
    async fn query<T>(
        &mut self,
        query: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<T>, Self::Error> 
    where 
        T: FromRow 
    {
        // Build query with parameters
        let mut sql = query.to_string();
        if !params.is_empty() {
            // Replace $1, $2, etc. with actual values
            for (i, param) in params.iter().enumerate() {
                let placeholder = format!("${}", i + 1);
                let value = param.to_sql().map_err(|e| {
                    sqlx::Error::Protocol(format!("Failed to convert parameter: {}", e))
                })?;
                sql = sql.replace(&placeholder, &value);
            }
        }
        
        // Execute the query
        let rows = sqlx::query(&sql)
            .fetch_all(&mut self.tx)
            .await?;
        
        // Convert rows to T
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let result = T::from_row(&row).map_err(|e| {
                sqlx::Error::Protocol(format!("Failed to convert row: {}", e))
            })?;
            results.push(result);
        }
        
        Ok(results)
    }
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Connection = PostgresConnection;
    type Tx = PostgresTransaction;
    
    async fn connection(&self) -> Result<Self::Connection, sqlx::Error> {
        let conn = self.pool.acquire().await?;
        Ok(PostgresConnection { conn })
    }
    
    async fn begin(&self) -> Result<Self::Tx, sqlx::Error> {
        let tx = self.pool.begin().await?;
        Ok(PostgresTransaction { tx })
    }
    
    async fn setup<F, Fut>(&self, setup_fn: F) -> Result<(), sqlx::Error>
    where
        F: FnOnce(Self::Connection) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), sqlx::Error>> + Send,
    {
        let conn = self.connection().await?;
        setup_fn(conn).await
    }
}

impl PostgresPool {
    pub async fn setup_test_db() -> Result<Self, sqlx::Error> {
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect("postgres://postgres:postgres@localhost:5432/postgres")
            .await?;
        
        // Generate unique DB name
        let db_name = format!("testkit_{}", Uuid::new_v4().to_string().replace("-", "_"));
        
        // Create the test database
        sqlx::query(&format!("CREATE DATABASE {}", db_name))
            .execute(&admin_pool)
            .await?;
        
        // Connect to the new database
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&format!("postgres://postgres:postgres@localhost:5432/{}", db_name))
            .await?;
        
        Ok(Self { pool, db_name })
    }
}

impl Drop for PostgresPool {
    fn drop(&mut self) {
        let db_name = self.db_name.clone();
        
        // Use tokio runtime to drop the database
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                // Connect to postgres database to drop the test database
                if let Ok(admin_pool) = PgPoolOptions::new()
                    .max_connections(1)
                    .connect("postgres://postgres:postgres@localhost:5432/postgres")
                    .await 
                {
                    // Terminate connections first
                    let _ = sqlx::query(&format!(
                        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}'",
                        db_name
                    ))
                    .execute(&admin_pool)
                    .await;
                    
                    // Drop the database
                    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {}", db_name))
                        .execute(&admin_pool)
                        .await;
                }
            });
        });
    }
} 