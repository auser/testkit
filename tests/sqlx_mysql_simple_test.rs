//! Integration tests for SQLx MySQL simple usage

#[cfg(feature = "sqlx-mysql")]
mod sqlx_mysql_simple_tests {
    use db_testkit::{
        backend::{Connection, DatabaseBackend},
        backends::SqlxMySqlBackend,
        init_tracing,
        test_db::TestDatabase,
        PoolConfig, Result,
    };
    use tracing::info;

    #[tokio::test]
    async fn test_sqlx_mysql_simple() -> Result<()> {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=debug");
        }
        init_tracing();

        info!("Starting SQLx MySQL simple test");

        // Use a direct connection string with explicit parameters
        let mysql_url = "mysql://root@mysql:3306?timeout=30&connect_timeout=30&pool_timeout=30&ssl-mode=DISABLED";
        info!("Using direct MySQL URL: {}", mysql_url);

        // Create the backend with retry logic
        let backend = SqlxMySqlBackend::new(mysql_url)?;
        info!("Successfully created SQLx MySQL backend");

        // Test connection with retry logic
        let max_retries = 3;
        let mut retry_count = 0;
        let mut last_error = None;

        while retry_count < max_retries {
            info!(
                "Testing connection to MySQL server (attempt {})...",
                retry_count + 1
            );
            match backend.connect().await {
                Ok(pool) => {
                    info!("Connection test successful on attempt {}", retry_count + 1);
                    // Explicitly drop the pool to ensure connections are closed
                    drop(pool);
                    break;
                }
                Err(e) => {
                    retry_count += 1;
                    last_error = Some(e);
                    if retry_count < max_retries {
                        let backoff = std::time::Duration::from_secs(2 * retry_count as u64);
                        info!(
                            "Connection attempt {} failed, retrying in {:?}...",
                            retry_count, backoff
                        );
                        tokio::time::sleep(backoff).await;
                    } else {
                        info!("All connection attempts failed");
                    }
                }
            }
        }

        if let Some(e) = last_error {
            if retry_count >= max_retries {
                return Err(e);
            }
        }

        let test_db = TestDatabase::new(backend, PoolConfig::default()).await?;

        let db_name = test_db.db_name.clone();
        info!("Created test database: {}", db_name);

        // Get a connection and run some queries
        let mut conn = test_db.connection().await?;

        // Create a table
        conn.execute(
            "CREATE TABLE test_table (id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(255))",
        )
        .await?;
        info!("Created test table");

        // Insert data
        conn.execute("INSERT INTO test_table (name) VALUES ('Alice'), ('Bob'), ('Charlie')")
            .await?;
        info!("Inserted test data");

        // For SQLx backends, use the sqlx interface directly
        use sqlx::Row;
        let sqlx_pool = conn.sqlx_pool();

        let rows = sqlx::query("SELECT id, name FROM test_table ORDER BY id")
            .fetch_all(sqlx_pool)
            .await?;

        // Display the results
        info!("Query results:");
        for row in rows {
            let id: i32 = row.get(0);
            let name: String = row.get(1);
            info!("  id: {}, name: {}", id, name);
        }

        // Count the rows
        let row = sqlx::query("SELECT COUNT(*) FROM test_table")
            .fetch_one(sqlx_pool)
            .await?;
        let count: i64 = row.get(0);
        info!("Total rows: {}", count);

        assert_eq!(count, 3, "Expected 3 rows in the table");

        // Explicitly drop resources to ensure proper cleanup
        info!("Explicitly cleaning up resources");

        // First drop the connection to close it properly
        drop(conn);

        // Store database name and backend for cleanup
        let db_name = test_db.db_name.clone();
        let backend = test_db.backend().clone();

        // Drop the TestDatabase instance which should trigger automatic cleanup
        info!("Dropping TestDatabase instance");
        drop(test_db);

        // Add a small delay to allow automatic cleanup to complete
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Verify the database was dropped, if not, force drop it
        let db_exists = std::process::Command::new("mysql")
            .args([
                "-h",
                "mysql",
                "-u",
                "root",
                "-e",
                &format!(
                    "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = '{}'",
                    db_name.as_str()
                ),
            ])
            .output()
            .map(|output| {
                let output_str = String::from_utf8_lossy(&output.stdout);
                !output_str.contains("0")
            })
            .unwrap_or(true);

        if db_exists {
            info!("Database {} still exists, forcing cleanup", db_name);

            // Try to terminate all connections
            if let Err(e) = backend.terminate_connections(&db_name).await {
                info!(
                    "Warning: Failed to terminate connections to {}: {}",
                    db_name, e
                );
            }

            // Try to drop the database one more time
            if let Err(e) = backend.drop_database(&db_name).await {
                info!("Failed to drop database through backend: {}", e);

                // Last resort: direct MySQL command
                info!("Using direct MySQL command to force drop database");
                let _ = std::process::Command::new("mysql")
                    .args([
                        "-h",
                        "mysql",
                        "-u",
                        "root",
                        "-e",
                        &format!("DROP DATABASE IF EXISTS `{}`", db_name.as_str()),
                    ])
                    .status();
            }
        } else {
            info!(
                "Database {} was successfully cleaned up by automatic mechanisms",
                db_name
            );
        }

        info!("SQLx MySQL simple test completed successfully");
        Ok(())
    }
}
