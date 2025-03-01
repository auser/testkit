#[cfg(feature = "sqlx-mysql")]
mod mysql_auto_cleanup_tests {
    use db_testkit::backend::Connection;
    use db_testkit::backends::sqlx::SqlxMySqlBackend;
    use db_testkit::test_db::TestDatabase;
    use db_testkit::{init_tracing, Result};
    use sqlx::Row;
    use std::time::Duration;
    use tokio::time::sleep;
    use tracing::{debug, error, info};

    // The test requires a running MySQL server with proper access permissions
    #[tokio::test]
    async fn test_mysql_auto_cleanup() -> Result<()> {
        init_tracing();
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=debug");
        }
        init_tracing();

        info!("=== Starting SQLx MySQL auto-cleanup test ===");

        // List databases before test
        debug!("--- Databases before test ---");
        let db_list_result = std::process::Command::new("mysql")
            .args(["-h", "mysql", "-u", "root", "-e", "SHOW DATABASES"])
            .status();

        match db_list_result {
            Ok(status) => info!("Database listing command completed with status: {}", status),
            Err(e) => info!("Database listing command failed: {}", e),
        }

        // Use a direct connection string with explicit parameters that match what works
        // with the command-line mysql tool
        let mysql_url = "mysql://root@mysql:3306?timeout=30&connect_timeout=30&pool_timeout=30&ssl-mode=DISABLED";
        info!("Using direct MySQL URL: {}", mysql_url);

        // Create the backend with more robust configuration
        info!("Creating SqlxMySqlBackend...");
        let backend = match SqlxMySqlBackend::new(mysql_url) {
            Ok(backend) => {
                info!("Successfully created SqlxMySqlBackend");
                backend
            }
            Err(e) => {
                error!("Failed to create SqlxMySqlBackend: {}", e);
                return Err(e);
            }
        };

        // Implement retry logic for the connection
        info!("Testing connection to MySQL server with retry logic...");
        let max_retries = 3;
        let mut retry_count = 0;
        let mut last_error = None;

        while retry_count < max_retries {
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
                        let backoff = Duration::from_secs(2 * retry_count as u64);
                        info!(
                            "Connection attempt {} failed, retrying in {:?}...",
                            retry_count, backoff
                        );
                        sleep(backoff).await;
                    } else {
                        error!("All connection attempts failed");
                    }
                }
            }
        }

        if let Some(e) = last_error {
            if retry_count >= max_retries {
                error!("Failed to connect after {} attempts: {}", max_retries, e);
                return Err(e);
            }
        }

        info!("Creating test database...");
        let test_db = match TestDatabase::new(backend, Default::default()).await {
            Ok(db) => {
                info!("Successfully created test database");
                db
            }
            Err(e) => {
                error!("Failed to create test database: {}", e);
                return Err(e);
            }
        };

        // Remember the database name for later verification
        let db_name_to_verify = test_db.db_name.to_string();
        info!("Created test database: {}", db_name_to_verify);

        // Run test operations
        {
            // Get a connection and verify it works
            info!("Getting connection...");
            let mut conn = match test_db.connection().await {
                Ok(conn) => {
                    info!("Successfully got connection");
                    conn
                }
                Err(e) => {
                    error!("Failed to get connection: {}", e);
                    return Err(e);
                }
            };

            // Execute a simple query to verify connection
            info!("Creating test table...");
            match conn.execute("CREATE TABLE test_table (id INT)").await {
                Ok(_) => info!("Table created successfully"),
                Err(e) => {
                    error!("Failed to create table: {}", e);
                    return Err(e);
                }
            }

            info!("Inserting test data...");
            match conn
                .execute("INSERT INTO test_table VALUES (1), (2), (3)")
                .await
            {
                Ok(_) => info!("Data inserted successfully"),
                Err(e) => {
                    error!("Failed to insert data: {}", e);
                    return Err(e);
                }
            }

            // Query the data to verify
            info!("Querying data...");
            let pool = conn.sqlx_pool();
            let row = match sqlx::query("SELECT COUNT(*) FROM test_table")
                .fetch_one(pool)
                .await
            {
                Ok(row) => {
                    info!("Query successful");
                    row
                }
                Err(e) => {
                    error!("Query failed: {}", e);
                    return Err(e.into());
                }
            };

            let count: i64 = row.get(0);
            assert_eq!(count, 3, "Expected 3 rows to be inserted");

            info!(
                "Test operations completed successfully on database: {}",
                db_name_to_verify
            );

            // Explicitly drop the connection to ensure it's not kept alive
            info!("Dropping connection...");
            drop(conn);
            info!("Connection dropped");
        }

        // Explicitly drop the database to trigger cleanup
        info!("Dropping test database...");
        drop(test_db);

        info!("--- Test database dropped, waiting for cleanup ---");
        sleep(Duration::from_secs(2)).await;

        // List databases after test
        debug!("--- Databases after test ---");
        let _ = std::process::Command::new("mysql")
            .args(["-h", "mysql", "-u", "root", "-e", "SHOW DATABASES"])
            .status();

        // Verify the specific database created in this test was cleaned up
        info!("Verifying database cleanup...");
        let output = std::process::Command::new("mysql")
            .args([
                "-h",
                "mysql",
                "-u",
                "root",
                "-e",
                &format!(
                    "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = '{}'",
                    db_name_to_verify
                ),
            ])
            .output()
            .expect("Failed to execute command");

        let output_str = String::from_utf8_lossy(&output.stdout);
        let count = output_str
            .trim()
            .lines()
            .last()
            .unwrap_or("1")
            .trim()
            .parse::<i32>()
            .unwrap_or(1);

        if count == 0 {
            info!(
                "Test database {} was properly cleaned up",
                db_name_to_verify
            );
        } else {
            panic!(
                "Test database {} was not properly cleaned up",
                db_name_to_verify
            );
        }

        info!("=== MySQL auto-cleanup test completed successfully ===");
        Ok(())
    }
}
