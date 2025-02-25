use async_trait::async_trait;
use mysql_async::{Conn, Opts, Pool as MyPool};

use crate::{
    backend::{Connection, DatabaseBackend, DatabasePool},
    error::{PoolError, Result},
    pool::PoolConfig,
    template::DatabaseName,
};

pub struct MySqlConnection {
    pub(crate) conn: Conn,
    connection_string: String,
}

#[async_trait]
impl Connection for MySqlConnection {
    type Transaction<'conn> = Transaction<'conn, Postgres>;

    async fn is_valid(&self) -> bool {
        self.conn.ping().await.is_ok()
    }

    async fn reset(&mut self) -> Result<()> {
        self.conn
            .reset()
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn execute(&mut self, sql: &str) -> Result<()> {
        self.conn
            .query_drop(sql)
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    fn connection_string(&self) -> String {
        self.connection_string.clone()
    }
}

#[derive(Debug, Clone)]
pub struct MySqlBackend {
    opts: Opts,
}

impl MySqlBackend {
    pub fn new(connection_string: &str) -> Result<Self> {
        let opts = Opts::from_url(connection_string)
            .map_err(|e| PoolError::ConfigError(format!("Invalid connection string: {}", e)))?;

        Ok(Self { opts })
    }

    fn get_database_url(&self, name: &DatabaseName) -> String {
        let mut opts = self.opts.clone();
        opts.db_name(Some(name.to_string()));
        opts.into_url().to_string()
    }
}

#[async_trait]
impl DatabaseBackend for MySqlBackend {
    type Connection = MySqlConnection;
    type Pool = MySqlPool;

    async fn create_database(&self, name: &DatabaseName) -> Result<()> {
        let pool = MyPool::new(self.opts.clone());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        conn.query_drop(&format!(
            "CREATE DATABASE IF NOT EXISTS `{}`",
            name.as_str()
        ))
        .await
        .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn drop_database(&self, name: &DatabaseName) -> Result<()> {
        // First terminate all connections
        self.terminate_connections(name).await?;

        let pool = MyPool::new(self.opts.clone());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        conn.query_drop(&format!("DROP DATABASE IF EXISTS `{}`", name.as_str()))
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn create_pool(&self, name: &DatabaseName, config: &PoolConfig) -> Result<Self::Pool> {
        let mut opts = self.opts.clone();
        opts.db_name(Some(name.to_string()));
        opts.pool_opts(mysql_async::PoolOpts::new().with_max_connections(config.max_size as u32));

        Ok(MySqlPool::new(opts))
    }

    async fn terminate_connections(&self, name: &DatabaseName) -> Result<()> {
        let pool = MyPool::new(self.opts.clone());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

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
        .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn create_database_from_template(
        &self,
        name: &DatabaseName,
        template: &DatabaseName,
    ) -> Result<()> {
        // MySQL doesn't have native template support, so we need to:
        // 1. Create new database
        self.create_database(name).await?;

        // 2. Get the schema from template
        let pool = MyPool::new(self.opts.clone());
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // Get all tables
        let tables: Vec<String> = conn
            .query_map(
                &format!(
                    r#"
                    SELECT table_name 
                    FROM information_schema.tables 
                    WHERE table_schema = '{}'
                    "#,
                    template.as_str()
                ),
                |table_name| table_name,
            )
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

        // For each table, copy structure and data
        for table in tables {
            // Create table in new database
            conn.query_drop(&format!(
                "CREATE TABLE `{}`.`{}` LIKE `{}`.`{}`",
                name, table, template, table
            ))
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;

            // Copy data
            conn.query_drop(&format!(
                "INSERT INTO `{}`.`{}` SELECT * FROM `{}`.`{}`",
                name, table, template, table
            ))
            .await
            .map_err(|e| PoolError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MySqlPool {
    pool: MyPool,
    connection_string: String,
}

impl MySqlPool {
    pub fn new(opts: Opts) -> Self {
        let connection_string = opts.to_string();
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
            .map_err(|e| PoolError::ConnectionAcquisitionFailed(e.to_string()))?;

        Ok(MySqlConnection {
            conn,
            connection_string: self.connection_string.clone(),
        })
    }

    async fn release(&self, _conn: Self::Connection) -> Result<()> {
        // Connection is automatically returned to the pool when dropped
        Ok(())
    }
}
