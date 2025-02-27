use std::{fs, path::PathBuf};

use async_trait::async_trait;

use crate::{
    backend::Connection,
    error::{PoolError, Result},
};

#[derive(Debug, Clone)]
pub enum SqlSource {
    Directory(PathBuf),
    File(PathBuf),
    Embedded(&'static [&'static str]),
}

impl SqlSource {
    fn read_sql_files(&self) -> Result<Vec<String>> {
        match self {
            SqlSource::Directory(path) => {
                let mut scripts = Vec::new();
                for entry in fs::read_dir(path).map_err(|e| {
                    PoolError::MigrationError(format!("Failed to read directory: {}", e))
                })? {
                    let entry = entry.map_err(|e| {
                        PoolError::MigrationError(format!("Failed to read directory entry: {}", e))
                    })?;
                    let path = entry.path();
                    if path.is_file() {
                        let sql = fs::read_to_string(&path).map_err(|e| {
                            PoolError::MigrationError(format!("Failed to read SQL file: {}", e))
                        })?;
                        scripts.push(sql);
                    }
                }
                Ok(scripts)
            }
            SqlSource::File(path) => {
                let sql = fs::read_to_string(path).map_err(|e| {
                    PoolError::MigrationError(format!("Failed to read SQL file: {}", e))
                })?;
                Ok(vec![sql])
            }
            SqlSource::Embedded(scripts) => Ok(scripts.iter().map(|s| s.to_string()).collect()),
        }
    }
}

#[async_trait]
pub trait RunSql {
    async fn run_sql_scripts(&mut self, source: &SqlSource) -> Result<()>;
}

#[async_trait]
impl<T> RunSql for T
where
    T: Connection + Send,
{
    async fn run_sql_scripts(&mut self, source: &SqlSource) -> Result<()> {
        let scripts = source.read_sql_files()?;
        for script in scripts {
            tracing::info!("Running SQL script");
            self.execute(&script).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "postgres")]
mod tests {
    use super::*;
    use crate::{
        backend::DatabasePool, backends::PostgresBackend, env::get_postgres_url, pool::PoolConfig,
        test_db::TestDatabaseTemplate,
    };

    #[tokio::test]
    async fn test_sql_scripts() {
        let backend = PostgresBackend::new(&get_postgres_url().unwrap())
            .await
            .unwrap();
        let template = TestDatabaseTemplate::new(backend, PoolConfig::default(), 5)
            .await
            .unwrap();

        // Create a temporary directory with SQL scripts
        let temp_dir = tempfile::tempdir().unwrap();
        let setup_path = temp_dir.path().join("setup.sql");
        fs::write(
            &setup_path,
            r#"
            CREATE TABLE users (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT
            );
            "#,
        )
        .unwrap();

        // Initialize template with SQL scripts
        template
            .initialize_template(|mut conn| async move {
                conn.run_sql_scripts(&SqlSource::File(setup_path)).await?;
                Ok(())
            })
            .await
            .unwrap();

        // Get a database and verify table was created
        let db = template.get_immutable_database().await.unwrap();
        let mut conn = db.get_pool().acquire().await.unwrap();

        // Verify table exists and has expected columns
        conn.execute(
            r#"
            INSERT INTO users (name, email)
            VALUES ('test', 'test@example.com');
            "#,
        )
        .await
        .unwrap();
    }
}
