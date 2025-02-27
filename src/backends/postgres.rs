use async_trait::async_trait;
use tokio_postgres::{Client, Config, NoTls};
use url::Url;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    test_db::DatabaseName,
};

#[derive(Debug, Clone)]
pub struct PostgresBackend {
    url: String,
}

pub struct PostgresConnection {
    client: Client,
}

#[derive(Debug, Clone)]
pub struct PostgresPool {
    connection_string: String,
}

impl PostgresBackend {
    pub async fn new(url: &str) -> Result<Self> {
        Ok(Self {
            url: url.to_string(),
        })
    }

    fn get_database_url(&self, name: &DatabaseName) -> Result<String> {
        let url = Url::parse(&self.url).map_err(|e| PoolError::InvalidUrl(e.to_string()))?;
        let mut config = Config::new();
        config.host(url.host_str().unwrap_or("localhost"));
        config.port(url.port().unwrap_or(5432));
        config.user(url.username());
        if let Some(pass) = url.password() {
            config.password(pass);
        }
        config.dbname(name.as_str());

        // Manually build connection string instead of using to_string()
        let mut conn_str = String::new();
        conn_str.push_str("postgres://");
        conn_str.push_str(url.username());
        if let Some(pass) = url.password() {
            conn_str.push(':');
            conn_str.push_str(pass);
        }
        conn_str.push('@');
        conn_str.push_str(url.host_str().unwrap_or("localhost"));
        conn_str.push(':');
        conn_str.push_str(&url.port().unwrap_or(5432).to_string());
        conn_str.push('/');
        conn_str.push_str(name.as_str());

        Ok(conn_str)
    }

    pub fn connection_string(&self) -> String {
        self.url.clone()
    }
}

#[async_trait]
impl Connection for PostgresConnection {
    type Transaction<'conn> = tokio_postgres::Transaction<'conn> where Self: 'conn;

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
        // Split the SQL into individual statements
        let statements: Vec<&str> = sql
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        // Execute each statement separately
        for stmt in statements {
            self.client.execute(stmt, &[]).await.map_err(|e| {
                PoolError::DatabaseError(format!("Failed to execute '{}': {}", stmt, e))
            })?;
        }
        Ok(())
    }

    async fn begin(&mut self) -> Result<Self::Transaction<'_>> {
        self.client
            .transaction()
            .await
            .map_err(|e| PoolError::TransactionError(e.to_string()))
    }
}

#[async_trait]
impl DatabaseBackend for PostgresBackend {
    type Connection = PostgresConnection;
    type Pool = PostgresPool;

    async fn connect(&self) -> Result<Self::Pool> {
        let (_client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(PostgresPool {
            connection_string: self.url.clone(),
        })
    }

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        client
            .execute(&format!("CREATE DATABASE \"{}\"", name), &[])
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections
        self.terminate_connections(name).await?;

        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        client
            .execute(&format!("DROP DATABASE IF EXISTS \"{}\"", name), &[])
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, _config: &PoolConfig) -> Result<Self::Pool> {
        let url = self.get_database_url(name)?;

        let (_client, connection) = tokio_postgres::connect(&url, NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(PostgresPool {
            connection_string: url,
        })
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
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
        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
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

    fn connection_string(&self, name: &DatabaseName) -> String {
        self.get_database_url(name).unwrap()
    }
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Connection = PostgresConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        // For tokio-postgres, we create a new client with the same connection
        let (client, connection) = tokio_postgres::connect(&self.connection_string, NoTls)
            .await
            .map_err(|e| PoolError::ConnectionAcquisitionFailed(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(PostgresConnection { client })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connection is automatically closed when dropped
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

#[cfg(test)]
#[cfg(feature = "postgres")]
mod tests {
    use super::*;
    use crate::prelude::*;
    use sqlx::Executor;
    use sqlx::Row;

    #[tokio::test]
    async fn test_postgres_backend() {
        with_test_db!(
            "postgres://postgres:postgres@postgres:5432/postgres",
            |_conn| async move {
                // No setup needed
                Ok(())
            },
            |db| async move {
                // Get a connection
                let mut conn = db.connection().await.unwrap();

                // Test basic query execution
                conn.execute("CREATE TABLE test (id SERIAL PRIMARY KEY, name TEXT)")
                    .await
                    .unwrap();
                conn.execute("INSERT INTO test (name) VALUES ('test')")
                    .await
                    .unwrap();

                // Test transaction
                let mut tx = conn.begin().await.unwrap();
                tx.execute("INSERT INTO test (name) VALUES ('test2')")
                    .await
                    .unwrap();
                tx.commit().await.unwrap();

                // Test connection validity
                assert!(conn.is_valid().await);

                // Test connection reset
                conn.reset().await.unwrap();
            }
        );
    }

    #[tokio::test]
    async fn test_postgres_connection_errors() -> Result<()> {
        // Test invalid connection string
        let backend =
            PostgresBackend::new("postgres://invalid:invalid@localhost:5432/postgres").await?;
        assert!(backend.connect().await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_postgres_database_operations() {
        with_test_db!(
            "postgres://postgres:postgres@postgres:5432/postgres",
            |_conn| async move {
                // Setup code goes here
                Ok(())
            },
            |db| async move {
                // Test multiple statement execution
                let conn = db.connection().await.unwrap();
                conn.execute(
                    "CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT); \
                     INSERT INTO users (name) VALUES ('user1'); \
                     INSERT INTO users (name) VALUES ('user2');",
                )
                .await
                .unwrap();

                // You can verify the inserted data if needed
                conn.execute("SELECT * FROM users").await.unwrap();
            }
        );
    }

    #[tokio::test]
    async fn test_postgres_database_operations_setup() {
        with_test_db!(
            "postgres://postgres:postgres@postgres:5432/postgres",
            |conn| async move {
                conn.execute("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT)")
                    .await
                    .unwrap();
                conn.execute("INSERT INTO users (id, name) VALUES (1, 'user1')")
                    .await
                    .unwrap();
                Ok(())
            },
            |db| async move {
                // You can verify the inserted data if needed
                let conn = db.connection().await.unwrap();
                let result = conn
                    .fetch_one("SELECT * FROM users WHERE id = 1")
                    .await
                    .unwrap();
                let val = result.get::<String, _>(1);
                assert_eq!(val, "user1");
            }
        );
    }
}
