#[cfg(feature = "sqlx-postgres")]
mod sqlx_postgres_auto_cleanup_tests {
    use db_testkit::{with_test_db, Result};
    use std::time::Duration;
    use tokio::time::sleep;
    use tracing::{debug, info};

    // Import both Connection and any other necessary traits/types
    use db_testkit::backend::Connection;

    #[tokio::test]
    async fn test_sqlx_postgres_auto_cleanup() -> Result<()> {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=info");
        }

        // Always initialize tracing
        let _ = tracing_subscriber::fmt::try_init();

        info!("=== Starting SQLx PostgreSQL auto-cleanup test ===");

        // List databases before test
        debug!("--- Databases before test ---");
        let _ = std::process::Command::new("psql")
            .args(["-h", "postgres", "-U", "postgres", "-c", "\\l", "-t"])
            .status();

        // This scope ensures that test_db is dropped before we check for cleanup
        {
            // Run a test with SQLx PostgreSQL database that will be auto-cleaned
            info!("--- Creating test database ---");
            with_test_db(|test_db| async move {
                // Log the database name for verification
                let db_name = test_db.db_name.clone();
                info!("Created test database: {}", db_name);

                // Get a connection and verify it works
                let mut conn = test_db.connection().await?;

                // Use the Connection trait methods explicitly
                Connection::execute(&mut conn, "CREATE TABLE test_table (id SERIAL PRIMARY KEY)")
                    .await?;
                Connection::execute(
                    &mut conn,
                    "INSERT INTO test_table (id) VALUES (1), (2), (3)",
                )
                .await?;

                // Simple verification query - we don't need to fetch the actual count
                // Just checking that the query executes without error
                let result =
                    Connection::execute(&mut conn, "SELECT COUNT(*) FROM test_table").await;
                assert!(result.is_ok(), "Should be able to query the table");

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
            .await
            .unwrap();

            info!("--- Test function completed, TestDatabase instance should be dropped soon ---");

            // Sleep briefly to ensure TestDatabase drop has fully completed
            sleep(Duration::from_millis(500)).await;
        }

        info!("--- After test scope, checking if database was cleaned up ---");

        // Ensure all cleanup has finished
        sleep(Duration::from_secs(1)).await;

        // List databases after test
        debug!("--- Databases after test (should be auto-cleaned up) ---");
        let _ = std::process::Command::new("psql")
            .args(["-h", "postgres", "-U", "postgres", "-c", "\\l", "-t"])
            .status();

        // Verify database was cleaned up
        let output = std::process::Command::new("psql")
            .args([
                "-h",
                "postgres",
                "-U",
                "postgres",
                "-t",
                "-c",
                "SELECT COUNT(*) FROM pg_database WHERE datname = 'testkit_sqlx_auto_cleanup_test'",
            ])
            .output()
            .expect("Failed to execute psql command");

        let output_str = String::from_utf8_lossy(&output.stdout);
        let count = output_str.trim().parse::<i32>().unwrap_or(1);

        debug!("Testkit databases count: {}", count);
        if count == 0 {
            info!("All testkit databases were properly cleaned up");
        } else {
            tracing::error!(
                "Testkit databases were not properly cleaned up: {}",
                output_str
            );
            assert!(false, "Testkit databases were not properly cleaned up");
        }

        info!("=== SQLx PostgreSQL auto-cleanup test completed successfully ===");
        Ok(())
    }
}
