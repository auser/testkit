#[cfg(feature = "sqlite")]
mod sqlite_auto_cleanup_tests {
    use db_testkit::backend::Connection;
    use db_testkit::{with_test_db, Result};
    use std::fs;
    use std::path::Path;
    use std::time::Duration;
    use tokio::time::sleep;
    use tracing::{debug, info};

    #[tokio::test]
    async fn test_sqlite_auto_cleanup() -> Result<()> {
        // Initialize logging for better visibility
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "db_testkit=debug,sqlx=info");
        }

        // Always initialize tracing
        let _ = tracing_subscriber::fmt::try_init();

        info!("=== Starting SQLite auto-cleanup test ===");

        // This scope ensures that test_db is dropped before we check for cleanup
        {
            // Run a test with SQLite database that will be auto-cleaned
            info!("--- Creating test database ---");
            with_test_db(|test_db| async move {
                // Log the database name for verification
                let db_name = test_db.db_name.clone();
                info!("Created test database: {}", db_name);

                // Get a connection and verify it works
                let mut conn = test_db.connection().await?;

                // Execute a simple query to verify connection
                conn.execute("CREATE TABLE test_table (id INTEGER PRIMARY KEY)")
                    .await?;
                conn.execute("INSERT INTO test_table (id) VALUES (1), (2), (3)")
                    .await?;

                // Query the data to verify
                let rows = conn.fetch("SELECT COUNT(*) FROM test_table").await?;
                assert_eq!(rows.len(), 1);

                // Get file path of the SQLite database
                let connection_string = test_db.connection_string();
                let file_path = connection_string.trim_start_matches("sqlite://");
                info!(
                    "Test operations completed successfully on database: {} (file: {})",
                    db_name, file_path
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

        info!("--- After test scope, checking if database files were cleaned up ---");

        // Ensure all cleanup has finished
        sleep(Duration::from_secs(1)).await;

        // Look for any testkit SQLite files
        let temp_dir = std::env::temp_dir();
        debug!(
            "Checking for SQLite files in temp directory: {:?}",
            temp_dir
        );

        let testkit_files = fs::read_dir(temp_dir)
            .expect("Failed to read temp directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                if let Ok(file_name) = entry.file_name().into_string() {
                    file_name.starts_with("testkit_")
                        && (file_name.ends_with(".db") || file_name.ends_with(".sqlite"))
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();

        if testkit_files.is_empty() {
            info!("All SQLite database files were properly cleaned up");
        } else {
            for file in &testkit_files {
                tracing::error!("Testkit database file not cleaned up: {:?}", file.path());
            }
            assert!(
                testkit_files.is_empty(),
                "SQLite database files were not properly cleaned up"
            );
        }

        info!("=== SQLite auto-cleanup test completed successfully ===");
        Ok(())
    }
}
