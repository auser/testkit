use async_trait::async_trait;
use tokio_postgres::{Client, Config, NoTls};
use url::Url;

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{DbError, Result},
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

impl PostgresConnection {
    /// Execute a query and return the rows
    pub async fn fetch(&mut self, sql: &str) -> Result<Vec<tokio_postgres::Row>> {
        self.client
            .query(sql, &[])
            .await
            .map_err(|e| DbError::new(format!("Failed to execute query '{}': {}", sql, e)))
    }

    /// Execute a query and return exactly one row
    pub async fn fetch_one(&mut self, sql: &str) -> Result<tokio_postgres::Row> {
        self.client
            .query_one(sql, &[])
            .await
            .map_err(|e| DbError::new(format!("Failed to execute query '{}': {}", sql, e)))
    }

    /// Execute a query and return at most one row (or None)
    pub async fn fetch_optional(&mut self, sql: &str) -> Result<Option<tokio_postgres::Row>> {
        self.client
            .query_opt(sql, &[])
            .await
            .map_err(|e| DbError::new(format!("Failed to execute query '{}': {}", sql, e)))
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
            self.client
                .execute(stmt, &[])
                .await
                .map_err(|e| DbError::new(format!("Failed to execute '{}': {}", stmt, e)))?;
        }
        Ok(())
    }

    async fn begin(&mut self) -> Result<Self::Transaction<'_>> {
        self.client
            .transaction()
            .await
            .map_err(|e| DbError::new(e.to_string()))
    }
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
        let url = Url::parse(&self.url).map_err(|e| DbError::new(e.to_string()))?;
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

    /// Create a test user with a random password
    pub async fn create_test_user(
        &self,
        _db_name: &DatabaseName,
        username: &str,
    ) -> Result<String> {
        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        // Generate a random password
        let password = format!("pw_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));

        // Create the role if it doesn't exist
        let create_role_sql = format!(
            "DO $$ BEGIN
                CREATE ROLE {} WITH LOGIN PASSWORD '{}' NOSUPERUSER NOCREATEDB NOCREATEROLE;
            EXCEPTION WHEN duplicate_object THEN
                RAISE NOTICE 'Role {} already exists, updating password';
                ALTER ROLE {} WITH PASSWORD '{}';
            END $$;",
            username, password, username, username, password
        );

        client
            .execute(&create_role_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(password)
    }

    /// Grant necessary privileges to the test user
    pub async fn grant_privileges(&self, db_name: &DatabaseName, username: &str) -> Result<()> {
        // Connect to the specific database
        let db_url = self.get_database_url(db_name)?;
        let (client, connection) = tokio_postgres::connect(&db_url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        // Grant privileges on the database
        let grant_connect_sql =
            format!("GRANT CONNECT ON DATABASE \"{}\" TO {};", db_name, username);
        client
            .execute(&grant_connect_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges on schema
        let grant_schema_sql = format!("GRANT USAGE ON SCHEMA public TO {};", username);
        client
            .execute(&grant_schema_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges on tables
        let grant_tables_sql = format!(
            "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO {};",
            username
        );
        client
            .execute(&grant_tables_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges on sequences
        let grant_sequences_sql = format!(
            "GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO {};",
            username
        );
        client
            .execute(&grant_sequences_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges for future tables and sequences
        let grant_future_tables_sql = format!(
            "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON TABLES TO {};",
            username
        );
        client
            .execute(&grant_future_tables_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        let grant_future_sequences_sql = format!(
            "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON SEQUENCES TO {};",
            username
        );
        client
            .execute(&grant_future_sequences_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    /// Get the admin connection string
    pub fn get_admin_connection_string(&self, name: &DatabaseName) -> String {
        // For PostgreSQL, we use the same connection string but with the specific database
        let mut url = url::Url::parse(&self.url).unwrap();
        url.set_path(&format!("/{}", name));
        url.to_string()
    }
}

#[async_trait]
impl DatabaseBackend for PostgresBackend {
    type Connection = PostgresConnection;
    type Pool = PostgresPool;

    async fn connect(&self) -> Result<Self::Pool> {
        let (_client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

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
            .map_err(|e| DbError::new(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        client
            .execute(&format!("CREATE DATABASE \"{}\"", name), &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections
        self.terminate_connections(name).await?;

        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        client
            .execute(&format!("DROP DATABASE IF EXISTS \"{}\"", name), &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, _config: &PoolConfig) -> Result<Self::Pool> {
        let url = self.get_database_url(name)?;

        let (_client, connection) = tokio_postgres::connect(&url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

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
            .map_err(|e| DbError::new(e.to_string()))?;

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
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

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
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        self.get_database_url(name)
            .unwrap_or_else(|_| "".to_string())
    }

    async fn create_test_user(
        &self,
        _name: &DatabaseName,
        username: &str,
    ) -> crate::error::Result<()> {
        let (client, connection) = tokio_postgres::connect(&self.url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        // Generate a random password
        let password = format!("pw_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));

        // Create the role if it doesn't exist
        let create_role_sql = format!(
            "DO $$ BEGIN
                CREATE ROLE {} WITH LOGIN PASSWORD '{}' NOSUPERUSER NOCREATEDB NOCREATEROLE;
            EXCEPTION WHEN duplicate_object THEN
                RAISE NOTICE 'Role {} already exists, updating password';
                ALTER ROLE {} WITH PASSWORD '{}';
            END $$;",
            username, password, username, username, password
        );

        client
            .execute(&create_role_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn grant_privileges(
        &self,
        name: &DatabaseName,
        username: &str,
    ) -> crate::error::Result<()> {
        // Connect to the specific database
        let db_url = self.get_database_url(name)?;
        let (client, connection) = tokio_postgres::connect(&db_url, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Spawn the connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        // Grant privileges on the database
        let grant_connect_sql = format!("GRANT CONNECT ON DATABASE \"{}\" TO {};", name, username);
        client
            .execute(&grant_connect_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges on schema
        let grant_schema_sql = format!("GRANT USAGE ON SCHEMA public TO {};", username);
        client
            .execute(&grant_schema_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges on tables
        let grant_tables_sql = format!(
            "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO {};",
            username
        );
        client
            .execute(&grant_tables_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges on sequences
        let grant_sequences_sql = format!(
            "GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO {};",
            username
        );
        client
            .execute(&grant_sequences_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Grant privileges for future tables and sequences
        let grant_future_tables_sql = format!(
            "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON TABLES TO {};",
            username
        );
        client
            .execute(&grant_future_tables_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        let grant_future_sequences_sql = format!(
            "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON SEQUENCES TO {};",
            username
        );
        client
            .execute(&grant_future_sequences_sql, &[])
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    fn get_admin_connection_string(&self, name: &DatabaseName) -> String {
        // For PostgreSQL, we use the same connection string but with the specific database
        let mut url = url::Url::parse(&self.url).unwrap();
        url.set_path(&format!("/{}", name));
        url.to_string()
    }
}

#[async_trait]
impl DatabasePool for PostgresPool {
    type Connection = PostgresConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        // For tokio-postgres, we create a new client with the same connection
        let (client, connection) = tokio_postgres::connect(&self.connection_string, NoTls)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

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
    // Tests module removed
}
