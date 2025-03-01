//! Simple MySQL test example
//!
//! This example demonstrates how to use db-testkit with MySQL.
//! Run this example with: cargo run --features mysql --example simple_mysql_test

use db_testkit::{backend::Connection, with_test_db, Result};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging for better visibility
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "db_testkit=debug,mysql=info");
    }

    // Always initialize tracing
    let _ = tracing_subscriber::fmt::try_init();

    info!("Starting MySQL example");

    // Run test with a temporary MySQL database
    with_test_db!(|test_db| async move {
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

        // Query the data to verify
        #[cfg(any(feature = "mysql", feature = "postgres"))]
        {
            let rows = conn
                .fetch("SELECT id, name FROM test_table ORDER BY id")
                .await?;

            // Display the results
            info!("Query results:");
            for row in rows {
                let id: i32 = row.get::<usize, i32>(0);
                let name: String = row.get::<usize, String>(1);
                info!("  id: {}, name: {}", id, name);
            }

            // Count the rows
            let count_rows = conn.fetch("SELECT COUNT(*) FROM test_table").await?;
            let count: i64 = count_rows[0].get::<usize, i64>(0);
            info!("Total rows: {}", count);
        }

        #[cfg(any(
            feature = "sqlx-postgres",
            feature = "sqlx-mysql",
            feature = "sqlx-sqlite"
        ))]
        {
            use sqlx::Row;

            // For SQLx backends, use the sqlx interface directly
            let rows = sqlx::query("SELECT id, name FROM test_table ORDER BY id")
                .fetch_all(conn.sqlx_pool())
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
                .fetch_one(conn.sqlx_pool())
                .await?;
            let count: i64 = row.get(0);
            info!("Total rows: {}", count);
        }

        info!("MySQL example completed successfully");
        Ok(())
    })
    .await
}
