#[cfg(feature = "mysql")]
mod mysql_auto_cleanup_tests {
    use db_testkit::backend::Connection;
    use db_testkit::backends::MySqlBackend;
    use db_testkit::env::get_mysql_url;
    use db_testkit::test_db::TestDatabase;
    use db_testkit::{init_tracing, Result};
    use std::time::Duration;
    use tokio::time::sleep;
    use tracing::{debug, info};

    #[tokio::test]
    async fn test_mysql_auto_cleanup() -> Result<()> {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,mysql_async=info");
        }
        init_tracing();

        info!("=== Starting MySQL auto-cleanup test ===");

        // List databases before test
        debug!("--- Databases before test ---");
        let _ = std::process::Command::new("mysql")
            .args(["-h", "mysql", "-u", "root", "-e", "SHOW DATABASES"])
            .status();

        // Create the backend and test database directly
        let backend = MySqlBackend::new(&get_mysql_url()?)?;
        let test_db = TestDatabase::new(backend, Default::default()).await?;

        // Remember the database name for later verification
        let db_name_to_verify = test_db.db_name.to_string();
        info!("Created test database: {}", db_name_to_verify);

        // Run test operations
        {
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
                db_name_to_verify
            );

            // Explicitly drop the connection to ensure it's not kept alive
            drop(conn);
        }

        // Explicitly drop the database to trigger cleanup
        drop(test_db);

        info!("--- Test database dropped, waiting for cleanup ---");
        sleep(Duration::from_secs(2)).await;

        // List databases after test
        debug!("--- Databases after test ---");
        let _ = std::process::Command::new("mysql")
            .args(["-h", "mysql", "-u", "root", "-e", "SHOW DATABASES"])
            .status();

        // Verify the specific database created in this test was cleaned up
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
