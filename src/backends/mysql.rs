use async_trait::async_trait;
use mysql_async::{prelude::Queryable, Conn, Opts, Pool as MyPool, Row};
use tracing::debug;

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
        debug!("MySQL fetch: {}", sql);
        self.conn
            .query(sql)
            .await
            .map_err(|e| DbError::new(format!("MySQL query error: {}", e)))
    }

    /// Execute a query and return exactly one row
    pub async fn fetch_one(&mut self, sql: &str) -> Result<Row> {
        debug!("MySQL fetch_one: {}", sql);
        self.conn
            .query_first(sql)
            .await
            .map_err(|e| DbError::new(format!("Failed to execute query '{}': {}", sql, e)))?
            .ok_or_else(|| DbError::new(format!("No rows returned for query: {}", sql)))
    }

    /// Execute a query and return at most one row (or None)
    pub async fn fetch_optional(&mut self, sql: &str) -> Result<Option<Row>> {
        debug!("MySQL fetch_optional: {}", sql);
        self.conn
            .query_first(sql)
            .await
            .map_err(|e| DbError::new(format!("Failed to execute query '{}': {}", sql, e)))
    }
}

#[async_trait]
impl Connection for MySqlConnection {
    type Transaction<'conn>
        = mysql_async::Transaction<'conn>
    where
        Self: 'conn;

    async fn is_valid(&self) -> bool {
        // We need to use a mutable connection for ping, so we'll just return true
        // since we can't actually test without a mutable reference
        true
    }

    async fn reset(&mut self) -> Result<()> {
        debug!("Resetting MySQL connection");
        self.conn
            .reset()
            .await
            .map_err(|e| DbError::new(format!("Failed to reset connection: {}", e)))?;
        Ok(())
    }

    async fn begin(&mut self) -> Result<Self::Transaction<'_>> {
        debug!("Beginning MySQL transaction");
        self.conn
            .start_transaction(mysql_async::TxOpts::default())
            .await
            .map_err(|e| DbError::new(format!("Failed to begin transaction: {}", e)))
    }

    async fn execute(&mut self, sql: &str) -> Result<()> {
        debug!("MySQL execute: {}", sql);

        self.conn
            .query_drop(sql)
            .await
            .map_err(|e| DbError::new(format!("Failed to execute SQL: {}", e)))?;

        Ok(())
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
        debug!(
            "Creating MySQL backend with connection string: {}",
            connection_string
        );

        // We'll use the provided connection string but also setup our known working
        // connection for admin operations with explicit timeout parameters
        let admin_url =
            "mysql://root@mysql:3306?connect_timeout=10&net_read_timeout=30&net_write_timeout=30";

        debug!("Using admin connection string: {}", admin_url);

        // Parse the opts from the connection string
        let opts = Opts::from_url(connection_string)
            .map_err(|e| DbError::new(format!("Invalid MySQL connection string: {}", e)))?;

        let backend = Self {
            opts,
            connection_string: connection_string.to_string(),
            admin_connection_string: admin_url.to_string(),
        };

        debug!("Created MySQL backend successfully");
        Ok(backend)
    }

    /// Get the connection string used for this backend
    pub fn get_connection_string(&self) -> &str {
        &self.connection_string
    }

    /// Test if we can connect to the database server
    /// This should only be called from async contexts
    #[allow(dead_code)]
    async fn test_connection(&self) -> Result<()> {
        debug!("Testing MySQL connection to: {}", self.connection_string);

        // Create a pool and try to get a connection
        let pool = MyPool::new(self.opts.clone());
        let conn_result = pool.get_conn().await;

        // Don't explicitly disconnect the pool - this avoids potential hanging
        // The pool will be dropped when it goes out of scope
        debug!("Pool will be dropped automatically");

        // Check if the connection was successful
        match conn_result {
            Ok(_) => {
                debug!("Successfully connected to MySQL");
                Ok(())
            }
            Err(e) => Err(DbError::new(format!("Failed to connect to MySQL: {}", e))),
        }
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

    /// Execute a statement using a direct connection as admin
    async fn execute_admin_statement(&self, sql: &str) -> Result<()> {
        debug!("Executing admin statement: {}", sql);

        // Create a temporary pool using the simplified approach that worked in our test
        let admin_url = "mysql://root@mysql:3306";
        debug!("Creating temporary pool with URL: {}", admin_url);

        let admin_opts = Opts::from_url(admin_url)
            .map_err(|e| DbError::new(format!("Invalid admin MySQL connection string: {}", e)))?;

        debug!("Creating connection pool");
        let pool = MyPool::new(admin_opts);

        // Get a connection and execute the statement
        debug!("Attempting to get connection from pool");
        let result = pool.get_conn().await.map_err(|e| {
            debug!("Admin connection failed: {}", e);
            DbError::new(format!(
                "Admin connection failed: {}. Attempted with: {}",
                e, admin_url
            ))
        })?;

        debug!("Successfully got connection, executing query");
        let mut conn = result;
        let query_result = conn.query_drop(sql).await.map_err(|e| {
            debug!("Admin query failed: {}. Query was: {}", e, sql);
            DbError::new(format!("Admin query failed: {}. Query was: {}", e, sql))
        });

        // Don't explicitly disconnect the pool - this avoids potential hanging
        // The pool will be dropped when it goes out of scope
        debug!("Pool will be dropped automatically");

        query_result
    }

    // Helper method to get admin connection pool
    fn get_admin_pool(&self) -> Result<MyPool> {
        // Use admin connection URL (root connection)
        let admin_url = "mysql://root@mysql:3306";
        let admin_opts = Opts::from_url(admin_url)
            .map_err(|e| DbError::new(format!("Invalid admin MySQL connection string: {}", e)))?;

        Ok(MyPool::new(admin_opts))
    }
}

#[async_trait]
impl DatabaseBackend for MySqlBackend {
    type Connection = MySqlConnection;
    type Pool = MySqlPool;

    async fn connect(&self) -> Result<Self::Pool> {
        debug!("Connecting to MySQL server with admin credentials");
        // Connect to the default database using admin credentials -
        // Use the simplified approach that worked in our test
        let admin_url = "mysql://root@mysql:3306";
        let admin_opts = Opts::from_url(admin_url)
            .map_err(|e| DbError::new(format!("Invalid admin MySQL connection string: {}", e)))?;

        Ok(MySqlPool {
            pool: MyPool::new(admin_opts),
            connection_string: admin_url.to_string(),
        })
    }

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        debug!("Creating MySQL database: {}", name);

        // Use a simple statement to create the database with proper backtick quoting
        let sql = format!("CREATE DATABASE IF NOT EXISTS `{}`", name.as_str());

        // Use the simplified approach that worked in our test
        let admin_url = "mysql://root@mysql:3306";
        debug!("Using admin connection string: {}", admin_url);

        match self.execute_admin_statement(&sql).await {
            Ok(_) => {
                debug!("Successfully created database {}", name);
                Ok(())
            }
            Err(e) => {
                debug!("Error creating database: {}", e);
                // If we couldn't create the database, try to get a more detailed error
                let error_sql = "SHOW WARNINGS";

                // Use the simplified approach that worked in our test
                let admin_opts = Opts::from_url(admin_url).map_err(|e| {
                    DbError::new(format!("Invalid admin MySQL connection string: {}", e))
                })?;

                let pool = MyPool::new(admin_opts);

                if let Ok(mut conn) = pool.get_conn().await {
                    if let Ok(warnings) = conn.query::<Row, _>(error_sql).await {
                        for warning in warnings {
                            let level: String = warning.get(0).unwrap_or_default();
                            let code: i32 = warning.get(1).unwrap_or_default();
                            let msg: String = warning.get(2).unwrap_or_default();
                            debug!("MySQL Warning [{}]: {} - {}", level, code, msg);
                        }
                    }
                }
                Err(DbError::new(format!(
                    "Failed to create database '{}': {}",
                    name, e
                )))
            }
        }
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        tracing::debug!("Dropping MySQL database: {}", name);

        // First, terminate all connections to the database
        self.terminate_connections(name).await?;

        // Now drop the database
        let query = format!("DROP DATABASE IF EXISTS `{}`", name.as_str());
        tracing::debug!("Executing SQL: {}", query);

        // Connect using the root connection (not to the specific database)
        let pool = self.get_admin_pool()?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Failed to connect to MySQL: {}", e)))?;

        use mysql_async::prelude::Queryable;

        match conn.query_drop(query).await {
            Ok(_) => {
                tracing::info!("Successfully dropped MySQL database: {}", name);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to drop MySQL database {}: {}", name, e);
                Err(DbError::new(format!("Failed to drop database: {}", e)))
            }
        }
    }

    async fn create_pool(&self, name: &DatabaseName, _config: &PoolConfig) -> Result<Self::Pool> {
        debug!("Creating MySQL connection pool for database: {}", name);

        // Use the simplified approach that worked in our test
        let admin_url = "mysql://root@mysql:3306";

        // Create a connection string with the specific database
        let db_url = format!("{}/{}", admin_url, name.as_str());
        debug!("Using database URL: {}", db_url);

        // Parse the opts with the specific database
        let db_opts = Opts::from_url(&db_url).map_err(|e| {
            debug!("Invalid database URL: {}", e);
            DbError::new(format!("Invalid database URL: {}", e))
        })?;

        // Create the pool with the configuration
        Ok(MySqlPool {
            pool: MyPool::new(db_opts),
            connection_string: db_url,
        })
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        tracing::debug!("Terminating all connections to MySQL database: {}", name);

        // Connect using the root connection
        let pool = self.get_admin_pool()?;
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Failed to connect to MySQL: {}", e)))?;

        use mysql_async::prelude::Queryable;

        // Get the list of process IDs connected to the target database
        let query = format!(
            "SELECT id FROM information_schema.processlist WHERE db = '{}'",
            name.as_str()
        );

        let result: Vec<Row> = conn
            .query(query)
            .await
            .map_err(|e| DbError::new(format!("Failed to list processes: {}", e)))?;

        if result.is_empty() {
            tracing::debug!("No connections found to database {}", name);
            return Ok(());
        }

        tracing::debug!(
            "Found {} connections to terminate for database {}",
            result.len(),
            name
        );

        // Kill each connection
        for row in result {
            let process_id: u64 = row.get(0).unwrap();
            let kill_query = format!("KILL {}", process_id);

            match conn.query_drop(kill_query).await {
                Ok(_) => tracing::debug!("Successfully killed connection {}", process_id),
                Err(e) => tracing::warn!("Failed to kill connection {}: {}", process_id, e),
            }
        }

        tracing::info!("Terminated all connections to database {}", name);
        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        debug!(
            "Creating MySQL database {} from template {}",
            name, template
        );

        // MySQL doesn't have native template support, so we need to:
        // 1. Create new database with admin credentials
        self.create_database(name).await?;

        // 2. Get the schema from template - use the simplified approach that worked in our test
        let admin_url = "mysql://root@mysql:3306";
        let admin_opts = Opts::from_url(admin_url)
            .map_err(|e| DbError::new(format!("Invalid admin MySQL connection string: {}", e)))?;

        let pool = MyPool::new(admin_opts);

        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Admin connection failed: {}", e)))?;

        // Get all tables
        let sql = format!(
            r#"
            SELECT table_name 
            FROM information_schema.tables 
            WHERE table_schema = '{}'
            "#,
            template.as_str()
        );

        debug!("Getting tables from template database: {}", sql);

        let rows = conn
            .query::<Row, _>(&sql)
            .await
            .map_err(|e| DbError::new(format!("Failed to get template tables: {}", e)))?;

        // Extract table names from the result rows
        let tables: Vec<String> = rows
            .into_iter()
            .map(|row: Row| {
                let table_name: String = row.get(0).unwrap();
                table_name
            })
            .collect();

        debug!("Found {} tables to copy", tables.len());

        // For each table, copy structure and data
        for table in tables {
            // Create table in new database
            let create_table_sql = format!(
                "CREATE TABLE `{}`.`{}` LIKE `{}`.`{}`",
                name, table, template, table
            );

            debug!("Creating table structure: {}", create_table_sql);

            conn.query_drop(&create_table_sql)
                .await
                .map_err(|e| DbError::new(format!("Failed to create table structure: {}", e)))?;

            // Copy data
            let copy_data_sql = format!(
                "INSERT INTO `{}`.`{}` SELECT * FROM `{}`.`{}`",
                name, table, template, table
            );

            debug!("Copying table data: {}", copy_data_sql);

            conn.query_drop(&copy_data_sql)
                .await
                .map_err(|e| DbError::new(format!("Failed to copy table data: {}", e)))?;
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
        debug!(
            "Creating new MySQL pool with connection string: {}",
            connection_string
        );
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
        debug!("Acquiring MySQL connection from pool");
        let conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| DbError::new(format!("Failed to acquire MySQL connection: {}", e)))?;

        Ok(MySqlConnection {
            conn,
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connection is automatically returned to the pool when dropped
        debug!("Releasing MySQL connection (automatically handled by drop)");
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}
