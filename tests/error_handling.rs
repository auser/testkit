use db_testkit::{backend::Connection, error::Result, init_tracing, with_test_db};
use tracing::info;

// Tests for error handling and edge cases
#[cfg(any(feature = "postgres", feature = "sqlx-postgres"))]
mod postgres_error_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_sql_error_handling() {
        init_tracing();
        info!("Testing error handling for invalid SQL statements");

        with_test_db!(|db| async move {
            // Get a connection
            let mut conn = db.connection().await?;

            // Execute invalid SQL and expect an error
            let result = conn.execute("SELECT * FROM non_existent_table").await;
            assert!(result.is_err(), "Expected error for invalid table name");

            if let Err(e) = result {
                info!("Received expected error: {}", e);
            }

            // Test another invalid SQL statement
            let result = conn.execute("CREATE TABLE test_table (id INT;").await;
            assert!(result.is_err(), "Expected error for invalid SQL syntax");

            if let Err(e) = result {
                info!("Received expected error: {}", e);
            }

            Ok(()) as Result<()>
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_connection_limits() {
        init_tracing();
        info!("Testing database connection limits");

        with_test_db!(|db| async move {
            // Test acquiring multiple connections simultaneously
            const NUM_CONNECTIONS: usize = 10;
            let mut connections = Vec::with_capacity(NUM_CONNECTIONS);

            for i in 0..NUM_CONNECTIONS {
                info!("Acquiring connection {}", i);
                let conn = db.connection().await?;
                connections.push(conn);
            }

            // Verify all connections are valid
            for (i, conn) in connections.iter().enumerate() {
                info!("Testing connection {}", i);
                assert!(conn.is_valid().await, "Connection {} should be valid", i);
            }

            // Release all connections
            info!("Releasing all connections");
            connections.clear();

            // Acquire one more connection after releasing
            let conn = db.connection().await?;
            assert!(
                conn.is_valid().await,
                "Connection should be valid after releasing others"
            );

            Ok(()) as Result<()>
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        init_tracing();
        info!("Testing transaction rollback functionality");

        with_test_db!(|db| async move {
            // Get a connection and set up a test table
            let mut conn = db.connection().await?;
            conn.execute("CREATE TABLE test_transactions (id INT PRIMARY KEY, value TEXT)")
                .await?;

            // Insert initial data
            conn.execute("INSERT INTO test_transactions (id, value) VALUES (1, 'initial')")
                .await?;

            // Start a transaction and make changes
            let mut tx = conn.begin().await?;
            tx.execute("UPDATE test_transactions SET value = 'updated' WHERE id = 1")
                .await?;
            tx.execute("INSERT INTO test_transactions (id, value) VALUES (2, 'new')")
                .await?;

            // Verify changes are visible within the transaction
            let rows = tx.fetch_all("SELECT * FROM test_transactions").await?;
            assert_eq!(rows.len(), 2, "Should see two rows within transaction");

            // Rollback the transaction
            tx.rollback().await?;

            // Verify the changes were not committed
            let rows = conn.fetch_all("SELECT * FROM test_transactions").await?;
            assert_eq!(rows.len(), 1, "Should only see one row after rollback");
            assert_eq!(
                rows[0].get::<String, _>("value"),
                "initial",
                "Value should not have changed"
            );

            Ok(()) as Result<()>
        })
        .await
        .unwrap();
    }
}

// MySQL error handling tests
#[cfg(any(feature = "mysql", feature = "sqlx-mysql"))]
mod mysql_error_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Ignoring until MySQL connectivity issues are resolved
    async fn test_mysql_invalid_sql_handling() {
        init_tracing();
        info!("Testing MySQL error handling for invalid SQL");

        // Need to specify the MySQL connection string explicitly
        let connection_string = "mysql://root@mysql:3306?timeout=30&connect_timeout=30&pool_timeout=30&ssl-mode=DISABLED";

        with_test_db!(connection_string, |db| async move {
            // Get a connection
            let mut conn = db.connection().await?;

            // Execute invalid SQL and expect an error
            let result = conn.execute("SELECT * FROM non_existent_table").await;
            assert!(result.is_err(), "Expected error for invalid table name");

            if let Err(e) = result {
                info!("Received expected error: {}", e);
            }

            // Test another invalid SQL statement with MySQL syntax
            let result = conn
                .execute("CREATE TABLE test_table (id INT PRIMARY KEY AUTO_INCREMENT;")
                .await;
            assert!(result.is_err(), "Expected error for invalid SQL syntax");

            if let Err(e) = result {
                info!("Received expected error: {}", e);
            }

            Ok(()) as Result<()>
        })
        .await
        .unwrap();
    }
}

// SQLite error handling tests
#[cfg(feature = "sqlx-sqlite")]
mod sqlite_error_tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_invalid_sql_handling() {
        init_tracing();
        info!("Testing SQLite error handling for invalid SQL");

        with_test_db!(|db| async move {
            // Get a connection
            let mut conn = db.connection().await?;

            // Execute invalid SQL and expect an error
            let result = conn.execute("SELECT * FROM non_existent_table").await;
            assert!(result.is_err(), "Expected error for invalid table name");

            if let Err(e) = result {
                info!("Received expected error: {}", e);
            }

            // Test another invalid SQL statement
            let result = conn
                .execute("CREATE TABLE test_table (id INTEGER PRIMARY KEY;")
                .await;
            assert!(result.is_err(), "Expected error for invalid SQL syntax");

            if let Err(e) = result {
                info!("Received expected error: {}", e);
            }

            Ok(()) as Result<()>
        })
        .await
        .unwrap();
    }
}
