#[cfg(feature = "mysql")]
mod mysql_auto_cleanup_tests {
    use db_testkit::backend::Connection;
    use db_testkit::{with_test_db, Result};
    use std::time::Duration;
    use tokio::time::sleep;
    use tracing::{debug, info};

    #[tokio::test]
    async fn test_mysql_auto_cleanup() -> Result<()> {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,mysql_async=info");
        }

        // Always initialize tracing
        let _ = tracing_subscriber::fmt::try_init();

        info!("=== Starting MySQL auto-cleanup test ===");

        // List databases before test
        debug!("--- Databases before test ---");
        let _ = std::process::Command::new("mysql")
            .args(["-h", "mysql", "-u", "root", "-e", "SHOW DATABASES"])
            .status();

        // This scope ensures that test_db is dropped before we check for cleanup
        {
            // Run a test with MySQL database that will be auto-cleaned
            info!("--- Creating test database ---");
            with_test_db(|test_db| async move {
                // Log the database name for verification
                let db_name = test_db.db_name.clone();
                info!("Created test database: {}", db_name);

                // Get a connection and verify it works
                let mut conn = test_db.connection().await?;

                // Execute a simple query to verify connection
                conn.execute("CREATE TABLE test_table (id INT)").await?;
                conn.execute("INSERT INTO test_table VALUES (1), (2), (3)")
                    .await?;

                // Query the data to verify
                let rows = conn.fetch("SELECT COUNT(*) FROM test_table").await?;
                assert_eq!(rows.len(), 1);

                info!(
                    "Test operations completed successfully on database: {}",
                    db_name
                );

                // Explicitly drop the connection to ensure it's not kept alive
                drop(conn);

                // Sleep briefly to allow for any async cleanup
                sleep(Duration::from_millis(100)).await;

                Ok(())
            })
            .await;

            info!("--- Test function completed, TestDatabase instance should be dropped soon ---");

            // Sleep briefly to ensure TestDatabase drop has fully completed
            sleep(Duration::from_millis(500)).await;
        }

        info!("--- After test scope, checking if database was cleaned up ---");

        // Ensure all cleanup has finished
        sleep(Duration::from_secs(1)).await;

        // List databases after test
        debug!("--- Databases after test ---");
        let _ = std::process::Command::new("mysql")
            .args(["-h", "mysql", "-u", "root", "-e", "SHOW DATABASES"])
            .status();

        // Verify no testkit databases remain
        let output = std::process::Command::new("mysql")
            .args(["-h", "mysql", "-u", "root", "-e", "SHOW DATABASES"])
            .output()
            .expect("Failed to execute command");

        let output_str = String::from_utf8_lossy(&output.stdout);
        let has_testkit = output_str.contains("testkit_");

        debug!("Testkit databases remain: {}", has_testkit);
        if !has_testkit {
            info!("All testkit databases were properly cleaned up");
        } else {
            panic!("Testkit databases were not properly cleaned up");
        }

        info!("=== MySQL auto-cleanup test completed successfully ===");
        Ok(())
    }
}
