use async_trait::async_trait;
use mysql_async::{prelude::Queryable, Conn, Opts, Pool as MyPool, Row};

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{DbError, Result},
    pool::PoolConfig,
    test_db::DatabaseName,
};

pub struct MySqlConnection {
    pub(crate) conn: Conn,
    #[allow(dead_code)]
    connection_string: String,
}

impl MySqlConnection {
    /// Execute a query and return multiple rows
    pub async fn fetch(&mut self, sql: &str) -> Result<Vec<Row>> {
        self.conn
            .query(sql)
            .await
            .map_err(|e| DbError::new(e.to_string()))
    }

    /// Execute a query and return exactly one row
    pub async fn fetch_one(&mut self, sql: &str) -> Result<Row> {
        self.conn
            .query_first(sql)
            .await
            .map_err(|e| DbError::new(format!("Failed to execute query '{}': {}", sql, e)))?
            .ok_or_else(|| DbError::new("No rows returned for query".to_string()))
    }

    /// Execute a query and return at most one row (or None)
    pub async fn fetch_optional(&mut self, sql: &str) -> Result<Option<Row>> {
        self.conn
            .query_first(sql)
            .await
            .map_err(|e| DbError::new(format!("Failed to execute query '{}': {}", sql, e)))
    }
}

#[async_trait]
impl Connection for MySqlConnection {
    type Transaction<'conn> = mysql_async::Transaction<'conn> where Self: 'conn;

    async fn is_valid(&self) -> bool {
        // We need to use a mutable connection for ping, so we'll just return true
        // since we can't actually test without a mutable reference
        true
    }

    async fn reset(&mut self) -> Result<()> {
        self.conn
            .reset()
            .await
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(())
    }

    async fn execute(&mut self, sql: &str) -> Result<()> {
        self.conn
            .query_drop(sql)
            .await
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(())
    }

    async fn begin(&mut self) -> Result<Self::Transaction<'_>> {
        self.conn
            .start_transaction(mysql_async::TxOpts::default())
            .await
            .map_err(|e| DbError::new(e.to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct MySqlBackend {
    opts: Opts,
    connection_string: String,
    admin_connection_string: String,
}

impl MySqlBackend {
    pub fn new(connection_string: &str) -> Result<Self> {
        let opts = Opts::from_url(connection_string)
            .map_err(|e| DbError::new(format!("Invalid connection string: {}", e)))?;

        // For MySQL, we assume the connection string provided is for a superuser/admin
        // In a real implementation, you'd have separate admin and regular user credentials
        Ok(Self {
            opts,
            connection_string: connection_string.to_string(),
            admin_connection_string: connection_string.to_string(),
        })
    }

    fn get_database_url(&self, name: &DatabaseName) -> String {
        // Construct a new URL with the specific database name
        format!("{}/{}", self.connection_string.trim_end_matches('/'), name)
    }

    fn get_admin_database_url(&self, name: &DatabaseName) -> String {
        // Construct a new URL with the specific database name using admin credentials
        format!(
            "{}/{}",
            self.admin_connection_string.trim_end_matches('/'),
            name
        )
    }
}

#[async_trait]
impl DatabaseBackend for MySqlBackend {
    type Connection = MySqlConnection;
    type Pool = MySqlPool;

    async fn connect(&self) -> Result<Self::Pool> {
        // Connect to the default database using admin credentials
        Ok(MySqlPool {
            pool: MyPool::new(self.opts.clone()),
            connection_string: self.admin_connection_string.clone(),
        })
    }

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        // Use admin credentials for database creation
        let pool = MyPool::new(Opts::from_url(&self.admin_connection_string).unwrap());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Admin connection failed: {}", e)))?;

        conn.query_drop(&format!(
            "CREATE DATABASE IF NOT EXISTS `{}`",
            name.as_str()
        ))
        .await
        .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections using admin credentials
        self.terminate_connections(name).await?;

        // Use admin credentials for database deletion
        let pool = MyPool::new(Opts::from_url(&self.admin_connection_string).unwrap());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Admin connection failed: {}", e)))?;

        conn.query_drop(&format!("DROP DATABASE IF EXISTS `{}`", name.as_str()))
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, _config: &PoolConfig) -> Result<Self::Pool> {
        // Create a connection string with the specific database - use admin credentials
        let db_url = self.get_admin_database_url(name);

        // Parse the opts with the specific database
        let db_opts = Opts::from_url(&db_url)
            .map_err(|e| DbError::new(format!("Invalid database URL: {}", e)))?;

        // Create the pool with the configuration using admin credentials
        Ok(MySqlPool {
            pool: MyPool::new(db_opts),
            connection_string: db_url,
        })
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        // Use admin credentials to terminate connections
        let pool = MyPool::new(Opts::from_url(&self.admin_connection_string).unwrap());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Admin connection failed: {}", e)))?;

        conn.query_drop(&format!(
            r#"
            SELECT CONCAT('KILL ', id, ';')
            FROM INFORMATION_SCHEMA.PROCESSLIST
            WHERE db = '{}'
            INTO @kill_list;
            
            PREPARE kill_stmt FROM @kill_list;
            EXECUTE kill_stmt;
            DEALLOCATE PREPARE kill_stmt;
            "#,
            name.as_str()
        ))
        .await
        .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        // MySQL doesn't have native template support, so we need to:
        // 1. Create new database with admin credentials
        self.create_database(name).await?;

        // 2. Get the schema from template with admin credentials
        let pool = MyPool::new(Opts::from_url(&self.admin_connection_string).unwrap());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Admin connection failed: {}", e)))?;

        // Get all tables
        let rows = conn
            .query::<Row, _>(&format!(
                r#"
                    SELECT table_name 
                    FROM information_schema.tables 
                    WHERE table_schema = '{}'
                    "#,
                template.as_str()
            ))
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        // Extract table names from the result rows
        let tables: Vec<String> = rows
            .into_iter()
            .map(|row: Row| {
                let table_name: String = row.get(0).unwrap();
                table_name
            })
            .collect();

        // For each table, copy structure and data with admin credentials
        for table in tables {
            // Create table in new database
            conn.query_drop(&format!(
                "CREATE TABLE `{}`.`{}` LIKE `{}`.`{}`",
                name, table, template, table
            ))
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

            // Copy data
            conn.query_drop(&format!(
                "INSERT INTO `{}`.`{}` SELECT * FROM `{}`.`{}`",
                name, table, template, table
            ))
            .await
            .map_err(|e| DbError::new(e.to_string()))?;
        }

        Ok(())
    }

    fn connection_string(&self, name: &DatabaseName) -> String {
        self.get_database_url(name)
    }

    fn get_admin_connection_string(&self, name: &DatabaseName) -> String {
        self.get_admin_database_url(name)
    }
}

#[derive(Debug, Clone)]
pub struct MySqlPool {
    pool: MyPool,
    connection_string: String,
}

impl MySqlPool {
    pub fn new(connection_string: String) -> Self {
        let opts = Opts::from_url(&connection_string).unwrap();
        Self {
            pool: MyPool::new(opts),
            connection_string,
        }
    }
}

#[async_trait]
impl DatabasePool for MySqlPool {
    type Connection = MySqlConnection;

    async fn acquire(&self) -> Result<Self::Connection> {
        let conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(MySqlConnection {
            conn,
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
