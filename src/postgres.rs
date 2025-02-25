use async_trait::async_trait;
use sqlx::{PgPool, postgres::{PgConnection, PgPoolOptions, PgRow}};
use sqlx::postgres::PgQueryResult;
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

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    type Error = sqlx::Error;
    
    async fn execute(&mut self, query: &str) -> Result<(), Self::Error> {
        sqlx::query(query)
            .execute(&mut self.conn)
            .await
            .map(|_| ())
    }
    
    async fn query<T>(&mut self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<T>, Self::Error> 
    where T: FromRow {
        // Implementation would use sqlx's query_as and bind parameters
        todo!()
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
    
    async fn query<T>(&mut self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<T>, Self::Error> 
    where T: FromRow {
        // Implementation would use sqlx's query_as and bind parameters
        todo!()
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
        let db_name = format!("test_db_{}", Uuid::new_v4().to_string().replace("-", "_"));
        
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